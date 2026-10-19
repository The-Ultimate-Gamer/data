//! Look and feel: the website's dark theme with its gold accent, plus small shared widgets.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Response, RichText, Stroke, TextStyle, Ui, Vec2, Visuals,
};

pub const BG: Color32 = Color32::from_rgb(0x09, 0x09, 0x0b);
pub const BG_RAISED: Color32 = Color32::from_rgb(0x0f, 0x0f, 0x12);
pub const SURFACE: Color32 = Color32::from_rgb(0x13, 0x13, 0x17);
pub const SURFACE_2: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1f);
pub const SURFACE_3: Color32 = Color32::from_rgb(0x23, 0x23, 0x29);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x26, 0x2d);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x39, 0x39, 0x42);
pub const TEXT: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf4);
pub const MUTED: Color32 = Color32::from_rgb(0xa8, 0xa8, 0xb3);
pub const SUBTLE: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x96);
pub const GOLD: Color32 = Color32::from_rgb(0xff, 0xc6, 0x1a);
pub const ON_GOLD: Color32 = Color32::from_rgb(0x14, 0x10, 0x06);
pub const GOLD_SOFT: Color32 = Color32::from_rgba_premultiplied(31, 24, 3, 31);
pub const CHALLENGER: Color32 = Color32::from_rgb(0xff, 0x6a, 0x3d);
pub const LIVE: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x84);
pub const INFO: Color32 = Color32::from_rgb(0x5c, 0xc8, 0xff);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x54, 0x68);

/// System fonts with wide symbol coverage, used (when present) after egui's own fonts so that
/// category and award icons render. Missing files are skipped.
const FALLBACK_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
    "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "C:\\Windows\\Fonts\\seguisym.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
];

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in FALLBACK_FONTS {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let name = format!("fallback:{path}");
        fonts.font_data.insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    ctx.set_fonts(fonts);
}

static MISSING_GLYPHS: std::sync::RwLock<Vec<char>> = std::sync::RwLock::new(Vec::new());

/// Remember which characters of these icons no installed font can draw, so they're left out.
pub fn check_glyphs(ctx: &egui::Context, icons: impl Iterator<Item = String>) {
    let font = FontId::new(15.0, FontFamily::Proportional);
    let mut missing = Vec::new();
    ctx.fonts_mut(|f| {
        for icon in icons {
            for c in icon.chars() {
                if !c.is_whitespace() && !is_invisible(c) && !f.has_glyph(&font, c) && !missing.contains(&c) {
                    missing.push(c);
                }
            }
        }
    });
    if let Ok(mut m) = MISSING_GLYPHS.write() {
        *m = missing;
    }
}

fn is_invisible(c: char) -> bool {
    matches!(c, '\u{fe0e}' | '\u{fe0f}' | '\u{200d}')
}

/// An icon from the data, without variation selectors and characters no font can draw.
pub fn icon(icon: Option<&str>) -> String {
    let missing = MISSING_GLYPHS.read().map(|m| m.clone()).unwrap_or_default();
    icon.unwrap_or("").chars().filter(|c| !is_invisible(*c) && !missing.contains(c)).collect()
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
        (TextStyle::Name("title".into()), FontId::new(18.0, FontFamily::Proportional)),
        (TextStyle::Name("big".into()), FontId::new(30.0, FontFamily::Proportional)),
    ]
    .into();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.combo_width = 180.0;
    style.spacing.text_edit_width = 260.0;

    let mut v = Visuals::dark();
    v.panel_fill = BG_RAISED;
    v.window_fill = SURFACE;
    v.extreme_bg_color = BG;
    v.faint_bg_color = SURFACE;
    v.code_bg_color = SURFACE_2;
    v.hyperlink_color = GOLD;
    v.selection.bg_fill = Color32::from_rgb(0x5a, 0x47, 0x0c);
    v.selection.stroke = Stroke::new(1.0, GOLD);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.override_text_color = Some(TEXT);
    let radius = CornerRadius::same(6);
    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.corner_radius = radius;
    v.widgets.inactive.bg_fill = SURFACE_2;
    v.widgets.inactive.weak_bg_fill = SURFACE_2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = radius;
    v.widgets.hovered.bg_fill = SURFACE_3;
    v.widgets.hovered.weak_bg_fill = SURFACE_3;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GOLD);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.corner_radius = radius;
    v.widgets.active.bg_fill = SURFACE_3;
    v.widgets.active.weak_bg_fill = SURFACE_3;
    v.widgets.active.bg_stroke = Stroke::new(1.5, GOLD);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.corner_radius = radius;
    v.widgets.open.corner_radius = radius;
    style.visuals = v;
    ctx.set_style_of(egui::Theme::Dark, style);
}

/// A raised panel grouping related content.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A card with a coloured edge, for highlights (live game, warnings, outcomes).
pub fn callout<R>(ui: &mut Ui, color: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.10))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.55)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn section_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).text_style(TextStyle::Name("title".into())).strong());
}

pub fn hint(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.label(RichText::new(text.into()).color(SUBTLE).small())
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(MUTED)
}

/// The main action on a screen.
pub fn primary(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(ON_GOLD).strong()).fill(GOLD).stroke(Stroke::NONE))
}

pub fn primary_enabled(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(ON_GOLD).strong()).fill(GOLD).stroke(Stroke::NONE),
    )
}

pub fn big_primary(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(ON_GOLD).strong().size(18.0))
            .fill(GOLD)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 44.0)),
    )
}

pub fn danger(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(DANGER)).stroke(Stroke::new(1.0, DANGER.gamma_multiply(0.6))))
}

/// A small coloured tag, e.g. "Live", "Demo", "Challenger".
pub fn badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(8, 1))
        .show(ui, |ui| ui.label(RichText::new(text).color(color).small().strong()))
        .response
}

pub fn status_badge(ui: &mut Ui, status: crate::model::Status) -> Response {
    use crate::model::Status;
    let color = match status {
        Status::Upcoming => INFO,
        Status::Ongoing => LIVE,
        Status::Completed => MUTED,
    };
    badge(ui, status.label(), color)
}

/// A large number with a caption, for the dashboard.
pub fn stat(ui: &mut Ui, value: impl ToString, caption: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(value.to_string()).text_style(TextStyle::Name("big".into())).strong().color(GOLD));
        ui.label(muted(caption).small());
    });
}

/// A labelled form field laid out in a grid row.
pub fn field<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.label(muted(label));
    let r = add(ui);
    ui.end_row();
    r
}

pub fn text(ui: &mut Ui, value: &mut String, hint_text: &str) -> Response {
    ui.add(egui::TextEdit::singleline(value).hint_text(hint_text))
}

pub fn narrow_text(ui: &mut Ui, value: &mut String, hint_text: &str, width: f32) -> Response {
    ui.add(egui::TextEdit::singleline(value).hint_text(hint_text).desired_width(width))
}

/// A generic "pick one of these" combo box; `None` is shown as `none_label` when allowed.
pub fn pick<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    value: &mut Option<T>,
    options: &[(T, String)],
    none_label: Option<&str>,
    width: f32,
) -> bool {
    let selected = value
        .and_then(|v| options.iter().find(|(o, _)| *o == v))
        .map_or_else(|| none_label.unwrap_or("Choose…").to_string(), |(_, label)| label.clone());
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).width(width).selected_text(selected).show_ui(ui, |ui| {
        if let Some(none) = none_label {
            changed |= ui.selectable_value(value, None, none).changed();
        }
        for (option, label) in options {
            changed |= ui.selectable_value(value, Some(*option), label).changed();
        }
    });
    changed
}

pub fn page_title(ui: &mut Ui, title: &str, subtitle: Option<&str>) {
    ui.add_space(4.0);
    ui.heading(RichText::new(title).strong());
    if let Some(sub) = subtitle {
        ui.label(muted(sub));
    }
    ui.add_space(6.0);
}
