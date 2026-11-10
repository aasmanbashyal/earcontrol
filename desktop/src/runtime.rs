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
}

pub struct AppState {
    repaint: Box<dyn Fn() + Send + Sync>,
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
            media_ready: self.native.lock().unwrap().is_some(),
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
        let event = ActivityEvent {
            id: self.event_id.fetch_add(1, Ordering::Relaxed),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            trigger: trigger.into(),
            source: source.into(),
            outcome: outcome.into(),
            detail: detail.into(),
        };
        let mut events = self.events.lock().unwrap();
        events.push_front(event);
        events.truncate(250);
        drop(events);
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
            self.event(
                &trigger,
                &source,
                "detected",
                "Command received. No shortcut sent.",
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
            self.event(
                &trigger,
                &source,
                "disabled",
                "This button has no enabled action.",
            );
            return;
        };
        let Some(target) = session.target else { return };
        if !native::is_foreground(&target) {
            self.event(
                &trigger,
                &source,
                "skipped",
                "The chosen window is not active.",
            );
            return;
        }
        if native::modifiers_held() {
            self.event(
                &trigger,
                &source,
                "skipped",
                "Release Ctrl, Alt, Shift, and Windows first.",
            );
            return;
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            self.event(&trigger, &source, "skipped", "A macro is still running.");
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
                Ok(()) => state.event(
                    &mapping.trigger,
                    &source,
                    "executed",
                    format!(
                        "{} · {} step{}",
                        mapping.name,
                        mapping.steps.len(),
                        if mapping.steps.len() == 1 { "" } else { "s" }
                    ),
                ),
                Err(message) => state.event(&mapping.trigger, &source, "cancelled", message),
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
            repaint: Box::new(repaint),
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
        match native::NativeInput::new(hwnd, callback, predicate) {
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
    pub fn events(&self) -> Vec<ActivityEvent> {
        self.events.lock().unwrap().iter().cloned().collect()
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
        self.native
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("Windows media controls are unavailable. Restart EarControl.")?
            .start(settings.keyboard_fallback)?;
        {
            let mut session = self.session.lock().unwrap();
            session.mode = mode.into();
            session.target = target;
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
}
