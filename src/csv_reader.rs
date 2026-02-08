use crate::types::Delimiter;
use encoding_rs::GBK;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// 检测 CSV 文件的分隔符
/// 读取前 10 行，统计各分隔符的列数一致性
pub fn detect_delimiter<P: AsRef<Path>>(path: P) -> Result<u8, String> {
    let file = File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut reader = BufReader::new(file);
    
    // 尝试读取前10行
    let mut lines = Vec::new();
    let mut buffer = Vec::new();
    
    for _ in 0..10 {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                // 尝试解码
                let line = decode_bytes(&buffer)?;
                if !line.trim().is_empty() {
                    lines.push(line);
                }
            }
            Err(e) => return Err(format!("读取文件失败: {}", e)),
        }
    }
    
    if lines.is_empty() {
        return Err("文件为空".to_string());
    }
    
    // 候选分隔符
    let delimiters = vec![b',', b';', b'\t', b'|'];
    let mut scores: HashMap<u8, i32> = HashMap::new();
    
    for &delim in &delimiters {
        let mut column_counts = Vec::new();
        
        for line in &lines {
            let count = line.bytes().filter(|&b| b == delim).count();
            column_counts.push(count);
        }
        
        // 计算一致性：如果所有行的分隔符数量相同，且大于0，得分高
        if !column_counts.is_empty() {
            let first = column_counts[0];
            if first > 0 && column_counts.iter().all(|&c| c == first) {
                scores.insert(delim, first as i32 * 100); // 完全一致，高分
            } else if first > 0 {
                // 部分一致，低分
                scores.insert(delim, first as i32);
            }
        }
    }
    
    // 选择得分最高的分隔符
    scores
        .iter()
        .max_by_key(|(_, &score)| score)
        .map(|(&delim, _)| delim)
        .ok_or_else(|| "无法检测分隔符".to_string())
}

/// 解码字节为字符串，优先UTF-8，失败时fallback到GBK
pub fn decode_bytes(bytes: &[u8]) -> Result<String, String> {
    // 去除 UTF-8 BOM
    let bytes = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    
    // 尝试 UTF-8
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok(s.to_string());
    }
    
    // 尝试 GBK
    let (cow, _, had_errors) = GBK.decode(bytes);
    if !had_errors {
        return Ok(cow.to_string());
    }
    
    // 最后尝试强制UTF-8，替换无效字符
    Ok(String::from_utf8_lossy(bytes).to_string())
}

/// CSV 读取器，支持编码检测和分隔符检测
pub struct CsvReader {
    delimiter: u8,
    max_record_size: usize,
}

impl CsvReader {
    pub fn new(delimiter: Delimiter, max_record_size: usize) -> Result<Self, String> {
        Ok(Self {
            delimiter: delimiter.to_byte().unwrap_or(b','), // 如果是Auto，这里会被后续覆盖
            max_record_size,
        })
    }
    
    /// 读取CSV文件，返回表头和数据行的迭代器
    pub fn read_csv<P: AsRef<Path>>(
        &mut self,
        path: P,
        delimiter: &Delimiter,
    ) -> Result<CsvIterator, String> {
        // 如果是自动检测，先检测分隔符
        let delim = if delimiter == &Delimiter::Auto {
            detect_delimiter(path.as_ref())?
        } else {
            delimiter.to_byte().unwrap()
        };
        
        self.delimiter = delim;
        
        // 读取整个文件内容
        let mut file = File::open(path.as_ref())
            .map_err(|e| format!("打开文件失败: {}", e))?;
        
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("读取文件失败: {}", e))?;
        
        // 解码
        let content = decode_bytes(&buffer)?;
        
        // 使用 csv crate 解析
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(true)
            .flexible(true) // 允许字段数不一致
            .from_reader(content.as_bytes());
        
        // 读取表头
        let headers = reader
            .headers()
            .map_err(|e| format!("读取表头失败: {}", e))?
            .clone();
        
        let headers: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
        
        // 读取所有记录
        let mut records = Vec::new();
        for result in reader.records() {
            match result {
                Ok(record) => {
                    let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                    records.push(row);
                }
                Err(e) => {
                    // 跳过错误行，继续处理
                    log::warn!("跳过无效行: {}", e);
                }
            }
        }
        
        Ok(CsvIterator {
            headers,
            records,
            current: 0,
            max_record_size: self.max_record_size,
        })
    }
}

/// CSV 迭代器
pub struct CsvIterator {
    pub headers: Vec<String>,
    records: Vec<Vec<String>>,
    current: usize,
    max_record_size: usize,
}

impl CsvIterator {
    pub fn total_rows(&self) -> usize {
        self.records.len()
    }
    
    /// 获取下一条记录作为 JSON 对象
    pub fn next_record(&mut self) -> Option<Result<serde_json::Value, String>> {
        if self.current >= self.records.len() {
            return None;
        }
        
        let record = &self.records[self.current];
        self.current += 1;
        
        // 构建 JSON 对象
        let mut map = serde_json::Map::new();
        
        for (i, value) in record.iter().enumerate() {
            let key = self.headers.get(i)
                .cloned()
                .unwrap_or_else(|| format!("field_{}", i));
            
            map.insert(key, serde_json::Value::String(value.clone()));
        }
        
        let json = serde_json::Value::Object(map);
        
        // 检查大小
        match serde_json::to_string(&json) {
            Ok(s) => {
                if s.len() > self.max_record_size {
                    Some(Err(format!(
                        "记录超过最大大小 {} bytes (实际: {} bytes)",
                        self.max_record_size,
                        s.len()
                    )))
                } else {
                    Some(Ok(json))
                }
            }
            Err(e) => Some(Err(format!("序列化 JSON 失败: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_decode_bytes() {
        let utf8 = "Hello, 世界".as_bytes();
        assert!(decode_bytes(utf8).is_ok());
        
        let utf8_bom = b"\xEF\xBB\xBFHello";
        assert_eq!(decode_bytes(utf8_bom).unwrap(), "Hello");
    }
}
