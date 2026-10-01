//! The game grid: a responsive wall of cards that lift on hover.

use egui::{pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui};

use crate::art::{ArtCache, ArtKind};
use crate::model::Source;
use crate::runner::Runner;
use crate::theme as th;
use crate::ui::widgets as w;
use crate::ui::{Action, Card};

/// Lays out cards left-to-right, wrapping to fill the available width evenly.
pub fn show(
    ui: &mut Ui,
    cards: &[Card],
    art: &mut ArtCache,
    runner: &Runner,
    selected: Option<&str>,
    actions: &mut Vec<Action>,
) {
    if cards.is_empty() {
        empty_state(ui);
        return;
    }

    let avail = ui.available_width();
    let columns = (((avail + th::CARD_GAP) / (th::CARD_W + th::CARD_GAP)).floor() as usize).max(1);
    // Spread any leftover width across the gaps rather than leaving a ragged
    // right margin.
    let used = columns as f32 * th::CARD_W;
    let gap = if columns > 1 {
        ((avail - used) / (columns - 1) as f32).clamp(th::CARD_GAP, th::CARD_GAP * 2.5)
    } else {
        th::CARD_GAP
    };

    for (row, chunk) in cards.chunks(columns).enumerate() {
        if row > 0 {
            ui.add_space(th::CARD_GAP + 6.0);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for card in chunk {
                draw_card(
                    ui,
                    card,
                    art,
                    runner,
                    selected == Some(card.id.as_str()),
                    actions,
                );
            }
        });
    }
    ui.add_space(th::CARD_GAP);
}

fn draw_card(
    ui: &mut Ui,
    card: &Card,
    art: &mut ArtCache,
    runner: &Runner,
    selected: bool,
    actions: &mut Vec<Action>,
) {
    let running = runner.is_running(&card.id);
    let starting = running && runner.is_starting(&card.id);

    let size = vec2(th::CARD_W, th::CARD_ART_H + th::CARD_LABEL_H);
    let (slot, response) = ui.allocate_exact_size(size, Sense::click());

    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.13);
    let select_t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("sel"), selected, 0.13);

    // Lift the whole card towards the viewer on hover.
    let rect = slot.translate(vec2(0.0, -5.0 * hover));
    let art_rect = Rect::from_min_size(rect.min, vec2(rect.width(), th::CARD_ART_H));
    let painter = ui.painter().clone();

    w::shadow(&painter, rect, th::R_LG, 0.3 + 0.6 * hover);
    painter.rect_filled(rect, th::R_LG, th::SURFACE);

    // Backdrop first: it shows through while real art loads, and sits behind
    // icons that have transparent padding.
    let backdrop = art.backdrop(ui.ctx(), &card.id);
    w::paint_cover(ui, art_rect, &backdrop, w::top_corners(th::R_LG));

    match art.art(card.art_request()) {
        Some(found) => match found.kind {
            ArtKind::Cover => {
                let texture = found.texture.clone();
                w::paint_cover(ui, art_rect, &texture, w::top_corners(th::R_LG));
            }
            ArtKind::Icon => {
                let texture = found.texture.clone();
                let icon_size = 104.0 + 6.0 * hover;
                let center = Rect::from_center_size(
                    pos2(art_rect.center().x, art_rect.center().y - 8.0),
                    vec2(icon_size, icon_size),
                );
                // A soft glow lifts the icon off the gradient.
                painter.circle_filled(
                    center.center(),
                    icon_size * 0.62,
                    Color32::from_black_alpha(42),
                );
                w::paint_contain(ui, center, &texture, icon_size);
            }
        },
        None => {
            // Nothing to show: fall back to the game's initials.
            painter.text(
                art_rect.center(),
                Align2::CENTER_CENTER,
                initials(&card.name),
                FontId::proportional(46.0),
                Color32::from_white_alpha(130),
            );
        }
    }

    // Darken the lower part of the art so the title stays legible.
    let scrim = Rect::from_min_max(
        pos2(art_rect.left(), art_rect.bottom() - 90.0),
        art_rect.right_bottom(),
    );
    w::paint_scrim(&painter, scrim, th::VOID);

    // A brightening wash on hover, so the art itself reacts to the pointer.
    if hover > 0.0 {
        painter.rect_filled(
            art_rect,
            w::top_corners(th::R_LG),
            Color32::from_white_alpha((14.0 * hover) as u8),
        );
    }

    // Store badge, top left.
    if let Some((label, color)) = badge_for(&card.source) {
        w::badge(
            &painter,
            pos2(art_rect.left() + 12.0, art_rect.top() + 12.0),
            label,
            color,
        );
    }

    // Title and metadata.
    let text_left = rect.left() + 14.0;
    let title = ui.painter().layout(
        card.name.clone(),
        FontId::proportional(14.0),
        th::TEXT,
        rect.width() - 28.0,
    );
    painter.galley(pos2(text_left, art_rect.bottom() + 13.0), title, th::TEXT);

    let (meta, meta_color) = if starting {
        ("Starting…".to_string(), th::ACCENT_HOVER)
    } else if running {
        ("Running".to_string(), th::MINT)
    } else if card.record.playtime_secs > 0 {
        (
            format!("{} played", th::format_playtime(card.record.playtime_secs)),
            th::TEXT_FAINT,
        )
    } else {
        ("Never played".to_string(), th::TEXT_FAINT)
    };
    painter.text(
        pos2(text_left, rect.bottom() - 17.0),
        Align2::LEFT_CENTER,
        &meta,
        FontId::proportional(11.5),
        meta_color,
    );

    if running {
        let color = if starting { th::ACCENT_HOVER } else { th::MINT };
        let center = pos2(rect.right() - 18.0, rect.bottom() - 17.0);
        // Pulse while we are waiting for the game to appear.
        let pulse = if starting {
            let t = ui.input(|i| i.time) as f32;
            0.55 + 0.45 * (t * 3.0).sin()
        } else {
            1.0
        };
        painter.circle_filled(center, 4.0, color.gamma_multiply(pulse));
    }

    if card.record.favorite {
        let center = pos2(art_rect.right() - 20.0, art_rect.top() + 20.0);
        painter.circle_filled(center, 13.0, Color32::from_black_alpha(140));
        w::paint_star(&painter, center, 8.0, th::GOLD);
    }

    if card.record.hidden {
        painter.text(
            pos2(art_rect.left() + 12.0, art_rect.bottom() - 16.0),
            Align2::LEFT_CENTER,
            "Hidden",
            FontId::proportional(10.5),
            th::TEXT_FAINT,
        );
    }

    // Border last so it sits above the art.
    let stroke = Stroke::new(
        1.0 + select_t,
        w::lerp_color(
            w::lerp_color(th::BORDER, th::BORDER_BRIGHT, hover),
            th::ACCENT,
            select_t,
        ),
    );
    painter.rect_stroke(rect, th::R_LG, stroke, StrokeKind::Inside);

    // The play affordance is registered after the card so it wins the click.
    let play_center = pos2(art_rect.center().x, art_rect.center().y - 8.0);
    let play_rect = Rect::from_center_size(play_center, vec2(58.0, 58.0));
    let play = ui.interact(play_rect, response.id.with("play"), Sense::click());
    let play_t =
        ui.ctx()
            .animate_bool_with_time(play.id, response.hovered() || play.hovered(), 0.13);
    if play_t > 0.0 {
        let radius = 23.0 + 4.0 * play_t + if play.hovered() { 2.0 } else { 0.0 };
        painter.circle_filled(
            play_center,
            radius + 4.0,
            Color32::from_black_alpha((110.0 * play_t) as u8),
        );
        let fill = if running { th::MINT } else { th::ACCENT };
        painter.circle_filled(play_center, radius, fill.gamma_multiply(play_t));
        painter.circle_stroke(
            play_center,
            radius,
            Stroke::new(1.5, th::ACCENT_HOVER.gamma_multiply(play_t * 0.8)),
        );
        w::paint_play_triangle(
            &painter,
            play_center,
            18.0 * play_t,
            Color32::from_white_alpha((255.0 * play_t) as u8),
        );
    }

    if play.clicked() && !running {
        actions.push(Action::Launch(card.id.clone()));
    }
    if response.clicked() {
        actions.push(Action::Select(Some(card.id.clone())));
    }
    if response.double_clicked() && !running {
        actions.push(Action::Launch(card.id.clone()));
    }

    let menu_target = if play.hovered() { &play } else { &response };
    menu_target.clone().context_menu(|ui| {
        ui.set_min_width(180.0);
        if ui
            .add_enabled(!running, egui::Button::new("Play"))
            .clicked()
        {
            actions.push(Action::Launch(card.id.clone()));
            ui.close();
        }
        let fav = if card.record.favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        };
        if ui.button(fav).clicked() {
            actions.push(Action::ToggleFavorite(card.id.clone()));
            ui.close();
        }
        if ui.button("Show details").clicked() {
            actions.push(Action::Select(Some(card.id.clone())));
            ui.close();
        }
        if ui.button("Open install folder").clicked() {
            actions.push(Action::OpenFolder(card.dir.clone()));
            ui.close();
        }
        if let Some(url) = card.source.store_page() {
            if ui.button("Open store page").clicked() {
                actions.push(Action::OpenUrl(url));
                ui.close();
            }
        }
        ui.separator();
        let hide = if card.record.hidden {
            "Unhide"
        } else {
            "Hide from library"
        };
        if ui.button(hide).clicked() {
            actions.push(Action::SetHidden(card.id.clone(), !card.record.hidden));
            ui.close();
        }
    });

    if response.hovered() || play.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// Local games get no badge: the absence of one is the signal.
fn badge_for(source: &Source) -> Option<(&'static str, Color32)> {
    match source {
        Source::Local => None,
        Source::Steam { .. } => Some(("STEAM", th::STEAM)),
        Source::Epic { .. } => Some(("EPIC", th::EPIC)),
        Source::Xbox { .. } => Some(("XBOX", th::XBOX)),
        Source::Roblox => Some(("ROBLOX", th::ROBLOX)),
        Source::Prism { .. } => Some(("PRISM", th::PRISM)),
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .filter(|c| c.is_alphanumeric())
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect()
}

fn empty_state(ui: &mut Ui) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("Nothing here")
                .size(19.0)
                .color(th::TEXT_DIM)
                .strong(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "No games matched. Try clearing the search or filters, \n\
                 or add the folder your games live in from the sidebar.",
            )
            .size(13.0)
            .color(th::TEXT_FAINT),
        );
    });
}
