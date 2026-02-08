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
            .with_inner_size([1000.0, 750.0])
            .with_title("CSV 批量导入 Elasticsearch"),
        ..Default::default()
    };
    
    // 启动应用
    eframe::run_native(
        "CSV Import",
        options,
        Box::new(|cc| {
            // 配置字体以支持中文
            setup_custom_fonts(&cc.egui_ctx);
            
            Box::new(CsvImportApp::default())
        }),
    )
}

/// 配置自定义字体以支持中文显示
fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    
    // 加载 Noto Sans SC 字体以支持简体中文
    fonts.font_data.insert(
        "noto_sans_sc".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../assets/NotoSansSC-Regular.otf"
        )),
    );
    
    // 将字体添加到字体家族列表的开头，优先使用
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "noto_sans_sc".to_owned());
    
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "noto_sans_sc".to_owned());
    
    ctx.set_fonts(fonts);
}
