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
    // 注意：自定义字体文件已禁用，因为当前字体文件已损坏
    // 应用将使用系统默认字体，在 Windows 11 上通常能正确显示中文
    // 
    // 如需使用自定义中文字体，请执行以下步骤：
    // 1. 从 https://fonts.google.com/noto/specimen/Noto+Sans+SC 下载有效的字体文件
    // 2. 将字体文件保存为 assets/NotoSansSC-Regular.otf
    // 3. 取消注释下面的代码
    
    let mut fonts = egui::FontDefinitions::default();
    
    // 取消注释以启用自定义字体（需要有效的字体文件）
    /*
    fonts.font_data.insert(
        "noto_sans_sc".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../assets/NotoSansSC-Regular.otf"
        )),
    );
    
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
    */
    
    ctx.set_fonts(fonts);
    log::info!("Using system default fonts / 使用系统默认字体");
}
