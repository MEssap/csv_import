use crate::import::ImportManager;
use crate::types::{Delimiter, EsConfig, ImportConfig, ImportStats, LogLevel, ProgressMessage};
use crossbeam::channel::{Receiver, Sender};
use eframe::egui;
use std::path::PathBuf;
use std::thread;

/// GUI 应用
pub struct CsvImportApp {
    // 配置
    es_address: String,
    es_username: String,
    es_password: String,
    index_name: String,
    batch_size: String,
    delimiter: Delimiter,
    
    // 文件选择
    selected_files: Vec<PathBuf>,
    
    // 状态
    is_importing: bool,
    current_file: String,
    imported: u64,
    total: u64,
    percentage: f32,
    
    // 日志
    logs: Vec<LogEntry>,
    
    // 统计
    stats: Option<ImportStats>,
    
    // 通信通道
    rx: Option<Receiver<ProgressMessage>>,
    tx: Option<Sender<ProgressMessage>>,
}

struct LogEntry {
    level: LogLevel,
    message: String,
}

impl Default for CsvImportApp {
    fn default() -> Self {
        Self {
            es_address: "http://192.168.1.200:9200".to_string(),
            es_username: String::new(),
            es_password: String::new(),
            index_name: String::new(),
            batch_size: "1000".to_string(),
            delimiter: Delimiter::Auto,
            
            selected_files: Vec::new(),
            
            is_importing: false,
            current_file: String::new(),
            imported: 0,
            total: 0,
            percentage: 0.0,
            
            logs: Vec::new(),
            stats: None,
            
            rx: None,
            tx: None,
        }
    }
}

impl eframe::App for CsvImportApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 处理来自后台线程的消息
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                self.handle_message(msg);
            }
        }
        
        // 如果正在导入，持续刷新UI
        if self.is_importing {
            ctx.request_repaint();
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("CSV 批量导入 Elasticsearch");
            
            ui.add_space(10.0);
            
            // 配置区
            ui.group(|ui| {
                ui.label("Elasticsearch 配置");
                
                ui.horizontal(|ui| {
                    ui.label("ES 地址:");
                    ui.text_edit_singleline(&mut self.es_address);
                });
                
                ui.horizontal(|ui| {
                    ui.label("用户名 (可选):");
                    ui.text_edit_singleline(&mut self.es_username);
                });
                
                ui.horizontal(|ui| {
                    ui.label("密码 (可选):");
                    ui.add(egui::TextEdit::singleline(&mut self.es_password).password(true));
                });
                
                ui.horizontal(|ui| {
                    ui.label("索引名称:");
                    ui.text_edit_singleline(&mut self.index_name);
                });
                
                ui.horizontal(|ui| {
                    ui.label("批次大小:");
                    ui.text_edit_singleline(&mut self.batch_size);
                });
                
                ui.horizontal(|ui| {
                    ui.label("分隔符:");
                    egui::ComboBox::from_id_source("delimiter")
                        .selected_text(self.delimiter.to_string())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.delimiter, Delimiter::Auto, "自动检测");
                            ui.selectable_value(&mut self.delimiter, Delimiter::Comma, "逗号 (,)");
                            ui.selectable_value(&mut self.delimiter, Delimiter::Semicolon, "分号 (;)");
                            ui.selectable_value(&mut self.delimiter, Delimiter::Tab, "制表符 (\\t)");
                            ui.selectable_value(&mut self.delimiter, Delimiter::Pipe, "竖线 (|)");
                        });
                });
            });
            
            ui.add_space(10.0);
            
            // 文件选择区
            ui.group(|ui| {
                ui.label("文件选择");
                
                ui.horizontal(|ui| {
                    if ui.button("选择文件").clicked() && !self.is_importing {
                        if let Some(files) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_files()
                        {
                            self.selected_files = files;
                        }
                    }
                    
                    if ui.button("选择文件夹").clicked() && !self.is_importing {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.selected_files.clear();
                            if let Ok(entries) = std::fs::read_dir(folder) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                                        self.selected_files.push(path);
                                    }
                                }
                            }
                        }
                    }
                    
                    if ui.button("清空选择").clicked() && !self.is_importing {
                        self.selected_files.clear();
                    }
                });
                
                ui.label(format!("已选择 {} 个文件", self.selected_files.len()));
                
                if !self.selected_files.is_empty() {
                    egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .show(ui, |ui| {
                            for file in &self.selected_files {
                                ui.label(file.display().to_string());
                            }
                        });
                }
            });
            
            ui.add_space(10.0);
            
            // 操作区
            ui.horizontal(|ui| {
                if ui.button("开始导入").clicked() && !self.is_importing {
                    self.start_import();
                }
                
                if ui.button("停止").clicked() && self.is_importing {
                    // 简单的停止机制：标记状态，后台线程会自然结束
                    self.is_importing = false;
                }
            });
            
            ui.add_space(10.0);
            
            // 进度区
            if self.is_importing || self.stats.is_some() {
                ui.group(|ui| {
                    ui.label("导入进度");
                    
                    if !self.current_file.is_empty() {
                        ui.label(format!("当前文件: {}", self.current_file));
                    }
                    
                    ui.horizontal(|ui| {
                        ui.add(egui::ProgressBar::new(self.percentage / 100.0)
                            .show_percentage());
                        ui.label(format!("{} / {}", self.imported, self.total));
                    });
                });
            }
            
            ui.add_space(10.0);
            
            // 统计区
            if let Some(stats) = &self.stats {
                ui.group(|ui| {
                    ui.label("导入统计");
                    ui.label(format!("成功: {}", stats.total_success));
                    ui.label(format!("失败: {}", stats.total_failed));
                    ui.label(format!("跳过: {}", stats.total_skipped));
                    ui.label(format!("耗时: {} 秒", stats.duration_secs));
                });
                
                ui.add_space(10.0);
            }
            
            // 日志区
            ui.group(|ui| {
                ui.label("日志");
                
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for log in &self.logs {
                            let color = match log.level {
                                LogLevel::Info => egui::Color32::WHITE,
                                LogLevel::Warning => egui::Color32::YELLOW,
                                LogLevel::Error => egui::Color32::RED,
                            };
                            
                            ui.colored_label(
                                color,
                                format!("[{}] {}", log.level.to_string(), log.message)
                            );
                        }
                    });
            });
        });
    }
}

impl CsvImportApp {
    fn start_import(&mut self) {
        // 验证输入
        if self.selected_files.is_empty() {
            self.add_log(LogLevel::Error, "请先选择文件".to_string());
            return;
        }
        
        if self.index_name.trim().is_empty() {
            self.add_log(LogLevel::Error, "请输入索引名称".to_string());
            return;
        }
        
        let batch_size = match self.batch_size.parse::<usize>() {
            Ok(size) if size > 0 => size,
            _ => {
                self.add_log(LogLevel::Error, "批次大小必须是正整数".to_string());
                return;
            }
        };
        
        // 重置状态
        self.is_importing = true;
        self.current_file.clear();
        self.imported = 0;
        self.total = 0;
        self.percentage = 0.0;
        self.logs.clear();
        self.stats = None;
        
        // 创建通道
        let (tx, rx) = crossbeam::channel::unbounded();
        self.rx = Some(rx);
        self.tx = Some(tx.clone());
        
        // 构建配置
        let config = ImportConfig {
            es_config: EsConfig {
                address: self.es_address.clone(),
                username: if self.es_username.is_empty() {
                    None
                } else {
                    Some(self.es_username.clone())
                },
                password: if self.es_password.is_empty() {
                    None
                } else {
                    Some(self.es_password.clone())
                },
                index_name: self.index_name.clone(),
                batch_size,
            },
            files: self.selected_files.clone(),
            delimiter: self.delimiter.clone(),
            max_record_size: 100 * 1024 * 1024, // 100MB
        };
        
        // 启动后台线程
        thread::spawn(move || {
            let manager = ImportManager::new(config, tx);
            if let Err(e) = manager.run() {
                log::error!("导入失败: {}", e);
            }
        });
    }
    
    fn handle_message(&mut self, msg: ProgressMessage) {
        match msg {
            ProgressMessage::Started { total_files } => {
                self.add_log(LogLevel::Info, format!("开始导入 {} 个文件", total_files));
            }
            ProgressMessage::FileStarted { file_name, estimated_rows } => {
                self.current_file = file_name.clone();
                self.total = estimated_rows;
                self.imported = 0;
                self.add_log(
                    LogLevel::Info,
                    format!("开始处理文件: {}，预估 {} 行", file_name, estimated_rows),
                );
            }
            ProgressMessage::Progress {
                current_file,
                imported,
                total,
                percentage,
            } => {
                self.current_file = current_file;
                self.imported = imported;
                self.total = total;
                self.percentage = percentage;
            }
            ProgressMessage::Log { level, message } => {
                self.add_log(level, message);
            }
            ProgressMessage::FileCompleted {
                file_name,
                success,
                failed,
                skipped,
            } => {
                self.add_log(
                    LogLevel::Info,
                    format!(
                        "文件 {} 完成: 成功 {}, 失败 {}, 跳过 {}",
                        file_name, success, failed, skipped
                    ),
                );
            }
            ProgressMessage::Completed {
                total_success,
                total_failed,
                total_skipped,
                duration_secs,
            } => {
                self.is_importing = false;
                self.stats = Some(ImportStats {
                    total_success,
                    total_failed,
                    total_skipped,
                    duration_secs,
                });
                self.add_log(LogLevel::Info, "导入完成！".to_string());
            }
            ProgressMessage::Error { message } => {
                self.add_log(LogLevel::Error, message);
                self.is_importing = false;
            }
        }
    }
    
    fn add_log(&mut self, level: LogLevel, message: String) {
        self.logs.push(LogEntry { level, message });
        
        // 限制日志数量
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }
}
