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
        // 设置视觉样式
        ctx.set_visuals(egui::Visuals {
            window_rounding: egui::Rounding::same(8.0),
            window_shadow: egui::epaint::Shadow {
                offset: egui::vec2(2.0, 2.0),
                blur: 8.0,
                spread: 0.0,
                color: egui::Color32::from_black_alpha(50),
            },
            ..egui::Visuals::dark()
        });
        
        // 处理来自后台线程的消息
        let messages: Vec<ProgressMessage> = if let Some(rx) = &self.rx {
            rx.try_iter().collect()
        } else {
            Vec::new()
        };
        
        for msg in messages {
            self.handle_message(msg);
        }
        
        // 如果正在导入，持续刷新UI
        if self.is_importing {
            ctx.request_repaint();
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(5.0);
            
            // 标题
            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("📊 CSV 批量导入 Elasticsearch")
                    .size(24.0)
                    .color(egui::Color32::from_rgb(100, 200, 255)));
            });
            
            ui.add_space(15.0);
            
            // 配置区
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(35, 38, 45))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("⚙️ Elasticsearch 配置")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 220, 150)));
                    
                    ui.add_space(8.0);
                    
                    egui::Grid::new("config_grid")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("ES 地址:");
                            ui.add_sized([500.0, 20.0], egui::TextEdit::singleline(&mut self.es_address));
                            ui.end_row();
                            
                            ui.label("用户名 (可选):");
                            ui.add_sized([500.0, 20.0], egui::TextEdit::singleline(&mut self.es_username));
                            ui.end_row();
                            
                            ui.label("密码 (可选):");
                            ui.add_sized([500.0, 20.0], egui::TextEdit::singleline(&mut self.es_password).password(true));
                            ui.end_row();
                            
                            ui.label("索引名称:");
                            ui.add_sized([500.0, 20.0], egui::TextEdit::singleline(&mut self.index_name));
                            ui.end_row();
                            
                            ui.label("批次大小:");
                            ui.add_sized([500.0, 20.0], egui::TextEdit::singleline(&mut self.batch_size));
                            ui.end_row();
                            
                            ui.label("分隔符:");
                            egui::ComboBox::from_id_source("delimiter")
                                .selected_text(self.delimiter.to_string())
                                .width(488.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.delimiter, Delimiter::Auto, "自动检测");
                                    ui.selectable_value(&mut self.delimiter, Delimiter::Comma, "逗号 (,)");
                                    ui.selectable_value(&mut self.delimiter, Delimiter::Semicolon, "分号 (;)");
                                    ui.selectable_value(&mut self.delimiter, Delimiter::Tab, "制表符 (\\t)");
                                    ui.selectable_value(&mut self.delimiter, Delimiter::Pipe, "竖线 (|)");
                                });
                            ui.end_row();
                        });
                });
            
            ui.add_space(12.0);
            
            // 文件选择区
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(35, 38, 45))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("📁 文件选择")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 220, 150)));
                    
                    ui.add_space(8.0);
                    
                    ui.horizontal(|ui| {
                        let button_height = 28.0;
                        
                        if ui.add_sized(
                            [120.0, button_height],
                            egui::Button::new("📄 选择文件")
                                .fill(egui::Color32::from_rgb(60, 120, 200))
                        ).clicked() && !self.is_importing {
                            if let Some(files) = rfd::FileDialog::new()
                                .add_filter("CSV", &["csv"])
                                .pick_files()
                            {
                                self.selected_files = files;
                            }
                        }
                        
                        if ui.add_sized(
                            [120.0, button_height],
                            egui::Button::new("📂 选择文件夹")
                                .fill(egui::Color32::from_rgb(60, 120, 200))
                        ).clicked() && !self.is_importing {
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
                        
                        if ui.add_sized(
                            [120.0, button_height],
                            egui::Button::new("🗑️ 清空选择")
                                .fill(egui::Color32::from_rgb(150, 60, 60))
                        ).clicked() && !self.is_importing {
                            self.selected_files.clear();
                        }
                    });
                    
                    ui.add_space(8.0);
                    
                    ui.label(egui::RichText::new(format!("已选择 {} 个文件", self.selected_files.len()))
                        .color(egui::Color32::from_rgb(200, 200, 200)));
                    
                    if !self.selected_files.is_empty() {
                        ui.add_space(5.0);
                        egui::ScrollArea::vertical()
                            .max_height(100.0)
                            .show(ui, |ui| {
                                for file in &self.selected_files {
                                    ui.label(egui::RichText::new(file.display().to_string())
                                        .color(egui::Color32::from_rgb(180, 180, 180)));
                                }
                            });
                    }
                });
            
            ui.add_space(12.0);
            
            // 操作区
            ui.horizontal(|ui| {
                let button_height = 36.0;
                
                if ui.add_sized(
                    [150.0, button_height],
                    egui::Button::new(egui::RichText::new("▶️ 开始导入").size(16.0))
                        .fill(egui::Color32::from_rgb(60, 160, 60))
                ).clicked() && !self.is_importing {
                    self.start_import();
                }
                
                if ui.add_sized(
                    [150.0, button_height],
                    egui::Button::new(egui::RichText::new("⏹️ 停止").size(16.0))
                        .fill(egui::Color32::from_rgb(180, 60, 60))
                ).clicked() && self.is_importing {
                    // 简单的停止机制：标记状态，后台线程会自然结束
                    self.is_importing = false;
                }
            });
            
            ui.add_space(12.0);
            
            // 进度区
            if self.is_importing || self.stats.is_some() {
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgb(35, 38, 45))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("⏳ 导入进度")
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 220, 150)));
                        
                        ui.add_space(8.0);
                        
                        if !self.current_file.is_empty() {
                            ui.label(egui::RichText::new(format!("当前文件: {}", self.current_file))
                                .color(egui::Color32::from_rgb(200, 200, 200)));
                        }
                        
                        ui.add_space(5.0);
                        
                        ui.horizontal(|ui| {
                            ui.add(egui::ProgressBar::new(self.percentage / 100.0)
                                .show_percentage()
                                .fill(egui::Color32::from_rgb(100, 200, 255)));
                            ui.label(egui::RichText::new(format!("{} / {}", self.imported, self.total))
                                .color(egui::Color32::from_rgb(180, 180, 180)));
                        });
                    });
            }
            
            ui.add_space(12.0);
            
            // 统计区
            if let Some(stats) = &self.stats {
                egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgb(35, 38, 45))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("📊 导入统计")
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 220, 150)));
                        
                        ui.add_space(8.0);
                        
                        egui::Grid::new("stats_grid")
                            .num_columns(2)
                            .spacing([15.0, 6.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("✅ 成功:").color(egui::Color32::from_rgb(100, 220, 100)));
                                ui.label(egui::RichText::new(format!("{}", stats.total_success))
                                    .color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.end_row();
                                
                                ui.label(egui::RichText::new("❌ 失败:").color(egui::Color32::from_rgb(220, 100, 100)));
                                ui.label(egui::RichText::new(format!("{}", stats.total_failed))
                                    .color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.end_row();
                                
                                ui.label(egui::RichText::new("⏭️ 跳过:").color(egui::Color32::from_rgb(220, 220, 100)));
                                ui.label(egui::RichText::new(format!("{}", stats.total_skipped))
                                    .color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.end_row();
                                
                                ui.label(egui::RichText::new("⏱️ 耗时:").color(egui::Color32::from_rgb(150, 200, 220)));
                                ui.label(egui::RichText::new(format!("{} 秒", stats.duration_secs))
                                    .color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.end_row();
                            });
                    });
                
                ui.add_space(12.0);
            }
            
            // 日志区
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(35, 38, 45))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("📝 日志")
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 220, 150)));
                    
                    ui.add_space(8.0);
                    
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for log in &self.logs {
                                let (color, icon) = match log.level {
                                    LogLevel::Info => (egui::Color32::from_rgb(200, 200, 200), "ℹ️"),
                                    LogLevel::Warning => (egui::Color32::from_rgb(255, 200, 100), "⚠️"),
                                    LogLevel::Error => (egui::Color32::from_rgb(255, 100, 100), "❌"),
                                };
                                
                                ui.colored_label(
                                    color,
                                    format!("{} {}", icon, log.message)
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
