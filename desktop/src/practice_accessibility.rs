//! AccessKit bridge for eframe 0.33 child windows. Its built-in adapter only
//! attaches to the root window; this uses public egui hooks and the same Windows
//! adapter without changing eframe or creating another application process.
use eframe::egui::{self, accesskit, Plugin};
use std::{
    cell::RefCell,
    collections::VecDeque,
    marker::PhantomData,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use windows::Win32::{
    Foundation::HWND,
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow, IsWindowVisible},
};

#[derive(Default)]
struct Actions {
    open: AtomicBool,
    pending: Mutex<VecDeque<accesskit::ActionRequest>>,
}

struct Handler {
    ctx: egui::Context,
    viewport: egui::ViewportId,
    actions: Arc<Actions>,
}

impl accesskit::ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        self.ctx.enable_accesskit();
        self.ctx.request_repaint_of(self.viewport);
        // The very next child frame supplies a complete egui tree.
        None
    }
}

impl accesskit::ActionHandler for Handler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        if self.actions.open.load(Ordering::Acquire) {
            let mut pending = self.actions.pending.lock().unwrap();
            if pending.len() < 64 {
                pending.push_back(request);
            }
            drop(pending);
            self.ctx.request_repaint_of(self.viewport);
        }
    }
}

struct NativeAdapter {
    owner: u64,
    adapter: accesskit_windows::SubclassingAdapter,
}

thread_local! {
    // Win32 subclassing and its destruction must stay on the window's thread.
    static ADAPTER: RefCell<Option<NativeAdapter>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct BridgePlugin {
    owner: u64,
    viewport: Option<egui::ViewportId>,
    actions: Option<Arc<Actions>>,
    ended_viewport: Option<egui::ViewportId>,
}

impl Plugin for BridgePlugin {
    fn debug_name(&self) -> &'static str {
        "Practice accessibility"
    }

    fn input_hook(&mut self, input: &mut egui::RawInput) {
        if Some(input.viewport_id) != self.viewport {
            return;
        }
        if let Some(actions) = &self.actions {
            let mut pending = actions.pending.lock().unwrap();
            if actions.open.load(Ordering::Acquire) {
                input
                    .events
                    .extend(pending.drain(..).map(egui::Event::AccessKitActionRequest));
            } else {
                pending.clear();
            }
        }
    }

    fn on_end_pass(&mut self, ctx: &egui::Context) {
        // end_pass pops the viewport before output_hook, so remember it here.
        self.ended_viewport = Some(ctx.viewport_id());
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        if self.ended_viewport.take() != self.viewport {
            return;
        }
        let events = ADAPTER.with(|slot| {
            let mut slot = slot.borrow_mut();
            let native = slot.as_mut().filter(|native| native.owner == self.owner)?;
            let update = output.platform_output.accesskit_update.take()?;
            native.adapter.update_if_active(|| update)
        });
        // Raising UIA events can re-enter Win32; release the thread-local borrow.
        if let Some(events) = events {
            events.raise();
        }
    }
}

/// Owns the child window's adapter. It deliberately cannot move to another thread.
pub struct Bridge {
    owner: u64,
    actions: Arc<Actions>,
    _window_thread: PhantomData<Rc<()>>,
}

impl Bridge {
    /// Attach while the child is still hidden, then let its caller show it.
    pub fn attach(ctx: &egui::Context, handle: isize) -> Result<Self, String> {
        let hwnd = HWND(handle as *mut _);
        let mut process = 0;
        let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process)) };
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool()
            || process != std::process::id()
            || thread != unsafe { GetCurrentThreadId() }
        {
            return Err("The practice window is not owned by this UI thread.".into());
        }
        if unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return Err(
                "Practice accessibility must initialize before its window is shown.".into(),
            );
        }
        if ADAPTER.with(|slot| slot.borrow().is_some()) {
            return Err("A practice accessibility adapter is already active.".into());
        }
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        let owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
        let actions = Arc::new(Actions::default());
        actions.open.store(true, Ordering::Release);
        let handler = || Handler {
            ctx: ctx.clone(),
            viewport: ctx.viewport_id(),
            actions: Arc::clone(&actions),
        };
        let adapter = accesskit_windows::SubclassingAdapter::new(
            accesskit_windows::HWND(handle as *mut _),
            handler(),
            handler(),
        );
        ADAPTER.with(|slot| {
            *slot.borrow_mut() = Some(NativeAdapter { owner, adapter });
        });
        ctx.add_plugin(BridgePlugin::default());
        ctx.with_plugin(|plugin: &mut BridgePlugin| {
            plugin.owner = owner;
            plugin.viewport = Some(ctx.viewport_id());
            plugin.actions = Some(Arc::clone(&actions));
        });
        Ok(Self {
            owner,
            actions,
            _window_thread: PhantomData,
        })
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.actions.open.store(false, Ordering::Release);
        self.actions.pending.lock().unwrap().clear();
        let _ = ADAPTER.try_with(|slot| {
            let native = {
                let mut slot = slot.borrow_mut();
                if slot
                    .as_ref()
                    .is_some_and(|native| native.owner == self.owner)
                {
                    slot.take()
                } else {
                    None
                }
            };
            // Unsubclass outside the RefCell borrow. The adapter also handles an
            // HWND that has already received WM_NCDESTROY during viewport close.
            drop(native);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_only_reach_the_open_practice_viewport() {
        let actions = Arc::new(Actions::default());
        actions.open.store(true, Ordering::Release);
        let request = accesskit::ActionRequest {
            action: accesskit::Action::Click,
            target: accesskit::NodeId(42),
            data: None,
        };
        actions.pending.lock().unwrap().push_back(request.clone());
        let mut plugin = BridgePlugin {
            viewport: Some(egui::ViewportId::from_hash_of("practice-test")),
            actions: Some(Arc::clone(&actions)),
            ..Default::default()
        };
        let mut root = egui::RawInput::default();
        plugin.input_hook(&mut root);
        assert!(root.events.is_empty());
        assert_eq!(actions.pending.lock().unwrap().len(), 1);
        let mut child = egui::RawInput {
            viewport_id: egui::ViewportId::from_hash_of("practice-test"),
            ..Default::default()
        };
        plugin.input_hook(&mut child);
        assert!(
            matches!(child.events.as_slice(), [egui::Event::AccessKitActionRequest(r)] if r.target == request.target)
        );
        actions.open.store(false, Ordering::Release);
        actions.pending.lock().unwrap().push_back(request);
        child.events.clear();
        plugin.input_hook(&mut child);
        assert!(child.events.is_empty());
        assert!(actions.pending.lock().unwrap().is_empty());
    }

    #[test]
    fn child_bridge_preserves_root_accessibility_output() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        ctx.add_plugin(BridgePlugin::default());
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.button("Root button").clicked();
            });
        });
        assert!(output.platform_output.accesskit_update.is_some());
    }

    #[test]
    fn reopening_does_not_route_actions_from_the_closed_viewport() {
        let closed = crate::practice::Practice::default();
        let reopened = crate::practice::Practice::default();
        assert_ne!(closed.viewport_id(), reopened.viewport_id());
        let actions = Arc::new(Actions::default());
        actions.open.store(true, Ordering::Release);
        actions
            .pending
            .lock()
            .unwrap()
            .push_back(accesskit::ActionRequest {
                action: accesskit::Action::Focus,
                target: accesskit::NodeId(7),
                data: None,
            });
        let mut plugin = BridgePlugin {
            viewport: Some(reopened.viewport_id()),
            actions: Some(Arc::clone(&actions)),
            ..Default::default()
        };
        let mut input = egui::RawInput {
            viewport_id: closed.viewport_id(),
            ..Default::default()
        };
        plugin.input_hook(&mut input);
        assert!(input.events.is_empty());
        assert_eq!(actions.pending.lock().unwrap().len(), 1);
        input.viewport_id = reopened.viewport_id();
        plugin.input_hook(&mut input);
        assert_eq!(input.events.len(), 1);
    }
}
