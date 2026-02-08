mod app;
mod chunk;
mod csv_reader;
mod es_client;
mod import;
mod types;

use app::CsvImportApp;

fn main() -> Result<(), eframe::Error> {
    // 初始化日志
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    // 配置窗口选项
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_title("CSV 批量导入 Elasticsearch"),
        ..Default::default()
    };
    
    // 启动应用
    eframe::run_native(
        "CSV Import",
        options,
        Box::new(|_cc| Box::new(CsvImportApp::default())),
    )
}
