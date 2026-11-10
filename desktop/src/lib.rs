pub mod design;
pub mod engine;
pub mod model;
pub mod native;
pub mod practice;
pub mod practice_accessibility;
pub mod runtime;
pub mod settings;
pub mod shortcuts;
pub mod ui;
pub mod window;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = window::SingleInstance::acquire()? else {
        return Ok(());
    };
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("EarControl")
            .with_inner_size([1160.0, 780.0])
            .with_min_inner_size([860.0, 620.0])
            .with_decorations(false)
            .with_icon(icon),
        centered: true,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "EarControl",
        options,
        Box::new(|cc| Ok(Box::new(ui::EarControl::new(cc)?))),
    )?;
    Ok(())
}
