use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// ES 单索引最大文档数 (i32::MAX)
pub const MAX_DOCS_PER_INDEX: i64 = 2_147_483_647;

/// 估算文件的总行数
pub fn estimate_total_rows<P: AsRef<Path>>(path: P) -> Result<u64, String> {
    let file = File::open(path.as_ref())
        .map_err(|e| format!("打开文件失败: {}", e))?;
    
    let file_size = file.metadata()
        .map_err(|e| format!("获取文件大小失败: {}", e))?
        .len();
    
    // 读取前1000行计算平均行字节数
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut total_bytes = 0u64;
    let mut line_count = 0u64;
    
    for _ in 0..1000 {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(n) => {
                total_bytes += n as u64;
                line_count += 1;
            }
            Err(_) => break,
        }
    }
    
    if line_count == 0 {
        return Ok(0);
    }
    
    // 计算平均行大小
    let avg_line_size = total_bytes / line_count;
    
    // 估算总行数（减去表头）
    let estimated_rows = if avg_line_size > 0 {
        (file_size / avg_line_size).saturating_sub(1) // 减去表头
    } else {
        0
    };
    
    Ok(estimated_rows)
}

/// 计算需要的分块数量
pub fn calculate_chunk_count(total_rows: u64) -> usize {
    if total_rows <= MAX_DOCS_PER_INDEX as u64 {
        1
    } else {
        ((total_rows as f64 / MAX_DOCS_PER_INDEX as f64).ceil() as usize).max(1)
    }
}

/// 生成索引名称
/// 如果只有一个分块，返回原始名称
/// 如果有多个分块，返回 {name}_01, {name}_02, ...
pub fn generate_index_name(base_name: &str, chunk_index: usize, total_chunks: usize) -> String {
    if total_chunks == 1 {
        base_name.to_string()
    } else {
        // 根据总分块数决定数字位数
        let digits = if total_chunks > 99 {
            ((total_chunks as f64).log10().floor() as usize) + 1
        } else {
            2
        };
        
        format!("{}_{:0width$}", base_name, chunk_index, width = digits)
    }
}

/// 分块管理器
pub struct ChunkManager {
    base_index_name: String,
    total_rows: u64,
    total_chunks: usize,
    rows_per_chunk: u64,
    current_chunk: usize,
    current_chunk_rows: u64,
}

impl ChunkManager {
    pub fn new(base_index_name: String, total_rows: u64) -> Self {
        let total_chunks = calculate_chunk_count(total_rows);
        let rows_per_chunk = if total_chunks > 1 {
            MAX_DOCS_PER_INDEX as u64
        } else {
            total_rows
        };
        
        Self {
            base_index_name,
            total_rows,
            total_chunks,
            rows_per_chunk,
            current_chunk: 1,
            current_chunk_rows: 0,
        }
    }
    
    pub fn total_chunks(&self) -> usize {
        self.total_chunks
    }
    
    pub fn current_index_name(&self) -> String {
        generate_index_name(&self.base_index_name, self.current_chunk, self.total_chunks)
    }
    
    /// 添加一行，如果需要切换到下一个chunk，返回true
    pub fn add_row(&mut self) -> bool {
        self.current_chunk_rows += 1;
        
        if self.current_chunk_rows >= self.rows_per_chunk && self.current_chunk < self.total_chunks {
            self.current_chunk += 1;
            self.current_chunk_rows = 0;
            true
        } else {
            false
        }
    }
    
    pub fn current_chunk_index(&self) -> usize {
        self.current_chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_index_name() {
        assert_eq!(generate_index_name("test", 1, 1), "test");
        assert_eq!(generate_index_name("test", 1, 2), "test_01");
        assert_eq!(generate_index_name("test", 2, 2), "test_02");
        assert_eq!(generate_index_name("test", 99, 100), "test_99");
        assert_eq!(generate_index_name("test", 100, 100), "test_100");
    }
    
    #[test]
    fn test_calculate_chunk_count() {
        assert_eq!(calculate_chunk_count(100), 1);
        assert_eq!(calculate_chunk_count(MAX_DOCS_PER_INDEX as u64), 1);
        assert_eq!(calculate_chunk_count(MAX_DOCS_PER_INDEX as u64 + 1), 2);
        assert_eq!(calculate_chunk_count(MAX_DOCS_PER_INDEX as u64 * 2), 2);
    }
}
