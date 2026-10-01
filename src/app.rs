//! The running application: all of the mutable state, and the only place it
//! changes.
//!
//! Drawing never touches the library directly. Each panel pushes [`Action`]s,
//! and every one of them is applied in [`App::apply`] after the frame has been
//! laid out, which keeps the state transitions in one readable list and the
//! borrows simple.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use crate::art::ArtCache;
use crate::discovery::collect_library;
use crate::model::{now_unix, Game, Library, Settings, Source};
use crate::persist::Store;
use crate::runner::Runner;
use crate::scanner;
use crate::theme as th;
use crate::ui::chrome::{header, status_bar};
use crate::ui::{details, grid, sidebar, Action, Card, Stats, UiState};

/// How long a toast stays on screen.
pub(crate) const TOAST_LIFETIME: Duration = Duration::from_secs(5);

/// Results streaming back from the background scan and size walk.
enum Message {
    Scanned(Vec<Game>),
    Size(String, u64),
}

pub struct App {
    lib: Library,
    settings: Settings,
    store: Store,
    art: ArtCache,
    runner: Runner,
    ui: UiState,

    tx: Sender<Message>,
    rx: Receiver<Message>,
    /// Folder sizes measured this session, reused across rescans so the numbers
    /// do not blink back to "measuring…".
    sizes: HashMap<String, u64>,
    pending_sizes: usize,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        th::apply(&cc.egui_ctx);

        let (store, settings, records) = Store::load();
        let covers_dir = store
            .path()
            .parent()
            .map(|p| p.join("covers"))
            .unwrap_or_else(|| PathBuf::from("covers"));
        let _ = std::fs::create_dir_all(&covers_dir);

        let (tx, rx) = channel();
        let mut app = Self {
            lib: Library {
                games: Vec::new(),
                records,
            },
            settings,
            store,
            art: ArtCache::new(covers_dir),
            runner: Runner::default(),
            ui: UiState::default(),
            tx,
            rx,
            sizes: HashMap::new(),
            pending_sizes: 0,
        };
        app.start_scan();
        app
    }

    /// Kicks off a library scan on a worker thread.
    fn start_scan(&mut self) {
        self.ui.scanning = true;
        let settings = self.settings.clone();
        let records = self.lib.records.clone();
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("orbit-scan".into())
            .spawn(move || {
                let _ = tx.send(Message::Scanned(collect_library(&settings, &records)));
            })
            .ok();
    }

    /// Pairs a scanned game with its saved record for the view layer.
    fn to_card(&self, game: &Game) -> Card {
        Card {
            id: game.id.clone(),
            name: game.name.clone(),
            dir: game.dir.clone(),
            exe: game.exe.clone(),
            alternatives: game.alternatives.clone(),
            source: game.source.clone(),
            cover_hint: game.cover_hint.clone(),
            size_bytes: game.size_bytes,
            record: self.lib.record(&game.id),
            last_played: self.lib.last_played(game),
        }
    }

    /// Measures any folder whose size we do not already know.
    fn start_size_walk(&mut self) {
        let todo: Vec<(String, PathBuf)> = self
            .lib
            .games
            .iter()
            .filter(|g| g.size_bytes.is_none() && !self.sizes.contains_key(&g.id))
            .map(|g| (g.id.clone(), g.dir.clone()))
            .collect();
        if todo.is_empty() {
            return;
        }
        self.pending_sizes += todo.len();
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("orbit-sizes".into())
            .spawn(move || {
                for (id, dir) in todo {
                    let bytes = scanner::dir_size(&dir);
                    if tx.send(Message::Size(id, bytes)).is_err() {
                        break;
                    }
                }
            })
            .ok();
    }

    fn drain_messages(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::Scanned(mut games) => {
                    // Reattach sizes we already measured, keeping whatever a
                    // store reported for itself.
                    for game in &mut games {
                        if game.size_bytes.is_none() {
                            game.size_bytes = self.sizes.get(&game.id).copied();
                        }
                    }
                    let found = games.len();
                    self.lib.games = games;
                    self.ui.scanning = false;
                    // Drop a selection that no longer exists.
                    if let Some(id) = self.ui.selected.clone() {
                        if self.lib.game(&id).is_none() {
                            self.ui.selected = None;
                        }
                    }
                    self.ui.toast(match found {
                        0 => "No games found. Add the folder your games live in.".to_string(),
                        1 => "Found 1 game.".to_string(),
                        n => format!("Found {n} games."),
                    });
                    self.start_size_walk();
                }
                Message::Size(id, bytes) => {
                    self.sizes.insert(id.clone(), bytes);
                    self.pending_sizes = self.pending_sizes.saturating_sub(1);
                    if let Some(game) = self.lib.games.iter_mut().find(|g| g.id == id) {
                        game.size_bytes = Some(bytes);
                    }
                }
            }
        }
    }

    /// Credits finished sessions with their play time.
    fn reap_sessions(&mut self) {
        for finished in self.runner.poll() {
            let name = self
                .lib
                .game(&finished.id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| finished.id.clone());

            if finished.suspicious {
                self.ui.toast(format!(
                    "{name} closed after {}s — if that was not intentional, pick a different executable in the details panel.",
                    finished.secs
                ));
            } else {
                let record = self.lib.record_mut(&finished.id);
                record.playtime_secs += finished.secs;
                self.ui.toast(format!(
                    "{name} — played for {}.",
                    th::format_playtime(finished.secs)
                ));
                self.store.mark_dirty();
            }
        }
    }

    /// Owned per-frame snapshot of the filtered, sorted library.
    fn cards(&self) -> Vec<Card> {
        self.lib
            .view(&self.ui.search, &self.settings)
            .into_iter()
            .map(|game| self.to_card(game))
            .collect()
    }

    fn launch(&mut self, id: &str, ctx: &egui::Context) {
        let Some(game) = self.lib.game(id).cloned() else {
            return;
        };
        // Each source knows how it wants to be started.
        let Some(launch) = game.source.launch_spec(game.exe.as_deref()) else {
            self.ui.toast(format!(
                "Nothing to launch for {}. Pick an executable in the details panel.",
                game.name
            ));
            return;
        };
        match self.runner.launch(&game.id, game.watch_root(), launch) {
            Ok(()) => {
                let record = self.lib.record_mut(&game.id);
                record.launches += 1;
                record.last_played = Some(now_unix());
                self.store.mark_dirty();
                self.ui.toast(match &game.source {
                    Source::Local => format!("Launching {}…", game.name),
                    other => format!("Asking {} to start {}…", other.label(), game.name),
                });
                if self.settings.minimize_on_launch {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
            Err(error) => self.ui.toast(error),
        }
    }

    fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Launch(id) => self.launch(&id, ctx),
            Action::Select(id) => {
                // Clicking the selected card again closes the panel.
                self.ui.selected = if self.ui.selected == id { None } else { id };
            }
            Action::ToggleFavorite(id) => {
                let record = self.lib.record_mut(&id);
                record.favorite = !record.favorite;
                self.store.mark_dirty();
            }
            Action::SetHidden(id, hidden) => {
                self.lib.record_mut(&id).hidden = hidden;
                if hidden && !self.settings.show_hidden && self.ui.selected.as_deref() == Some(&id)
                {
                    self.ui.selected = None;
                }
                self.store.mark_dirty();
            }
            Action::Rename(id, name) => {
                let previous = self.lib.game(&id).map(|g| g.name.clone());
                let record = self.lib.record_mut(&id);
                // An empty name means "go back to the folder name".
                record.custom_name = if name.is_empty() {
                    None
                } else {
                    Some(name.clone())
                };
                if let Some(game) = self.lib.games.iter_mut().find(|g| g.id == id) {
                    game.name = if name.is_empty() {
                        game.dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| id.clone())
                    } else {
                        name
                    };
                }
                self.store.mark_dirty();
                if let Some(previous) = previous {
                    self.ui.toast(format!("Renamed {previous}."));
                }
            }
            Action::SetExe(id, exe) => {
                self.lib.record_mut(&id).custom_exe = Some(exe.clone());
                if let Some(game) = self.lib.games.iter_mut().find(|g| g.id == id) {
                    // The old target becomes an alternative so it can be chosen back.
                    let old = game.exe.replace(exe.clone());
                    game.alternatives.retain(|p| *p != exe);
                    if let Some(old) = old.filter(|old| *old != exe) {
                        if !game.alternatives.contains(&old) {
                            game.alternatives.push(old);
                            game.alternatives.sort();
                        }
                    }
                }
                self.art.invalidate(&id);
                self.store.mark_dirty();
                self.ui.toast(format!(
                    "Now launching {}",
                    exe.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Action::PickExe(id) => {
                let start = self.lib.game(&id).map(|g| g.dir.clone());
                let mut dialog = rfd::FileDialog::new().add_filter("Executable", &["exe"]);
                if let Some(start) = start {
                    dialog = dialog.set_directory(start);
                }
                if let Some(path) = dialog.pick_file() {
                    self.apply(Action::SetExe(id, path), ctx);
                }
            }
            Action::OpenFolder(path) | Action::OpenPath(path) => {
                if let Err(error) = open::that_detached(&path) {
                    self.ui
                        .toast(format!("Could not open {}: {error}", path.display()));
                }
            }
            Action::ResetStats(id) => {
                let record = self.lib.record_mut(&id);
                record.playtime_secs = 0;
                record.launches = 0;
                record.last_played = None;
                self.store.mark_dirty();
                self.ui.toast("Stats reset.");
            }
            Action::Rescan => {
                self.art.clear();
                self.start_scan();
            }
            Action::AddLibraryPath => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    if !self.settings.library_paths.contains(&path) {
                        self.settings.library_paths.push(path);
                        self.store.mark_dirty();
                        self.start_scan();
                    }
                }
            }
            Action::RemoveLibraryPath(path) => {
                self.settings.library_paths.retain(|p| *p != path);
                self.store.mark_dirty();
                self.start_scan();
            }
            Action::OpenUrl(url) => {
                if let Err(error) = open::that_detached(&url) {
                    self.ui.toast(format!("Could not open {url}: {error}"));
                }
            }
            Action::SetSourceFilter(kind) => {
                self.settings.source_filter = kind;
                self.store.mark_dirty();
            }
            Action::SetSort(sort) => {
                self.settings.sort = sort;
                self.store.mark_dirty();
            }
            Action::SettingsChanged => self.store.mark_dirty(),
        }
    }

    fn stats(&self) -> Stats {
        Stats {
            games: self.lib.games.len(),
            by_source: self.lib.count_by_source(),
            playtime_secs: self.lib.total_playtime(),
            size_bytes: self.lib.total_size(),
            sizes_pending: self.pending_sizes > 0,
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let [r, g, b, a] = th::BG.to_array();
        [
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        self.drain_messages();
        self.art.pump(ctx);
        self.reap_sessions();
        self.store.maybe_flush(&self.settings, &self.lib.records);

        // Make sure a last write lands before the window goes away.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.store.flush(&self.settings, &self.lib.records);
        }

        let cards = self.cards();
        let stats = self.stats();
        let mut actions: Vec<Action> = Vec::new();

        // Global shortcuts.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.ui.selected.is_some() {
            self.ui.selected = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) && !self.ui.scanning {
            actions.push(Action::Rescan);
        }
        let typing = ctx.memory(|m| m.focused().is_some());
        if !typing && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(id) = self.ui.selected.clone() {
                actions.push(Action::Launch(id));
            }
        }

        egui::Panel::left("sidebar")
            .exact_size(sidebar::WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(th::SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| {
                sidebar::show(
                    ui,
                    &mut self.settings,
                    &mut self.ui.search,
                    &stats,
                    self.ui.scanning,
                    &mut actions,
                );
            });

        let selected_card = self
            .ui
            .selected
            .as_ref()
            .and_then(|id| cards.iter().find(|c| &c.id == id).cloned())
            // The panel should stay usable for a hidden game opened from the grid.
            .or_else(|| {
                self.ui
                    .selected
                    .as_ref()
                    .and_then(|id| self.lib.game(id))
                    .map(|game| self.to_card(game))
            });

        let mut details_open = selected_card.is_some();
        egui::Panel::right("details")
            .exact_size(details::WIDTH)
            .resizable(false)
            .drag_to_open(false)
            .frame(
                egui::Frame::NONE
                    .fill(th::SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(18, 18)),
            )
            .show_collapsible(ui, &mut details_open, |ui| {
                if let Some(card) = &selected_card {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            details::show(
                                ui,
                                card,
                                &mut self.art,
                                &self.runner,
                                &mut self.ui.rename_buffer,
                                &mut actions,
                            );
                        });
                }
            });

        if !details_open {
            self.ui.selected = None;
        }
        // The hero's close button reports through temp storage.
        if ctx.data_mut(|d| {
            d.remove_temp::<bool>(egui::Id::new("close_details"))
                .is_some()
        }) {
            self.ui.selected = None;
        }

        egui::Panel::bottom("status")
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(th::BG)
                    .inner_margin(egui::Margin::symmetric(26, 8)),
            )
            .show(ui, |ui| {
                status_bar(ui, &mut self.ui, self.runner.any_running())
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(th::BG)
                    .inner_margin(egui::Margin::symmetric(26, 0)),
            )
            .show(ui, |ui| {
                // A violet bloom across the top of the library, so the window
                // looks lit from one place rather than being a flat field.
                let glow = egui::Rect::from_min_size(
                    ui.max_rect().min - egui::vec2(26.0, 0.0),
                    egui::vec2(ui.max_rect().width() + 52.0, 270.0),
                );
                crate::ui::widgets::paint_glow(ui.painter(), glow, th::ACCENT_DEEP, 0.17);
                header(ui, &cards, &self.ui, &self.settings);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let backdrop = ui.interact(
                            ui.max_rect(),
                            ui.id().with("backdrop"),
                            egui::Sense::click(),
                        );
                        if backdrop.clicked() {
                            actions.push(Action::Select(None));
                        }
                        grid::show(
                            ui,
                            &cards,
                            &mut self.art,
                            &self.runner,
                            self.ui.selected.as_deref(),
                            &mut actions,
                        );
                    });
            });

        for action in actions {
            self.apply(action, ctx);
        }

        // Sessions and the size walk need us to wake up even when idle. The
        // "starting" pulse animates, so it needs a faster tick than the rest.
        if self.runner.any_running() {
            let starting = self
                .lib
                .games
                .iter()
                .any(|g| self.runner.is_starting(&g.id));
            let period = if starting { 60 } else { 500 };
            ctx.request_repaint_after(Duration::from_millis(period));
        } else if self.ui.scanning || self.pending_sizes > 0 || self.ui.status.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}
