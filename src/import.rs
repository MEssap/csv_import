use crate::chunk::{estimate_total_rows, ChunkManager};
use crate::csv_reader::CsvReader;
use crate::es_client::EsClient;
use crate::types::{ImportConfig, LogLevel, ProgressMessage};
use crossbeam::channel::Sender;
use std::time::Instant;

/// 导入管理器
pub struct ImportManager {
    config: ImportConfig,
    tx: Sender<ProgressMessage>,
}

impl ImportManager {
    pub fn new(config: ImportConfig, tx: Sender<ProgressMessage>) -> Self {
        Self { config, tx }
    }
    
    /// 执行导入
    pub fn run(&self) -> Result<(), String> {
        let start_time = Instant::now();
        
        // 发送开始消息
        self.send_message(ProgressMessage::Started {
            total_files: self.config.files.len(),
        });
        
        let mut total_success = 0u64;
        let mut total_failed = 0u64;
        let mut total_skipped = 0u64;
        
        // 创建 ES 客户端
        let es_client = EsClient::new(self.config.es_config.clone())
            .map_err(|e| {
                self.send_error(format!("创建 ES 客户端失败: {}", e));
                e
            })?;
        
        // 测试连接
        self.send_log(LogLevel::Info, "正在测试 Elasticsearch 连接...".to_string());
        es_client.test_connection().map_err(|e| {
            self.send_error(format!("Elasticsearch 连接测试失败: {}", e));
            e
        })?;
        self.send_log(LogLevel::Info, "Elasticsearch 连接成功".to_string());
        
        // 估算所有文件的总行数，用于决定是否需要索引分块
        let mut all_estimated_rows = 0u64;
        for file_path in &self.config.files {
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            match estimate_total_rows(file_path) {
                Ok(rows) => {
                    all_estimated_rows += rows;
                    self.send_log(
                        LogLevel::Info,
                        format!("文件 {} 估算行数: {}", file_name, rows),
                    );
                }
                Err(e) => {
                    self.send_log(
                        LogLevel::Warning,
                        format!("估算文件 {} 行数失败: {}", file_name, e),
                    );
                }
            }
        }
        
        self.send_log(
            LogLevel::Info,
            format!("所有文件估算总行数: {}", all_estimated_rows),
        );
        
        // 基于所有文件的总行数创建分块管理器
        let mut chunk_manager = ChunkManager::new(
            self.config.es_config.index_name.clone(),
            all_estimated_rows,
        );
        
        let total_chunks = chunk_manager.total_chunks();
        if total_chunks > 1 {
            self.send_log(
                LogLevel::Info,
                format!("数据将分成 {} 个索引", total_chunks),
            );
        }
        
        // 处理每个文件
        for file_path in &self.config.files {
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            self.send_log(LogLevel::Info, format!("开始处理文件: {}", file_name));
            
            // 读取 CSV
            let mut csv_reader = CsvReader::new(
                self.config.delimiter.clone(),
                self.config.max_record_size,
            )
            .map_err(|e| {
                self.send_error(format!("创建 CSV 读取器失败: {}", e));
                e
            })?;
            
            let mut csv_iter = csv_reader
                .read_csv(file_path, &self.config.delimiter)
                .map_err(|e| {
                    self.send_error(format!("读取 CSV 文件失败: {}", e));
                    e
                })?;
            
            let actual_total = csv_iter.total_rows() as u64;
            
            self.send_message(ProgressMessage::FileStarted {
                file_name: file_name.clone(),
                estimated_rows: actual_total,
            });
            
            self.send_log(
                LogLevel::Info,
                format!("实际读取到 {} 行数据", actual_total),
            );
            
            // 批量导入
            let mut file_success = 0u64;
            let mut file_failed = 0u64;
            let mut file_skipped = 0u64;
            let mut batch = Vec::new();
            let mut imported = 0u64;
            
            loop {
                match csv_iter.next_record() {
                    Some(Ok(json)) => {
                        batch.push(json);
                        
                        // 达到 batch size，执行导入
                        if batch.len() >= self.config.es_config.batch_size {
                            let result = self.import_batch(
                                &es_client,
                                &chunk_manager.current_index_name(),
                                &batch,
                            );
                            
                            file_success += result.0;
                            file_failed += result.1;
                            imported += batch.len() as u64;
                            
                            // 发送进度
                            self.send_progress(
                                file_name.clone(),
                                imported,
                                actual_total,
                            );
                            
                            batch.clear();
                            
                            // 检查是否需要切换分块
                            for _ in 0..result.0 {
                                if chunk_manager.add_row() {
                                    self.send_log(
                                        LogLevel::Info,
                                        format!(
                                            "切换到新索引: {}",
                                            chunk_manager.current_index_name()
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // 记录跳过
                        self.send_log(
                            LogLevel::Warning,
                            format!("跳过记录: {}", e),
                        );
                        file_skipped += 1;
                        imported += 1;
                        
                        self.send_progress(
                            file_name.clone(),
                            imported,
                            actual_total,
                        );
                    }
                    None => break,
                }
            }
            
            // 导入剩余的批次
            if !batch.is_empty() {
                let result = self.import_batch(
                    &es_client,
                    &chunk_manager.current_index_name(),
                    &batch,
                );
                
                file_success += result.0;
                file_failed += result.1;
                imported += batch.len() as u64;
                
                self.send_progress(
                    file_name.clone(),
                    imported,
                    actual_total,
                );
            }
            
            // 文件完成
            self.send_message(ProgressMessage::FileCompleted {
                file_name: file_name.clone(),
                success: file_success,
                failed: file_failed,
                skipped: file_skipped,
            });
            
            total_success += file_success;
            total_failed += file_failed;
            total_skipped += file_skipped;
        }
        
        // 全部完成
        let duration = start_time.elapsed().as_secs();
        self.send_message(ProgressMessage::Completed {
            total_success,
            total_failed,
            total_skipped,
            duration_secs: duration,
        });
        
        Ok(())
    }
    
    fn import_batch(
        &self,
        client: &EsClient,
        index_name: &str,
        batch: &[serde_json::Value],
    ) -> (u64, u64) {
        match client.bulk_index(index_name, batch.to_vec()) {
            Ok(result) => {
                if result.failed > 0 {
                    for error in &result.errors {
                        self.send_log(LogLevel::Warning, format!("导入错误: {}", error));
                    }
                }
                (result.success as u64, result.failed as u64)
            }
            Err(e) => {
                self.send_log(LogLevel::Error, format!("Bulk 请求失败: {}", e));
                (0, batch.len() as u64)
            }
        }
    }
    
    fn send_message(&self, msg: ProgressMessage) {
        let _ = self.tx.send(msg);
    }
    
    fn send_log(&self, level: LogLevel, message: String) {
        self.send_message(ProgressMessage::Log { level, message });
    }
    
    fn send_error(&self, message: String) {
        self.send_message(ProgressMessage::Error { message });
    }
    
    fn send_progress(&self, current_file: String, imported: u64, total: u64) {
        let percentage = if total > 0 {
            (imported as f32 / total as f32 * 100.0).min(100.0)
        } else {
            0.0
        };
        
        self.send_message(ProgressMessage::Progress {
            current_file,
            imported,
            total,
            percentage,
        });
    }
}
