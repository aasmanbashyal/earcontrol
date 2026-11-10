use crate::{
    engine,
    model::{ActivityEvent, Mapping, RuntimeStatus, Settings, TargetWindow},
    native, settings,
};
use std::collections::VecDeque;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Default)]
struct Session {
    mode: String,
    target: Option<TargetWindow>,
    generation: u64,
    capture_event_start: u64,
    latest_capture_event_id: Option<u64>,
}

pub struct AppState {
    repaint: Arc<dyn Fn() + Send + Sync>,
    events: Mutex<VecDeque<ActivityEvent>>,
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
    pub fn status(&self) -> RuntimeStatus {
        let session = self.session.lock().unwrap();
        RuntimeStatus {
            mode: session.mode.clone(),
            target: session.target.clone(),
            running_macro: self.busy.load(Ordering::Acquire),
            media_ready: self.capture_health().input_thread_alive,
            message: match session.mode.as_str() {
                "control" => "Keep the chosen window and its presentation tab active.",
                "monitor" => "Listening for headphone commands. No shortcuts are sent.",
                _ => "Choose a profile and a window to get started.",
            }
            .into(),
        }
    }

    fn publish_status(&self) {
        (self.repaint)();
    }

    fn event(&self, trigger: &str, source: &str, outcome: &str, detail: impl Into<String>) {
        self.publish_event(trigger, source, outcome, detail.into(), None);
    }

    fn capture_event(
        &self,
        trigger: &str,
        source: &str,
        outcome: &str,
        detail: impl Into<String>,
        generation: u64,
    ) {
        self.publish_event(trigger, source, outcome, detail.into(), Some(generation));
    }

    fn publish_event(
        &self,
        trigger: &str,
        source: &str,
        outcome: &str,
        detail: String,
        generation: Option<u64>,
    ) {
        let event = ActivityEvent {
            id: self.event_id.fetch_add(1, Ordering::Relaxed),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            trigger: trigger.into(),
            source: source.into(),
            outcome: outcome.into(),
            detail,
        };
        let event_id = event.id;
        let mut events = self.events.lock().unwrap();
        events.push_front(event);
        events.truncate(250);
        drop(events);
        if let Some(generation) = generation {
            let mut session = self.session.lock().unwrap();
            if session.generation == generation
                && event_id >= session.capture_event_start
                && ["monitor", "control"].contains(&session.mode.as_str())
            {
                session.latest_capture_event_id = Some(event_id);
            }
        }
        (self.repaint)();
    }

    pub fn stop(&self) -> Result<(), String> {
        {
            let mut session = self.session.lock().unwrap();
            session.generation = session.generation.wrapping_add(1);
            session.mode = "idle".into();
        }
        self.last_events.lock().unwrap().reset();
        let result = self
            .native
            .lock()
            .unwrap()
            .as_ref()
            .map(|input| input.stop())
            .unwrap_or(Ok(()));
        self.publish_status();
        result
    }

    // Called by the keyboard hook. A contended lock means pass through.
    fn should_consume(&self, trigger: &str) -> bool {
        let Ok(session) = self.session.try_lock() else {
            return false;
        };
        if session.mode != "control" {
            return false;
        }
        let Some(target) = &session.target else {
            return false;
        };
        if !native::is_foreground(target) || native::modifiers_held() {
            return false;
        }
        let Ok(settings) = self.settings.try_lock() else {
            return false;
        };
        settings
            .profiles
            .iter()
            .find(|profile| profile.id == settings.active_profile_id)
            .and_then(|profile| {
                profile
                    .mappings
                    .iter()
                    .find(|mapping| mapping.trigger == trigger)
            })
            .is_some_and(|mapping| mapping.enabled && !mapping.steps.is_empty())
    }

    fn on_command(
        self: &Arc<Self>,
        trigger: String,
        source: String,
        native_generation: u64,
        captured_at: Instant,
    ) {
        let session = self.session.lock().unwrap().clone();
        if session.mode == "idle" {
            return;
        }
        if !self
            .native
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|input| input.generation() == native_generation)
        {
            return;
        }
        let settings = self.settings.lock().unwrap().clone();
        if !self.last_events.lock().unwrap().accept(
            &trigger,
            &source,
            captured_at,
            settings.cooldown_ms,
        ) {
            return;
        }
        if session.mode == "monitor" {
            self.capture_event(
                &trigger,
                &source,
                "detected",
                "Command received. No shortcut sent.",
                session.generation,
            );
            return;
        }
        let mapping = settings
            .profiles
            .iter()
            .find(|profile| profile.id == settings.active_profile_id)
            .and_then(|profile| {
                profile
                    .mappings
                    .iter()
                    .find(|mapping| mapping.trigger == trigger)
            })
            .cloned();
        let Some(mapping) = mapping.filter(|mapping| mapping.enabled && !mapping.steps.is_empty())
        else {
            self.capture_event(
                &trigger,
                &source,
                "disabled",
                "This button has no enabled action.",
                session.generation,
            );
            return;
        };
        let Some(target) = session.target else { return };
        if !native::is_foreground(&target) {
            self.capture_event(
                &trigger,
                &source,
                "skipped",
                "The chosen window is not active.",
                session.generation,
            );
            return;
        }
        if native::modifiers_held() {
            self.capture_event(
                &trigger,
                &source,
                "skipped",
                "Release Ctrl, Alt, Shift, and Windows first.",
                session.generation,
            );
            return;
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            self.capture_event(
                &trigger,
                &source,
                "skipped",
                "A macro is still running.",
                session.generation,
            );
            return;
        }
        self.launch_macro(mapping, target, session.generation, source, 0);
    }

    fn ensure_target(&self, target: &TargetWindow, generation: u64) -> Result<(), String> {
        if self.session.lock().unwrap().generation != generation {
            return Err("Macro cancelled.".into());
        }
        if !native::is_foreground(target) {
            return Err("Macro stopped because the chosen window lost focus.".into());
        }
        if native::modifiers_held() {
            return Err("Macro stopped because a modifier key is held.".into());
        }
        Ok(())
    }

    fn wait_cancellable(
        &self,
        millis: u64,
        generation: u64,
        target: Option<&TargetWindow>,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_millis(millis);
        loop {
            if self.session.lock().unwrap().generation != generation {
                return Err("Macro cancelled.".into());
            }
            if let Some(target) = target {
                self.ensure_target(target, generation)?;
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            thread::sleep((deadline - now).min(Duration::from_millis(20)));
        }
    }

    fn launch_macro(
        self: &Arc<Self>,
        mapping: Mapping,
        target: TargetWindow,
        generation: u64,
        source: String,
        countdown: u64,
    ) {
        self.publish_status();
        let state = Arc::clone(self);
        thread::spawn(move || {
            let result = (|| {
                if countdown > 0 {
                    state.wait_cancellable(countdown, generation, None)?;
                }
                engine::execute_macro(
                    &mapping.steps,
                    || state.ensure_target(&target, generation),
                    native::send_keys,
                    native::send_text,
                    |ms| state.wait_cancellable(ms, generation, Some(&target)),
                )
            })();
            match result {
                Ok(()) => state.capture_event(
                    &mapping.trigger,
                    &source,
                    "executed",
                    format!(
                        "{} · {} step{}",
                        mapping.name,
                        mapping.steps.len(),
                        if mapping.steps.len() == 1 { "" } else { "s" }
                    ),
                    generation,
                ),
                Err(message) => {
                    state.capture_event(&mapping.trigger, &source, "cancelled", message, generation)
                }
            }
            state.busy.store(false, Ordering::Release);
            state.publish_status();
        });
    }
}

impl AppState {
    pub fn new(path: PathBuf, repaint: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        let (config, warning) = settings::load_settings(&path);
        Arc::new(Self {
            repaint: Arc::new(repaint),
            events: Mutex::new(VecDeque::new()),
            settings: Mutex::new(config),
            session: Mutex::new(Session {
                mode: "idle".into(),
                ..Default::default()
            }),
            native: Mutex::new(None),
            busy: AtomicBool::new(false),
            event_id: AtomicU64::new(1),
            last_events: Mutex::new(engine::DedupGate::default()),
            path,
            warning: Mutex::new(warning),
        })
    }

    pub fn initialize(self: &Arc<Self>, hwnd: isize) {
        let callback_state = Arc::downgrade(self);
        let weak = Arc::downgrade(self);
        let callback = Arc::new(move |trigger, source, generation, captured_at| {
            if let Some(state) = callback_state.upgrade() {
                state.on_command(trigger, source, generation, captured_at);
            }
        });
        let predicate = Arc::new(move |trigger: &str| {
            weak.upgrade()
                .is_some_and(|state| state.should_consume(trigger))
        });
        // A joined native/route worker must not temporarily become the last
        // AppState owner, which would make its destructor join its own thread.
        let health_changed = Arc::clone(&self.repaint);
        match native::NativeInput::new(hwnd, callback, predicate, health_changed) {
            Ok(input) => *self.native.lock().unwrap() = Some(input),
            Err(error) => {
                *self.warning.lock().unwrap() =
                    Some(format!("Windows media controls unavailable: {error}"))
            }
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
    pub fn warning(&self) -> Option<String> {
        self.warning.lock().unwrap().clone()
    }
    pub fn capture_health(&self) -> native::CaptureHealth {
        self.native.lock().unwrap().as_ref().map_or_else(
            || native::CaptureHealth {
                route: native::CaptureRoute::Unavailable,
                ..Default::default()
            },
            native::NativeInput::capture_health,
        )
    }

    pub fn retry_capture(&self) -> Result<(), String> {
        let session = self.session.lock().unwrap().clone();
        if !["monitor", "control"].contains(&session.mode.as_str()) {
            return Err("Start Monitor or control before retrying input.".into());
        }
        self.set_mode(&session.mode, session.target)
    }
    pub fn events(&self) -> Vec<ActivityEvent> {
        self.events.lock().unwrap().iter().cloned().collect()
    }
    /// The Buttons preview only needs one command, not a copy of its history.
    pub fn latest_command_event(&self) -> Option<ActivityEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .find(|event| crate::model::TRIGGERS.contains(&event.trigger.as_str()))
            .cloned()
    }
    pub fn latest_capture_command_event(&self) -> Option<ActivityEvent> {
        let (start, id) = {
            let session = self.session.lock().unwrap();
            (
                session.capture_event_start,
                session.latest_capture_event_id?,
            )
        };
        self.events
            .lock()
            .unwrap()
            .iter()
            .find(|event| event.id == id && event.id >= start)
            .cloned()
    }
    pub fn clear_events(&self) {
        self.events.lock().unwrap().clear();
    }

    pub fn save(&self, settings: Settings) -> Result<(), String> {
        crate::model::validate_settings(&settings)?;
        self.stop()?;
        settings::save_settings(&self.path, &settings)?;
        *self.settings.lock().unwrap() = settings;
        self.publish_status();
        Ok(())
    }

    pub fn set_mode(&self, mode: &str, target: Option<TargetWindow>) -> Result<(), String> {
        if !["idle", "monitor", "control"].contains(&mode) {
            return Err("Unknown control mode.".into());
        }
        if mode == "idle" {
            return self.stop();
        }
        let settings = self.settings();
        crate::model::validate_settings(&settings)?;
        if mode == "control"
            && target
                .as_ref()
                .is_none_or(|window| !crate::window::target_exists(window))
        {
            return Err("Choose an open target window first.".into());
        }
        self.stop()?;
        let capture_event_start = self.event_id.load(Ordering::Relaxed);
        {
            let mut session = self.session.lock().unwrap();
            session.mode = mode.into();
            session.target = target;
            session.capture_event_start = capture_event_start;
            session.latest_capture_event_id = None;
        }
        // Establish the runtime generation before native enable can produce its
        // first callback. A startup failure rolls back to idle and invalidates
        // queued reports, so no partially started control session remains.
        let started = self
            .native
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| "Windows media controls are unavailable. Restart EarControl.".to_owned())
            .and_then(|input| input.start(settings.keyboard_fallback));
        if let Err(error) = started {
            let _ = self.stop();
            return Err(error);
        }
        self.event(
            "session",
            "EarControl",
            "info",
            if mode == "control" {
                "Control started. Switch to your chosen window."
            } else {
                "Button monitor started."
            },
        );
        self.publish_status();
        Ok(())
    }

    pub fn preview(self: &Arc<Self>, mapping_id: &str, target: TargetWindow) -> Result<(), String> {
        let settings = self.settings();
        crate::model::validate_settings(&settings)?;
        let mapping = settings
            .profiles
            .iter()
            .find(|profile| profile.id == settings.active_profile_id)
            .and_then(|profile| {
                profile
                    .mappings
                    .iter()
                    .find(|mapping| mapping.id == mapping_id)
            })
            .cloned()
            .ok_or("Action not found.")?;
        if !mapping.enabled || mapping.steps.is_empty() {
            return Err("Enable an action with at least one step before testing.".into());
        }
        if !crate::window::target_exists(&target) {
            return Err("Choose an open target window first.".into());
        }
        self.stop()?;
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("Wait for the previous macro to finish cancelling.".into());
        }
        let generation = self.session.lock().unwrap().generation;
        self.event(
            &mapping.trigger,
            "Preview",
            "info",
            "Test starts in 3 seconds. Switch to the chosen window. Stop cancels it.",
        );
        self.launch_macro(mapping, target, generation, "Preview".into(), 3000);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_does_not_start_capture_from_idle() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        assert!(runtime.retry_capture().is_err());
        assert_eq!(runtime.status().mode, "idle");
        assert!(!runtime.capture_health().input_thread_alive);
        assert_eq!(
            runtime.capture_health().route,
            native::CaptureRoute::Unavailable
        );
    }

    #[test]
    fn failed_capture_start_rolls_back_runtime_mode() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        assert!(runtime.set_mode("monitor", None).is_err());
        assert_eq!(runtime.status().mode, "idle");
        assert!(runtime.latest_capture_command_event().is_none());
        assert!(!runtime.capture_health().input_thread_alive);
    }

    #[test]
    fn current_capture_preview_excludes_old_history_and_late_old_macro_outcomes() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        runtime.event("previous", "test", "detected", "old history");
        {
            let mut session = runtime.session.lock().unwrap();
            session.mode = "monitor".into();
            session.generation = 1;
            session.capture_event_start = runtime.event_id.load(Ordering::Relaxed);
        }
        assert!(runtime.latest_capture_command_event().is_none());
        runtime.capture_event("next", "test", "detected", "current report", 1);
        assert_eq!(
            runtime.latest_capture_command_event().unwrap().trigger,
            "next"
        );
        {
            let mut session = runtime.session.lock().unwrap();
            session.generation = 2;
            session.capture_event_start = runtime.event_id.load(Ordering::Relaxed);
            session.latest_capture_event_id = None;
        }
        runtime.capture_event(
            "next",
            "test",
            "cancelled",
            "old macro finished after retry",
            1,
        );
        assert!(runtime.latest_capture_command_event().is_none());
        assert_eq!(runtime.latest_command_event().unwrap().outcome, "cancelled");
        runtime.capture_event("play_pause", "test", "detected", "new report", 2);
        assert_eq!(
            runtime.latest_capture_command_event().unwrap().trigger,
            "play_pause"
        );
        runtime.clear_events();
        assert!(runtime.latest_capture_command_event().is_none());
    }
    #[test]
    fn activity_is_bounded_and_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        for _ in 0..300 {
            runtime.event("next", "test", "detected", "received");
        }
        let events = runtime.events();
        assert_eq!(events.len(), 250);
        assert_eq!(events[0].id, 300);
        runtime.clear_events();
        assert!(runtime.events().is_empty());
    }
    #[test]
    fn save_rejects_invalid_draft_and_stop_invalidates_pending_work() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        let mut invalid = runtime.settings();
        invalid.profiles.clear();
        assert!(runtime.save(invalid).is_err());
        assert_eq!(runtime.settings(), Settings::default());
        let generation = runtime.session.lock().unwrap().generation;
        runtime.stop().unwrap();
        assert!(runtime.wait_cancellable(0, generation, None).is_err());
        assert_eq!(runtime.status().mode, "idle");
    }

    #[test]
    fn button_preview_returns_latest_command_through_full_history_and_session_events() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        for index in 0..300 {
            runtime.event("previous", "test", "detected", format!("event {index}"));
        }
        runtime.event("next", "test", "detected", "latest button");
        runtime.event("session", "test", "info", "session changed");
        let latest = runtime.latest_command_event().unwrap();
        assert_eq!(latest.trigger, "next");
        assert_eq!(latest.detail, "latest button");
        assert_eq!(runtime.events().len(), 250);
        runtime.clear_events();
        assert!(runtime.latest_command_event().is_none());
    }
}
