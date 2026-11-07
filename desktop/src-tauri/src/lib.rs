pub mod model;
pub mod native;
pub mod settings;
pub mod shortcuts;
pub mod engine;

use model::{ActivityEvent, Bootstrap, DeviceInfo, Mapping, RuntimeStatus, Settings, TargetWindow};
use std::{path::PathBuf, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Arc, Mutex}, thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Default)]
struct Session { mode: String, target: Option<TargetWindow>, generation: u64 }

pub struct AppState {
    app: AppHandle,
    settings: Mutex<Settings>,
    session: Mutex<Session>,
    native: Mutex<Option<native::NativeInput>>,
    busy: AtomicBool,
    event_id: AtomicU64,
    last_events: Mutex<engine::DedupGate>,
    path: PathBuf,
    warning: Mutex<Option<String>>,
}

impl AppState {
    fn status(&self) -> RuntimeStatus {
        let session = self.session.lock().unwrap();
        RuntimeStatus {
            mode: session.mode.clone(), target: session.target.clone(),
            running_macro: self.busy.load(Ordering::Acquire),
            media_ready: self.native.lock().unwrap().is_some(),
            message: match session.mode.as_str() {
                "control" => "Keep the chosen window and its presentation tab active.",
                "monitor" => "Listening for headphone commands. No shortcuts are sent.",
                _ => "Choose a profile and a window to get started.",
            }.into(),
        }
    }

    fn publish_status(&self) { let _ = self.app.emit("status", self.status()); }

    fn event(&self, trigger: &str, source: &str, outcome: &str, detail: impl Into<String>) {
        let event = ActivityEvent {
            id: self.event_id.fetch_add(1, Ordering::Relaxed),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            trigger: trigger.into(), source: source.into(), outcome: outcome.into(), detail: detail.into(),
        };
        let _ = self.app.emit("activity", event);
    }

    fn stop(&self) -> Result<(), String> {
        {
            let mut session = self.session.lock().unwrap();
            session.generation = session.generation.wrapping_add(1);
            session.mode = "idle".into();
        }
        self.last_events.lock().unwrap().reset();
        let result = self.native.lock().unwrap().as_ref().map(|input| input.stop()).unwrap_or(Ok(()));
        self.publish_status();
        result
    }

    // Called by the keyboard hook. A contended lock means pass through.
    fn should_consume(&self, trigger: &str) -> bool {
        let Ok(session) = self.session.try_lock() else { return false };
        if session.mode != "control" { return false; }
        let Some(target) = &session.target else { return false };
        if !native::is_foreground(target) || native::modifiers_held() { return false; }
        let Ok(settings) = self.settings.try_lock() else { return false };
        settings.profiles.iter().find(|profile| profile.id == settings.active_profile_id)
            .and_then(|profile| profile.mappings.iter().find(|mapping| mapping.trigger == trigger))
            .is_some_and(|mapping| mapping.enabled && !mapping.steps.is_empty())
    }

    fn on_command(self: &Arc<Self>, trigger: String, source: String, native_generation: u64, captured_at: Instant) {
        let session = self.session.lock().unwrap().clone();
        if session.mode == "idle" { return; }
        if !self.native.lock().unwrap().as_ref().is_some_and(|input| input.generation() == native_generation) { return; }
        let settings = self.settings.lock().unwrap().clone();
        if !self.last_events.lock().unwrap().accept(&trigger, &source, captured_at, settings.cooldown_ms) { return; }
        if session.mode == "monitor" {
            self.event(&trigger, &source, "detected", "Command received. No shortcut sent.");
            return;
        }
        let mapping = settings.profiles.iter().find(|profile| profile.id == settings.active_profile_id)
            .and_then(|profile| profile.mappings.iter().find(|mapping| mapping.trigger == trigger)).cloned();
        let Some(mapping) = mapping.filter(|mapping| mapping.enabled && !mapping.steps.is_empty()) else {
            self.event(&trigger, &source, "disabled", "This button has no enabled action."); return;
        };
        let Some(target) = session.target else { return };
        if !native::is_foreground(&target) {
            self.event(&trigger, &source, "skipped", "The chosen window is not active."); return;
        }
        if native::modifiers_held() {
            self.event(&trigger, &source, "skipped", "Release Ctrl, Alt, Shift, and Windows first."); return;
        }
        if self.busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            self.event(&trigger, &source, "skipped", "A macro is still running."); return;
        }
        self.launch_macro(mapping, target, session.generation, source, 0);
    }

    fn ensure_target(&self, target: &TargetWindow, generation: u64) -> Result<(), String> {
        if self.session.lock().unwrap().generation != generation { return Err("Macro cancelled.".into()); }
        if !native::is_foreground(target) { return Err("Macro stopped because the chosen window lost focus.".into()); }
        if native::modifiers_held() { return Err("Macro stopped because a modifier key is held.".into()); }
        Ok(())
    }

    fn wait_cancellable(&self, millis: u64, generation: u64, target: Option<&TargetWindow>) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_millis(millis);
        loop {
            if self.session.lock().unwrap().generation != generation { return Err("Macro cancelled.".into()); }
            if let Some(target) = target { self.ensure_target(target, generation)?; }
            let now = Instant::now();
            if now >= deadline { return Ok(()); }
            thread::sleep((deadline - now).min(Duration::from_millis(20)));
        }
    }

    fn launch_macro(self: &Arc<Self>, mapping: Mapping, target: TargetWindow, generation: u64, source: String, countdown: u64) {
        self.publish_status();
        let state = Arc::clone(self);
        thread::spawn(move || {
            let result = (|| {
                if countdown > 0 { state.wait_cancellable(countdown, generation, None)?; }
                engine::execute_macro(
                    &mapping.steps,
                    || state.ensure_target(&target, generation),
                    native::send_keys,
                    native::send_text,
                    |ms| state.wait_cancellable(ms, generation, Some(&target)),
                )
            })();
            match result {
                Ok(()) => state.event(&mapping.trigger, &source, "executed", format!("{} · {} step{}", mapping.name, mapping.steps.len(), if mapping.steps.len() == 1 { "" } else { "s" })),
                Err(message) => state.event(&mapping.trigger, &source, "cancelled", message),
            }
            state.busy.store(false, Ordering::Release);
            state.publish_status();
        });
    }
}

#[tauri::command]
fn get_bootstrap(state: State<'_, Arc<AppState>>) -> Bootstrap {
    Bootstrap { settings: state.settings.lock().unwrap().clone(), status: state.status(), version: env!("CARGO_PKG_VERSION").into(), warning: state.warning.lock().unwrap().clone() }
}

#[tauri::command]
fn get_status(state: State<'_, Arc<AppState>>) -> RuntimeStatus { state.status() }

#[tauri::command]
fn list_windows(app: AppHandle) -> Result<Vec<TargetWindow>, String> {
    let mut windows = native::list_windows()?;
    if let Some(practice) = app.get_webview_window("practice") {
        windows.insert(0, TargetWindow { handle: practice.hwnd().map_err(|e| e.to_string())?.0 as isize, process_id: std::process::id(), title: "EarControl practice deck".into() });
    }
    Ok(windows)
}

#[tauri::command]
async fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    tauri::async_runtime::spawn_blocking(native::list_devices).await.map_err(|error| error.to_string())?
}

#[tauri::command]
fn save_settings(settings: Settings, state: State<'_, Arc<AppState>>) -> Result<Settings, String> {
    model::validate_settings(&settings)?;
    state.stop()?;
    settings::save_settings(&state.path, &settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    state.publish_status();
    Ok(settings)
}

#[tauri::command]
fn set_mode(mode: String, target: Option<TargetWindow>, state: State<'_, Arc<AppState>>) -> Result<RuntimeStatus, String> {
    if !["idle", "monitor", "control"].contains(&mode.as_str()) { return Err("Unknown control mode.".into()); }
    if mode == "idle" { state.stop()?; return Ok(state.status()); }
    let settings = state.settings.lock().unwrap().clone();
    model::validate_settings(&settings)?;
    if mode == "control" && target.as_ref().is_none_or(|window| window.handle == 0 || window.process_id == 0) {
        return Err("Choose a target window first.".into());
    }
    state.stop()?;
    {
        let input = state.native.lock().unwrap();
        input.as_ref().ok_or("Windows media controls are unavailable. Restart EarControl.")?.start(settings.keyboard_fallback)?;
    }
    {
        let mut session = state.session.lock().unwrap();
        session.mode = mode.clone();
        session.target = target;
    }
    state.event("session", "EarControl", "info", if mode == "control" { "Control started. Switch to your chosen window." } else { "Button monitor started." });
    state.publish_status();
    Ok(state.status())
}

#[tauri::command]
fn open_bluetooth_settings() -> Result<(), String> { native::open_bluetooth_settings() }

#[tauri::command]
async fn open_practice(app: AppHandle) -> Result<TargetWindow, String> {
    let window = if let Some(window) = app.get_webview_window("practice") { window } else {
        WebviewWindowBuilder::new(&app, "practice", WebviewUrl::App("practice.html".into()))
            .title("EarControl practice deck").inner_size(960.0, 640.0).min_inner_size(600.0, 400.0).decorations(false).center().focused(false)
            .build().map_err(|error| error.to_string())?
    };
    window.show().map_err(|error| error.to_string())?;
    Ok(TargetWindow { handle: window.hwnd().map_err(|error| error.to_string())?.0 as isize, process_id: std::process::id(), title: "EarControl practice deck".into() })
}

#[tauri::command]
fn preview_macro(mapping_id: String, target: TargetWindow, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let settings = state.settings.lock().unwrap().clone();
    model::validate_settings(&settings)?;
    let mapping = settings.profiles.iter().find(|profile| profile.id == settings.active_profile_id)
        .and_then(|profile| profile.mappings.iter().find(|mapping| mapping.id == mapping_id)).cloned().ok_or("Action not found.")?;
    if mapping.steps.is_empty() { return Err("Add a step before testing this macro.".into()); }
    state.stop()?;
    if state.busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() { return Err("Wait for the previous macro to finish cancelling.".into()); }
    let generation = state.session.lock().unwrap().generation;
    state.event(&mapping.trigger, "Preview", "info", "Test starts in 3 seconds. Switch to the chosen window. Stop cancels it.");
    state.inner().launch_macro(mapping, target, generation, "Preview".into(), 3000);
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") { let _ = window.show(); let _ = window.unminimize(); let _ = window.set_focus(); }
        }))
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("profiles.json");
            let (config, warning) = settings::load_settings(&path);
            let runtime = Arc::new(AppState {
                app: app.handle().clone(), settings: Mutex::new(config),
                session: Mutex::new(Session { mode: "idle".into(), ..Default::default() }),
                native: Mutex::new(None), busy: AtomicBool::new(false), event_id: AtomicU64::new(1),
                last_events: Mutex::new(engine::DedupGate::default()), path, warning: Mutex::new(warning),
            });
            let weak = Arc::downgrade(&runtime);
            let callback_state = Arc::downgrade(&runtime);
            let callback = Arc::new(move |trigger, source, native_generation, captured_at| {
                if let Some(state) = callback_state.upgrade() {
                    state.on_command(trigger, source, native_generation, captured_at);
                }
            });
            let predicate = Arc::new(move |trigger: &str| weak.upgrade().is_some_and(|state| state.should_consume(trigger)));
            let window = app.get_webview_window("main").expect("main window");
            match native::NativeInput::new(window.hwnd()?.0 as isize, callback, predicate) {
                Ok(input) => *runtime.native.lock().unwrap() = Some(input),
                Err(error) => {
                    // Keep configuration available even when Windows media registration fails.
                    *runtime.warning.lock().unwrap() = Some(format!("Windows media controls unavailable: {error}"));
                }
            }
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_bootstrap, get_status, list_windows, list_devices, save_settings, set_mode, open_bluetooth_settings, open_practice, preview_macro])
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                if let Some(state) = window.try_state::<Arc<AppState>>() { let _ = state.stop(); }
                window.app_handle().exit(0);
            }
        })
        .build(tauri::generate_context!()).expect("Could not start EarControl");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(state) = app.try_state::<Arc<AppState>>() { let _ = state.stop(); }
        }
    });
}
