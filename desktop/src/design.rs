use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Painter, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub soft: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub hover: Color32,
    pub input_line: Color32,
    pub primary: Color32,
    pub primary_text: Color32,
    pub green: Color32,
    pub danger: Color32,
}
impl Palette {
    pub fn get(ctx: &egui::Context) -> Self {
        Self::for_dark(ctx.style().visuals.dark_mode)
    }
    pub fn for_dark(dark: bool) -> Self {
        let gray = Color32::from_gray;
        if dark {
            Self {
                bg: gray(16),
                sidebar: gray(17),
                surface: gray(23),
                soft: gray(27),
                text: gray(237),
                muted: gray(166),
                line: gray(48),
                hover: gray(36),
                input_line: gray(111),
                primary: gray(237),
                primary_text: gray(23),
                green: Color32::from_rgb(122, 212, 172),
                danger: Color32::from_rgb(255, 154, 166),
            }
        } else {
            Self {
                bg: gray(250),
                sidebar: gray(243),
                surface: Color32::WHITE,
                soft: gray(245),
                text: gray(39),
                muted: gray(98),
                line: gray(222),
                hover: gray(237),
                input_line: gray(133),
                primary: gray(39),
                primary_text: Color32::WHITE,
                green: Color32::from_rgb(24, 115, 79),
                danger: Color32::from_rgb(187, 37, 53),
            }
        }
    }
}
pub fn text(value: impl Into<String>, size: f32) -> RichText {
    RichText::new(value).font(FontId::proportional(size))
}
pub fn strong(value: impl Into<String>, size: f32) -> RichText {
    RichText::new(value).font(FontId::new(size, FontFamily::Name("semibold".into())))
}
pub fn font(size: f32, bold: bool) -> FontId {
    if bold {
        FontId::new(size, FontFamily::Name("semibold".into()))
    } else {
        FontId::proportional(size)
    }
}

#[derive(Clone, Copy)]
pub enum Icon {
    Buttons,
    Macros,
    Headphones,
    Activity,
    Settings,
    Monitor,
    Radio,
    Play,
    Stop,
    Next,
    Previous,
    PlayPause,
    Volume,
    Muted,
    ChevronRight,
    ChevronDown,
    ArrowRight,
    ArrowLeft,
    Refresh,
    Keyboard,
    Text,
    Check,
    Close,
    Sun,
    Moon,
    Plus,
    Up,
    Down,
    Trash,
    Copy,
    Save,
    Download,
    Upload,
    External,
    More,
    Clock,
    Archive,
    Minimize,
    Maximize,
}
pub fn draw_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let p =
        |x: f32, y: f32| rect.min + egui::vec2(x / 24.0 * rect.width(), y / 24.0 * rect.height());
    let stroke = Stroke::new(1.5_f32, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    let box_ = |x: f32, y: f32, w: f32, h: f32, r: f32| {
        painter.rect_stroke(
            Rect::from_min_max(p(x, y), p(x + w, y + h)),
            r,
            stroke,
            StrokeKind::Inside,
        );
    };
    match icon {
        Icon::Buttons => {
            for (y, x) in [(5., 8.), (12., 16.), (19., 10.)] {
                line((3., y), (21., y));
                painter.circle_filled(p(x, y), 2.2, Palette::for_dark(color.r() > 180).surface);
                painter.circle_stroke(p(x, y), 2.2, stroke);
            }
        }
        Icon::Macros => {
            for y in [5., 12., 19.] {
                line((9., y), (21., y));
                painter.circle_filled(p(4., y), 1.2, color);
            }
        }
        Icon::Headphones => {
            let pts: Vec<_> = (0..=20)
                .map(|i| {
                    let a = std::f32::consts::PI + (i as f32 / 20.) * std::f32::consts::PI;
                    p(12. + 9. * a.cos(), 12. + 9. * a.sin())
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
            box_(3., 11., 4., 9., 2.);
            box_(17., 11., 4., 9., 2.);
        }
        Icon::Activity => path(&[
            (2., 12.),
            (6., 12.),
            (9., 4.),
            (14., 20.),
            (17., 12.),
            (22., 12.),
        ]),
        Icon::Settings => {
            for (x, y) in [(5., 7.), (12., 16.), (19., 9.)] {
                line((x, 3.), (x, 21.));
                painter.circle_filled(p(x, y), 2.3, Palette::for_dark(color.r() > 180).surface);
                painter.circle_stroke(p(x, y), 2.3, stroke);
            }
        }
        Icon::Monitor => {
            box_(2., 3., 20., 14., 2.);
            line((12., 17.), (12., 22.));
            line((8., 22.), (16., 22.));
        }
        Icon::Radio => {
            painter.circle_filled(rect.center(), 1.6, color);
            for radius in [5., 9.] {
                for sign in [-1., 1.] {
                    let pts = (0..=12)
                        .map(|i| {
                            let a = -0.9 + i as f32 / 12. * 1.8;
                            p(12. + sign * radius * a.cos(), 12. + radius * a.sin())
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, stroke));
                }
            }
        }
        Icon::Play => path(&[(6., 3.), (20., 12.), (6., 21.), (6., 3.)]),
        Icon::PlayPause => {
            path(&[(3., 4.), (13., 12.), (3., 20.), (3., 4.)]);
            line((17., 4.), (17., 20.));
            line((21., 4.), (21., 20.));
        }
        Icon::Next => {
            path(&[(4., 4.), (16., 12.), (4., 20.), (4., 4.)]);
            line((20., 4.), (20., 20.));
        }
        Icon::Previous => {
            path(&[(20., 4.), (8., 12.), (20., 20.), (20., 4.)]);
            line((4., 4.), (4., 20.));
        }
        Icon::Stop => box_(5., 5., 14., 14., 1.),
        Icon::Volume | Icon::Muted => {
            path(&[
                (3., 9.),
                (7., 9.),
                (12., 4.),
                (12., 20.),
                (7., 15.),
                (3., 15.),
                (3., 9.),
            ]);
            if matches!(icon, Icon::Muted) {
                line((17., 9.), (23., 15.));
                line((23., 9.), (17., 15.));
            } else {
                path(&[(16., 8.), (18., 10.), (18., 14.), (16., 16.)]);
                path(&[(19., 4.), (22., 8.), (23., 12.), (22., 16.), (19., 20.)]);
            }
        }
        Icon::ChevronRight => path(&[(9., 5.), (16., 12.), (9., 19.)]),
        Icon::ChevronDown => path(&[(5., 9.), (12., 16.), (19., 9.)]),
        Icon::ArrowRight => {
            line((3., 12.), (21., 12.));
            path(&[(14., 5.), (21., 12.), (14., 19.)]);
        }
        Icon::ArrowLeft => {
            line((3., 12.), (21., 12.));
            path(&[(10., 5.), (3., 12.), (10., 19.)]);
        }
        Icon::Refresh => {
            path(&[(20., 7.), (17., 4.), (10., 3.), (5., 6.), (3., 11.)]);
            path(&[(20., 3.), (20., 8.), (15., 8.)]);
            path(&[(4., 17.), (7., 20.), (14., 21.), (19., 18.), (21., 13.)]);
            path(&[(4., 21.), (4., 16.), (9., 16.)]);
        }
        Icon::Keyboard => {
            box_(2., 5., 20., 14., 2.);
            for y in [9., 12.] {
                for x in [6., 10., 14., 18.] {
                    painter.circle_filled(p(x, y), 0.7, color);
                }
            }
            line((7., 16.), (17., 16.));
        }
        Icon::Text => {
            line((3., 4.), (21., 4.));
            line((12., 4.), (12., 21.));
            line((8., 21.), (16., 21.));
        }
        Icon::Check => path(&[(4., 12.), (9., 17.), (20., 6.)]),
        Icon::Close => {
            line((5., 5.), (19., 19.));
            line((19., 5.), (5., 19.));
        }
        Icon::Sun => {
            painter.circle_stroke(rect.center(), rect.width() * 0.2, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                line(
                    (12. + 8. * a.cos(), 12. + 8. * a.sin()),
                    (12. + 11. * a.cos(), 12. + 11. * a.sin()),
                );
            }
        }
        Icon::Moon => path(&[
            (20., 16.),
            (16., 21.),
            (9., 21.),
            (4., 17.),
            (3., 10.),
            (7., 4.),
            (12., 3.),
            (9., 7.),
            (9., 12.),
            (13., 16.),
            (20., 16.),
        ]),
        Icon::Plus => {
            line((4., 12.), (20., 12.));
            line((12., 4.), (12., 20.));
        }
        Icon::Up => {
            line((12., 21.), (12., 3.));
            path(&[(5., 10.), (12., 3.), (19., 10.)]);
        }
        Icon::Down => {
            line((12., 3.), (12., 21.));
            path(&[(5., 14.), (12., 21.), (19., 14.)]);
        }
        Icon::Trash => {
            line((3., 6.), (21., 6.));
            path(&[(6., 6.), (7., 21.), (17., 21.), (18., 6.)]);
            box_(9., 2., 6., 4., 1.);
            line((10., 10.), (10., 17.));
            line((14., 10.), (14., 17.));
        }
        Icon::Copy => {
            box_(8., 8., 13., 13., 2.);
            path(&[(16., 5.), (16., 3.), (3., 3.), (3., 16.), (5., 16.)]);
        }
        Icon::Save => {
            path(&[
                (3., 3.),
                (18., 3.),
                (21., 6.),
                (21., 21.),
                (3., 21.),
                (3., 3.),
            ]);
            box_(7., 3., 9., 6., 0.);
            box_(7., 14., 10., 7., 0.);
        }
        Icon::Download | Icon::Upload => {
            path(&[(3., 15.), (3., 21.), (21., 21.), (21., 15.)]);
            if matches!(icon, Icon::Download) {
                line((12., 2.), (12., 15.));
                path(&[(7., 10.), (12., 15.), (17., 10.)]);
            } else {
                line((12., 15.), (12., 2.));
                path(&[(7., 7.), (12., 2.), (17., 7.)]);
            }
        }
        Icon::External => {
            path(&[(14., 3.), (21., 3.), (21., 10.)]);
            line((21., 3.), (10., 14.));
            path(&[(10., 3.), (3., 3.), (3., 21.), (21., 21.), (21., 14.)]);
        }
        Icon::More => {
            for x in [5., 12., 19.] {
                painter.circle_filled(p(x, 12.), 1.5, color);
            }
        }
        Icon::Clock => {
            painter.circle_stroke(rect.center(), rect.width() * 0.4, stroke);
            path(&[(12., 6.), (12., 12.), (16., 14.)]);
        }
        Icon::Archive => {
            box_(3., 3., 18., 5., 1.);
            box_(5., 8., 14., 13., 1.);
            line((10., 12.), (14., 12.));
        }
        Icon::Minimize => line((5., 12.), (19., 12.)),
        Icon::Maximize => box_(6., 6., 12., 12., 0.),
    }
}
pub fn brand(painter: &Painter, rect: Rect, color: Color32, bg: Color32) {
    let at =
        |x: f32, y: f32| rect.min + egui::vec2(x / 24. * rect.width(), y / 24. * rect.height());
    painter.add(egui::Shape::convex_polygon(
        vec![
            at(13., 6.),
            at(17., 6.),
            at(21., 9.),
            at(22., 13.),
            at(20., 18.),
            at(16., 21.),
            at(12., 20.),
            at(9., 17.),
            at(10., 13.),
            at(13., 10.),
        ],
        color,
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            at(6., 3.),
            at(10., 4.),
            at(12., 7.),
            at(11., 11.),
            at(8., 15.),
            at(5., 15.),
            at(2., 12.),
            at(2., 8.),
            at(4., 4.),
        ],
        color,
        Stroke::NONE,
    ));
    painter.line_segment(
        [at(6., 6.), at(4., 10.)],
        Stroke::new(rect.width() * 0.07, bg),
    );
}
pub fn button(ui: &mut Ui, label: &str, icon: Option<Icon>, primary: bool) -> Response {
    let palette = Palette::get(ui.ctx());
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font(13., true), palette.text);
    let width = galley.size().x + 28. + if icon.is_some() { 22. } else { 0. };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 36.), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let color = if primary {
        palette.primary_text
    } else {
        palette.text
    };
    let fill = if primary {
        palette.primary
    } else if response.hovered() {
        palette.hover
    } else {
        palette.surface
    };
    let alpha = if ui.is_enabled() { 1. } else { 0.45 };
    ui.painter().rect(
        rect,
        7.,
        fill.gamma_multiply(alpha),
        Stroke::new(
            1.0_f32,
            if primary {
                palette.primary
            } else {
                palette.input_line
            }
            .gamma_multiply(alpha),
        ),
        StrokeKind::Inside,
    );
    let x = rect.min.x + 14.;
    if let Some(icon) = icon {
        draw_icon(
            ui.painter(),
            Rect::from_center_size(egui::pos2(x + 7.5, rect.center().y), Vec2::splat(15.)),
            icon,
            color.gamma_multiply(alpha),
        );
    }
    ui.painter().text(
        egui::pos2(x + if icon.is_some() { 22. } else { 0. }, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font(13., true),
        color.gamma_multiply(alpha),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.),
            8.,
            Stroke::new(1.5_f32, palette.text),
            StrokeKind::Outside,
        );
    }
    response
}
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str, size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    paint_icon_button(ui, rect, &response, icon, label);
    response.on_hover_text(label)
}
fn paint_icon_button(ui: &Ui, rect: Rect, response: &Response, icon: Icon, label: &str) {
    let p = Palette::get(ui.ctx());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if response.hovered() {
        ui.painter().rect_filled(rect, 6., p.hover);
    }
    draw_icon(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(16.)),
        icon,
        p.muted
            .gamma_multiply(if ui.is_enabled() { 1. } else { 0.4 }),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(2.),
            4.,
            Stroke::new(1.0_f32, p.text),
            StrokeKind::Inside,
        );
    }
}
pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(38., 24.), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    let p = Palette::get(ui.ctx());
    ui.painter().rect(
        rect,
        12.,
        if *value { p.primary } else { p.hover },
        Stroke::new(1.0_f32, if *value { p.primary } else { p.input_line }),
        StrokeKind::Inside,
    );
    ui.painter().circle_filled(
        egui::pos2(
            rect.left() + if *value { 26. } else { 12. },
            rect.center().y,
        ),
        8.,
        if *value { p.primary_text } else { p.muted },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.),
            14.,
            Stroke::new(1.0_f32, p.text),
            StrokeKind::Outside,
        );
    }
    response
}
pub fn window_controls(ui: &mut Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    for (index, icon, label) in [
        (0, Icon::Minimize, "Minimize window"),
        (1, Icon::Maximize, "Maximize window"),
        (2, Icon::Close, "Close window"),
    ] {
        let r = Rect::from_min_size(
            rect.min + egui::vec2(index as f32 * 46., 0.),
            egui::vec2(46., 36.),
        );
        let response = ui.interact(r, ui.id().with(label), Sense::click());
        paint_icon_button(ui, r, &response, icon, label);
        if response.clicked() {
            ctx.send_viewport_cmd(match index {
                0 => egui::ViewportCommand::Minimized(true),
                1 => egui::ViewportCommand::Maximized(
                    !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                ),
                _ => egui::ViewportCommand::Close,
            });
        }
    }
}
pub fn configure_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let p = Palette::for_dark(style.visuals.dark_mode);
        style.spacing.item_spacing = egui::vec2(8., 8.);
        style.spacing.button_padding = egui::vec2(12., 8.);
        style.spacing.interact_size = egui::vec2(36., 36.);
        style.spacing.combo_width = 180.;
        style.visuals.override_text_color = Some(p.text);
        style.visuals.weak_text_color = Some(p.muted);
        style.visuals.panel_fill = p.bg;
        style.visuals.window_fill = p.surface;
        style.visuals.extreme_bg_color = p.surface;
        style.visuals.text_edit_bg_color = Some(p.surface);
        style.visuals.faint_bg_color = p.soft;
        style.visuals.window_stroke = Stroke::new(1.0_f32, p.line);
        style.visuals.selection.bg_fill = p.hover;
        style.visuals.selection.stroke = Stroke::new(1.0_f32, p.text);
        style.visuals.hyperlink_color = p.muted;
        style.visuals.error_fg_color = p.danger;
        style.visuals.window_corner_radius = 10.into();
        for (key, size) in [
            (egui::TextStyle::Body, 14.),
            (egui::TextStyle::Button, 13.),
            (egui::TextStyle::Small, 12.),
            (egui::TextStyle::Heading, 16.),
        ] {
            style.text_styles.insert(key, FontId::proportional(size));
        }
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.bg_fill = p.surface;
            widget.weak_bg_fill = p.surface;
            widget.bg_stroke = Stroke::new(1.0_f32, p.input_line);
            widget.fg_stroke = Stroke::new(1.5_f32, p.text);
            widget.corner_radius = 6.into();
        }
        style.visuals.widgets.hovered.bg_fill = p.hover;
        style.visuals.widgets.hovered.weak_bg_fill = p.hover;
        style.visuals.widgets.active.bg_fill = p.hover;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    });
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), fallback);
    if let Some(windir) = std::env::var_os("WINDIR") {
        add_windows_fonts(&mut fonts, &std::path::Path::new(&windir).join("Fonts"));
    }
    ctx.set_fonts(fonts);
}

fn add_windows_fonts(fonts: &mut egui::FontDefinitions, directory: &std::path::Path) {
    for (name, file, family) in [
        ("Segoe", "segoeui.ttf", FontFamily::Proportional),
        (
            "Segoe Semibold",
            "seguisb.ttf",
            FontFamily::Name("semibold".into()),
        ),
    ] {
        if let Ok(data) = std::fs::read(directory.join(file)) {
            fonts
                .font_data
                .insert(name.into(), Arc::new(egui::FontData::from_owned(data)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    // Nirmala ships as a font collection on recent Windows versions. Keep
    // Windows fonts optional and retain the bundled fallbacks ahead of them.
    for (name, candidates) in [
        ("Nirmala UI", &["Nirmala.ttf", "Nirmala.ttc"][..]),
        ("Segoe UI Emoji", &["seguiemj.ttf"][..]),
    ] {
        let Some(data) = candidates
            .iter()
            .find_map(|file| std::fs::read(directory.join(file)).ok())
        else {
            continue;
        };
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_owned(data)));
        for family in [
            FontFamily::Proportional,
            FontFamily::Monospace,
            FontFamily::Name("semibold".into()),
        ] {
            fonts.families.entry(family).or_default().push(name.into());
        }
    }
}

/// Borderless windows still expose the same edge resizing as standard Windows chrome.
pub fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let Some(pos) = ctx.pointer_hover_pos() else {
        return;
    };
    let rect = ctx.viewport_rect();
    let left = pos.x <= rect.left() + 5.0;
    let right = pos.x >= rect.right() - 5.0;
    let top = pos.y <= rect.top() + 5.0;
    let bottom = pos.y >= rect.bottom() - 5.0;
    use egui::ResizeDirection as D;
    let direction = match (left, right, top, bottom) {
        (true, _, true, _) => D::NorthWest,
        (_, true, true, _) => D::NorthEast,
        (true, _, _, true) => D::SouthWest,
        (_, true, _, true) => D::SouthEast,
        (true, _, _, _) => D::West,
        (_, true, _, _) => D::East,
        (_, _, true, _) => D::North,
        (_, _, _, true) => D::South,
        _ => return,
    };
    ctx.set_cursor_icon(match direction {
        D::North | D::South => egui::CursorIcon::ResizeVertical,
        D::East | D::West => egui::CursorIcon::ResizeHorizontal,
        D::NorthEast | D::SouthWest => egui::CursorIcon::ResizeNeSw,
        D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
    });
    if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_windows_fonts_preserve_bundled_fallbacks() {
        let directory = tempfile::tempdir().unwrap();
        let mut fonts = egui::FontDefinitions::default();
        let original = fonts.clone();
        add_windows_fonts(&mut fonts, directory.path());
        assert_eq!(fonts, original);
    }

    #[test]
    #[cfg(windows)]
    fn installed_windows_fallbacks_cover_nepali_and_crab_in_all_text_families() {
        let Some(windir) = std::env::var_os("WINDIR") else {
            return;
        };
        let directory = std::path::Path::new(&windir).join("Fonts");
        if !directory.join("seguiemj.ttf").exists()
            || !(directory.join("Nirmala.ttf").exists() || directory.join("Nirmala.ttc").exists())
        {
            return;
        }
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts_mut(|fonts| {
                for family in [
                    FontFamily::Proportional,
                    FontFamily::Monospace,
                    FontFamily::Name("semibold".into()),
                ] {
                    let font = FontId::new(16.0, family);
                    assert!(
                        fonts.has_glyphs(&font, "नमस्ते🦀"),
                        "Missing Unicode glyphs for {font:?}"
                    );
                }
            });
        });
    }
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 200.0),
            )),
            events,
            ..Default::default()
        }
    }
    #[test]
    fn custom_toggle_supports_pointer_and_keyboard_activation() {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let mut value = false;
        let mut rect = Rect::NOTHING;
        let mut id = egui::Id::NULL;
        let mut frame = |events| {
            let _ = ctx.run(input(events), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = toggle(ui, &mut value, "Enable Play / pause");
                    rect = response.rect;
                    id = response.id;
                });
            });
        };
        frame(Vec::new());
        drop(frame);
        let pos = rect.center();
        for pressed in [true, false] {
            let _ = ctx.run(
                input(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]),
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = toggle(ui, &mut value, "Enable Play / pause");
                        id = response.id;
                    });
                },
            );
        }
        assert!(value, "Pointer activation must turn the mapping on");
        ctx.memory_mut(|m| m.request_focus(id));
        let _ = ctx.run(
            input(vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]),
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    toggle(ui, &mut value, "Enable Play / pause");
                });
            },
        );
        assert!(
            !value,
            "Space on the focused toggle must turn the mapping off"
        );
    }
}
