// main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod pose;
mod prompt;
mod ui_canvas;
mod ui_panels;
mod json_loader;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    // Load icon
    let icon_data = {
        let icon_bytes = include_bytes!("../assets/icon-256.png");
        let image = image::load_from_memory(icon_bytes)
            .expect("Failed to load icon")
            .to_rgba8();
        let (width, height) = image.dimensions();
        egui::IconData {
            rgba: image.into_raw(),
            width,
            height,
        }
    };
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1200.0, 700.0])
            .with_decorations(false)  // Disable OS title bar
            .with_icon(std::sync::Arc::new(icon_data)),
        centered: true,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "PromptPuppet",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("noto_sans".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/NotoSans-Regular.ttf"))));
            fonts.font_data.insert("noto_emoji".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/NotoEmoji-Regular.ttf"))));
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
                .insert(0, "noto_sans".to_owned());
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
                .push("noto_emoji".to_owned());
            fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap()
                .push("noto_emoji".to_owned());
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app::PromptPuppetApp::new(cc)))
        }),
    )
}