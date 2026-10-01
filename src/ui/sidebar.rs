//! Left rail: branding, search, filters, sort, sources and totals.

use egui::{vec2, Align, Layout, RichText, Ui};

use crate::model::{Filter, Settings, Sort, SourceKind};
use crate::theme as th;
use crate::ui::widgets as w;
use crate::ui::{Action, Stats};

pub const WIDTH: f32 = 244.0;

/// Label for a source chip. `None` is the "everything" chip.
fn source_label(kind: Option<SourceKind>) -> &'static str {
    match kind {
        None => "All sources",
        Some(SourceKind::Local) => "Local folders",
        Some(SourceKind::Steam) => "Steam",
        Some(SourceKind::Epic) => "Epic Games",
        Some(SourceKind::Xbox) => "Xbox",
        Some(SourceKind::Roblox) => "Roblox",
        Some(SourceKind::Prism) => "Prism / Minecraft",
    }
}

pub fn show(
    ui: &mut Ui,
    settings: &mut Settings,
    search: &mut String,
    stats: &Stats,
    scanning: bool,
    actions: &mut Vec<Action>,
) {
    ui.add_space(22.0);
    wordmark(ui);
    ui.add_space(18.0);

    let search_response = ui.add_sized(
        vec2(ui.available_width(), 33.0),
        egui::TextEdit::singleline(search)
            .hint_text(RichText::new("Search library").color(th::TEXT_FAINT))
            .margin(egui::Margin::symmetric(10, 8)),
    );
    // Ctrl+F or "/" focus the search box from anywhere.
    let focus_requested = ui.input(|i| {
        i.key_pressed(egui::Key::F) && i.modifiers.command || i.key_pressed(egui::Key::Slash)
    });
    if focus_requested && !search_response.has_focus() {
        search_response.request_focus();
    }
    if search_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        search.clear();
    }

    footer(ui, stats);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(16.0);
            w::section_label(ui, "Library");
            for filter in Filter::ALL {
                if w::chip(ui, filter.label(), settings.filter == filter).clicked() {
                    settings.filter = filter;
                    actions.push(Action::SettingsChanged);
                }
                ui.add_space(2.0);
            }

            ui.add_space(14.0);
            w::section_label(ui, "Sources");
            let chips = std::iter::once(None).chain(crate::model::SOURCE_KINDS.map(Some));
            for kind in chips {
                // Hide a source chip entirely when nothing came from it.
                let count = stats.count(kind);
                if kind.is_some() && count == 0 {
                    continue;
                }
                let selected = settings.source_filter == kind;
                if w::chip_with_count(ui, source_label(kind), count, selected).clicked() {
                    actions.push(Action::SetSourceFilter(kind));
                }
                ui.add_space(2.0);
            }

            ui.add_space(14.0);
            w::section_label(ui, "Sort by");
            egui::ComboBox::from_id_salt("sort")
                .selected_text(RichText::new(settings.sort.label()).size(13.0))
                .width(ui.available_width() - 8.0)
                .show_ui(ui, |ui| {
                    for sort in Sort::ALL {
                        if ui
                            .selectable_label(settings.sort == sort, sort.label())
                            .clicked()
                        {
                            actions.push(Action::SetSort(sort));
                        }
                    }
                });

            ui.add_space(14.0);
            w::section_label(ui, "Folders");
            let mut remove = None;
            for path in &settings.library_paths {
                let label = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("• {label}"))
                                .size(12.5)
                                .color(th::TEXT_DIM),
                        )
                        .truncate(),
                    )
                    .on_hover_text(path.to_string_lossy());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("×").size(14.0).color(th::TEXT_FAINT),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Stop scanning this folder")
                            .clicked()
                        {
                            remove = Some(path.clone());
                        }
                    });
                });
            }
            if let Some(path) = remove {
                actions.push(Action::RemoveLibraryPath(path));
            }
            ui.add_space(4.0);
            if w::ghost_button(ui, "+  Add folder").clicked() {
                actions.push(Action::AddLibraryPath);
            }
            let rescan_label = if scanning {
                "Scanning…"
            } else {
                "↻  Rescan"
            };
            if w::ghost_button(ui, rescan_label).clicked() && !scanning {
                actions.push(Action::Rescan);
            }

            ui.add_space(14.0);
            w::section_label(ui, "Include");
            // Each source can be turned off; doing so changes what is in the
            // library, so it needs a rescan. The last two are view-only.
            let sources: [(&str, &mut bool); 5] = [
                ("Steam", &mut settings.scan_steam),
                ("Epic Games", &mut settings.scan_epic),
                ("Xbox", &mut settings.scan_xbox),
                ("Roblox", &mut settings.scan_roblox),
                ("Prism / Minecraft", &mut settings.scan_prism),
            ];
            let mut rescan_needed = false;
            for (label, value) in sources {
                if ui
                    .checkbox(value, RichText::new(label).size(12.5).color(th::TEXT_DIM))
                    .changed()
                {
                    rescan_needed = true;
                    actions.push(Action::SettingsChanged);
                }
            }
            if rescan_needed {
                actions.push(Action::Rescan);
            }

            ui.add_space(10.0);
            w::section_label(ui, "View");
            let view: [(&str, &mut bool); 2] = [
                ("Minimize on launch", &mut settings.minimize_on_launch),
                ("Show hidden games", &mut settings.show_hidden),
            ];
            for (label, value) in view {
                if ui
                    .checkbox(value, RichText::new(label).size(12.5).color(th::TEXT_DIM))
                    .changed()
                {
                    actions.push(Action::SettingsChanged);
                }
            }
            ui.add_space(14.0);
        });
}

/// Totals pinned to the bottom of the rail. A nested panel rather than a
/// bottom-up layout, so the scroll area above it gets a height that excludes
/// this space instead of drawing underneath it.
fn footer(ui: &mut Ui, stats: &Stats) {
    egui::Panel::bottom("sidebar_totals")
        .resizable(false)
        .drag_to_open(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::NONE
                .fill(th::SIDEBAR)
                .inner_margin(egui::Margin {
                    top: 10,
                    bottom: 12,
                    ..Default::default()
                }),
        )
        .show(ui, |ui| {
            // A hairline above the totals separates them from the scrolling list.
            let (rule, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
            w::paint_horizontal_gradient(
                ui.painter(),
                rule,
                th::BORDER,
                egui::Color32::TRANSPARENT,
            );
            ui.add_space(8.0);

            let size = if stats.sizes_pending && stats.size_bytes == 0 {
                "measuring…".to_string()
            } else {
                th::format_size(stats.size_bytes)
            };
            w::info_row(ui, "Games", &stats.games.to_string());
            w::info_row(ui, "Playtime", &th::format_playtime(stats.playtime_secs));
            w::info_row(ui, "On disk", &size);
        });
}

/// The logo mark and name: a ring with a body travelling around it.
fn wordmark(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.add_space(3.0);
        let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), egui::Sense::hover());
        let center = rect.center();

        ui.painter()
            .circle_filled(center, 11.5, th::ACCENT_DEEP.gamma_multiply(0.4));
        ui.painter()
            .circle_stroke(center, 10.0, egui::Stroke::new(1.4, th::ACCENT));
        // The orbiting body, parked up and to the right.
        let angle = -std::f32::consts::FRAC_PI_4;
        let body = egui::pos2(center.x + 10.0 * angle.cos(), center.y + 10.0 * angle.sin());
        ui.painter().circle_filled(body, 4.0, th::SIDEBAR);
        ui.painter().circle_filled(body, 2.8, th::ACCENT_HOVER);

        ui.add_space(5.0);
        ui.label(RichText::new("Orbit").size(20.0).color(th::TEXT).strong());
    });
}
