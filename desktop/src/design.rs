use crate::model::AccentColor;
use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Painter, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use std::sync::{Arc, OnceLock};

const ACCENT_ID: &str = "app_accent";

pub fn set_accent(ctx: &egui::Context, accent: AccentColor) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(ACCENT_ID), accent));
}

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
        let accent = ctx
            .data(|data| data.get_temp(egui::Id::new(ACCENT_ID)))
            .unwrap_or_default();
        Self::for_accent(ctx.style().visuals.dark_mode, accent)
    }
    pub fn for_accent(dark: bool, accent: AccentColor) -> Self {
        let mut palette = Self::for_dark(dark);
        let (light_theme, dark_theme) = match accent {
            AccentColor::Neutral => return palette,
            AccentColor::Blue => {
                palette.primary = Color32::from_rgb(39, 103, 228);
                palette.primary_text = Color32::WHITE;
                return palette;
            }
            AccentColor::Green => ([23, 112, 50], [96, 207, 115]),
            AccentColor::Amber => ([135, 78, 8], [247, 186, 100]),
            AccentColor::Red => ([190, 35, 44], [255, 121, 121]),
        };
        let [r, g, b] = if dark { dark_theme } else { light_theme };
        palette.primary = Color32::from_rgb(r, g, b);
        palette
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

pub fn singleline(value: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(value)
        .margin(egui::Margin::symmetric(10, 8))
        .min_size(egui::vec2(0.0, 36.0))
        .vertical_align(egui::Align::Center)
}

pub fn multiline(value: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(value).margin(egui::Margin::symmetric(10, 8))
}

pub fn slider(ui: &mut Ui, slider: egui::Slider<'_>) -> Response {
    let p = Palette::get(ui.ctx());
    ui.scope(|ui| {
        ui.spacing_mut().slider_rail_height = 6.0;
        let visuals = ui.visuals_mut();
        visuals.selection.bg_fill = p.primary;
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
        ] {
            widget.bg_fill = p.primary;
            widget.fg_stroke = Stroke::new(2.0_f32, p.primary);
        }
        visuals.widgets.active.fg_stroke = Stroke::new(2.0_f32, p.primary_text);

        // egui uses the inactive thumb fill for the rail too. Recolor the first
        // slider shape (the full rail) and retain egui's interaction and geometry.
        let layer = ui.layer_id();
        let rail = ui
            .ctx()
            .graphics_mut(|layers| layers.entry(layer).next_idx());
        let response = ui.add(slider.trailing_fill(true));
        let opacity = ui.painter().opacity();
        let accent = p.primary.gamma_multiply(opacity);
        let rail_fill = p.input_line.gamma_multiply(opacity);
        ui.ctx().graphics_mut(|layers| {
            layers.entry(layer).mutate_shape(rail, |shape| {
                if let egui::Shape::Rect(rect) = &mut shape.shape {
                    if rect.fill == accent && rect.stroke.is_empty() {
                        rect.fill = rail_fill;
                    }
                }
            });
        });
        response
    })
    .inner
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
                line((3., y), (x - 3., y));
                line((x + 3., y), (21., y));
                painter.circle_stroke(p(x, y), rect.width() * 3.0 / 24.0, stroke);
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
            let mut outline = Vec::with_capacity(32);
            for tooth in 0..8 {
                let angle = tooth as f32 * std::f32::consts::FRAC_PI_4;
                for (offset, radius) in [(-0.32, 7.5), (-0.17, 10.0), (0.17, 10.0), (0.32, 7.5)] {
                    outline.push(p(
                        12.0 + radius * (angle + offset).cos(),
                        12.0 + radius * (angle + offset).sin(),
                    ));
                }
            }
            painter.add(egui::Shape::closed_line(outline, stroke));
            painter.circle_stroke(rect.center(), rect.width() * 3.0 / 24.0, stroke);
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
    let enabled = ui.is_enabled();
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font(13., true), palette.text);
    let width = galley.size().x + 28. + if icon.is_some() { 22. } else { 0. };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 36.), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let (fill, color, border) = if !enabled {
        (palette.soft, palette.muted, palette.input_line)
    } else if primary {
        (palette.primary, palette.primary_text, palette.primary)
    } else {
        let fill = if response.hovered() {
            palette.hover
        } else {
            palette.surface
        };
        (fill, palette.text, palette.input_line)
    };
    let mut painter = ui.painter().clone();
    if !enabled {
        // The disabled colors already distinguish the button. Keep them readable
        // by undoing egui's disabled fade while preserving other opacity.
        painter.set_opacity(painter.opacity() / ui.visuals().disabled_alpha());
    }
    painter.rect(
        rect,
        7.,
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    let x = rect.min.x + 14.;
    if let Some(icon) = icon {
        draw_icon(
            &painter,
            Rect::from_center_size(egui::pos2(x + 7.5, rect.center().y), Vec2::splat(15.)),
            icon,
            color,
        );
    }
    painter.text(
        egui::pos2(x + if icon.is_some() { 22. } else { 0. }, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font(13., true),
        color,
    );
    if response.has_focus() {
        painter.rect_stroke(
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
#[derive(Clone, PartialEq)]
struct DashKey {
    geometry: [u32; 4],
    color: Color32,
    options: egui::epaint::TessellationOptions,
}

#[derive(Clone, Default)]
struct DashCache(std::collections::VecDeque<(DashKey, Arc<egui::epaint::Mesh>)>);

fn dash_mesh(key: &DashKey) -> egui::epaint::Mesh {
    let [left, right, y, pixels_per_point] = key.geometry.map(f32::from_bits);
    let mut tessellator =
        egui::epaint::Tessellator::new(pixels_per_point, key.options, [1, 1], Vec::new());
    let mut mesh = egui::epaint::Mesh::default();
    let mut x = left;
    while x < right {
        tessellator.tessellate_line_segment(
            [egui::pos2(x, y), egui::pos2((x + 3.0).min(right), y)],
            Stroke::new(1.0_f32, key.color),
            &mut mesh,
        );
        x += 6.0;
    }
    mesh
}

/// Reuse egui's exact dash tessellation instead of rebuilding hundreds of
/// individual shapes on every repaint. Only decorative geometry is cached.
pub fn dashed_line(painter: &Painter, left: f32, right: f32, y: f32, color: Color32) {
    if !left.is_finite() || !right.is_finite() || !y.is_finite() || right <= left {
        return;
    }
    let key = DashKey {
        geometry: [left, right, y, painter.ctx().pixels_per_point()].map(f32::to_bits),
        color,
        options: painter.ctx().tessellation_options(|options| *options),
    };
    let mesh = painter.ctx().data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<DashCache>(egui::Id::new("dash_meshes"));
        if let Some((_, mesh)) = cache.0.iter().find(|(cached, _)| cached == &key) {
            return Arc::clone(mesh);
        }
        let mesh = Arc::new(dash_mesh(&key));
        if cache.0.len() == 32 {
            cache.0.pop_front();
        }
        cache.0.push_back((key, Arc::clone(&mesh)));
        mesh
    });
    painter.add(egui::Shape::Mesh(mesh));
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
    let mut fonts = emergency_fonts();
    add_windows_fonts(&mut fonts, system_fonts());
    ctx.set_fonts(fonts);
}

fn emergency_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Emergency Mono".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/EmergencyMono.ttf"
        ))),
    );
    for family in [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name("semibold".into()),
    ] {
        fonts.families.insert(family, vec!["Emergency Mono".into()]);
    }
    fonts
}

enum FontBytes {
    #[cfg(windows)]
    Mapped {
        mapping: memmap2::Mmap,
        // Keep the read-only sharing lock for the complete mapping lifetime.
        _file: std::fs::File,
    },
    Owned(Box<[u8]>),
}

impl FontBytes {
    fn as_slice(&self) -> &[u8] {
        match self {
            #[cfg(windows)]
            Self::Mapped { mapping, .. } => mapping,
            Self::Owned(bytes) => bytes,
        }
    }
}

struct SystemFont {
    name: &'static str,
    primary_family: Option<FontFamily>,
    bytes: FontBytes,
}

fn load_font_bytes(path: &std::path::Path) -> Option<FontBytes> {
    #[cfg(windows)]
    {
        use std::{io::Read, os::windows::fs::OpenOptionsExt};
        // FILE_SHARE_READ excludes concurrent writes/deletion. Font mappings are
        // only made from the OS font directory and never expose a writable view.
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)
            .ok()?;
        // SAFETY: the retained handle denies writers/truncation and deletion;
        // these installed font files remain immutable for the mapping lifetime.
        if let Ok(mapping) = unsafe { memmap2::MmapOptions::new().no_probe_handle().map(&file) } {
            return Some(FontBytes::Mapped {
                mapping,
                _file: file,
            });
        }
        // Restricted systems may refuse mapping. A single cached owned copy
        // still avoids the extra copy epaint makes from owned FontData.
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        Some(FontBytes::Owned(bytes.into_boxed_slice()))
    }
    #[cfg(not(windows))]
    {
        std::fs::read(path)
            .ok()
            .map(|bytes| FontBytes::Owned(bytes.into_boxed_slice()))
    }
}

fn read_system_fonts(directory: &std::path::Path) -> Vec<SystemFont> {
    let mut fonts = Vec::new();
    for (name, candidates, primary_family) in [
        (
            "Segoe",
            &["segoeui.ttf"][..],
            Some(FontFamily::Proportional),
        ),
        (
            "Segoe Semibold",
            &["seguisb.ttf"][..],
            Some(FontFamily::Name("semibold".into())),
        ),
        (
            "Consolas",
            &["consola.ttf"][..],
            Some(FontFamily::Monospace),
        ),
        ("Nirmala UI", &["Nirmala.ttf", "Nirmala.ttc"][..], None),
        ("Segoe UI Emoji", &["seguiemj.ttf"][..], None),
    ] {
        if let Some(bytes) = candidates
            .iter()
            .find_map(|file| load_font_bytes(&directory.join(file)))
        {
            fonts.push(SystemFont {
                name,
                primary_family,
                bytes,
            });
        }
    }
    fonts
}

fn system_fonts() -> &'static [SystemFont] {
    static FONTS: OnceLock<Vec<SystemFont>> = OnceLock::new();
    FONTS.get_or_init(|| {
        std::env::var_os("WINDIR")
            .map(|windir| read_system_fonts(&std::path::Path::new(&windir).join("Fonts")))
            .unwrap_or_default()
    })
}

fn add_windows_fonts(fonts: &mut egui::FontDefinitions, sources: &'static [SystemFont]) {
    for source in sources {
        fonts.font_data.insert(
            source.name.into(),
            Arc::new(egui::FontData::from_static(source.bytes.as_slice())),
        );
        if let Some(family) = &source.primary_family {
            fonts
                .families
                .entry(family.clone())
                .or_default()
                .insert(0, source.name.into());
        } else {
            for family in [
                FontFamily::Proportional,
                FontFamily::Monospace,
                FontFamily::Name("semibold".into()),
            ] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push(source.name.into());
            }
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
    fn cached_dashes_match_original_tessellation_and_stay_bounded() {
        for pixels_per_point in [1.0, 1.25, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(pixels_per_point);
            let left = 17.25;
            let right = 321.5;
            let y = 24.75;
            let color = Color32::from_gray(122);
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                dashed_line(&painter, left, right, y, color);
            });
            let key = DashKey {
                geometry: [left, right, y, output.pixels_per_point].map(f32::to_bits),
                color,
                options: ctx.tessellation_options(|o| *o),
            };
            let cached = dash_mesh(&key);
            let mut shapes = Vec::new();
            let mut x = left;
            while x < right {
                shapes.push(egui::epaint::ClippedShape {
                    clip_rect: Rect::EVERYTHING,
                    shape: egui::Shape::line_segment(
                        [egui::pos2(x, y), egui::pos2((x + 3.0).min(right), y)],
                        Stroke::new(1.0_f32, color),
                    ),
                });
                x += 6.0;
            }
            let original = ctx.tessellate(shapes, output.pixels_per_point);
            let egui::epaint::Primitive::Mesh(original) = &original[0].primitive else {
                panic!("Expected a mesh");
            };
            assert_eq!(
                cached, *original,
                "Dash geometry changed at {pixels_per_point}x DPI"
            );
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                for offset in 0..100 {
                    dashed_line(&painter, left, right, y + offset as f32, color);
                }
            });
            ctx.data_mut(|data| {
                assert_eq!(
                    data.get_temp_mut_or_default::<DashCache>(egui::Id::new("dash_meshes"))
                        .0
                        .len(),
                    32
                );
            });
        }
    }

    #[test]
    fn missing_windows_fonts_preserve_bundled_fallbacks() {
        let directory = tempfile::tempdir().unwrap();
        assert!(read_system_fonts(directory.path()).is_empty());
        let mut fonts = emergency_fonts();
        let original = fonts.clone();
        add_windows_fonts(&mut fonts, &[]);
        assert_eq!(fonts, original);
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts_mut(|fonts| {
                for family in [
                    FontFamily::Proportional,
                    FontFamily::Monospace,
                    FontFamily::Name("semibold".into()),
                ] {
                    assert!(
                        fonts.has_glyphs(&FontId::new(14.0, family), "EarControl Save & start 123")
                    );
                }
            });
        });
    }

    #[test]
    #[cfg(windows)]
    fn mapped_font_bytes_stay_read_only_until_the_font_is_released() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("font.bin");
        std::fs::write(&path, b"stable font bytes").unwrap();
        let bytes = load_font_bytes(&path).unwrap();
        assert!(matches!(bytes, FontBytes::Mapped { .. }));
        assert_eq!(bytes.as_slice(), b"stable font bytes");
        assert!(std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(bytes);
        std::fs::write(&path, b"released").unwrap();
    }

    #[test]
    fn system_fonts_are_cached_and_borrowed_without_copying_font_files() {
        let sources = system_fonts();
        let mut first = egui::FontDefinitions::empty();
        let mut second = egui::FontDefinitions::empty();
        add_windows_fonts(&mut first, sources);
        add_windows_fonts(&mut second, system_fonts());
        for source in sources {
            let a = &first.font_data[source.name].font;
            let b = &second.font_data[source.name].font;
            assert!(matches!(a, std::borrow::Cow::Borrowed(_)));
            assert_eq!(a.as_ptr(), source.bytes.as_slice().as_ptr());
            assert_eq!(a.as_ptr(), b.as_ptr());
        }
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
    fn disabled_primary_button_keeps_readable_text_in_both_themes() {
        for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.set_theme(theme);
            set_accent(&ctx, AccentColor::Amber);
            let output = ctx.run(input(Vec::new()), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = ui
                        .add_enabled_ui(false, |ui| {
                            button(ui, "Save & start", Some(Icon::Play), true)
                        })
                        .inner;
                    assert!(!response.enabled());
                });
            });
            let palette = Palette::get(&ctx);
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.text() == "Save & start"
                        && text.fallback_color == palette.muted
                        && text.opacity_factor == 1.0)
            }));
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
