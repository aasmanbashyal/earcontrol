pub mod design;
pub mod engine;
pub mod model;
pub mod native;
pub mod practice;
#[cfg_attr(target_os = "macos", path = "practice_accessibility_macos.rs")]
pub mod practice_accessibility;
pub mod runtime;
pub mod settings;
pub mod shortcuts;
pub mod ui;
pub mod window;

/// Chooses the Windows or macOS wording for text about the operating system.
pub fn platform_text(windows: &'static str, macos: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        macos
    } else {
        windows
    }
}

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
            // macOS keeps its native title bar and traffic-light buttons.
            .with_decorations(cfg!(target_os = "macos"))
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
