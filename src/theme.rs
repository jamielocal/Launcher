//! One palette and the egui style derived from it.
//!
//! The scheme runs from near-black to dark purple, with every surface a step
//! along that same ramp rather than a neutral grey, so the whole window reads as
//! one material lit from a single direction.

use egui::{Color32, CornerRadius, Stroke};

// The ramp: black at the bottom, violet at the top.
pub const VOID: Color32 = Color32::from_rgb(5, 4, 9);
pub const BG: Color32 = Color32::from_rgb(10, 8, 16);
pub const SIDEBAR: Color32 = Color32::from_rgb(14, 11, 23);
pub const SURFACE: Color32 = Color32::from_rgb(22, 17, 36);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(33, 25, 54);
pub const BORDER: Color32 = Color32::from_rgb(44, 33, 72);
pub const BORDER_BRIGHT: Color32 = Color32::from_rgb(63, 46, 102);

pub const TEXT: Color32 = Color32::from_rgb(242, 238, 250);
pub const TEXT_DIM: Color32 = Color32::from_rgb(169, 156, 198);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(112, 100, 140);

pub const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(167, 139, 250);
pub const ACCENT_DEEP: Color32 = Color32::from_rgb(91, 47, 184);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(42, 26, 77);

pub const GOLD: Color32 = Color32::from_rgb(230, 182, 76);
/// Signals a game being up. Kept mint rather than green so it sits with purple.
pub const MINT: Color32 = Color32::from_rgb(61, 220, 151);

/// Source accents, used for the badge on each card. Chosen to stay legible on a
/// dark violet tile and to be told apart from the mint "running" dot.
pub const STEAM: Color32 = Color32::from_rgb(102, 160, 225);
pub const EPIC: Color32 = Color32::from_rgb(198, 188, 216);
pub const XBOX: Color32 = Color32::from_rgb(124, 196, 94);
pub const ROBLOX: Color32 = Color32::from_rgb(232, 86, 78);
pub const PRISM: Color32 = Color32::from_rgb(91, 200, 245);

pub const R_SM: CornerRadius = CornerRadius::same(7);
pub const R_MD: CornerRadius = CornerRadius::same(12);
pub const R_LG: CornerRadius = CornerRadius::same(16);

/// Card geometry. The art area is a 3:4 portrait, like most store capsules.
pub const CARD_W: f32 = 188.0;
pub const CARD_ART_H: f32 = 236.0;
pub const CARD_LABEL_H: f32 = 58.0;
pub const CARD_GAP: f32 = 18.0;

pub fn apply(ctx: &egui::Context) {
    use egui::{FontFamily::Proportional, FontId, TextStyle};

    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(21.0, Proportional)),
            (TextStyle::Body, FontId::new(13.5, Proportional)),
            (TextStyle::Button, FontId::new(13.5, Proportional)),
            (TextStyle::Small, FontId::new(11.5, Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(12.0, egui::FontFamily::Monospace),
            ),
        ]
        .into();

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = BG;
        v.window_fill = SURFACE;
        v.extreme_bg_color = VOID;
        v.faint_bg_color = SURFACE;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_corner_radius = R_MD;
        v.menu_corner_radius = R_MD;
        v.selection.bg_fill = ACCENT_SOFT;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT_HOVER;
        v.override_text_color = Some(TEXT);
        v.window_shadow = egui::epaint::Shadow {
            offset: [0, 10],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(170),
        };
        v.popup_shadow = v.window_shadow;

        // Widgets: flat by default, lifting only on hover.
        v.widgets.noninteractive.bg_fill = SURFACE;
        v.widgets.noninteractive.weak_bg_fill = SURFACE;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
        v.widgets.noninteractive.corner_radius = R_SM;

        v.widgets.inactive.bg_fill = SURFACE;
        v.widgets.inactive.weak_bg_fill = SURFACE;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.inactive.corner_radius = R_SM;

        v.widgets.hovered.bg_fill = SURFACE_HOVER;
        v.widgets.hovered.weak_bg_fill = SURFACE_HOVER;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_BRIGHT);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.hovered.corner_radius = R_SM;
        v.widgets.hovered.expansion = 0.0;

        v.widgets.active.bg_fill = ACCENT_SOFT;
        v.widgets.active.weak_bg_fill = ACCENT_SOFT;
        v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.active.corner_radius = R_SM;
        v.widgets.active.expansion = 0.0;

        v.widgets.open.bg_fill = SURFACE_HOVER;
        v.widgets.open.weak_bg_fill = SURFACE_HOVER;
        v.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_BRIGHT);
        v.widgets.open.corner_radius = R_SM;

        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 8.0);
        s.button_padding = egui::vec2(12.0, 7.0);
        s.menu_margin = egui::Margin::same(6);
        s.scroll.bar_width = 9.0;
        s.scroll.bar_inner_margin = 3.0;
        s.scroll.floating = true;
    });
}

/// "12h 34m" / "48m" / "—", for playtime readouts.
pub fn format_playtime(secs: u64) -> String {
    if secs == 0 {
        return "—".into();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    match (hours, minutes) {
        (0, 0) => "<1m".into(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

pub fn format_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{bytes} B")
    }
}

/// Coarse relative time: precision past "a few days ago" is not useful here.
pub fn format_relative(unix: Option<i64>) -> String {
    let Some(then) = unix else {
        return "Never played".into();
    };
    let delta = crate::model::now_unix() - then;
    match delta {
        d if d < 120 => "Just now".into(),
        d if d < 3600 => format!("{} minutes ago", d / 60),
        d if d < 7200 => "An hour ago".into(),
        d if d < 86_400 => format!("{} hours ago", d / 3600),
        d if d < 172_800 => "Yesterday".into(),
        d if d < 2_592_000 => format!("{} days ago", d / 86_400),
        d if d < 5_184_000 => "A month ago".into(),
        d if d < 31_536_000 => format!("{} months ago", d / 2_592_000),
        d => format!("{} years ago", d / 31_536_000),
    }
}

/// The window and taskbar icon, drawn in code so the build needs no assets.
pub fn window_icon() -> egui::IconData {
    const SIZE: usize = 64;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = (SIZE as f32 - 1.0) / 2.0;
    let radius = center - 1.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 - center, y as f32 - center);
            let dist = (dx * dx + dy * dy).sqrt();
            // One pixel of feathering keeps the circle from looking jagged.
            let coverage = ((radius - dist) + 0.5).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }

            let t = y as f32 / (SIZE - 1) as f32;
            let mut r = (70.0 + 50.0 * (1.0 - t)) as u8;
            let mut g = (95.0 + 50.0 * (1.0 - t)) as u8;
            let mut b = (200.0 + 55.0 * (1.0 - t)) as u8;

            // A play triangle punched out in white.
            if in_triangle(x as f32, y as f32, SIZE as f32) {
                (r, g, b) = (255, 255, 255);
            }

            let i = (y * SIZE + x) * 4;
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = (coverage * 255.0) as u8;
        }
    }

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

/// Point-in-triangle test for the icon's play glyph.
fn in_triangle(x: f32, y: f32, size: f32) -> bool {
    let left = size * 0.38;
    let right = size * 0.70;
    let top = size * 0.28;
    let bottom = size * 0.72;
    if x < left || x > right {
        return false;
    }
    // Linearly narrow the vertical span as x moves right.
    let t = (x - left) / (right - left);
    let half = (bottom - top) / 2.0 * (1.0 - t);
    let mid = (top + bottom) / 2.0;
    y >= mid - half && y <= mid + half
}
