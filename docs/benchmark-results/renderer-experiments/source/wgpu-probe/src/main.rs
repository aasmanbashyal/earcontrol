#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, egui_wgpu};
use std::sync::Arc;

struct Probe;

impl eframe::App for Probe {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(250)))
            .show(ctx, |_| {});
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let memory_usage = std::env::args().any(|arg| arg == "--memory-usage");
    let mut setup = egui_wgpu::WgpuSetupCreateNew::default();
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .shader_compiler = wgpu::Dx12Compiler::Fxc;
    setup.device_descriptor = Arc::new(move |_| wgpu::DeviceDescriptor {
        label: Some("EarControl diagnostic DX12 device"),
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 8192,
            ..wgpu::Limits::default()
        },
        memory_hints: if memory_usage {
            wgpu::MemoryHints::MemoryUsage
        } else {
            wgpu::MemoryHints::Performance
        },
        ..Default::default()
    });
    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../../desktop/assets/icon.png"))?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EarControl")
            .with_inner_size([1160.0, 780.0])
            .with_min_inner_size([860.0, 620.0])
            .with_decorations(false)
            .with_icon(icon),
        centered: true,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup.into(),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "EarControl",
        options,
        Box::new(move |cc| {
            let render = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("No WGPU render state")?;
            let adapter = render.adapter.get_info();
            if adapter.backend != wgpu::Backend::Dx12 {
                return Err("The diagnostic must use Direct3D 12.".into());
            }
            println!(
                "adapter={:?};memory_policy={};compiler=FXC",
                adapter,
                if memory_usage {
                    "MemoryUsage"
                } else {
                    "Performance"
                }
            );
            Ok(Box::new(Probe))
        }),
    )?;
    Ok(())
}
