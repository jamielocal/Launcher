//! Right-hand drawer for the selected game: hero art, the play button, stats
//! and the per-game overrides.

use egui::{pos2, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Ui};

use crate::art::{ArtCache, ArtKind};
use crate::model::Source;
use crate::runner::Runner;
use crate::theme as th;
use crate::ui::widgets as w;
use crate::ui::{Action, Card};

pub const WIDTH: f32 = 338.0;
const HERO_H: f32 = 190.0;

pub fn show(
    ui: &mut Ui,
    card: &Card,
    art: &mut ArtCache,
    runner: &Runner,
    rename_buffer: &mut Option<(String, String)>,
    actions: &mut Vec<Action>,
) {
    let running = runner.is_running(&card.id);
    let starting = running && runner.is_starting(&card.id);

    hero(ui, card, art);
    ui.add_space(14.0);

    ui.label(
        RichText::new(&card.name)
            .size(19.0)
            .color(th::TEXT)
            .strong(),
    );
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        if let Some((label, color)) = source_tag(&card.source) {
            ui.label(RichText::new(label).size(11.0).color(color).strong());
            ui.label(RichText::new("·").size(11.0).color(th::TEXT_FAINT));
        }
        ui.label(
            RichText::new(th::format_relative(card.last_played))
                .size(12.0)
                .color(th::TEXT_FAINT),
        );
    });

    ui.add_space(14.0);

    let label = if starting {
        "Starting…".to_string()
    } else if running {
        match runner.elapsed(&card.id) {
            Some(secs) => format!("Running · {}", th::format_playtime(secs.max(60))),
            None => "Running".to_string(),
        }
    } else {
        "Play".to_string()
    };
    if w::primary_button(ui, &label, !running).clicked() {
        actions.push(Action::Launch(card.id.clone()));
    }
    if starting {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Waiting for the store to bring the game up.")
                .size(11.0)
                .color(th::TEXT_FAINT),
        );
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let fav = if card.record.favorite {
            "★  Favorited"
        } else {
            "☆  Favorite"
        };
        if w::ghost_button(ui, fav).clicked() {
            actions.push(Action::ToggleFavorite(card.id.clone()));
        }
        if w::ghost_button(ui, "Open folder").clicked() {
            actions.push(Action::OpenFolder(card.dir.clone()));
        }
    });
    if let Some(url) = card.source.store_page() {
        ui.add_space(2.0);
        if w::ghost_button(ui, "Store page").clicked() {
            actions.push(Action::OpenUrl(url));
        }
    }

    ui.add_space(16.0);
    w::section_label(ui, "Stats");
    w::info_row(
        ui,
        "Play time",
        &th::format_playtime(card.record.playtime_secs),
    );
    w::info_row(ui, "Times launched", &card.record.launches.to_string());
    w::info_row(
        ui,
        "Size on disk",
        &card
            .size_bytes
            .map(th::format_size)
            .unwrap_or_else(|| "measuring…".into()),
    );

    ui.add_space(16.0);
    w::section_label(ui, "Launches");
    ui.label(
        RichText::new(card.launch_label())
            .size(12.0)
            .color(th::TEXT_DIM),
    )
    .on_hover_text(match card.exe.as_deref() {
        Some(exe) => exe.to_string_lossy().into_owned(),
        None => card.dir.to_string_lossy().into_owned(),
    });

    if card.exe_is_editable() {
        if !card.alternatives.is_empty() {
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt(("exe", &card.id))
                .selected_text(RichText::new("Use a different executable").size(12.5))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for alternative in &card.alternatives {
                        let name = alternative
                            .strip_prefix(&card.dir)
                            .unwrap_or(alternative)
                            .to_string_lossy()
                            .into_owned();
                        if ui.selectable_label(false, name).clicked() {
                            actions.push(Action::SetExe(card.id.clone(), alternative.clone()));
                        }
                    }
                });
        }
        ui.add_space(4.0);
        if w::ghost_button(ui, "Browse…").clicked() {
            actions.push(Action::PickExe(card.id.clone()));
        }
    } else {
        ui.add_space(4.0);
        ui.label(
            RichText::new(launch_note(&card.source))
                .size(11.0)
                .color(th::TEXT_FAINT),
        );
    }

    ui.add_space(16.0);
    w::section_label(ui, "Rename");
    // The buffer is keyed by id so switching games does not carry text over.
    let entry = rename_buffer.get_or_insert_with(|| (card.id.clone(), card.name.clone()));
    if entry.0 != card.id {
        *entry = (card.id.clone(), card.name.clone());
    }
    let response = ui.add_sized(
        vec2(ui.available_width(), 30.0),
        egui::TextEdit::singleline(&mut entry.1).margin(egui::Margin::symmetric(9, 6)),
    );
    let committed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if committed && entry.1.trim() != card.name {
        actions.push(Action::Rename(card.id.clone(), entry.1.trim().to_string()));
    }

    ui.add_space(18.0);
    ui.separator();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let hide = if card.record.hidden { "Unhide" } else { "Hide" };
        if w::ghost_button(ui, hide).clicked() {
            actions.push(Action::SetHidden(card.id.clone(), !card.record.hidden));
        }
        if w::ghost_button(ui, "Reset stats").clicked() {
            actions.push(Action::ResetStats(card.id.clone()));
        }
    });

    ui.add_space(10.0);
    ui.label(
        RichText::new(format!(
            "Drop a cover image named {}.png into the covers folder to replace the icon.",
            crate::model::file_key(&card.id)
        ))
        .size(11.0)
        .color(th::TEXT_FAINT),
    );
    ui.add_space(4.0);
    if w::ghost_button(ui, "Open covers folder").clicked() {
        actions.push(Action::OpenPath(art.covers_dir().to_path_buf()));
    }
    ui.add_space(12.0);
}

/// Wide banner at the top of the panel, using the same art as the card.
fn hero(ui: &mut Ui, card: &Card, art: &mut ArtCache) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), HERO_H), egui::Sense::hover());
    let painter = ui.painter().clone();

    let backdrop = art.backdrop(ui.ctx(), &card.id);
    w::paint_cover(ui, rect, &backdrop, th::R_MD);

    match art.art(card.art_request()) {
        Some(found) => {
            let texture = found.texture.clone();
            match found.kind {
                ArtKind::Cover => w::paint_cover(ui, rect, &texture, th::R_MD),
                ArtKind::Icon => {
                    let target = Rect::from_center_size(rect.center(), vec2(100.0, 100.0));
                    painter.circle_filled(rect.center(), 62.0, Color32::from_black_alpha(48));
                    w::paint_contain(ui, target, &texture, 100.0);
                }
            }
        }
        None => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                card.name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string(),
                FontId::proportional(48.0),
                Color32::from_white_alpha(140),
            );
        }
    }

    w::paint_scrim(
        &painter,
        Rect::from_min_max(pos2(rect.left(), rect.bottom() - 70.0), rect.right_bottom()),
        th::VOID,
    );
    painter.rect_stroke(
        rect,
        th::R_MD,
        egui::Stroke::new(1.0, th::BORDER),
        egui::StrokeKind::Inside,
    );

    // Close affordance in the corner of the hero.
    let close_rect = Rect::from_center_size(
        pos2(rect.right() - 18.0, rect.top() + 18.0),
        vec2(26.0, 26.0),
    );
    let close = ui.interact(
        close_rect,
        ui.id().with(("close", &card.id)),
        egui::Sense::click(),
    );
    let t = ui
        .ctx()
        .animate_bool_with_time(close.id, close.hovered(), 0.1);
    painter.circle_filled(
        close_rect.center(),
        12.0,
        Color32::from_black_alpha((130.0 + 80.0 * t) as u8),
    );
    painter.text(
        close_rect.center(),
        Align2::CENTER_CENTER,
        "×",
        FontId::proportional(16.0),
        w::lerp_color(th::TEXT_DIM, th::TEXT, t),
    );
    if close.clicked() {
        ui.data_mut(|d| d.insert_temp(egui::Id::new("close_details"), true));
    }
    if close.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Keep the panel's own layout flowing left-aligned afterwards.
    ui.with_layout(Layout::top_down(Align::Min), |_| {});
}

/// Why this game's launch target cannot be changed.
fn launch_note(source: &Source) -> &'static str {
    match source {
        Source::Steam { .. } | Source::Epic { .. } => {
            "Started through its store, so the overlay, cloud saves and updates all keep working."
        }
        Source::Xbox { .. } => {
            "Started through the shell by package identity, which is the only way a packaged game will run."
        }
        Source::Prism { .. } => "Started by Prism Launcher, which manages the instance and its Java.",
        Source::Roblox => "Roblox replaces its install folder on every update, so the path is resolved each scan.",
        Source::Local => "",
    }
}

fn source_tag(source: &Source) -> Option<(&'static str, Color32)> {
    match source {
        Source::Local => Some(("LOCAL", th::TEXT_FAINT)),
        Source::Steam { .. } => Some(("STEAM", th::STEAM)),
        Source::Epic { .. } => Some(("EPIC", th::EPIC)),
        Source::Xbox { .. } => Some(("XBOX", th::XBOX)),
        Source::Roblox => Some(("ROBLOX", th::ROBLOX)),
        Source::Prism { .. } => Some(("PRISM", th::PRISM)),
    }
}
