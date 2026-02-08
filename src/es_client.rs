use crate::types::{BulkResponse, EsConfig};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::time::Duration;

/// Elasticsearch 客户端
pub struct EsClient {
    client: Client,
    config: EsConfig,
}

impl EsClient {
    pub fn new(config: EsConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
        
        Ok(Self { client, config })
    }
    
    /// 发送 Bulk 请求
    /// 使用指数退避重试策略，最多重试3次
    pub fn bulk_index(
        &self,
        index_name: &str,
        documents: Vec<serde_json::Value>,
    ) -> Result<BulkResult, String> {
        let mut retry_count = 0;
        let max_retries = 3;
        let mut delay = Duration::from_secs(1);
        
        loop {
            match self.try_bulk_index(index_name, &documents) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if retry_count >= max_retries {
                        return Err(format!("Bulk 请求失败（重试 {} 次后）: {}", max_retries, e));
                    }
                    
                    log::warn!("Bulk 请求失败，{}秒后重试 ({}/{}): {}", 
                        delay.as_secs(), retry_count + 1, max_retries, e);
                    
                    std::thread::sleep(delay);
                    delay *= 2; // 指数退避
                    retry_count += 1;
                }
            }
        }
    }
    
    fn try_bulk_index(
        &self,
        index_name: &str,
        documents: &[serde_json::Value],
    ) -> Result<BulkResult, String> {
        // 构建 Bulk 请求体
        let mut body = String::new();
        
        for doc in documents {
            // 索引操作行
            let index_action = serde_json::json!({
                "index": {
                    "_index": index_name
                }
            });
            body.push_str(&serde_json::to_string(&index_action)
                .map_err(|e| format!("序列化索引操作失败: {}", e))?);
            body.push('\n');
            
            // 文档数据行
            body.push_str(&serde_json::to_string(doc)
                .map_err(|e| format!("序列化文档失败: {}", e))?);
            body.push('\n');
        }
        
        // 发送请求
        let url = format!("{}/_bulk", self.config.address);
        
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/x-ndjson"));
        
        let mut request = self.client
            .post(&url)
            .headers(headers)
            .body(body);
        
        // 添加认证
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            request = request.basic_auth(username, Some(password));
        }
        
        let response = request
            .send()
            .map_err(|e| format!("发送请求失败: {}", e))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().unwrap_or_else(|_| "无法读取错误信息".to_string());
            return Err(format!("HTTP 错误 {}: {}", status, error_text));
        }
        
        let bulk_response: BulkResponse = response
            .json()
            .map_err(|e| format!("解析响应失败: {}", e))?;
        
        // 统计成功和失败
        let mut success = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        
        for item in &bulk_response.items {
            let detail = item.index.as_ref()
                .or(item.create.as_ref())
                .or(item.update.as_ref());
            
            if let Some(detail) = detail {
                if detail.status >= 200 && detail.status < 300 {
                    success += 1;
                } else {
                    failed += 1;
                    if let Some(error) = &detail.error {
                        errors.push(format!("{}: {}", error.error_type, error.reason));
                    }
                }
            }
        }
        
        Ok(BulkResult {
            success,
            failed,
            errors,
        })
    }
    
    /// 测试连接
    pub fn test_connection(&self) -> Result<(), String> {
        let url = &self.config.address;
        
        let mut request = self.client.get(url);
        
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            request = request.basic_auth(username, Some(password));
        }
        
        let response = request
            .send()
            .map_err(|e| format!("连接失败: {}", e))?;
        
        if !response.status().is_success() {
            return Err(format!("连接失败: HTTP {}", response.status()));
        }
        
        Ok(())
    }
}

/// Bulk 操作结果
#[derive(Debug)]
pub struct BulkResult {
    pub success: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}
