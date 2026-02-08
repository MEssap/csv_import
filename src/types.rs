use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Elasticsearch 配置
#[derive(Clone, Debug)]
pub struct EsConfig {
    pub address: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub index_name: String,
    pub batch_size: usize,
}

impl Default for EsConfig {
    fn default() -> Self {
        Self {
            address: "http://192.168.1.200:9200".to_string(),
            username: None,
            password: None,
            index_name: String::new(),
            batch_size: 1000,
        }
    }
}

/// CSV 分隔符类型
#[derive(Clone, Debug, PartialEq)]
pub enum Delimiter {
    Auto,
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn to_byte(&self) -> Option<u8> {
        match self {
            Delimiter::Auto => None,
            Delimiter::Comma => Some(b','),
            Delimiter::Semicolon => Some(b';'),
            Delimiter::Tab => Some(b'\t'),
            Delimiter::Pipe => Some(b'|'),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Delimiter::Auto => "自动检测".to_string(),
            Delimiter::Comma => "逗号 (,)".to_string(),
            Delimiter::Semicolon => "分号 (;)".to_string(),
            Delimiter::Tab => "制表符 (\\t)".to_string(),
            Delimiter::Pipe => "竖线 (|)".to_string(),
        }
    }
}

/// 导入配置
#[derive(Clone, Debug)]
pub struct ImportConfig {
    pub es_config: EsConfig,
    pub files: Vec<PathBuf>,
    pub delimiter: Delimiter,
    pub max_record_size: usize, // 单条记录最大字节数，默认 100MB
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            es_config: EsConfig::default(),
            files: Vec::new(),
            delimiter: Delimiter::Auto,
            max_record_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// 导入进度消息
#[derive(Clone, Debug)]
pub enum ProgressMessage {
    Started {
        total_files: usize,
    },
    FileStarted {
        file_name: String,
        estimated_rows: u64,
    },
    Progress {
        current_file: String,
        imported: u64,
        total: u64,
        percentage: f32,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    FileCompleted {
        file_name: String,
        success: u64,
        failed: u64,
        skipped: u64,
    },
    Completed {
        total_success: u64,
        total_failed: u64,
        total_skipped: u64,
        duration_secs: u64,
    },
    Error {
        message: String,
    },
}

/// 日志级别
#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn to_string(&self) -> &str {
        match self {
            LogLevel::Info => "信息",
            LogLevel::Warning => "警告",
            LogLevel::Error => "错误",
        }
    }
}

/// 导入统计
#[derive(Clone, Debug, Default)]
pub struct ImportStats {
    pub total_success: u64,
    pub total_failed: u64,
    pub total_skipped: u64,
    pub duration_secs: u64,
}

/// Elasticsearch Bulk 操作
#[derive(Serialize, Deserialize, Debug)]
pub struct BulkIndexAction {
    pub index: BulkIndexMeta,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkIndexMeta {
    #[serde(rename = "_index")]
    pub index: String,
}

/// Elasticsearch Bulk 响应
#[derive(Serialize, Deserialize, Debug)]
pub struct BulkResponse {
    pub took: u64,
    pub errors: bool,
    pub items: Vec<BulkResponseItem>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkResponseItem {
    pub index: Option<BulkResponseItemDetail>,
    pub create: Option<BulkResponseItemDetail>,
    pub update: Option<BulkResponseItemDetail>,
    pub delete: Option<BulkResponseItemDetail>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkResponseItemDetail {
    #[serde(rename = "_index")]
    pub index: String,
    #[serde(rename = "_id")]
    pub id: Option<String>,
    pub status: u16,
    pub error: Option<BulkError>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub reason: String,
}
