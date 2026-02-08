import csv
import json
import os
import re
import time
from pathlib import Path

try:
    from elasticsearch import Elasticsearch, helpers
except ImportError as exc:
    raise SystemExit("缺少依赖：pip install elasticsearch") from exc

try:
    from tqdm import tqdm
except ImportError as exc:
    raise SystemExit("缺少依赖：pip install tqdm") from exc

# ---------- 配置区 ----------
ES_URL = os.getenv("ES_URL", "http://192.168.1.200:9200")
INDEX_NAME = os.getenv("INDEX_NAME")  # 未设置时使用文件名作为索引名
CSV_GLOB = os.getenv("CSV_GLOB", "*.csv")

USERNAME = os.getenv("ES_USERNAME")
PASSWORD = os.getenv("ES_PASSWORD")
API_KEY = os.getenv("ES_API_KEY")

HAS_HEADER = True
DELIMITER = "\t"
ENCODING = "utf-8"

CHUNK_SIZE_DOCS = 20000
THREAD_COUNT = 8
QUEUE_SIZE = 16
REQUEST_TIMEOUT = 180

FILE_CHUNK_BYTES = 512 * 1024 * 1024  # 512MB
# ---------------------------


def get_es_client():
    if API_KEY:
        return Elasticsearch(
            ES_URL,
            api_key=API_KEY,
            max_retries=5,
            retry_on_timeout=True,
            retry_on_status=(429, 500, 502, 503, 504),
        )
    if USERNAME and PASSWORD:
        return Elasticsearch(
            ES_URL,
            basic_auth=(USERNAME, PASSWORD),
            max_retries=5,
            retry_on_timeout=True,
            retry_on_status=(429, 500, 502, 503, 504),
        )
    return Elasticsearch(
        ES_URL,
        max_retries=5,
        retry_on_timeout=True,
        retry_on_status=(429, 500, 502, 503, 504),
    )


def normalize_index_name(name: str) -> str:
    normalized = re.sub(r"[^a-z0-9-_]+", "_", name.lower()).strip("._-")
    return normalized or "csv_import"


def load_state(state_path: Path):
    if state_path.exists():
        with state_path.open("r", encoding="utf-8") as f:
            return json.load(f)
    return {"done_chunks": []}


def save_state(state_path: Path, done_chunks):
    tmp = state_path.with_suffix(state_path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as f:
        json.dump({"done_chunks": done_chunks}, f)
    os.replace(tmp, state_path)


def read_headers(csv_path: Path):
    if not HAS_HEADER:
        return None
    with csv_path.open("rb") as f:
        header_line = f.readline().decode(ENCODING).rstrip("\n")
        return next(csv.reader([header_line], delimiter=DELIMITER))


def iter_chunk_ranges(file_size):
    start = 0
    while start < file_size:
        end = min(start + FILE_CHUNK_BYTES, file_size)
        yield start, end
        start = end


def log_bad_line(bad_lines_path: Path, offset, raw_line, reason):
    with bad_lines_path.open("a", encoding="utf-8") as f:
        f.write(f"[offset={offset}] reason={reason} line={raw_line}\n")


def generate_actions_for_chunk(
    csv_path: Path,
    start,
    end,
    headers,
    pbar,
    index_name: str,
    bad_lines_path: Path,
):
    with csv_path.open("rb") as f:
        f.seek(start)

        if start != 0:
            f.readline()

        if start == 0 and HAS_HEADER:
            f.readline()

        while f.tell() < end:
            pos = f.tell()
            line = f.readline()
            if not line:
                break
            pbar.update(len(line))

            try:
                line_decoded = line.decode(ENCODING).rstrip("\n")
                row = next(csv.reader([line_decoded], delimiter=DELIMITER))

                if headers:
                    if len(row) != len(headers):
                        log_bad_line(bad_lines_path, pos, line_decoded, "column_mismatch")
                        continue
                    doc = dict(zip(headers, row))
                else:
                    doc = {f"col{i}": v for i, v in enumerate(row)}

                yield {"_index": index_name, "_source": doc}

            except Exception as e:
                try:
                    line_text = line.decode(ENCODING, errors="replace").rstrip("\n")
                except Exception:
                    line_text = "<decode_failed>"
                log_bad_line(bad_lines_path, pos, line_text, f"parse_error:{e}")


def import_csv_file(es, csv_path: Path, index_name: str):
    headers = read_headers(csv_path) if HAS_HEADER else None
    file_size = csv_path.stat().st_size
    state_path = csv_path.with_suffix(".import_state.json")
    bad_lines_path = csv_path.with_suffix(".bad_lines.log")

    state = load_state(state_path)
    done = set(state.get("done_chunks", []))

    for idx, (start, end) in enumerate(iter_chunk_ranges(file_size)):
        if idx in done:
            continue

        print(f"[{csv_path.name}] 导入分段 {idx}: {start} - {end}")
        t0 = time.perf_counter()

        with tqdm(total=end - start, unit="B", unit_scale=True, desc=f"{csv_path.name} Chunk {idx}") as pbar:
            for _ok, _ in helpers.parallel_bulk(
                es,
                generate_actions_for_chunk(
                    csv_path,
                    start,
                    end,
                    headers,
                    pbar,
                    index_name,
                    bad_lines_path,
                ),
                chunk_size=CHUNK_SIZE_DOCS,
                thread_count=THREAD_COUNT,
                queue_size=QUEUE_SIZE,
                request_timeout=REQUEST_TIMEOUT,
            ):
                pass

        elapsed = time.perf_counter() - t0
        chunk_bytes = end - start
        speed = chunk_bytes / elapsed / (1024 * 1024)
        print(f"[{csv_path.name}] 分段 {idx} 完成: 耗时 {elapsed:.2f}s, 平均 {speed:.2f} MB/s")

        done.add(idx)
        save_state(state_path, sorted(done))

    print(f"[{csv_path.name}] 全部分段完成")


def main():
    csv_files = sorted(Path.cwd().glob(CSV_GLOB))
    if not csv_files:
        print(f"当前目录未找到 CSV 文件（匹配: {CSV_GLOB}）")
        return

    es = get_es_client()

    for csv_path in csv_files:
        index_name = INDEX_NAME or normalize_index_name(csv_path.stem)
        print(f"开始导入: {csv_path.name} -> 索引 {index_name}")
        import_csv_file(es, csv_path, index_name)

    print("全部 CSV 导入完成")


if __name__ == "__main__":
    main()
