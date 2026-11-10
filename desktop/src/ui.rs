use crate::{
    design::{self, Icon, Palette},
    model::{self, DeviceInfo, MacroStep, Mapping, Profile, Settings, TargetWindow},
    native,
    practice::Practice,
    runtime::AppState,
    settings, window,
};
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    sync::{mpsc, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum View {
    #[default]
    Buttons,
    Macros,
    Devices,
    Activity,
    Settings,
}
const VIEWS: [(View, &str, &str); 5] = [
    (
        View::Buttons,
        "Buttons",
        "Choose what your headphone buttons do.",
    ),
    (View::Macros, "Macros", "Edit the steps a button runs."),
    (
        View::Devices,
        "Devices",
        "Bluetooth audio devices paired with this PC.",
    ),
    (
        View::Activity,
        "Activity",
        "See which commands arrive and what happens next.",
    ),
    (
        View::Settings,
        "Settings",
        "Appearance, input behavior, and backups.",
    ),
];
pub fn trigger_label(trigger: &str) -> &str {
    match trigger {
        "play_pause" => "Play / pause",
        "next" => "Next track",
        "previous" => "Previous track",
        "stop" => "Stop playback",
        "volume_up" => "Volume up",
        "volume_down" => "Volume down",
        "mute" => "Mute",
        other => other,
    }
}
pub fn step_summary(step: &MacroStep) -> String {
    match step {
        MacroStep::Key { keys } => keys.clone(),
        MacroStep::Delay { ms } => format!("Wait {ms} ms"),
        MacroStep::Text { text } => format!("Type {} characters", text.chars().count()),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ReviewFocus {
    ProfileName(String),
    ProfileDescription(String),
    ActionName(String),
    Step(String, usize),
    Summary(String),
}

pub struct EarControl {
    runtime: Arc<AppState>,
    draft: Settings,
    saved: Settings,
    validation: Option<String>,
    review_focus: Option<ReviewFocus>,
    pub view: View,
    selected_mapping: usize,
    target: Option<TargetWindow>,
    windows: Vec<TargetWindow>,
    window_search: String,
    devices: Vec<DeviceInfo>,
    device_scan: Option<mpsc::Receiver<Result<Vec<DeviceInfo>, String>>>,
    device_error: Option<String>,
    notice: Option<String>,
    profiles_open: bool,
    delete_profile: Option<String>,
    import_pending: Option<Settings>,
    activity_filter: usize,
    recording: Option<(String, usize, egui::Id)>,
    volume_open: bool,
    practice_open: bool,
    practice_pending: bool,
    practice: Practice,
}

impl EarControl {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = cc.egui_ctx.clone();
        configure_style(&ctx);
        let runtime = AppState::new(window::settings_path()?, move || {
            ctx.request_repaint_of(egui::ViewportId::ROOT)
        });
        let RawWindowHandle::Win32(handle) = cc.window_handle()?.as_raw() else {
            return Err("EarControl requires Windows.".into());
        };
        runtime.initialize(handle.hwnd.get());
        let mut app = Self::from_runtime(runtime);
        app.refresh_windows();
        app.refresh_devices(&cc.egui_ctx);
        Ok(app)
    }

    /// Also used by headless UI checks and benchmarks. Does not initialize capture.
    pub fn from_runtime(runtime: Arc<AppState>) -> Self {
        let draft = runtime.settings();
        Self {
            saved: draft.clone(),
            draft,
            validation: None,
            review_focus: None,
            notice: runtime.warning(),
            runtime,
            view: View::Buttons,
            selected_mapping: 0,
            target: None,
            windows: Vec::new(),
            window_search: String::new(),
            devices: Vec::new(),
            device_scan: None,
            device_error: None,
            profiles_open: false,
            delete_profile: None,
            import_pending: None,
            activity_filter: 0,
            recording: None,
            volume_open: false,
            practice_open: false,
            practice_pending: false,
            practice: Practice::default(),
        }
    }
    fn profile_index(&self) -> usize {
        self.draft
            .profiles
            .iter()
            .position(|p| p.id == self.draft.active_profile_id)
            .unwrap_or(0)
    }
    fn report(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.notice = Some(error);
        }
    }
    fn save(&mut self) -> Result<(), String> {
        if let Err(error) = model::validate_settings(&self.draft) {
            self.validation = Some(error.clone());
            self.review_issue();
            return Err(error);
        }
        self.runtime.save(self.draft.clone())?;
        self.saved = self.draft.clone();
        self.validation = None;
        Ok(())
    }
    fn review_issue(&mut self) {
        let Some((profile, mapping, focus)) = first_issue(&self.draft) else {
            return;
        };
        self.draft.active_profile_id = self.draft.profiles[profile].id.clone();
        self.recording = None;
        self.review_focus = Some(focus);
        if let Some(mapping) = mapping {
            self.selected_mapping = mapping;
            self.view = View::Macros;
            self.profiles_open = false;
        } else {
            self.profiles_open = true;
        }
    }
    fn prepare_export(&mut self) -> Result<Settings, String> {
        if self.draft != self.saved {
            self.save()?;
        }
        Ok(self.draft.clone())
    }
    fn prepare_import(&mut self, path: Option<std::path::PathBuf>) {
        let Some(path) = path else { return };
        match settings::read_settings(&path) {
            Ok(Some(settings)) => self.import_pending = Some(settings),
            Ok(None) => self.notice = Some("Settings file was not found.".into()),
            Err(error) => self.notice = Some(error),
        }
    }
    fn mode(&mut self, mode: &str) {
        let result = (|| {
            if mode != "idle" && self.draft != self.saved {
                self.save()?;
            }
            self.runtime.set_mode(mode, self.target.clone())
        })();
        self.report(result);
    }
    fn test_macro(&mut self) {
        let result = (|| {
            let target = self.target.clone().ok_or("Choose a target window first.")?;
            if self.draft != self.saved {
                self.save()?;
            }
            let mapping =
                &self.draft.profiles[self.profile_index()].mappings[self.selected_mapping];
            self.runtime.preview(&mapping.id, target)?;
            self.notice = Some(
                "Test starts in 3 seconds. Switch to the target window. Stop cancels it.".into(),
            );
            Ok(())
        })();
        self.report(result);
    }
    fn refresh_windows(&mut self) {
        match native::list_windows() {
            Ok(mut windows) => {
                if let Some(practice) = window::practice_target() {
                    windows.insert(0, practice);
                }
                self.target = self.target.as_ref().and_then(|old| {
                    windows
                        .iter()
                        .find(|w| w.handle == old.handle && w.process_id == old.process_id)
                        .cloned()
                });
                self.windows = windows;
            }
            Err(error) => self.notice = Some(error),
        }
    }
    fn refresh_devices(&mut self, ctx: &egui::Context) {
        if self.device_scan.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.device_scan = Some(rx);
        self.device_error = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(native::list_devices());
            ctx.request_repaint();
        });
    }
    fn start_practice(&mut self, ctx: &egui::Context) {
        let result = self.runtime.stop();
        self.report(result);
        self.practice_open = true;
        self.practice_pending = true;
        ctx.send_viewport_cmd_to(self.practice.viewport_id(), egui::ViewportCommand::Focus);
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        ctx.set_theme(match self.draft.theme.as_str() {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        });
        if let Some(scan) = &self.device_scan {
            if let Ok(result) = scan.try_recv() {
                match result {
                    Ok(mut devices) => {
                        devices.sort_by(|a, b| {
                            b.connected.cmp(&a.connected).then(a.name.cmp(&b.name))
                        });
                        self.devices = devices;
                    }
                    Err(error) => self.device_error = Some(error),
                }
                self.device_scan = None;
            }
        }
        self.capture_shortcut(ctx);
        let before = self.draft.clone();
        let status = self.runtime.status();
        let active = status.mode != "idle" || status.running_macro;
        if self.recording.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S))
        {
            let result = self.save();
            self.report(result);
        }
        let palette = Palette::get(ctx);
        let compact = ctx.content_rect().width() <= 1050.0;
        let sidebar_width = if compact { 190.0 } else { 216.0 };
        egui::SidePanel::left("navigation")
            .exact_width(sidebar_width)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(palette.sidebar)
                    .inner_margin(egui::Margin::symmetric(14, 16)),
            )
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(9.0);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(25.0, 25.0), egui::Sense::hover());
                    design::brand(ui.painter(), rect, palette.text, palette.sidebar);
                    ui.add_space(1.0);
                    ui.label(design::strong(
                        "EarControl",
                        if compact { 17.0 } else { 19.0 },
                    ));
                });
                ui.add_space(26.0);
                ui.label(design::text("Profile", 12.0).color(palette.muted));
                egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(egui::Stroke::new(1.0_f32, palette.line))
                    .corner_radius(6)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.horizontal(|ui| {
                            let current = self.draft.profiles[self.profile_index()].name.clone();
                            ui.scope(|ui| {
                                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                                ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                                egui::ComboBox::from_id_salt("profile")
                                    .selected_text(design::text(current, 13.0))
                                    .width(sidebar_width - 64.0)
                                    .show_ui(ui, |ui| {
                                        for profile in &self.draft.profiles {
                                            ui.selectable_value(
                                                &mut self.draft.active_profile_id,
                                                profile.id.clone(),
                                                &profile.name,
                                            );
                                        }
                                    });
                            });
                            if design::icon_button(
                                ui,
                                Icon::More,
                                "Manage profiles",
                                egui::vec2(34.0, 36.0),
                            )
                            .clicked()
                            {
                                self.profiles_open = true;
                            }
                        });
                    });
                ui.add_space(18.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                for (view, label, _) in VIEWS {
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 44.0),
                        egui::Sense::click(),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Button,
                            true,
                            self.view == view,
                            label,
                        )
                    });
                    if self.view == view || response.hovered() {
                        ui.painter().rect_filled(rect, 7.0, palette.hover);
                    }
                    let color = if self.view == view {
                        palette.text
                    } else {
                        palette.muted
                    };
                    design::draw_icon(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 21.0, rect.center().y),
                            egui::Vec2::splat(17.0),
                        ),
                        view_icon(view),
                        color,
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 43.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        design::font(14.0, self.view == view),
                        color,
                    );
                    if response.has_focus() {
                        ui.painter().rect_stroke(
                            rect.shrink(2.0),
                            5.0,
                            egui::Stroke::new(1.0_f32, palette.text),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if response.clicked() {
                        self.view = view;
                        self.recording = None;
                        if view == View::Devices {
                            self.refresh_devices(ctx);
                        }
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let label = if self.device_scan.is_some() {
                        "Checking devices".into()
                    } else if self.device_error.is_some() {
                        "Device status unavailable".into()
                    } else {
                        format!(
                            "{} connected devices",
                            self.devices.iter().filter(|d| d.connected).count()
                        )
                    };
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 32.0),
                        egui::Sense::click(),
                    );
                    ui.painter().circle_filled(
                        egui::pos2(rect.left() + 8.0, rect.center().y),
                        3.0,
                        if self.devices.iter().any(|d| d.connected) {
                            palette.green
                        } else {
                            palette.muted
                        },
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 22.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        design::font(11.0, false),
                        palette.muted,
                    );
                    design::draw_icon(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            egui::pos2(rect.right() - 7.0, rect.center().y),
                            egui::Vec2::splat(13.0),
                        ),
                        Icon::ChevronRight,
                        palette.muted,
                    );
                    if response.clicked() {
                        self.view = View::Devices;
                        self.refresh_devices(ctx);
                    }
                });
                ui.painter().vline(
                    ui.max_rect().right() + 14.0,
                    ui.max_rect().y_range(),
                    egui::Stroke::new(1.0_f32, palette.line),
                );
            });
        egui::TopBottomPanel::top("session")
            .exact_height(if compact { 102.0 } else { 86.0 })
            .frame(egui::Frame::new().fill(palette.surface))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let chrome = egui::Rect::from_min_size(
                    egui::pos2(rect.right() - 138.0, rect.top()),
                    egui::vec2(138.0, 36.0),
                );
                let drag = ui.interact(
                    egui::Rect::from_min_max(
                        rect.min,
                        egui::pos2(
                            chrome.left(),
                            rect.top() + if compact { 28.0 } else { 12.0 },
                        ),
                    ),
                    ui.id().with("drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                        !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                    ));
                }
                design::window_controls(ui, chrome);
                let x = rect.left() + if compact { 18.0 } else { 28.0 };
                let top = rect.top() + if compact { 32.0 } else { 12.0 };
                let actions_width = if compact { 300.0 } else { 315.0 };
                let available = rect.width() - if compact { 36.0 } else { 56.0 };
                let target_width = (available - actions_width - 20.0).max(150.0);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                        egui::pos2(x, top),
                        egui::vec2(target_width, 64.0),
                    )),
                    |ui| {
                        ui.label(design::text("Target window", 12.0).color(palette.muted));
                        ui.add_enabled_ui(can_select_target(&status), |ui| {
                            egui::Frame::new()
                                .fill(palette.surface)
                                .stroke(egui::Stroke::new(1.0_f32, palette.input_line))
                                .corner_radius(7)
                                .inner_margin(egui::Margin::symmetric(8, 0))
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(
                                            egui::vec2(17.0, 36.0),
                                            egui::Sense::hover(),
                                        );
                                        design::draw_icon(
                                            ui.painter(),
                                            egui::Rect::from_center_size(
                                                r.center(),
                                                egui::Vec2::splat(17.0),
                                            ),
                                            Icon::Monitor,
                                            palette.muted,
                                        );
                                        ui.scope(|ui| {
                                            ui.visuals_mut().widgets.inactive.bg_stroke =
                                                egui::Stroke::NONE;
                                            ui.visuals_mut().widgets.hovered.bg_stroke =
                                                egui::Stroke::NONE;
                                            let title = self
                                                .target
                                                .as_ref()
                                                .map(|w| w.title.as_str())
                                                .unwrap_or("Choose a window to control");
                                            egui::ComboBox::from_id_salt("target")
                                                .width((target_width - 74.0).max(70.0))
                                                .selected_text(design::text(title, 12.0))
                                                .show_ui(ui, |ui| {
                                                    ui.add(
                                                        egui::TextEdit::singleline(
                                                            &mut self.window_search,
                                                        )
                                                        .hint_text("Search windows"),
                                                    );
                                                    ui.selectable_value(
                                                        &mut self.target,
                                                        None,
                                                        "No target",
                                                    );
                                                    for window in &self.windows {
                                                        if window.title.to_lowercase().contains(
                                                            &self.window_search.to_lowercase(),
                                                        ) {
                                                            ui.selectable_value(
                                                                &mut self.target,
                                                                Some(window.clone()),
                                                                &window.title,
                                                            );
                                                        }
                                                    }
                                                });
                                        });
                                        if design::icon_button(
                                            ui,
                                            Icon::Refresh,
                                            "Refresh target windows",
                                            egui::vec2(28.0, 36.0),
                                        )
                                        .clicked()
                                        {
                                            self.refresh_windows();
                                        }
                                    });
                                });
                        });
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                        egui::pos2(x + target_width + 20.0, top + 22.0),
                        egui::vec2(actions_width, 40.0),
                    )),
                    |ui| {
                        ui.horizontal(|ui| {
                            let label = if status.running_macro {
                                "Running macro"
                            } else if status.mode == "control" {
                                "Controlling"
                            } else if status.mode == "monitor" {
                                "Monitoring"
                            } else {
                                "Idle"
                            };
                            let width = ui
                                .painter()
                                .layout_no_wrap(
                                    label.into(),
                                    design::font(12.0, false),
                                    palette.muted,
                                )
                                .size()
                                .x
                                + 31.0;
                            let (r, _) = ui
                                .allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::hover());
                            ui.painter().rect(
                                r,
                                15.0,
                                palette.soft,
                                egui::Stroke::new(1.0_f32, palette.line),
                                egui::StrokeKind::Inside,
                            );
                            let color = if active { palette.green } else { palette.muted };
                            ui.painter().circle_filled(
                                egui::pos2(r.left() + 12.0, r.center().y),
                                3.0,
                                color,
                            );
                            ui.painter().text(
                                egui::pos2(r.left() + 23.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                label,
                                design::font(12.0, false),
                                color,
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if active {
                                        if design::button(ui, "Stop", Some(Icon::Stop), false)
                                            .clicked()
                                        {
                                            self.mode("idle");
                                        }
                                    } else {
                                        ui.add_enabled_ui(
                                            self.target.is_some() && self.validation.is_none(),
                                            |ui| {
                                                if design::button(
                                                    ui,
                                                    if self.draft != self.saved {
                                                        "Save & start"
                                                    } else {
                                                        "Start control"
                                                    },
                                                    Some(Icon::Play),
                                                    true,
                                                )
                                                .clicked()
                                                {
                                                    self.mode("control");
                                                }
                                            },
                                        );
                                        if design::button(ui, "Monitor", Some(Icon::Radio), false)
                                            .clicked()
                                        {
                                            self.mode("monitor");
                                        }
                                    }
                                },
                            );
                        });
                    },
                );
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom(),
                    egui::Stroke::new(1.0_f32, palette.line),
                );
            });
        egui::TopBottomPanel::top("session_help")
            .exact_height(40.0)
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .inner_margin(egui::Margin::symmetric(if compact { 22 } else { 32 }, 10)),
            )
            .show(ctx, |ui| {
                let help = if status.running_macro {
                    "Switch to the target window. Stop cancels the test."
                } else if status.mode == "monitor" {
                    "Press a headphone button to see its command. Monitoring does not send keys."
                } else if status.mode == "control" {
                    "Switch to the target window. Actions run only while it is in front."
                } else if self.target.is_none() {
                    "Choose an open window above, or open the practice deck to try your mappings."
                } else {
                    "Ready. Start control, then switch to the target window."
                };
                ui.label(design::text(help, 12.0).color(palette.muted));
            });
        if self.draft != self.saved {
            egui::TopBottomPanel::bottom("save_bar")
                .frame(
                    egui::Frame::new()
                        .fill(palette.soft)
                        .stroke(egui::Stroke::new(1.0_f32, palette.line))
                        .inner_margin(12),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(design::text(
                            self.validation.as_deref().unwrap_or("Unsaved changes"),
                            12.0,
                        ));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_enabled_ui(self.validation.is_none(), |ui| {
                                if design::button(ui, "Save changes", Some(Icon::Save), true)
                                    .clicked()
                                {
                                    let result = self.save();
                                    self.report(result);
                                }
                            });
                            if design::button(ui, "Discard", None, false).clicked() {
                                self.draft = self.saved.clone();
                                self.recording = None;
                                self.review_focus = None;
                            }
                            if self.validation.is_some()
                                && design::button(ui, "Review issue", None, false).clicked()
                            {
                                self.review_issue();
                            }
                        });
                    });
                });
        }
        if let Some(message) = self.notice.clone() {
            egui::TopBottomPanel::bottom("notice")
                .frame(
                    egui::Frame::new()
                        .fill(palette.surface)
                        .stroke(egui::Stroke::new(1.0_f32, palette.line))
                        .inner_margin(10),
                )
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(design::text(message, 12.0));
                        if ui.small_button("Dismiss").clicked() {
                            self.notice = None;
                        }
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(palette.bg)
                    .inner_margin(egui::Margin::symmetric(24, 16)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().scroll.floating_allocated_width = 8.0;
                egui::ScrollArea::vertical()
                    .id_salt(("content", format!("{:?}", self.view)))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;
                                ui.spacing_mut().interact_size.y = 30.0;
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::Vec2::splat(22.0),
                                        egui::Sense::hover(),
                                    );
                                    design::draw_icon(
                                        ui.painter(),
                                        rect,
                                        view_icon(self.view),
                                        palette.muted,
                                    );
                                    ui.label(design::strong(
                                        if self.view == View::Buttons {
                                            "Button mappings"
                                        } else {
                                            VIEWS
                                                .iter()
                                                .find(|(v, _, _)| *v == self.view)
                                                .unwrap()
                                                .1
                                        },
                                        24.0,
                                    ));
                                });
                                ui.label(
                                    design::text(
                                        VIEWS.iter().find(|(v, _, _)| *v == self.view).unwrap().2,
                                        13.0,
                                    )
                                    .color(palette.muted),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if design::button(
                                        ui,
                                        "Practice deck",
                                        Some(Icon::Monitor),
                                        false,
                                    )
                                    .clicked()
                                    {
                                        self.start_practice(ctx);
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                        match self.view {
                            View::Buttons => self.buttons(ui),
                            View::Macros => self.macros(ui),
                            View::Devices => self.devices(ui),
                            View::Activity => self.activity(ui),
                            View::Settings => self.preferences(ui),
                        }
                    });
            });

        self.profile_dialog(ctx);
        self.import_dialog(ctx);
        if self.draft != before {
            self.validation = model::validate_settings(&self.draft).err();
            if status.mode == "control" || status.running_macro {
                let result = self.runtime.stop();
                self.report(result);
            }
        }
        if self.practice_open {
            let id = self.practice.viewport_id();
            ctx.show_viewport_immediate(
                id,
                egui::ViewportBuilder::default()
                    .with_title("EarControl practice deck")
                    .with_decorations(false)
                    .with_visible(self.practice.visible())
                    .with_inner_size([960.0, 640.0])
                    .with_min_inner_size([600.0, 400.0]),
                |ctx, _| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        self.practice_open = false;
                        ctx.request_repaint_of(ctx.parent_viewport_id());
                        return;
                    }
                    self.practice.show(ctx);
                },
            );
            if self.practice_pending {
                if let Some(target) = window::practice_target() {
                    self.target = Some(target);
                    self.practice_pending = false;
                    self.refresh_windows();
                    self.notice = Some(
                        "Practice deck selected. Start control, then switch to the deck.".into(),
                    );
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
            if !self.practice_open {
                self.practice = Practice::default();
                self.practice_pending = false;
                if self
                    .target
                    .as_ref()
                    .is_some_and(|w| w.process_id == std::process::id())
                {
                    self.target = None;
                    let result = self.runtime.stop();
                    self.report(result);
                }
            }
        }
    }

    fn buttons(&mut self, ui: &mut egui::Ui) {
        let p = Palette::get(ui.ctx());

        let pi = self.profile_index();

        let profile = &mut self.draft.profiles[pi];

        let mut edit = None;

        egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.line))
            .corner_radius(10)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;

                egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.spacing_mut().interact_size.y = 28.0;

                    ui.horizontal(|ui| {
                        ui.label(design::strong(&profile.name, 16.0));
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0_f32, p.line))
                            .corner_radius(5)
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.label(design::text(format!("{} of 7 enabled", profile.mappings.iter().filter(|m| m.enabled).count()), 11.0).color(p.muted));
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if design::icon_button(ui, Icon::More, "Manage profiles", egui::vec2(32.0, 28.0)).clicked() {
                                self.profiles_open = true;
                            }
                        });
                    });

                    if !profile.description.is_empty() {
                        ui.label(design::text(&profile.description, 12.0).color(p.muted));
                    }
                });

                let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::hover());

                ui.painter().rect_filled(r, 0, p.soft);
                ui.painter().hline(r.x_range(), r.top(), egui::Stroke::new(1.0_f32, p.line));
                ui.painter().hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0_f32, p.line));

                let command_width = (r.width() - 140.0) * 0.36;

                for (x, label) in [
                    (r.left() + 22.0, "Headphone button"),
                    (r.left() + 22.0 + command_width + 24.0, "Action / shortcut"),
                    (r.right() - 60.0, "On"),
                ] {
                    ui.painter().text(
                        egui::pos2(x, r.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        design::font(11.0, false),
                        p.muted,
                    );
                }

                for (index, mapping) in profile
                    .mappings
                    .iter_mut()
                    .enumerate()
                    .filter(|(_, m)| !matches!(m.trigger.as_str(), "volume_up" | "volume_down" | "mute"))
                {
                    if mapping_row(ui, mapping) {
                        edit = Some(index);
                    }
                }

                let (r, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::click());
                response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Volume controls"));
                ui.painter().rect_filled(r, 0, p.soft);

                design::draw_icon(
                    ui.painter(),
                    egui::Rect::from_center_size(egui::pos2(r.left() + 28.0, r.center().y), egui::Vec2::splat(14.0)),
                    if self.volume_open { Icon::ChevronDown } else { Icon::ChevronRight },
                    p.muted,
                );

                ui.painter().text(
                    egui::pos2(r.left() + 44.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Volume controls",
                    design::font(12.0, false),
                    p.muted,
                );

                ui.painter().text(
                    egui::pos2(r.right() - 22.0, r.center().y),
                    egui::Align2::RIGHT_CENTER,
                    format!(
                        "{} enabled",
                        profile
                            .mappings
                            .iter()
                            .filter(|m| matches!(m.trigger.as_str(), "volume_up" | "volume_down" | "mute") && m.enabled)
                            .count()
                    ),
                    design::font(10.0, false),
                    p.muted,
                );

                if response.clicked() {
                    self.volume_open = !self.volume_open;
                }

                if self.volume_open {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 10)).show(ui, |ui| {
                        ui.label(design::text("Requires keyboard media-key events. Keep keyboard media keys enabled in Settings.", 12.0).color(p.muted));
                    });

                    for (index, mapping) in profile
                        .mappings
                        .iter_mut()
                        .enumerate()
                        .filter(|(_, m)| matches!(m.trigger.as_str(), "volume_up" | "volume_down" | "mute"))
                    {
                        if mapping_row(ui, mapping) {
                            edit = Some(index);
                        }
                    }
                }
            });

        if let Some(index) = edit {
            self.selected_mapping = index;

            self.view = View::Macros;

            self.recording = None;
        }

        ui.add_space(4.0);

        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 22.0;
            ui.horizontal_wrapped(|ui| {
                ui.label(design::text("This profile applies to all devices. Monitor identifies each button's command.", 12.0).color(p.muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.link(design::text("View devices  →", 11.0)).clicked() {
                        self.view = View::Devices;
                        self.refresh_devices(ui.ctx());
                    }
                });
            });
        });

        ui.add_space(6.0);

        egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.line))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                ui.scope(|ui| {
                    ui.spacing_mut().interact_size.y = 22.0;
                    ui.horizontal(|ui| {
                        ui.label(design::strong("Button check", 16.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.link(design::text("View activity  →", 11.0)).clicked() {
                                self.view = View::Activity;
                            }
                        });
                    });
                });

                ui.separator();

                let events = self.runtime.events();

                if let Some(event) = events.iter().find(|e| model::TRIGGERS.contains(&e.trigger.as_str())) {
                    event_row(ui, event, false);
                } else {
                    let monitoring = self.runtime.status().mode == "monitor";

                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(44.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 9.0, p.soft);
                        design::draw_icon(ui.painter(), rect.shrink(11.0), Icon::Radio, p.muted);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_enabled_ui(!monitoring, |ui| {
                                if design::button(ui, if monitoring { "Listening" } else { "Monitor" }, Some(Icon::Radio), false).clicked() {
                                    self.mode("monitor");
                                }
                            });

                            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;
                                ui.label(design::strong(
                                    if monitoring {
                                        "Waiting for a headphone command"
                                    } else {
                                        "Check what your buttons send"
                                    },
                                    13.0,
                                ));
                                ui.label(
                                    design::text(
                                        if monitoring {
                                            "Press a headphone button. Its command will appear here."
                                        } else {
                                            "Monitor listens for commands without sending shortcuts."
                                        },
                                        12.0,
                                    )
                                    .color(p.muted),
                                );
                            });
                        });
                    });
                }
            });

        ui.add_space(12.0);

        ui.collapsing(design::text("Getting started", 12.0).color(p.muted), |ui| {
            ui.label(design::text("1. Pair your headphones with this PC in Bluetooth settings.", 12.0).color(p.muted));
            ui.label(design::text("2. Start Monitor and try each gesture to identify its command.", 12.0).color(p.muted));
            ui.label(design::text("3. Choose a target window, start control, then switch to that window.", 12.0).color(p.muted));
            ui.label(
                design::text(
                    "Windows reports media commands, not tap counts or the earbud side. Browser shortcuts go to the focused tab or field.",
                    12.0,
                )
                .color(p.muted),
            );
        });
    }

    fn macros(&mut self, ui: &mut egui::Ui) {
        let p = Palette::get(ui.ctx());

        let pi = self.profile_index();

        let profile = &mut self.draft.profiles[pi];

        let sidebar = if ui.available_width() < 700.0 {
            135.0
        } else {
            160.0
        };

        let editor_width = ui.available_width() - sidebar - 25.0;

        let mut test = false;

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(sidebar, 0.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.label(design::text("Button", 12.0).color(p.muted));
                ui.add_space(6.0);

                for (index, mapping) in profile.mappings.iter().enumerate() {
                    let (r, response) = ui.allocate_exact_size(egui::vec2(sidebar - 13.0, 54.0), egui::Sense::click());
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::Button, true, self.selected_mapping == index, trigger_label(&mapping.trigger))
                    });
                    if self.selected_mapping == index || response.hovered() {
                        ui.painter().rect_filled(r, 5.0, p.hover);
                    }
                    design::draw_icon(
                        ui.painter(),
                        egui::Rect::from_center_size(egui::pos2(r.left() + 15.0, r.top() + 20.0), egui::Vec2::splat(16.0)),
                        trigger_icon(&mapping.trigger),
                        p.muted,
                    );
                    ui.painter().text(
                        egui::pos2(r.left() + 31.0, r.top() + 18.0),
                        egui::Align2::LEFT_CENTER,
                        trigger_label(&mapping.trigger),
                        design::font(13.0, false),
                        if self.selected_mapping == index { p.text } else { p.muted },
                    );
                    ui.painter().text(
                        egui::pos2(r.left() + 31.0, r.top() + 38.0),
                        egui::Align2::LEFT_CENTER,
                        if mapping.enabled {
                            format!("{} {}", mapping.steps.len(), if mapping.steps.len() == 1 { "step" } else { "steps" })
                        } else {
                            "Disabled".into()
                        },
                        design::font(11.0, false),
                        p.muted,
                    );
                    if response.clicked() {
                        self.selected_mapping = index;
                        self.recording = None;
                    }
                }

                ui.painter()
                    .vline(ui.min_rect().right() + 8.0, ui.min_rect().y_range(), egui::Stroke::new(1.0_f32, p.line));
            });

            ui.add_space(9.0);

            ui.allocate_ui_with_layout(egui::vec2(editor_width, 0.0), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                let mapping = &mut profile.mappings[self.selected_mapping];

                ui.label(design::text("Headphone button", 12.0).color(p.muted));

                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(17.0), egui::Sense::hover());
                    design::draw_icon(ui.painter(), r, trigger_icon(&mapping.trigger), p.muted);
                    ui.label(design::text(trigger_label(&mapping.trigger), 14.0));
                    ui.label(design::text(format!("›   {} steps", mapping.steps.len()), 11.0).color(p.muted));
                });
                ui.add_space(8.0);

                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2((editor_width - 110.0).max(120.0), 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            let label = ui.label(design::text("Action name", 12.0).color(p.muted));
                            let response = ui
                                .add(
                                    egui::TextEdit::singleline(&mut mapping.name)
                                        .id(egui::Id::new(("action_name", &mapping.id)))
                                        .desired_width(f32::INFINITY)
                                        .char_limit(80),
                                )
                                .labelled_by(label.id);
                            focus_review(&mut self.review_focus, ReviewFocus::ActionName(mapping.id.clone()), &response);
                        },
                    );

                    ui.vertical(|ui| {
                        ui.add_space(25.0);
                        ui.horizontal(|ui| {
                            ui.label(design::text(if mapping.enabled { "Enabled" } else { "Disabled" }, 12.0).color(p.muted));
                            ui.add_enabled_ui(!mapping.steps.is_empty(), |ui| {
                                design::toggle(ui, &mut mapping.enabled, "Enable mapping");
                            });
                        });
                    });
                });
                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    ui.label(design::strong("Steps", 14.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(design::text("Run in order", 12.0).color(p.muted));
                    });
                });

                let len = mapping.steps.len();
                let mut remove = None;
                let mut swap = None;

                for (index, step) in mapping.steps.iter_mut().enumerate() {
                    ui.push_id((&mapping.id, index), |ui| {
                        egui::Frame::new()
                            .fill(p.surface)
                            .stroke(egui::Stroke::new(1.0_f32, p.line))
                            .corner_radius(6)
                            .inner_margin(14)
                            .show(ui, |ui| {
                                ui.set_min_width((editor_width - 28.0).max(100.0));
                                ui.horizontal_top(|ui| {
                                    ui.label(design::text(format!("{:02}", index + 1), 11.0).monospace().color(p.muted));

                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            let (icon, title) = match step {
                                                MacroStep::Key { .. } => (Icon::Keyboard, "Press a shortcut"),
                                                MacroStep::Delay { .. } => (Icon::Clock, "Wait"),
                                                MacroStep::Text { .. } => (Icon::Text, "Type text"),
                                            };
                                            let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::hover());
                                            design::draw_icon(ui.painter(), r, icon, p.muted);
                                            ui.label(design::text(title, 13.0));

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if design::icon_button(ui, Icon::Trash, "Remove step", egui::Vec2::splat(28.0)).clicked() {
                                                    remove = Some(index);
                                                }

                                                ui.add_enabled_ui(index + 1 < len, |ui| {
                                                    if design::icon_button(ui, Icon::Down, "Move step down", egui::Vec2::splat(28.0)).clicked() {
                                                        swap = Some((index, index + 1));
                                                    }
                                                });

                                                ui.add_enabled_ui(index > 0, |ui| {
                                                    if design::icon_button(ui, Icon::Up, "Move step up", egui::Vec2::splat(28.0)).clicked() {
                                                        swap = Some((index, index - 1));
                                                    }
                                                });
                                            });
                                        });

                                        match step {
                                            MacroStep::Key { keys } => {
                                                ui.horizontal_wrapped(|ui| {
                                                    let response = ui.add(
                                                        egui::TextEdit::singleline(keys)
                                                            .id(egui::Id::new(("step", &mapping.id, index)))
                                                            .font(egui::FontId::monospace(13.0))
                                                            .desired_width((ui.available_width() - 110.0).max(90.0))
                                                            .hint_text("For example, Ctrl+Shift+L"),
                                                    );
                                                    focus_review(&mut self.review_focus, ReviewFocus::Step(mapping.id.clone(), index), &response);

                                                    let recording = self.recording.as_ref().is_some_and(|(id, i, _)| id == &mapping.id && *i == index);

                                                    let button = design::button(ui, if recording { "Cancel" } else { "Record" }, Some(Icon::Keyboard), false);

                                                    if button.clicked() {
                                                        self.recording = if recording {
                                                            None
                                                        } else {
                                                            button.request_focus();
                                                            Some((mapping.id.clone(), index, button.id))
                                                        };
                                                    }
                                                });
                                                if let Err(error) = crate::shortcuts::parse_shortcut(keys) {
                                                    ui.label(design::text(error, 12.0).color(p.danger));
                                                }
                                            }
                                            MacroStep::Delay { ms } => {
                                                ui.horizontal_wrapped(|ui| {
                                                    let response = ui.add(egui::DragValue::new(ms).range(0..=30000).speed(50.0));
                                                    focus_review(&mut self.review_focus, ReviewFocus::Step(mapping.id.clone(), index), &response);
                                                    ui.label(design::text("milliseconds", 12.0).color(p.muted));
                                                    if editor_width > 450.0 {
                                                        for preset in [250, 500, 1000] {
                                                            if ui.small_button(format!("{preset} ms")).clicked() {
                                                                *ms = preset;
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                            MacroStep::Text { text } => {
                                                let response = ui.add(
                                                    egui::TextEdit::multiline(text)
                                                        .id(egui::Id::new(("step", &mapping.id, index)))
                                                        .desired_rows(3)
                                                        .desired_width(f32::INFINITY)
                                                        .hint_text("Text to type into the selected window"),
                                                );
                                                focus_review(&mut self.review_focus, ReviewFocus::Step(mapping.id.clone(), index), &response);
                                            }
                                        }
                                    });
                                });
                            });
                    });
                }

                if let Some(index) = remove {
                    mapping.steps.remove(index);
                    self.recording = None;
                    if mapping.steps.is_empty() {
                        mapping.enabled = false;
                    }
                }

                if let Some((a, b)) = swap {
                    mapping.steps.swap(a, b);
                    self.recording = None;
                }

                if mapping.steps.is_empty() {
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0_f32, p.line))
                        .corner_radius(6)
                        .inner_margin(24)
                        .show(ui, |ui| {
                            ui.set_min_width((editor_width - 48.0).max(100.0));
                            ui.vertical_centered(|ui| {
                                ui.label(design::strong("No steps yet", 13.0));
                                ui.label(design::text("Add a shortcut, delay, or text below.", 12.0).color(p.muted));
                            });
                        });
                }

                if self.recording.is_some() {
                    ui.label(design::text("Press a key or shortcut, including Escape. Click Cancel to stop recording.", 11.0).color(p.muted));
                }

                if let Some(error) = &self.validation {
                    let response = ui.add(egui::Label::new(design::text(error, 12.0).color(p.danger)).sense(egui::Sense::click()));
                    focus_review(&mut self.review_focus, ReviewFocus::Summary(mapping.id.clone()), &response);
                }

                ui.add_space(6.0);
                ui.add_enabled_ui(mapping.steps.len() < 32, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(design::text("Add a step", 12.0).color(p.muted));
                        for (label, icon, step) in [
                            ("Shortcut", Icon::Keyboard, MacroStep::Key { keys: "Right".into() }),
                            ("Delay", Icon::Clock, MacroStep::Delay { ms: 500 }),
                            ("Text", Icon::Text, MacroStep::Text { text: String::new() }),
                        ] {
                            if design::button(ui, label, Some(icon), false).clicked() {
                                if mapping.steps.is_empty() {
                                    mapping.enabled = true;
                                }
                                mapping.steps.push(step);
                            }
                        }
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.label(design::text("Preview", 12.0).color(p.muted));
                ui.horizontal_wrapped(|ui| {
                    for (index, step) in mapping.steps.iter().enumerate() {
                        if index > 0 {
                            ui.label(design::text("›", 14.0).color(p.muted));
                        }
                        egui::Frame::new()
                            .fill(p.soft)
                            .stroke(egui::Stroke::new(1.0_f32, p.line))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::symmetric(7, 4))
                            .show(ui, |ui| {
                                ui.label(design::text(step_summary(step), 11.0).monospace());
                            });
                    }
                });

                ui.add_space(16.0);
                ui.separator();
                ui.label(
                    design::text(
                        if self.target.is_none() {
                            "Choose a target window above to test this macro."
                        } else {
                            "Test saves changes and starts in 3 seconds. Switch to the target window."
                        },
                        12.0,
                    )
                    .color(p.muted),
                );

                let can_test =
                    mapping.enabled && !mapping.steps.is_empty() && self.target.is_some() && self.validation.is_none() && !self.runtime.status().running_macro;

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(can_test, |ui| {
                        if design::button(ui, "Test macro", Some(Icon::Play), false).clicked() {
                            test = true;
                        }
                    });
                });
            });
        });

        if test {
            self.test_macro();
        }
    }

    fn capture_shortcut(&mut self, ctx: &egui::Context) {
        let Some((id, index, widget)) = self.recording.clone() else {
            return;
        };
        if !ctx.memory(|m| m.has_focus(widget)) {
            self.recording = None;
            return;
        }
        let event = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((key, modifiers)) = event {
            let shortcut = recorded_shortcut(key, modifiers, native::win_key_held());
            if let Some(shortcut) = shortcut {
                let pi = self.profile_index();
                if let Some(mapping) = self.draft.profiles[pi]
                    .mappings
                    .iter_mut()
                    .find(|m| m.id == id)
                {
                    if let Some(MacroStep::Key { keys }) = mapping.steps.get_mut(index) {
                        *keys = shortcut;
                    }
                }
                ctx.input_mut(|i| {
                    i.events
                        .retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)))
                });
                self.recording = None;
                self.validation = model::validate_settings(&self.draft).err();
                let result = self.runtime.stop();
                self.report(result);
            }
        }
    }

    fn devices(&mut self, ui: &mut egui::Ui) {
        let p = Palette::get(ui.ctx());
        egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.line))
            .corner_radius(10)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 14)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(design::strong("Paired audio devices", 16.0));
                            ui.label(
                                design::text(
                                    format!(
                                        "{} connected · {} paired",
                                        self.devices.iter().filter(|d| d.connected).count(),
                                        self.devices.len()
                                    ),
                                    12.0,
                                )
                                .color(p.muted),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_enabled_ui(self.device_scan.is_none(), |ui| {
                                if design::button(
                                    ui,
                                    if self.device_scan.is_some() { "Refreshing…" } else { "Refresh" },
                                    Some(Icon::Refresh),
                                    false,
                                )
                                .clicked()
                                {
                                    self.refresh_devices(ui.ctx());
                                }
                            });
                        });
                    });
                });
                ui.separator();
                if let Some(error) = &self.device_error {
                    ui.label(design::text(error, 12.0).color(p.danger));
                }
                if self.devices.is_empty() {
                    empty_state(
                        ui,
                        Icon::Headphones,
                        if self.device_scan.is_some() {
                            "Checking devices…"
                        } else {
                            "No Bluetooth audio devices found"
                        },
                        "Pair your headphones in Windows Bluetooth settings, then refresh this list.",
                    );
                }
                for device in &self.devices {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 18)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(36.0), egui::Sense::hover());
                            ui.painter().rect_stroke(r, 7.0, egui::Stroke::new(1.0_f32, p.line), egui::StrokeKind::Inside);
                            design::draw_icon(ui.painter(), r.shrink(7.0), Icon::Headphones, p.muted);
                            ui.vertical(|ui| {
                                ui.label(design::strong(&device.name, 14.0));
                                ui.label(design::text(format!("{} · {}", device.kind, device.transport), 12.0).color(p.muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    design::text(if device.connected { "●  Connected" } else { "●  Disconnected" }, 11.0).color(if device.connected {
                                        p.green
                                    } else {
                                        p.muted
                                    }),
                                );
                            });
                        });
                    });
                    ui.separator();
                }
                egui::Frame::new().fill(p.soft).inner_margin(egui::Margin::symmetric(20, 12)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(design::text("Windows manages pairing and connections.", 11.0).color(p.muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.link(design::text("Bluetooth settings  ↗", 12.0)).clicked() {
                                self.report(native::open_bluetooth_settings());
                            }
                        });
                    });
                });
            });
        ui.add_space(20.0);
        ui.label(design::strong("Using more than one device", 16.0));
        ui.label(design::text("All devices share the active profile. Windows media events do not identify the source device, so EarControl cannot assign a separate profile to each headset.",12.0).color(p.muted));
        ui.add_space(10.0);
        ui.label(design::strong("Buttons not appearing?", 16.0));
        ui.label(design::text("Start Monitor and try a button. Pause other media apps and select EarControl in Windows media controls. Some Bluetooth LE devices may be absent from this list even when their commands work.",12.0).color(p.muted));
        if design::button(ui, "Start monitor", Some(Icon::Radio), false).clicked() {
            self.mode("monitor");
        }
    }

    fn activity(&mut self, ui: &mut egui::Ui) {
        let p = Palette::get(ui.ctx());
        let events = self.runtime.events();
        egui::Frame::new()
            .fill(p.surface)
            .stroke(egui::Stroke::new(1.0_f32, p.line))
            .corner_radius(10)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(20, 14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(design::strong(
                                    format!("Received commands   {}", events.len()),
                                    16.0,
                                ));
                                ui.label(
                                    design::text(
                                        "Up to 250 recent events. Cleared when the app closes.",
                                        12.0,
                                    )
                                    .color(p.muted),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_enabled_ui(!events.is_empty(), |ui| {
                                        if design::icon_button(
                                            ui,
                                            Icon::Trash,
                                            "Clear activity",
                                            egui::Vec2::splat(36.0),
                                        )
                                        .clicked()
                                        {
                                            self.runtime.clear_events();
                                        }
                                    });
                                    let labels =
                                        ["All events", "Actions", "Detected", "Needs attention"];
                                    egui::ComboBox::from_id_salt("activity_filter")
                                        .selected_text(design::text(
                                            labels[self.activity_filter],
                                            12.0,
                                        ))
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for (index, label) in labels.iter().enumerate() {
                                                ui.selectable_value(
                                                    &mut self.activity_filter,
                                                    index,
                                                    *label,
                                                );
                                            }
                                        });
                                },
                            );
                        });
                    });
                ui.separator();
                let visible: Vec<_> = events
                    .iter()
                    .filter(|event| match self.activity_filter {
                        1 => event.outcome == "executed",
                        2 => event.outcome == "detected",
                        3 => ["cancelled", "skipped", "error"].contains(&event.outcome.as_str()),
                        _ => true,
                    })
                    .collect();
                if visible.is_empty() {
                    let monitoring = self.runtime.status().mode == "monitor";
                    empty_state(
                        ui,
                        Icon::Activity,
                        if events.is_empty() {
                            if monitoring {
                                "Waiting for a button press"
                            } else {
                                "No commands yet"
                            }
                        } else {
                            "No matching events"
                        },
                        if events.is_empty() {
                            if monitoring {
                                "Try a headphone gesture. Its command will appear here."
                            } else {
                                "Start Monitor, then try a headphone gesture."
                            }
                        } else {
                            "Choose another filter to see recent activity."
                        },
                    );
                    if events.is_empty() && !monitoring {
                        ui.vertical_centered(|ui| {
                            if design::button(ui, "Start monitor", Some(Icon::Radio), false)
                                .clicked()
                            {
                                self.mode("monitor");
                            }
                        });
                        ui.add_space(24.0);
                    }
                }
                for event in visible {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(20, 15))
                        .show(ui, |ui| {
                            event_row(ui, event, true);
                        });
                    ui.separator();
                }
            });
    }

    fn preferences(&mut self, ui: &mut egui::Ui) {
        let p = Palette::get(ui.ctx());
        let mut export = false;
        let mut import = false;
        ui.scope(|ui| {
            ui.set_max_width(680.0);
            section_heading(ui, Icon::Sun, "Appearance");
            ui.label(design::text("Choose a theme for this app.", 12.0).color(p.muted));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for (theme, label, icon) in [
                    ("light", "Light", Icon::Sun),
                    ("dark", "Dark", Icon::Moon),
                    ("system", "System", Icon::Monitor),
                ] {
                    ui.scope(|ui| {
                        if self.draft.theme == theme {
                            ui.visuals_mut().widgets.inactive.bg_fill = p.hover;
                        }
                        let response = design::button(ui, label, Some(icon), false);
                        if self.draft.theme == theme {
                            ui.painter().rect_stroke(
                                response.rect,
                                6.0,
                                egui::Stroke::new(1.5_f32, p.text),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.clicked() {
                            self.draft.theme = theme.into();
                        }
                    });
                }
            });
            section_divider(ui);
            section_heading(ui, Icon::Keyboard, "Input");
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(design::strong("Keyboard media keys", 13.0));
                    ui.label(
                        design::text(
                            "Capture standard media keys, including optional volume controls.",
                            12.0,
                        )
                        .color(p.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design::toggle(
                        ui,
                        &mut self.draft.keyboard_fallback,
                        "Listen for keyboard media keys",
                    );
                });
            });
            ui.add_space(14.0);
            let mut repeat = self.draft.cooldown_ms != 0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(design::strong("Repeat delay", 13.0));
                    ui.label(
                        design::text("Ignore repeated commands within this time.", 12.0)
                            .color(p.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if design::toggle(ui, &mut repeat, "Enable repeat delay").changed() {
                        self.draft.cooldown_ms = if repeat { 250 } else { 0 };
                    }
                });
            });
            ui.add_enabled_ui(repeat, |ui| {
                let mut value = self.draft.cooldown_ms.max(80);
                ui.add(
                    egui::Slider::new(&mut value, 80..=2000)
                        .suffix(" ms")
                        .show_value(true),
                );
                if repeat {
                    self.draft.cooldown_ms = value;
                }
            });
            ui.label(
                design::text(
                    "Duplicate Windows reports are still filtered when repeat delay is off.",
                    11.0,
                )
                .color(p.muted),
            );
            section_divider(ui);
            section_heading(ui, Icon::Archive, "Backup");
            ui.label(
                design::text(
                    "Export profiles and preferences, or restore a saved setup.",
                    12.0,
                )
                .color(p.muted),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                export =
                    design::button(ui, "Export settings", Some(Icon::Download), false).clicked();
                import = design::button(ui, "Import settings", Some(Icon::Upload), false).clicked();
            });
            ui.add_space(5.0);
            ui.label(
                design::text(
                    "Targets and activity history are excluded from backups.",
                    12.0,
                )
                .color(p.muted),
            );
            section_divider(ui);
            ui.horizontal_wrapped(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(22.0), egui::Sense::hover());
                design::brand(ui.painter(), r, p.text, p.bg);
                ui.label(design::strong("EarControl", 12.0));
                ui.label(
                    design::text(format!("Version {}", env!("CARGO_PKG_VERSION")), 11.0)
                        .color(p.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(design::text("Windows · Local profiles", 11.0).color(p.muted));
                });
            });
        });
        if export {
            let result = (|| {
                let export = self.prepare_export()?;
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name("earcontrol-profiles.json")
                    .save_file()
                {
                    settings::save_settings(&path, &export)?;
                    self.notice = Some("Settings exported.".into());
                }
                Ok(())
            })();
            self.report(result);
        }
        if import {
            let path = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file();
            self.prepare_import(path);
        }
    }

    fn profile_dialog(&mut self, ctx: &egui::Context) {
        if !self.profiles_open {
            return;
        }
        let p = Palette::get(ctx);
        let response = egui::Modal::new(egui::Id::new("profiles"))
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .stroke(egui::Stroke::new(1.0_f32, p.line))
                    .corner_radius(10)
                    .inner_margin(22),
            )
            .show(ctx, |ui| {
                ui.set_width(516.0);
                ui.horizontal(|ui| {
                    ui.label(design::strong("Profiles", 17.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if design::icon_button(
                            ui,
                            Icon::Close,
                            "Close profiles",
                            egui::Vec2::splat(28.0),
                        )
                        .clicked()
                        {
                            self.profiles_open = false;
                        }
                    });
                });
                ui.label(
                    design::text(
                        "A profile stores the actions assigned to your buttons.",
                        12.0,
                    )
                    .color(p.muted),
                );
                ui.add_space(12.0);
                let count = self.draft.profiles.len();
                let mut remove = None;
                egui::ScrollArea::vertical()
                    .max_height(350.0)
                    .show(ui, |ui| {
                        for profile in &mut self.draft.profiles {
                            ui.push_id(&profile.id, |ui| {
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.radio_value(
                                            &mut self.draft.active_profile_id,
                                            profile.id.clone(),
                                            "",
                                        );
                                        let response = ui.add(
                                            egui::TextEdit::singleline(&mut profile.name)
                                                .id(egui::Id::new(("profile_name", &profile.id)))
                                                .char_limit(80)
                                                .desired_width(240.0),
                                        );
                                        focus_review(
                                            &mut self.review_focus,
                                            ReviewFocus::ProfileName(profile.id.clone()),
                                            &response,
                                        );
                                        if ui
                                            .add_enabled(count > 1, egui::Button::new("Delete"))
                                            .clicked()
                                        {
                                            self.delete_profile = Some(profile.id.clone());
                                        }
                                    });
                                    let response = ui.add(
                                        egui::TextEdit::singleline(&mut profile.description)
                                            .id(egui::Id::new(("profile_description", &profile.id)))
                                            .char_limit(500)
                                            .desired_width(f32::INFINITY)
                                            .hint_text("Description, optional"),
                                    );
                                    focus_review(
                                        &mut self.review_focus,
                                        ReviewFocus::ProfileDescription(profile.id.clone()),
                                        &response,
                                    );
                                    if self.delete_profile.as_ref() == Some(&profile.id) {
                                        ui.horizontal(|ui| {
                                            ui.label("Remove this profile and its mappings?");
                                            if ui.button("Cancel").clicked() {
                                                self.delete_profile = None;
                                            }
                                            if ui.button("Remove").clicked() {
                                                remove = Some(profile.id.clone());
                                            }
                                        });
                                    }
                                });
                            });
                        }
                    });
                if let Some(id) = remove {
                    remove_profile(&mut self.draft, &id);
                    self.delete_profile = None;
                    self.recording = None;
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(count < 32, egui::Button::new("New profile"))
                        .clicked()
                    {
                        add_profile(&mut self.draft, false);
                        self.recording = None;
                    }
                    if ui
                        .add_enabled(count < 32, egui::Button::new("Duplicate active"))
                        .clicked()
                    {
                        add_profile(&mut self.draft, true);
                        self.recording = None;
                    }
                });
                if let Err(error) = model::validate_settings(&self.draft) {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                    if first_issue(&self.draft).is_some_and(|(_, mapping, _)| mapping.is_some())
                        && design::button(ui, "Review action", None, false).clicked()
                    {
                        self.review_issue();
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(format!("{} of 32 profiles", self.draft.profiles.len()));
                    if ui
                        .add_enabled_ui(model::validate_settings(&self.draft).is_ok(), |ui| {
                            design::button(ui, "Save changes", Some(Icon::Check), true)
                        })
                        .inner
                        .clicked()
                    {
                        match self.save() {
                            Ok(()) => self.profiles_open = false,
                            Err(error) => self.notice = Some(error),
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.profiles_open = false;
                    }
                });
            });
        if response.should_close() {
            self.profiles_open = false;
        }
    }
    fn import_dialog(&mut self, ctx: &egui::Context) {
        let Some(import) = self.import_pending.clone() else {
            return;
        };
        let response = egui::Modal::new(egui::Id::new("import_confirm")).show(ctx, |ui| {
            ui.heading("Import settings");
            ui.label(
                "This replaces your current profiles and preferences. Active control will stop.",
            );
            ui.label(format!(
                "{} profiles: {}",
                import.profiles.len(),
                import
                    .profiles
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.import_pending = None;
                }
                if ui.button("Replace and import").clicked() {
                    match self.runtime.save(import.clone()) {
                        Ok(()) => {
                            self.draft = import.clone();
                            self.saved = import.clone();
                            self.validation = None;
                            self.import_pending = None;
                            self.recording = None;
                            self.review_focus = None;
                            self.notice = Some("Profiles and settings imported.".into());
                        }
                        Err(error) => self.notice = Some(error),
                    }
                }
            });
        });
        if response.should_close() {
            self.import_pending = None;
        }
    }
}

impl eframe::App for EarControl {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            let _ = self.runtime.stop();
        }
        self.show(ctx);
        design::resize_edges(ctx);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.runtime.stop();
    }
}

pub fn configure_style(ctx: &egui::Context) {
    design::configure_style(ctx);
}

fn view_icon(view: View) -> Icon {
    match view {
        View::Buttons => Icon::Buttons,
        View::Macros => Icon::Macros,
        View::Devices => Icon::Headphones,
        View::Activity => Icon::Activity,
        View::Settings => Icon::Settings,
    }
}

fn fresh_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "native-{:x}-{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn can_select_target(status: &model::RuntimeStatus) -> bool {
    status.mode != "control" && !status.running_macro
}

/// Locate an editable field for the validator's first draft error.
fn first_issue(settings: &Settings) -> Option<(usize, Option<usize>, ReviewFocus)> {
    let invalid_name = |name: &str| {
        name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control)
    };
    for (pi, profile) in settings.profiles.iter().enumerate() {
        if invalid_name(&profile.name) {
            return Some((pi, None, ReviewFocus::ProfileName(profile.id.clone())));
        }
        if profile.description.chars().count() > 500 {
            return Some((
                pi,
                None,
                ReviewFocus::ProfileDescription(profile.id.clone()),
            ));
        }
        for (mi, mapping) in profile.mappings.iter().enumerate() {
            let issue = |focus| Some((pi, Some(mi), focus));
            if invalid_name(&mapping.name) {
                return issue(ReviewFocus::ActionName(mapping.id.clone()));
            }
            if mapping.steps.len() > 32 || (mapping.enabled && mapping.steps.is_empty()) {
                return issue(ReviewFocus::Summary(mapping.id.clone()));
            }
            let mut delay = 0_u64;
            let mut text_length = 0_usize;
            for (index, step) in mapping.steps.iter().enumerate() {
                let invalid = match step {
                    MacroStep::Key { keys } => crate::shortcuts::parse_shortcut(keys).is_err(),
                    MacroStep::Delay { ms } => {
                        delay = delay.saturating_add(*ms);
                        delay > 30_000
                    }
                    MacroStep::Text { text } => {
                        text_length = text_length.saturating_add(text.chars().count());
                        text_length > 2_000 || text.contains('\0')
                    }
                };
                if invalid {
                    return issue(ReviewFocus::Step(mapping.id.clone(), index));
                }
            }
        }
    }
    None
}

fn focus_review(pending: &mut Option<ReviewFocus>, field: ReviewFocus, response: &egui::Response) {
    if pending.as_ref() == Some(&field) {
        response.request_focus();
        response.scroll_to_me(Some(egui::Align::Center));
        *pending = None;
    }
}

fn trigger_icon(trigger: &str) -> Icon {
    match trigger {
        "play_pause" => Icon::PlayPause,
        "next" => Icon::Next,
        "previous" => Icon::Previous,
        "stop" => Icon::Stop,
        "mute" => Icon::Muted,
        _ => Icon::Volume,
    }
}
fn section_heading(ui: &mut egui::Ui, icon: Icon, title: &str) {
    let p = Palette::get(ui.ctx());
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(17.0), egui::Sense::hover());
        design::draw_icon(ui.painter(), r, icon, p.muted);
        ui.label(design::strong(title, 16.0));
    });
}
fn section_divider(ui: &mut egui::Ui) {
    ui.add_space(18.0);
    let p = Palette::get(ui.ctx());
    let (r, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    let mut x = r.left();
    while x < r.right() {
        ui.painter().line_segment(
            [
                egui::pos2(x, r.top()),
                egui::pos2((x + 3.0).min(r.right()), r.top()),
            ],
            egui::Stroke::new(1.0_f32, p.line),
        );
        x += 6.0;
    }
    ui.add_space(17.0);
}
fn empty_state(ui: &mut egui::Ui, icon: Icon, title: &str, detail: &str) {
    let p = Palette::get(ui.ctx());
    ui.add_space(36.0);
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::hover());
        design::draw_icon(ui.painter(), r, icon, p.muted);
        ui.add_space(8.0);
        ui.label(design::strong(title, 14.0));
        ui.label(design::text(detail, 12.0).color(p.muted));
    });
    ui.add_space(36.0);
}
fn mapping_row(ui: &mut egui::Ui, mapping: &mut Mapping) -> bool {
    let p = Palette::get(ui.ctx());
    let (r, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::hover());
    let color = if mapping.enabled { p.text } else { p.muted };
    ui.painter()
        .hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0_f32, p.line));
    let icon = egui::Rect::from_center_size(
        egui::pos2(r.left() + 39.0, r.center().y),
        egui::Vec2::splat(34.0),
    );
    ui.painter().rect(
        icon,
        8.0,
        p.soft,
        egui::Stroke::new(1.0_f32, p.line),
        egui::StrokeKind::Inside,
    );
    design::draw_icon(
        ui.painter(),
        icon.shrink(8.0),
        trigger_icon(&mapping.trigger),
        p.muted,
    );
    ui.painter().text(
        egui::pos2(r.left() + 68.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        trigger_label(&mapping.trigger),
        design::font(13.0, true),
        color,
    );
    let command_width = (r.width() - 140.0) * 0.36;
    let action = egui::Rect::from_min_max(
        egui::pos2(r.left() + 22.0 + command_width + 14.0, r.top() + 8.0),
        egui::pos2(r.right() - 84.0, r.bottom() - 8.0),
    );
    let response = ui.interact(
        action,
        ui.id().with((&mapping.id, "edit")),
        egui::Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("Edit {}: {}", trigger_label(&mapping.trigger), mapping.name),
        )
    });
    if response.hovered() {
        ui.painter().rect_filled(action, 5.0, p.soft);
    }
    let key = if let [MacroStep::Key { keys }] = mapping.steps.as_slice() {
        Some(keys.as_str())
    } else {
        None
    };
    let title = if mapping.steps.is_empty() {
        "Add an action"
    } else {
        &mapping.name
    };
    let painter = ui
        .painter()
        .with_clip_rect(action.shrink2(egui::vec2(10.0, 0.0)));
    let title_y = if mapping.steps.len() > 1 {
        action.center().y - 8.0
    } else {
        action.center().y
    };
    painter.text(
        egui::pos2(action.left() + 10.0, title_y),
        egui::Align2::LEFT_CENTER,
        title,
        design::font(14.0, true),
        color,
    );
    if let Some(keys) = key {
        let width = (keys.chars().count() as f32 * 6.6 + 14.0).min(action.width() * 0.4);
        let chip = egui::Rect::from_center_size(
            egui::pos2(action.right() - 25.0 - width / 2.0, action.center().y),
            egui::vec2(width, 22.0),
        );
        painter.rect(
            chip,
            4.0,
            p.soft,
            egui::Stroke::new(1.0_f32, p.line),
            egui::StrokeKind::Inside,
        );
        painter.text(
            chip.center(),
            egui::Align2::CENTER_CENTER,
            keys,
            egui::FontId::monospace(11.0),
            color,
        );
    } else if mapping.steps.len() > 1 {
        painter.text(
            egui::pos2(action.left() + 10.0, action.center().y + 10.0),
            egui::Align2::LEFT_CENTER,
            format!("{} steps", mapping.steps.len()),
            egui::FontId::monospace(11.0),
            p.muted,
        );
    }
    design::draw_icon(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(action.right() - 8.0, action.center().y),
            egui::Vec2::splat(14.0),
        ),
        Icon::ChevronRight,
        p.muted,
    );
    let mut edit = response.clicked();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt((&mapping.id, "enabled"))
            .max_rect(egui::Rect::from_min_size(
                egui::pos2(r.right() - 60.0, r.center().y - 12.0),
                egui::vec2(38.0, 24.0),
            )),
    );
    if design::toggle(
        &mut child,
        &mut mapping.enabled,
        &format!("Enable {}", trigger_label(&mapping.trigger)),
    )
    .changed()
        && mapping.steps.is_empty()
    {
        mapping.enabled = false;
        edit = true;
    }
    edit
}
fn event_row(ui: &mut egui::Ui, event: &model::ActivityEvent, full: bool) {
    let p = Palette::get(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(18.0), egui::Sense::hover());
        design::draw_icon(ui.painter(), r, trigger_icon(&event.trigger), p.muted);
        ui.vertical(|ui| {
            ui.label(design::strong(trigger_label(&event.trigger), 13.0));
            ui.label(design::text(&event.detail, 12.0).color(p.muted));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                design::text(&event.outcome, 11.0).color(if event.outcome == "executed" {
                    p.green
                } else {
                    p.muted
                }),
            );
            if full {
                ui.label(
                    design::text(
                        format!(
                            "{:02}:{:02}:{:02}",
                            event.timestamp / 3_600_000 % 24,
                            event.timestamp / 60_000 % 60,
                            event.timestamp / 1000 % 60
                        ),
                        11.0,
                    )
                    .color(p.muted),
                );
            }
        });
    });
}
pub fn add_profile(settings: &mut Settings, duplicate: bool) {
    if settings.profiles.len() >= 32 {
        return;
    }
    let id = fresh_id();
    let mut profile = if duplicate {
        let mut profile = settings
            .profiles
            .iter()
            .find(|p| p.id == settings.active_profile_id)
            .unwrap()
            .clone();
        profile.name = format!("{} copy", profile.name.chars().take(70).collect::<String>());
        for mapping in &mut profile.mappings {
            mapping.id = fresh_id();
        }
        profile
    } else {
        Profile {
            id: String::new(),
            name: "New profile".into(),
            description: "Your own set of headphone controls.".into(),
            accent: "#a64619".into(),
            mappings: model::TRIGGERS
                .iter()
                .map(|trigger| Mapping {
                    id: fresh_id(),
                    trigger: (*trigger).into(),
                    name: trigger_label(trigger).into(),
                    enabled: false,
                    steps: Vec::new(),
                })
                .collect(),
        }
    };
    profile.id = id.clone();
    settings.active_profile_id = id;
    settings.profiles.push(profile);
}
pub fn remove_profile(settings: &mut Settings, id: &str) {
    if settings.profiles.len() <= 1 {
        return;
    }
    settings.profiles.retain(|p| p.id != id);
    if settings.active_profile_id == id {
        settings.active_profile_id = settings.profiles[0].id.clone();
    }
}
pub fn recorded_shortcut(key: egui::Key, modifiers: egui::Modifiers, win: bool) -> Option<String> {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if win {
        parts.push("Win");
    }
    let key = match key {
        egui::Key::ArrowRight => "Right",
        egui::Key::ArrowLeft => "Left",
        egui::Key::ArrowUp => "Up",
        egui::Key::ArrowDown => "Down",
        other => other.name(),
    };
    parts.push(key);
    let shortcut = parts.join("+");
    crate::shortcuts::parse_shortcut(&shortcut)
        .ok()
        .map(|_| shortcut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_export_and_cancelled_import_do_not_touch_runtime_or_saved_settings() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let changes = Arc::new(AtomicUsize::new(0));
        let observed = changes.clone();
        let runtime = AppState::new(path.clone(), move || {
            observed.fetch_add(1, Ordering::Relaxed);
        });
        let mut app = EarControl::from_runtime(runtime);
        let before = app.draft.clone();
        changes.store(0, Ordering::Relaxed);
        assert_eq!(app.prepare_export().unwrap(), before);
        app.prepare_import(None);
        assert!(!path.exists(), "Unchanged export must not rewrite settings");
        assert_eq!(
            changes.load(Ordering::Relaxed),
            0,
            "Neither operation may stop capture"
        );

        let imported_path = dir.path().join("import.json");
        let imported = Settings {
            theme: "dark".into(),
            ..Settings::default()
        };
        settings::save_settings(&imported_path, &imported).unwrap();
        app.prepare_import(Some(imported_path));
        assert_eq!(app.import_pending, Some(imported));
        assert_eq!(app.draft, before);
        assert_eq!(changes.load(Ordering::Relaxed), 0);

        let ctx = egui::Context::default();
        configure_style(&ctx);
        let _ = frame(&mut app, &ctx, Vec::new());
        let _ = frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.import_pending.is_none(), "Escape should cancel import");
        assert_eq!(app.draft, before);
        assert_eq!(app.saved, before);
        assert_eq!(changes.load(Ordering::Relaxed), 0);

        app.import_pending = Some(before.clone());
        let _ = frame(&mut app, &ctx, Vec::new());
        click_at(&mut app, &ctx, egui::pos2(10.0, 10.0));
        assert!(
            app.import_pending.is_none(),
            "The backdrop should cancel import"
        );
        assert_eq!(changes.load(Ordering::Relaxed), 0);

        app.draft.theme = "dark".into();
        assert_eq!(app.prepare_export().unwrap(), app.draft);
        assert_eq!(app.saved, app.draft);
        assert_eq!(settings::read_settings(&path).unwrap().unwrap(), app.draft);
        assert!(
            changes.load(Ordering::Relaxed) > 0,
            "Exporting edits still persists them"
        );
    }

    #[test]
    fn target_selection_remains_available_while_monitoring() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        let mut status = runtime.status();
        assert!(can_select_target(&status));
        status.mode = "monitor".into();
        assert!(can_select_target(&status));
        status.mode = "control".into();
        assert!(!can_select_target(&status));
        status.mode = "idle".into();
        status.running_macro = true;
        assert!(!can_select_target(&status));
    }

    #[test]
    fn review_issue_selects_the_invalid_profile_and_focuses_its_macro_field() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        let mut app = EarControl::from_runtime(runtime);
        app.draft.profiles[1].mappings[2].name.clear();
        app.validation = model::validate_settings(&app.draft).err();
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let output = frame(&mut app, &ctx, Vec::new());
        click_at(&mut app, &ctx, text_position(&output, "Review issue"));
        assert_eq!(app.profile_index(), 1);
        assert_eq!(app.selected_mapping, 2);
        assert_eq!(app.view, View::Macros);
        assert!(!app.profiles_open);
        let _ = frame(&mut app, &ctx, Vec::new());
        let expected = egui::Id::new(("action_name", &app.draft.profiles[1].mappings[2].id));
        assert!(ctx.memory(|memory| memory.has_focus(expected)));
        assert!(app.review_focus.is_none());

        app.draft.profiles[1].mappings[2].name = "Fixed action".into();
        app.draft.profiles[2].name.clear();
        app.review_issue();
        assert_eq!(app.profile_index(), 2);
        assert!(app.profiles_open);
        let _ = frame(&mut app, &ctx, Vec::new());
        let expected = egui::Id::new(("profile_name", &app.draft.profiles[2].id));
        assert!(ctx.memory(|memory| memory.has_focus(expected)));
    }

    fn frame(
        app: &mut EarControl,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1160.0, 780.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.show(ctx),
        )
    }
    fn text_position(output: &egui::FullOutput, target: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == target => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing rendered control: {target}"))
    }
    fn click_at(app: &mut EarControl, ctx: &egui::Context, pos: egui::Pos2) {
        for pressed in [true, false] {
            let _ = frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn header_monitor_receives_click_and_exposes_capture_failure() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        let mut app = EarControl::from_runtime(runtime);
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let output = frame(&mut app, &ctx, Vec::new());
        let pos = text_position(&output, "Monitor");
        assert!(
            pos.y < 86.0,
            "The monitor control should be in the session bar"
        );
        click_at(&mut app, &ctx, pos);
        assert!(
            app.notice
                .as_ref()
                .is_some_and(|message| message.contains("Windows media controls are unavailable")),
            "The header drag region must not swallow the Monitor click"
        );
    }
    #[test]
    fn button_feedback_stays_visible_and_edit_links_open_the_macro_editor() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        let mut app = EarControl::from_runtime(runtime);
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let output = frame(&mut app, &ctx, Vec::new());
        let feedback = text_position(&output, "Button check");
        assert!(feedback.y < 660.0, "Button check at {feedback:?}");
        let monitor = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Monitor" && text.pos.y > 86.0 => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .expect("Button check should offer Monitor");
        assert!(
            monitor.x > 1000.0,
            "Monitor belongs at the card's right edge"
        );
        assert!(monitor.y < 750.0, "Button check must remain above the fold");
        let practice = text_position(&output, "Practice deck");
        assert!(practice.x > 1000.0 && practice.y < 200.0);
        let pos = text_position(&output, "Next slide");
        click_at(&mut app, &ctx, pos);
        assert_eq!(app.view, View::Macros);
        assert_eq!(app.selected_mapping, 0);
    }
    #[test]
    fn profile_crud_keeps_valid_unique_mappings_and_at_least_one_profile() {
        let mut settings = Settings::default();
        add_profile(&mut settings, false);
        add_profile(&mut settings, true);
        model::validate_settings(&settings).unwrap();
        assert_ne!(
            settings.profiles[3].mappings[0].id,
            settings.profiles[4].mappings[0].id
        );
        while settings.profiles.len() > 1 {
            let id = settings.active_profile_id.clone();
            remove_profile(&mut settings, &id);
            model::validate_settings(&settings).unwrap();
        }
        let id = settings.active_profile_id.clone();
        remove_profile(&mut settings, &id);
        assert_eq!(settings.profiles.len(), 1);
    }
    #[test]
    fn recording_canonicalizes_keys_and_preserves_modifiers() {
        assert_eq!(
            recorded_shortcut(
                egui::Key::ArrowRight,
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                false
            ),
            Some("Ctrl+Shift+Right".into())
        );
        assert_eq!(
            recorded_shortcut(egui::Key::Escape, egui::Modifiers::NONE, true),
            Some("Win+Escape".into())
        );
    }
    #[test]
    fn every_screen_renders_at_minimum_and_normal_size_in_both_themes() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = AppState::new(dir.path().join("profiles.json"), || {});
        let mut app = EarControl::from_runtime(runtime);
        for size in [[860.0, 620.0], [1160.0, 780.0]] {
            for theme in ["light", "dark"] {
                for (view, _, _) in VIEWS {
                    app.view = view;
                    app.draft.theme = theme.into();
                    let ctx = egui::Context::default();
                    configure_style(&ctx);
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(size[0], size[1]),
                        )),
                        ..Default::default()
                    };
                    let output = ctx.run(input, |ctx| app.show(ctx));
                    assert!(!output.shapes.is_empty());
                    assert!(!ctx
                        .tessellate(output.shapes, output.pixels_per_point)
                        .is_empty());
                }
            }
        }
    }
}
