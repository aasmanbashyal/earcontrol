use crate::model::MacroStep;
use crate::shortcuts::parse_shortcut;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Runs a validated macro in order, stopping at the first cancellation or failure.
/// Native effects are injected so ordering and cancellation can be tested without
/// sending input to the desktop. Each text batch stays on Unicode character boundaries.
pub fn execute_macro(
    steps: &[MacroStep],
    mut check_target: impl FnMut() -> Result<(), String>,
    mut send_keys: impl FnMut(&[u16]) -> Result<(), String>,
    mut send_text: impl FnMut(&str) -> Result<(), String>,
    mut wait: impl FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    for step in steps {
        check_target()?;
        match step {
            MacroStep::Key { keys } => send_keys(&parse_shortcut(keys)?)?,
            MacroStep::Delay { ms } => wait(*ms)?,
            MacroStep::Text { text } => {
                let characters: Vec<_> = text.chars().collect();
                for chunk in characters.chunks(32) {
                    check_target()?;
                    send_text(&chunk.iter().collect::<String>())?;
                }
            }
        }
    }
    Ok(())
}

/// Drops repeat or duplicate reports for one physical media command.
/// Rejected events do not move the cooldown window forward.
/// Pass the capture timestamp, never the worker's processing time: a busy worker
/// must not turn two reports of one press into two commands.
/// A zero cooldown disables same-source repeat filtering. Cross-source reports
/// retain a short guard because Windows can report one press through both APIs.
#[derive(Default)]
pub struct DedupGate {
    last_accepted: HashMap<String, (Instant, String)>,
}

impl DedupGate {
    pub fn accept(&mut self, trigger: &str, source: &str, now: Instant, cooldown_ms: u64) -> bool {
        if let Some((previous_at, previous_source)) = self.last_accepted.get(trigger) {
            let threshold_ms = if previous_source == source {
                cooldown_ms
            } else {
                cooldown_ms.max(180)
            };
            // A timestamp from before the last accepted event is stale, not a new press.
            let Some(elapsed) = now.checked_duration_since(*previous_at) else {
                return false;
            };
            if elapsed < Duration::from_millis(threshold_ms) {
                return false;
            }
        }
        self.last_accepted
            .insert(trigger.into(), (now, source.into()));
        true
    }

    pub fn reset(&mut self) {
        self.last_accepted.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn macros_preserve_key_delay_and_text_order() {
        let effects = RefCell::new(Vec::new());
        execute_macro(
            &[
                MacroStep::Key {
                    keys: "Ctrl+F5".into(),
                },
                MacroStep::Delay { ms: 500 },
                MacroStep::Text {
                    text: "Hello".into(),
                },
            ],
            || Ok(()),
            |keys| {
                effects.borrow_mut().push(format!("keys:{keys:?}"));
                Ok(())
            },
            |text| {
                effects.borrow_mut().push(format!("text:{text}"));
                Ok(())
            },
            |ms| {
                effects.borrow_mut().push(format!("wait:{ms}"));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            *effects.borrow(),
            ["keys:[17, 116]", "wait:500", "text:Hello"]
        );
    }

    #[test]
    fn focus_loss_prevents_the_next_step_from_sending_input() {
        let focus_checks = Cell::new(0);
        let effects = RefCell::new(Vec::new());
        let result = execute_macro(
            &[
                MacroStep::Key {
                    keys: "Right".into(),
                },
                MacroStep::Key {
                    keys: "Left".into(),
                },
                MacroStep::Text {
                    text: "must not be typed".into(),
                },
            ],
            || {
                focus_checks.set(focus_checks.get() + 1);
                if focus_checks.get() > 1 {
                    Err("focus lost".into())
                } else {
                    Ok(())
                }
            },
            |keys| {
                effects.borrow_mut().extend_from_slice(keys);
                Ok(())
            },
            |_| panic!("text after focus loss"),
            |_| panic!("unexpected delay"),
        );
        assert_eq!(result, Err("focus lost".into()));
        assert_eq!(*effects.borrow(), [0x27]);
    }

    #[test]
    fn cancelling_a_delay_prevents_all_remaining_steps() {
        let effects = RefCell::new(Vec::new());
        let result = execute_macro(
            &[
                MacroStep::Key {
                    keys: "Right".into(),
                },
                MacroStep::Delay { ms: 30_000 },
                MacroStep::Key {
                    keys: "Left".into(),
                },
            ],
            || Ok(()),
            |keys| {
                effects.borrow_mut().extend_from_slice(keys);
                Ok(())
            },
            |_| panic!("unexpected text"),
            |_| Err("cancelled".into()),
        );
        assert_eq!(result, Err("cancelled".into()));
        assert_eq!(*effects.borrow(), [0x27]);
    }

    #[test]
    fn failed_key_delivery_does_not_run_later_steps() {
        let result = execute_macro(
            &[
                MacroStep::Key {
                    keys: "Right".into(),
                },
                MacroStep::Text {
                    text: "later".into(),
                },
            ],
            || Ok(()),
            |_| Err("Windows rejected input".into()),
            |_| panic!("text after failed input"),
            |_| panic!("unexpected delay"),
        );
        assert_eq!(result, Err("Windows rejected input".into()));
    }

    #[test]
    fn long_unicode_text_is_batched_without_splitting_characters() {
        let text = "नमस्ते 🌿 café ".repeat(7);
        let chunks = RefCell::new(Vec::<String>::new());
        execute_macro(
            &[MacroStep::Text { text: text.clone() }],
            || Ok(()),
            |_| panic!("unexpected key"),
            |chunk| {
                chunks.borrow_mut().push(chunk.into());
                Ok(())
            },
            |_| panic!("unexpected delay"),
        )
        .unwrap();
        let chunks = chunks.borrow();
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 32));
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn stop_prevents_the_next_text_batch() {
        let cancelled = Cell::new(false);
        let typed = RefCell::new(String::new());
        let result = execute_macro(
            &[MacroStep::Text {
                text: "x".repeat(100),
            }],
            || {
                if cancelled.get() {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            },
            |_| panic!("unexpected key"),
            |chunk| {
                typed.borrow_mut().push_str(chunk);
                cancelled.set(true);
                Ok(())
            },
            |_| panic!("unexpected delay"),
        );
        assert_eq!(result, Err("cancelled".into()));
        assert_eq!(typed.borrow().len(), 32);
    }

    #[test]
    fn duplicate_sources_cannot_double_advance_one_command() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        assert!(gate.accept("next", "Keyboard media key", now, 80));
        assert!(!gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(100),
            80
        ));
        assert!(gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(180),
            80
        ));
    }

    #[test]
    fn disabled_repeat_delay_accepts_rapid_presses_from_the_same_source() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        for milliseconds in [0, 1, 2, 79, 80] {
            assert!(gate.accept(
                "next",
                "Keyboard media key",
                now + Duration::from_millis(milliseconds),
                0
            ));
        }
    }

    #[test]
    fn disabled_repeat_delay_still_filters_duplicate_windows_reports() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        assert!(gate.accept("next", "Keyboard media key", now, 0));
        assert!(!gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(100),
            0
        ));
        assert!(gate.accept(
            "next",
            "Keyboard media key",
            now + Duration::from_millis(101),
            0
        ));
        assert!(gate.accept(
            "previous",
            "Windows media session",
            now + Duration::from_millis(102),
            0
        ));
    }

    #[test]
    fn repeated_reports_do_not_extend_the_cooldown() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        assert!(gate.accept("play_pause", "Windows media session", now, 250));
        assert!(!gate.accept(
            "play_pause",
            "Windows media session",
            now + Duration::from_millis(100),
            250
        ));
        assert!(!gate.accept(
            "play_pause",
            "Windows media session",
            now + Duration::from_millis(249),
            250
        ));
        assert!(gate.accept(
            "play_pause",
            "Windows media session",
            now + Duration::from_millis(250),
            250
        ));
    }

    #[test]
    fn different_buttons_have_independent_cooldowns() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        assert!(gate.accept("next", "Windows media session", now, 500));
        assert!(gate.accept(
            "previous",
            "Windows media session",
            now + Duration::from_millis(20),
            500
        ));
        assert!(!gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(30),
            500
        ));
    }

    #[test]
    fn restarting_a_session_clears_old_button_cooldowns() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        assert!(gate.accept("next", "Windows media session", now, 2_000));
        gate.reset();
        assert!(gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(10),
            2_000
        ));
    }

    #[test]
    fn delayed_old_event_cannot_rewind_the_gate() {
        let mut gate = DedupGate::default();
        let now = Instant::now();
        assert!(gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(100),
            180
        ));
        assert!(!gate.accept("next", "Windows media session", now, 180));
        assert!(gate.accept(
            "next",
            "Windows media session",
            now + Duration::from_millis(280),
            180
        ));
    }
}
