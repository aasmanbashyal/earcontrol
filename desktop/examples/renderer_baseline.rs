//! Diagnostic memory control: an empty window using the application's renderer.
//! `--native-init` additionally initializes the idle Windows media bridge.
//! It requires an isolated EARCONTROL_SETTINGS_PATH and never starts capture.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use earcontrol_lib::{design::Palette, runtime::AppState};
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::Arc;

struct RendererBaseline {
    _runtime: Option<Arc<AppState>>,
}

impl eframe::App for RendererBaseline {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Palette::get(ctx).bg))
            .show(ctx, |_| {});
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let native_init = std::env::args().any(|argument| argument == "--native-init");
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
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
        Box::new(move |cc| {
            let runtime = if native_init {
                let path = std::env::var_os("EARCONTROL_SETTINGS_PATH")
                    .ok_or("The native diagnostic requires an isolated settings path.")?;
                let ctx = cc.egui_ctx.clone();
                let runtime = AppState::new(path.into(), move || {
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                });
                let RawWindowHandle::Win32(handle) = cc.window_handle()?.as_raw() else {
                    return Err("This diagnostic requires Windows.".into());
                };
                runtime.initialize(handle.hwnd.get());
                if !runtime.status().media_ready {
                    return Err(runtime
                        .warning()
                        .unwrap_or("Native initialization failed.".into())
                        .into());
                }
                Some(runtime)
            } else {
                None
            };
            Ok(Box::new(RendererBaseline { _runtime: runtime }))
        }),
    )?;
    Ok(())
}
