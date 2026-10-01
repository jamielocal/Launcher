//! Small painting and widget helpers shared by the sidebar, grid and details
//! panel. Everything here is hand-painted so the app can keep a consistent
//! look that egui's stock widgets do not give us.

use egui::epaint::Shadow;
use egui::load::SizedTexture;
use egui::{
    pos2, vec2, Color32, CornerRadius, Painter, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, TextureHandle, Ui,
};

use crate::theme as th;

/// Soft drop shadow under a rounded rect.
pub fn shadow(painter: &Painter, rect: Rect, cr: CornerRadius, strength: f32) {
    if strength <= 0.0 {
        return;
    }
    let shadow = Shadow {
        offset: [0, (6.0 * strength) as i8],
        blur: (22.0 * strength) as u8,
        spread: 0,
        color: Color32::from_black_alpha((110.0 * strength) as u8),
    };
    painter.add(shadow.as_shape(rect, cr));
}

/// Draws a texture so that it covers `rect` entirely, cropping the overflow
/// instead of distorting the image.
pub fn paint_cover(ui: &Ui, rect: Rect, texture: &TextureHandle, cr: CornerRadius) {
    let size = texture.size_vec2();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let src_ratio = size.x / size.y;
    let dst_ratio = rect.aspect_ratio();
    // Shrink the UV window on the axis that would otherwise overflow.
    let uv = if src_ratio > dst_ratio {
        let frac = dst_ratio / src_ratio;
        Rect::from_center_size(pos2(0.5, 0.5), vec2(frac, 1.0))
    } else {
        let frac = src_ratio / dst_ratio;
        Rect::from_center_size(pos2(0.5, 0.5), vec2(1.0, frac))
    };
    egui::Image::new(SizedTexture::from_handle(texture))
        .uv(uv)
        .corner_radius(cr)
        .paint_at(ui, rect);
}

/// Draws a texture centred inside `rect`, scaled to fit without cropping.
pub fn paint_contain(ui: &Ui, rect: Rect, texture: &TextureHandle, max_size: f32) {
    let size = texture.size_vec2();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let scale = (max_size / size.x.max(size.y)).min(f32::MAX);
    let drawn = size * scale;
    let target = Rect::from_center_size(rect.center(), drawn);
    egui::Image::new(SizedTexture::from_handle(texture)).paint_at(ui, target);
}

/// A two-stop vertical gradient, painted as a single quad with coloured
/// corners. Everything else that fades in this app is expressed through it.
pub fn paint_vertical_gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    let idx = mesh.vertices.len() as u32;
    for (pos, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color,
        });
    }
    mesh.indices
        .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// A transparent-to-`base` scrim, so light cover art still carries white text
/// at the bottom of a tile.
pub fn paint_scrim(painter: &Painter, rect: Rect, base: Color32) {
    paint_vertical_gradient(
        painter,
        rect,
        Color32::TRANSPARENT,
        base.gamma_multiply(0.92),
    );
}

/// Left-to-right counterpart of [`paint_vertical_gradient`].
pub fn paint_horizontal_gradient(painter: &Painter, rect: Rect, left: Color32, right: Color32) {
    let mut mesh = egui::Mesh::default();
    let idx = mesh.vertices.len() as u32;
    for (pos, color) in [
        (rect.left_top(), left),
        (rect.right_top(), right),
        (rect.right_bottom(), right),
        (rect.left_bottom(), left),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color,
        });
    }
    mesh.indices
        .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// A wash of colour fading downwards, used to light the top of the library.
pub fn paint_glow(painter: &Painter, rect: Rect, color: Color32, strength: f32) {
    paint_vertical_gradient(
        painter,
        rect,
        color.gamma_multiply(strength),
        Color32::TRANSPARENT,
    );
}

/// Rounds only the top two corners, for art that meets a flat label strip.
pub fn top_corners(cr: CornerRadius) -> CornerRadius {
    CornerRadius {
        nw: cr.nw,
        ne: cr.ne,
        sw: 0,
        se: 0,
    }
}

/// A small uppercase pill, used to mark which store a game came from.
/// `top_left` is where the pill starts; its size follows the text.
pub fn badge(painter: &Painter, top_left: egui::Pos2, label: &str, color: Color32) {
    let font = egui::FontId::proportional(9.5);
    let galley = painter.layout_no_wrap(label.to_string(), font.clone(), color);
    let rect = Rect::from_min_size(top_left, vec2(galley.size().x + 14.0, 18.0));
    painter.rect_filled(rect, CornerRadius::same(9), Color32::from_black_alpha(170));
    painter.rect_stroke(
        rect,
        CornerRadius::same(9),
        Stroke::new(1.0, color.gamma_multiply(0.45)),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
}

/// A compact toggle chip, used for filters and boolean settings.
pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let text_color = if selected { th::TEXT } else { th::TEXT_DIM };
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(12.5),
        th::TEXT,
    );
    let size = vec2(galley.size().x + 22.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered() || selected, 0.12);
    let fill = if selected {
        th::ACCENT_SOFT
    } else {
        Color32::from_rgba_unmultiplied(
            th::SURFACE_HOVER.r(),
            th::SURFACE_HOVER.g(),
            th::SURFACE_HOVER.b(),
            (170.0 * t) as u8,
        )
    };
    ui.painter().rect_filled(rect, th::R_SM, fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            th::R_SM,
            Stroke::new(1.0, th::ACCENT),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.5),
        text_color,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A chip with a right-aligned count, for the source filters.
pub fn chip_with_count(ui: &mut Ui, label: &str, count: usize, selected: bool) -> Response {
    let font = egui::FontId::proportional(12.5);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered() || selected, 0.12);

    let fill = if selected {
        th::ACCENT_SOFT
    } else {
        th::SURFACE_HOVER.gamma_multiply(0.75 * t)
    };
    ui.painter().rect_filled(rect, th::R_SM, fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            th::R_SM,
            Stroke::new(1.0, th::ACCENT),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        pos2(rect.left() + 11.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font.clone(),
        if selected { th::TEXT } else { th::TEXT_DIM },
    );
    ui.painter().text(
        pos2(rect.right() - 11.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        egui::FontId::proportional(11.0),
        th::TEXT_FAINT,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The big accent call-to-action in the details panel.
pub fn primary_button(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    let height = 42.0;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered() && enabled, 0.1);

    let fill = if !enabled {
        th::SURFACE_HOVER
    } else if response.is_pointer_button_down_on() {
        th::ACCENT
    } else {
        lerp_color(th::ACCENT, th::ACCENT_HOVER, t)
    };
    shadow(
        ui.painter(),
        rect,
        th::R_MD,
        if enabled { 0.35 + 0.3 * t } else { 0.0 },
    );
    ui.painter().rect_filled(rect, th::R_MD, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.5),
        if enabled {
            Color32::WHITE
        } else {
            th::TEXT_FAINT
        },
    );

    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A flat secondary button that only shows a background on hover.
pub fn ghost_button(ui: &mut Ui, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(13.0),
        th::TEXT,
    );
    let size = vec2(
        ui.available_width().min(galley.size().x + 26.0).max(60.0),
        32.0,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.1);

    ui.painter().rect_filled(
        rect,
        th::R_SM,
        Color32::from_rgba_unmultiplied(
            th::SURFACE_HOVER.r(),
            th::SURFACE_HOVER.g(),
            th::SURFACE_HOVER.b(),
            (210.0 * t) as u8,
        ),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(13.0),
        lerp_color(th::TEXT_DIM, th::TEXT, t),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A label/value line, used throughout the details panel.
pub fn info_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(th::TEXT_FAINT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.5).color(th::TEXT_DIM));
        });
    });
}

/// Section heading in the sidebar and details panel.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(10.5)
            .color(th::TEXT_FAINT)
            .strong(),
    );
    ui.add_space(4.0);
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

/// Filled triangle used for the play affordance; drawn rather than relying on
/// a glyph so it stays crisp at any size.
pub fn paint_play_triangle(painter: &Painter, center: egui::Pos2, size: f32, color: Color32) {
    let half = size / 2.0;
    // Nudged right so the triangle looks optically centred in its circle.
    let cx = center.x + size * 0.08;
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(cx - half * 0.7, center.y - half),
            pos2(cx - half * 0.7, center.y + half),
            pos2(cx + half * 0.9, center.y),
        ],
        color,
        Stroke::NONE,
    ));
}

/// Five-pointed star for the favourite toggle.
pub fn paint_star(painter: &Painter, center: egui::Pos2, radius: f32, color: Color32) {
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let r = if i % 2 == 0 { radius } else { radius * 0.44 };
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        points.push(pos2(center.x + r * angle.cos(), center.y + r * angle.sin()));
    }
    // A star is concave, so fan it from the centre instead of using a hull.
    let mut mesh = egui::Mesh::default();
    let c = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: egui::epaint::WHITE_UV,
        color,
    });
    for p in &points {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv: egui::epaint::WHITE_UV,
            color,
        });
    }
    for i in 0..10u32 {
        mesh.indices
            .extend_from_slice(&[c, c + 1 + i, c + 1 + (i + 1) % 10]);
    }
    painter.add(egui::Shape::mesh(mesh));
}
