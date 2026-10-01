//! The window's own furniture: the library header and the status bar.

use egui::RichText;

use crate::app::TOAST_LIFETIME;
use crate::model::Settings;
use crate::theme as th;
use crate::ui::widgets as w;
use crate::ui::{Card, UiState};

pub fn header(ui: &mut egui::Ui, cards: &[Card], state: &UiState, settings: &Settings) {
    ui.add_space(26.0);
    ui.horizontal(|ui| {
        let title = if state.search.trim().is_empty() {
            settings.filter.label().to_string()
        } else {
            format!("Results for “{}”", state.search.trim())
        };
        ui.label(RichText::new(title).size(23.0).color(th::TEXT).strong());
        ui.add_space(10.0);
        // The count sits in a pill so it reads as metadata, not as a subtitle.
        let text = match cards.len() {
            1 => "1 game".to_string(),
            n => format!("{n} games"),
        };
        let galley = ui.painter().layout_no_wrap(
            text.clone(),
            egui::FontId::proportional(11.5),
            th::TEXT_DIM,
        );
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(galley.size().x + 18.0, 22.0),
            egui::Sense::hover(),
        );
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(11), th::SURFACE);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &text,
            egui::FontId::proportional(11.5),
            th::TEXT_DIM,
        );
    });
    ui.add_space(16.0);

    // A hairline that fades out to the right, closing off the header.
    let (rule, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    w::paint_horizontal_gradient(ui.painter(), rule, th::BORDER, egui::Color32::TRANSPARENT);
    ui.add_space(20.0);
}

pub fn status_bar(ui: &mut egui::Ui, state: &mut UiState, any_running: bool) {
    // Expire the toast rather than leaving stale text on screen.
    if let Some((_, since)) = &state.status {
        if since.elapsed() > TOAST_LIFETIME {
            state.status = None;
        }
    }

    ui.horizontal(|ui| {
        match &state.status {
            Some((message, since)) => {
                // Fade out over the last second of the toast's life.
                let remaining = TOAST_LIFETIME.saturating_sub(since.elapsed()).as_secs_f32();
                let alpha = remaining.min(1.0).clamp(0.0, 1.0);
                ui.label(
                    RichText::new(message)
                        .size(12.0)
                        .color(th::TEXT_DIM.gamma_multiply(alpha)),
                );
            }
            None => {
                ui.label(
                    RichText::new("Double-click a game to play  ·  F5 rescans  ·  / searches")
                        .size(12.0)
                        .color(th::TEXT_FAINT),
                );
            }
        }
        if any_running {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("● In game").size(12.0).color(th::MINT));
            });
        }
    });
}
