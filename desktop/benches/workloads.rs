use earcontrol_lib::{
    engine,
    model::{self, MacroStep, Settings},
    practice::Practice,
    runtime::AppState,
    shortcuts,
    ui::{self, EarControl, View},
};
use eframe::egui;
use serde_json::{json, Value};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

// Batch timing amortizes the timer cost. Quantiles describe batch means, not
// individual-event latency. OS delivery, GPU submission and real delays are excluded.
fn measure(name: &str, iterations: usize, mut operation: impl FnMut()) -> Value {
    for _ in 0..100 {
        operation();
    }
    let mut samples = Vec::new();
    for _ in 0..21 {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        samples.push(start.elapsed().as_secs_f64() * 1_000_000.0 / iterations as f64);
    }
    samples.sort_by(f64::total_cmp);
    json!({"workload":name, "iterations_per_batch":iterations, "batches":samples.len(), "median_us":samples[samples.len()/2], "p95_batch_mean_us":samples[19], "batch_mean_us":samples})
}
fn main() {
    let mut results = Vec::new();
    let settings = Settings::default();
    let encoded = serde_json::to_string(&settings).unwrap();
    results.push(measure("parse_shortcut_ctrl_shift_f5", 10000, || {
        black_box(shortcuts::parse_shortcut(black_box("Ctrl+Shift+F5")).unwrap());
    }));
    results.push(measure("validate_default_profiles", 1000, || {
        model::validate_settings(black_box(&settings)).unwrap();
    }));
    results.push(measure("decode_and_validate_profiles", 1000, || {
        let decoded: Settings = serde_json::from_str(black_box(&encoded)).unwrap();
        model::validate_settings(black_box(&decoded)).unwrap();
    }));
    let mut gate = engine::DedupGate::default();
    let mut captured = Instant::now();
    results.push(measure("dedup_accept_command", 10000, || {
        captured += Duration::from_millis(300);
        black_box(gate.accept(black_box("next"), "Windows media session", captured, 250));
    }));
    let macro_steps = vec![
        MacroStep::Key {
            keys: "Ctrl+F5".into(),
        },
        MacroStep::Text {
            text: "Hello नमस्ते 🦀".repeat(30),
        },
    ];
    results.push(measure("macro_dispatch_unicode_no_os_io", 1000, || {
        engine::execute_macro(
            black_box(&macro_steps),
            || Ok(()),
            |keys| {
                black_box(keys);
                Ok(())
            },
            |text| {
                black_box(text);
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();
    }));
    let directory = tempfile::tempdir().unwrap();
    let runtime = AppState::new(directory.path().join("profiles.json"), || {});
    let mut app = EarControl::from_runtime(runtime);
    let ctx = egui::Context::default();
    ui::configure_style(&ctx);
    for (view, name) in [
        (View::Buttons, "ui_buttons"),
        (View::Macros, "ui_macros"),
        (View::Devices, "ui_devices"),
        (View::Activity, "ui_activity"),
        (View::Settings, "ui_settings"),
    ] {
        app.view = view;
        results.push(measure(name, 200, || {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1160.0, 780.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| app.show(ctx));
            black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
        }));
    }
    let mut practice = Practice::default();
    results.push(measure("ui_practice", 200, || {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 640.0),
            )),
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| practice.show(ctx));
        black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
    }));
    // Diagnostic for large configurations. The original eleven workload rows
    // above remain unchanged so historical comparisons use the same inputs.
    let mut loaded = Settings::default();
    let template = loaded.profiles[0].clone();
    loaded.profiles = (0..32)
        .map(|index| {
            let mut profile = template.clone();
            profile.id = format!("benchmark-{index}");
            profile.name = format!("Benchmark profile {index}");
            for mapping in &mut profile.mappings {
                mapping.id = format!("{}-{}", profile.id, mapping.trigger);
                mapping.steps = vec![MacroStep::Text {
                    text: "नमस्ते 🦀 ".repeat(150),
                }];
            }
            profile
        })
        .collect();
    loaded.active_profile_id = loaded.profiles[0].id.clone();
    let loaded_path = directory.path().join("loaded-profiles.json");
    earcontrol_lib::settings::save_settings(&loaded_path, &loaded).unwrap();
    let mut loaded_app = EarControl::from_runtime(AppState::new(loaded_path, || {}));
    results.push(measure(
        "ui_buttons_32_profiles_unicode_macros",
        200,
        || {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1160.0, 780.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| loaded_app.show(ctx));
            black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
        },
    ));
    println!("{}", serde_json::to_string_pretty(&json!({"version":env!("CARGO_PKG_VERSION"), "units":"microseconds", "method":"21 warmed batches, CPU only", "results":results})).unwrap());
}
