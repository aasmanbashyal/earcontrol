use crate::design::{self, Icon, Palette};
use eframe::egui::{
    self, Align, Align2, Color32, Id, Key, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Slides,
    Blank,
    Ended,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Next,
    Previous,
    First,
    Last,
    Blank,
    End,
    Resume,
    Restart,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub index: usize,
    pub mode: Mode,
}
impl State {
    pub fn apply(&mut self, command: Command, count: usize) {
        use Command::*;
        match command {
            Restart => *self = Self::default(),
            End => self.mode = Mode::Ended,
            Resume => self.mode = Mode::Slides,
            _ if self.mode == Mode::Ended => {}
            Blank => {
                self.mode = if self.mode == Mode::Blank {
                    Mode::Slides
                } else {
                    Mode::Blank
                }
            }
            _ if self.mode == Mode::Blank => self.mode = Mode::Slides,
            First => self.index = 0,
            Last => self.index = count.saturating_sub(1),
            Next => self.index = (self.index + 1).min(count.saturating_sub(1)),
            Previous => self.index = self.index.saturating_sub(1),
        }
    }
}

pub fn command(key: Key, modifiers: egui::Modifiers) -> Option<Command> {
    if modifiers.ctrl || modifiers.alt || modifiers.mac_cmd {
        return None;
    }
    Some(match key {
        Key::ArrowRight | Key::ArrowDown | Key::PageDown | Key::Space => Command::Next,
        Key::ArrowLeft | Key::ArrowUp | Key::PageUp => Command::Previous,
        Key::Home => Command::First,
        Key::End => Command::Last,
        Key::B => Command::Blank,
        Key::Escape => Command::End,
        Key::Enter => Command::Resume,
        _ => return None,
    })
}

pub const SLIDES: [(&str, &str, &str); 5] = [
    ("Try your buttons.", "Start control in EarControl, then return to this window. Press the headphone button mapped to Right.", "A successful press moves to slide 02."),
    ("One press. One slide.", "Your next-slide shortcut reached this window. Press the button mapped to Left to go back, or continue with Right.", "Try a few quick presses to check the repeat setting."),
    ("Move at your pace.", "Next and previous move one slide at a time. Home returns to the first slide. End jumps to the last.", "Keyboard shortcuts and headphone macros use the same controls here."),
    ("Pause the screen.", "Press B to blank the slide. Press B or a navigation key to reveal it. Escape ends the presentation.", "After Escape, press Enter or choose Resume to continue."),
    ("Test the rest.", "Click the text field to test a text macro. Click this slide again when you want to return to navigation.", "Use Restart deck to repeat the check from the beginning."),
];
const LABELS: [&str; 5] = [
    "Start here",
    "Next slide",
    "Navigation",
    "Presentation controls",
    "Text macros",
];
fn slide_id() -> Id {
    Id::new("practice_slide_navigation")
}
fn text_id() -> Id {
    Id::new("practice_text_input")
}

pub struct Practice {
    viewport: egui::ViewportId,
    pub state: State,
    text: String,
    last_key: String,
    received: u64,
    keyboard_reference: bool,
    accessibility: Option<crate::practice_accessibility::Bridge>,
    visible: bool,
    initial_focus: bool,
    focus_visible: bool,
    initialization_frames: u8,
    error: Option<String>,
}

impl Default for Practice {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_DECK: AtomicU64 = AtomicU64::new(1);
        Self {
            // A closed viewport may still exist until the next parent pass.
            // Reopening must not inherit its Close event or native visibility.
            viewport: egui::ViewportId::from_hash_of((
                "practice",
                NEXT_DECK.fetch_add(1, Ordering::Relaxed),
            )),
            state: State::default(),
            text: String::new(),
            last_key: String::new(),
            received: 0,
            keyboard_reference: false,
            accessibility: None,
            visible: false,
            initial_focus: false,
            focus_visible: false,
            initialization_frames: 0,
            error: None,
        }
    }
}

impl Practice {
    pub fn viewport_id(&self) -> egui::ViewportId {
        self.viewport
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    fn control(&mut self, ctx: &egui::Context, command: Command) {
        self.state.apply(command, SLIDES.len());
        ctx.memory_mut(|m| m.request_focus(slide_id()));
    }

    fn keyboard_input(&mut self, ctx: &egui::Context) {
        // egui does not report Windows/Super as a modifier on Windows.
        if crate::native::win_key_held() {
            return;
        }
        let focused = ctx.memory(|m| m.focused());
        if focused == Some(text_id()) {
            return;
        }
        let mut handled = Vec::new();
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                // Enter and Space activate a focused control. Other deck keys still
                // navigate, matching the browser deck's keyboard behavior.
                if focused.is_some()
                    && focused != Some(slide_id())
                    && matches!(key, Key::Space | Key::Enter)
                {
                    continue;
                }
                if let Some(command) = command(key, modifiers) {
                    self.state.apply(command, SLIDES.len());
                    self.last_key = key.name().replace("Arrow", "");
                    self.received += 1;
                    handled.push(key);
                }
            }
        }
        // Do not also activate a button or scroll when a deck command used this key.
        if !handled.is_empty() {
            ctx.input_mut(|input| {
                input.events.retain(
                    |event| !matches!(event, egui::Event::Key {key, ..} if handled.contains(key)),
                )
            });
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.initial_focus {
            ctx.memory_mut(|m| m.request_focus(slide_id()));
            self.initial_focus = true;
        }
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.focus_visible = false;
        } else if ctx.input(|i| i.key_pressed(Key::Tab)) {
            self.focus_visible = true;
        }
        if !self.visible && ctx.viewport_id() == self.viewport {
            if let Some(target) = crate::window::practice_target() {
                match crate::practice_accessibility::Bridge::attach(ctx, target.handle) {
                    Ok(bridge) => self.accessibility = Some(bridge),
                    Err(error) => self.error = Some(error),
                }
                self.visible = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            } else {
                self.initialization_frames = self.initialization_frames.saturating_add(1);
                if self.initialization_frames >= 10 {
                    self.error =
                        Some("Could not attach accessibility to the practice window.".into());
                    self.visible = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(20));
                }
            }
        }
        self.keyboard_input(ctx);
        let palette = Palette::get(ctx);
        let narrow = ctx.content_rect().width() <= 680.0;
        self.header(ctx, palette, narrow);
        self.footer(ctx, palette, narrow);
        if let Some(error) = &self.error {
            egui::TopBottomPanel::bottom("practice_error").show(ctx, |ui| {
                ui.label(design::text(error, 12.0).color(palette.danger));
            });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(palette.bg))
            .show(ctx, |ui| {
                let margin = if narrow {
                    19.0
                } else if ui.available_width() <= 850.0 {
                    20.0
                } else {
                    27.0
                };
                let body_height = (ui.available_height() - 2.0 * margin).max(385.0);
                egui::ScrollArea::vertical()
                    .id_salt("practice_body_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
                            let body_width = ui.available_width();
                            if narrow {
                                self.workspace(ui, palette, body_width, 355.0);
                                ui.add_space(24.0);
                                let column = (ui.available_width() - 23.0) / 2.0;
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(column, 160.0),
                                        Layout::top_down(Align::Min),
                                        |ui| self.received_tool(ui, palette),
                                    );
                                    ui.add_space(13.0);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(column, 240.0),
                                        Layout::top_down(Align::Min),
                                        |ui| self.text_tool(ui, palette),
                                    );
                                });
                                self.reference(ui, palette);
                            } else {
                                let gap = if body_width < 800.0 { 20.0 } else { 27.0 };
                                let tools_width = if body_width < 800.0 { 200.0 } else { 230.0 };
                                let workspace_width = body_width - tools_width - gap;
                                let workspace_height = body_height;
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(workspace_width, workspace_height),
                                        Layout::top_down(Align::Min),
                                        |ui| {
                                            self.workspace(
                                                ui,
                                                palette,
                                                workspace_width,
                                                workspace_height,
                                            )
                                        },
                                    );
                                    ui.add_space(gap - ui.spacing().item_spacing.x);
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(tools_width, workspace_height),
                                        Layout::top_down(Align::Min),
                                        |ui| {
                                            self.received_tool(ui, palette);
                                            ui.add_space(20.0);
                                            self.text_tool(ui, palette);
                                            ui.add_space(20.0);
                                            self.reference(ui, palette);
                                        },
                                    );
                                });
                            }
                        });
                    });
            });
        design::resize_edges(ctx);
    }

    fn header(&mut self, ctx: &egui::Context, palette: Palette, narrow: bool) {
        let height = if narrow { 62.0 } else { 70.0 };
        egui::TopBottomPanel::top("practice_header")
            .exact_height(height)
            .frame(egui::Frame::NONE.fill(palette.bg))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let chrome = Rect::from_min_size(
                    Pos2::new(rect.right() - 138.0, rect.top()),
                    Vec2::new(138.0, 36.0),
                );
                let restart = Rect::from_min_size(
                    Pos2::new(chrome.left() - 142.0, rect.top() + (height - 36.0) / 2.0),
                    Vec2::new(124.0, 36.0),
                );
                let drag_rect =
                    Rect::from_min_max(rect.min, Pos2::new(restart.left() - 8.0, rect.bottom()));
                let response =
                    ui.interact(drag_rect, ui.id().with("drag"), Sense::click_and_drag());
                if response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if response.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                        !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                    ));
                }
                let brand_rect = Rect::from_min_size(
                    Pos2::new(rect.left() + 26.0, rect.top() + (height - 22.0) / 2.0),
                    Vec2::splat(22.0),
                );
                design::brand(ui.painter(), brand_rect, palette.text, palette.bg);
                ui.put(
                    Rect::from_min_size(
                        brand_rect.right_top() + Vec2::new(10.0, 1.0),
                        Vec2::new(72.0, 22.0),
                    ),
                    egui::Label::new(design::strong("EarControl", 13.0)),
                );
                let separator_x = brand_rect.right() + 94.0;
                ui.painter().line_segment(
                    [
                        Pos2::new(separator_x, rect.center().y - 7.0),
                        Pos2::new(separator_x, rect.center().y + 7.0),
                    ],
                    Stroke::new(1.0_f32, palette.line),
                );
                ui.put(
                    Rect::from_min_size(
                        Pos2::new(separator_x + 11.0, brand_rect.top() + 1.0),
                        Vec2::new(80.0, 22.0),
                    ),
                    egui::Label::new(design::text("Practice deck", 11.0).color(palette.muted)),
                );
                let mut action_ui = ui.new_child(egui::UiBuilder::new().max_rect(restart));
                if design::button(&mut action_ui, "Restart deck", Some(Icon::Refresh), false)
                    .clicked()
                {
                    self.control(ctx, Command::Restart);
                }
                design::window_controls(ui, chrome);
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom(),
                    Stroke::new(1.0_f32, palette.line),
                );
            });
    }

    fn footer(&self, ctx: &egui::Context, palette: Palette, narrow: bool) {
        let typing = ctx.memory(|m| m.has_focus(text_id()));
        egui::TopBottomPanel::bottom("practice_footer")
            .exact_height(if narrow { 48.0 } else { 40.0 })
            .frame(egui::Frame::NONE.fill(palette.bg))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.x_range(),
                    rect.top(),
                    Stroke::new(1.0_f32, palette.line),
                );
                let mut left = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(
                            rect.left_center() + Vec2::new(26.0, -10.0),
                            Vec2::new(140.0, 20.0),
                        ))
                        .layout(Layout::left_to_right(Align::Center)),
                );
                let (dot, _) = left.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                left.painter()
                    .circle_filled(dot.center(), 3.0, palette.muted);
                left.label(
                    design::text(
                        if typing {
                            "Text input"
                        } else {
                            "Slide navigation"
                        },
                        10.0,
                    )
                    .color(palette.muted),
                );
                let width = if narrow { 270.0 } else { 390.0 };
                let mut right = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(
                            Pos2::new(rect.right() - 26.0 - width, rect.top() + 9.0),
                            Vec2::new(width, 30.0),
                        ))
                        .layout(Layout::right_to_left(Align::Center)),
                );
                right.label(
                    design::text(
                        "Start control in EarControl, then keep this window in front.",
                        10.0,
                    )
                    .color(palette.muted),
                );
            });
    }

    fn workspace(&mut self, ui: &mut egui::Ui, palette: Palette, width: f32, height: f32) {
        let padding = if width < 500.0 { 20.0 } else { 27.0 };
        let inner_width = width - padding * 2.0;
        let (title, body, hint) = match self.state.mode {
            Mode::Slides => SLIDES[self.state.index],
            Mode::Blank => (
                "Screen blanked.",
                "Press B or a navigation key to reveal this slide.",
                "",
            ),
            Mode::Ended => (
                "Presentation ended.",
                "Press Enter or choose Resume to return to this slide.",
                "",
            ),
        };
        let title_size = (width * 0.055).clamp(24.0, 34.0);
        let body_size = if width < 500.0 { 12.0 } else { 13.0 };
        let content_height = measure(ui, title, title_size, inner_width, None)
            + 18.0
            + measure(ui, body, body_size, inner_width.min(430.0), Some(23.0))
            + if self.state.mode == Mode::Slides {
                42.0 + measure(ui, hint, 11.0, inner_width.min(430.0), Some(18.0))
            } else {
                44.0 + 21.0 + 40.0
            };
        let card_height = (height - 55.0).max(300.0).max(content_height + 150.0);
        let (card, _) = ui.allocate_exact_size(Vec2::new(width, card_height), Sense::hover());
        let response = ui.interact(card, slide_id(), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Slide navigation")
        });
        if response.clicked() {
            response.request_focus();
        }
        ui.painter().rect(
            card,
            8.0,
            palette.surface,
            Stroke::new(1.0_f32, palette.line),
            StrokeKind::Inside,
        );
        if response.has_focus() && self.focus_visible {
            ui.painter().rect_stroke(
                card.expand(3.0),
                8.0,
                Stroke::new(1.0_f32, palette.muted),
                StrokeKind::Inside,
            );
        }
        let inner = card.shrink2(Vec2::new(padding, 24.0));
        let mut meta = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
            inner.min,
            Vec2::new(inner.width(), 20.0),
        )));
        meta.horizontal(|ui| {
            let label = match self.state.mode {
                Mode::Slides => LABELS[self.state.index],
                Mode::Blank => "Screen paused",
                Mode::Ended => "Presentation ended",
            };
            ui.label(design::text(label, 11.0).color(palette.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let counter = ui.label(
                    design::text(format!("{:02} / 05", self.state.index + 1), 10.0)
                        .monospace()
                        .color(palette.muted),
                );
                ui.ctx().accesskit_node_builder(counter.id, |node| {
                    node.set_label(format!("Slide {} of 5", self.state.index + 1));
                });
            });
        });
        let content_top =
            inner.top() + 44.0 + ((inner.height() - 86.0 - content_height) / 2.0).max(0.0);
        let content_rect = Rect::from_min_max(
            Pos2::new(inner.left(), content_top),
            Pos2::new(inner.right(), inner.bottom() - 40.0),
        );
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        content.spacing_mut().item_spacing.y = 0.0;
        if self.state.mode != Mode::Slides {
            let (icon, _) = content.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            design::draw_icon(
                content.painter(),
                icon,
                if self.state.mode == Mode::Blank {
                    Icon::Monitor
                } else {
                    Icon::Check
                },
                palette.muted,
            );
            content.add_space(16.0);
        }
        let heading = content.label(design::text(title, title_size).color(palette.text));
        ui.ctx().accesskit_node_builder(heading.id, |node| {
            node.set_role(egui::accesskit::Role::Heading);
            node.set_live(egui::accesskit::Live::Polite);
        });
        content.add_space(18.0);
        content.set_max_width(inner.width().min(430.0));
        content.label(
            design::text(body, if width < 500.0 { 12.0 } else { 13.0 })
                .line_height(Some(23.0))
                .color(palette.muted),
        );
        if self.state.mode == Mode::Slides {
            content.add_space(24.0);
            dashed(&content, content.cursor().top(), palette.line);
            content.add_space(18.0);
            content.label(
                design::text(hint, 11.0)
                    .line_height(Some(18.0))
                    .color(palette.muted),
            );
        } else {
            content.add_space(21.0);
            if design::button(&mut content, "Resume", Some(Icon::ArrowRight), false).clicked() {
                self.control(ui.ctx(), Command::Resume);
            }
        }
        let bottom_y = inner.bottom() - 21.0;
        dashed_line(
            ui.painter(),
            inner.left(),
            inner.right(),
            bottom_y - 16.0,
            palette.line,
        );
        let typing = ui.ctx().memory(|m| m.has_focus(text_id()));
        ui.painter().text(
            Pos2::new(inner.left(), bottom_y),
            Align2::LEFT_CENTER,
            if typing {
                "Click this slide to navigate"
            } else {
                "Ready for navigation"
            },
            egui::FontId::proportional(10.0),
            palette.muted,
        );
        for index in 0..5 {
            ui.painter().circle_filled(
                Pos2::new(inner.right() - 42.0 + index as f32 * 10.0, bottom_y),
                2.5,
                if index == self.state.index {
                    palette.muted
                } else {
                    palette.line
                },
            );
        }
        ui.add_space(15.0);
        let nav_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(width, 40.0));
        ui.allocate_rect(nav_rect, Sense::hover());
        let mut previous = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_size(nav_rect.min, Vec2::new(110.0, 40.0))),
        );
        if self.state.index == 0 || self.state.mode != Mode::Slides {
            previous.disable();
        }
        let previous = design::button(&mut previous, "Previous", Some(Icon::ArrowLeft), false);
        ui.ctx()
            .accesskit_node_builder(previous.id, |node| node.set_label("Previous slide"));
        if previous.clicked() {
            self.control(ui.ctx(), Command::Previous);
        }
        ui.painter().text(
            nav_rect.center(),
            Align2::CENTER_CENTER,
            format!("{} of 5", self.state.index + 1),
            egui::FontId::monospace(10.0),
            palette.muted,
        );
        let mut next = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
            Pos2::new(nav_rect.right() - 85.0, nav_rect.top()),
            Vec2::new(85.0, 40.0),
        )));
        if self.state.index >= 4 || self.state.mode != Mode::Slides {
            next.disable();
        }
        let next = design::button(&mut next, "Next", Some(Icon::ArrowRight), false);
        ui.ctx()
            .accesskit_node_builder(next.id, |node| node.set_label("Next slide"));
        if next.clicked() {
            self.control(ui.ctx(), Command::Next);
        }
    }

    fn tool_heading(
        ui: &mut egui::Ui,
        palette: Palette,
        icon: Icon,
        title: &str,
    ) -> egui::Response {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
            design::draw_icon(ui.painter(), rect, icon, palette.muted);
            ui.label(design::strong(title, 12.0))
        })
        .inner
    }

    fn received_tool(&self, ui: &mut egui::Ui, palette: Palette) {
        ui.add_space(4.0);
        Self::tool_heading(ui, palette, Icon::Keyboard, "Received shortcut");
        ui.add_space(14.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 33.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                if self.received == 0 {
                    ui.label(design::text("No shortcuts yet", 10.0).color(palette.muted));
                } else {
                    egui::Frame::NONE
                        .fill(palette.soft)
                        .stroke(Stroke::new(1.0_f32, palette.line))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(design::text(&self.last_key, 11.0).monospace());
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let received = ui.label(
                            design::text(format!("{} received", self.received), 10.0)
                                .color(palette.muted),
                        );
                        ui.ctx().accesskit_node_builder(received.id, |node| {
                            node.set_label(format!(
                                "{}, {} received",
                                self.last_key, self.received
                            ));
                            node.set_live(egui::accesskit::Live::Polite);
                        });
                    });
                }
            },
        );
        ui.add_space(12.0);
        ui.label(
            design::text(
                "Keyboard input and headphone actions appear here while this window is active.",
                11.0,
            )
            .line_height(Some(19.0))
            .color(palette.muted),
        );
        ui.add_space(21.0);
        dashed(ui, ui.cursor().top(), palette.line);
    }

    fn text_tool(&mut self, ui: &mut egui::Ui, palette: Palette) {
        ui.add_space(4.0);
        let label = Self::tool_heading(ui, palette, Icon::Text, "Text macro test");
        ui.add_space(14.0);
        let edit = ui
            .add_sized(
                [ui.available_width(), 100.0],
                egui::TextEdit::multiline(&mut self.text)
                    .id(text_id())
                    .desired_width(f32::INFINITY)
                    .desired_rows(4)
                    .char_limit(10000)
                    .font(egui::FontId::proportional(12.0))
                    .margin(Vec2::splat(10.0))
                    .hint_text("Click here, then run a text macro."),
            )
            .labelled_by(label.id);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                design::text(format!("{} characters", self.text.chars().count()), 10.0)
                    .color(palette.muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.text.is_empty(),
                        egui::Button::new(design::text("Clear", 10.0)).frame(false),
                    )
                    .clicked()
                {
                    self.text.clear();
                }
            });
        });
        ui.add_space(12.0);
        ui.label(
            design::text(
                if edit.has_focus() {
                    "Text field focused. Click the slide to resume navigation."
                } else {
                    "Text stays in this window and clears when it closes."
                },
                11.0,
            )
            .line_height(Some(19.0))
            .color(palette.muted),
        );
        ui.add_space(21.0);
        dashed(ui, ui.cursor().top(), palette.line);
    }

    fn reference(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(140.0, 24.0), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Keyboard reference")
        });
        design::draw_icon(
            ui.painter(),
            Rect::from_center_size(rect.left_center() + Vec2::new(5.0, 0.0), Vec2::splat(10.0)),
            if self.keyboard_reference {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            palette.muted,
        );
        ui.painter().text(
            rect.left_center() + Vec2::new(16.0, 0.0),
            Align2::LEFT_CENTER,
            "Keyboard reference",
            design::font(11.0, false),
            palette.muted,
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0_f32, palette.muted),
                StrokeKind::Inside,
            );
        }
        if response.clicked() {
            self.keyboard_reference = !self.keyboard_reference;
        }
        if self.keyboard_reference {
            ui.add_space(15.0);
            for (label, keys) in [
                ("Next / previous", "←  →"),
                ("First / last", "Home  End"),
                ("Blank screen", "B"),
                ("End presentation", "Esc"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(design::text(label, 10.0).color(palette.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(design::text(keys, 10.0).monospace());
                    });
                });
            }
            ui.add_space(12.0);
            ui.label(
                design::text(
                    "Page Up, Page Down, Up, Down, and Space also navigate.",
                    11.0,
                )
                .line_height(Some(19.0))
                .color(palette.muted),
            );
        }
    }
}

fn measure(ui: &egui::Ui, text: &str, size: f32, width: f32, line_height: Option<f32>) -> f32 {
    let mut job = egui::text::LayoutJob::simple(
        text.into(),
        design::font(size, false),
        ui.visuals().text_color(),
        width,
    );
    for section in &mut job.sections {
        section.format.line_height = line_height;
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job).size().y)
}

fn dashed(ui: &egui::Ui, y: f32, color: Color32) {
    dashed_line(
        ui.painter(),
        ui.max_rect().left(),
        ui.max_rect().right(),
        y,
        color,
    );
}
fn dashed_line(painter: &egui::Painter, left: f32, right: f32, y: f32, color: Color32) {
    design::dashed_line(painter, left, right, y, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_reveals_without_skipping_and_ended_ignores_navigation() {
        let mut state = State::default();
        state.apply(Command::Next, 5);
        state.apply(Command::Blank, 5);
        state.apply(Command::Next, 5);
        assert_eq!(
            state,
            State {
                index: 1,
                mode: Mode::Slides
            }
        );
        state.apply(Command::End, 5);
        state.apply(Command::Next, 5);
        assert_eq!(state.index, 1);
        state.apply(Command::Resume, 5);
        state.apply(Command::Last, 5);
        state.apply(Command::Next, 5);
        assert_eq!(state.index, 4);
        state.apply(Command::Restart, 5);
        state.apply(Command::Previous, 5);
        assert_eq!(state, State::default());
    }
    #[test]
    fn modified_shortcuts_do_not_navigate() {
        assert_eq!(command(Key::ArrowRight, egui::Modifiers::CTRL), None);
        assert_eq!(command(Key::B, egui::Modifiers::NONE), Some(Command::Blank));
    }
    fn key_frame(app: &mut Practice, ctx: &egui::Context, key: Key, focused: Option<Id>) {
        if let Some(id) = focused {
            ctx.memory_mut(|m| m.request_focus(id));
        }
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| app.keyboard_input(ctx),
        );
    }
    #[test]
    fn typing_owns_arrow_keys_while_slide_focus_navigates() {
        let ctx = egui::Context::default();
        let mut app = Practice::default();
        key_frame(&mut app, &ctx, Key::ArrowRight, Some(text_id()));
        assert_eq!(app.state.index, 0);
        assert_eq!(app.received, 0);
        key_frame(&mut app, &ctx, Key::ArrowRight, Some(slide_id()));
        assert_eq!(app.state.index, 1);
        assert_eq!(app.last_key, "Right");
        assert_eq!(app.received, 1);
    }
    #[test]
    fn focused_button_owns_enter_but_other_deck_keys_work() {
        let ctx = egui::Context::default();
        let mut app = Practice::default();
        app.state.mode = Mode::Ended;
        key_frame(&mut app, &ctx, Key::Enter, Some(Id::new("resume_button")));
        assert_eq!(app.state.mode, Mode::Ended);
        key_frame(&mut app, &ctx, Key::Enter, Some(slide_id()));
        assert_eq!(app.state.mode, Mode::Slides);
        key_frame(
            &mut app,
            &ctx,
            Key::ArrowRight,
            Some(Id::new("next_button")),
        );
        assert_eq!(app.state.index, 1);
    }

    #[test]
    fn slide_content_and_navigation_do_not_overlap_at_compact_widths() {
        for width in [300.0, 548.0, 649.0] {
            let ctx = egui::Context::default();
            design::configure_style(&ctx);
            for (index, slide) in SLIDES.iter().enumerate() {
                let mut app = Practice::default();
                app.state.index = index;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000.0, 1000.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .show(ctx, |ui| app.workspace(ui, Palette::get(ctx), width, 355.0));
                    },
                );
                let bounds = |label: &str| {
                    output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == label => {
                                Some(text.visual_bounding_rect())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("Missing text {label}"))
                };
                assert!(
                    bounds(slide.2).bottom() < bounds("Ready for navigation").top() - 8.0,
                    "Hint overlaps footer at {width}"
                );
                assert!(
                    bounds("Ready for navigation").bottom() < bounds("Previous").top(),
                    "Navigation overlaps card at {width}"
                );
            }
        }
    }

    #[test]
    fn escape_ends_the_deck_without_requesting_window_close() {
        let ctx = egui::Context::default();
        design::configure_style(&ctx);
        let mut app = Practice::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 640.0))),
                events: vec![egui::Event::Key {
                    key: Key::Escape,
                    physical_key: Some(Key::Escape),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| app.show(ctx),
        );
        assert_eq!(app.state.mode, Mode::Ended);
        assert!(output
            .viewport_output
            .values()
            .all(|viewport| !viewport.commands.contains(&egui::ViewportCommand::Close)));
    }

    #[test]
    fn custom_practice_button_remains_keyboard_activatable() {
        let ctx = egui::Context::default();
        design::configure_style(&ctx);
        let mut app = Practice::default();
        app.state.mode = Mode::Ended;
        let mut button_id = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                button_id = Some(design::button(ui, "Resume", Some(Icon::ArrowRight), false).id);
            });
        });
        ctx.memory_mut(|memory| memory.request_focus(button_id.unwrap()));
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::Enter,
                    physical_key: Some(Key::Enter),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                app.keyboard_input(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    if design::button(ui, "Resume", Some(Icon::ArrowRight), false).clicked() {
                        app.control(ctx, Command::Resume);
                    }
                });
            },
        );
        assert_eq!(app.state.mode, Mode::Slides);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(slide_id()));
    }

    #[test]
    fn accessible_tree_exposes_navigation_text_and_live_slide_updates() {
        let ctx = egui::Context::default();
        design::configure_style(&ctx);
        ctx.enable_accesskit();
        let mut app = Practice::default();
        let input = |events| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 640.0))),
            events,
            ..Default::default()
        };
        let output = ctx.run(input(Vec::new()), |ctx| app.show(ctx));
        assert_eq!(ctx.memory(|m| m.focused()), Some(slide_id()));
        let tree = output.platform_output.accesskit_update.unwrap();
        let next = tree
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("Next slide"))
            .unwrap();
        assert_eq!(next.1.role(), egui::accesskit::Role::Button);
        let text = tree
            .nodes
            .iter()
            .find(|(_, n)| n.role() == egui::accesskit::Role::MultilineTextInput)
            .unwrap();
        assert!(!text.1.labelled_by().is_empty());
        assert!(tree
            .nodes
            .iter()
            .any(|(_, n)| n.role() == egui::accesskit::Role::Heading
                && n.live() == Some(egui::accesskit::Live::Polite)));
        assert!(tree
            .nodes
            .iter()
            .any(|(_, n)| n.label() == Some("Slide 1 of 5")));
        let event = |action, target| {
            egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action,
                target,
                data: None,
            })
        };
        let _ = ctx.run(
            input(vec![event(egui::accesskit::Action::Click, next.0)]),
            |ctx| app.show(ctx),
        );
        assert_eq!(app.state.index, 1, "UIA Invoke must activate Next");
        let _ = ctx.run(
            input(vec![event(egui::accesskit::Action::Focus, text.0)]),
            |ctx| app.show(ctx),
        );
        assert_eq!(ctx.memory(|m| m.focused()), Some(text_id()));
        key_frame(&mut app, &ctx, Key::ArrowRight, Some(text_id()));
        assert_eq!(
            app.state.index, 1,
            "UIA-focused editor must own navigation keys"
        );
    }
}
