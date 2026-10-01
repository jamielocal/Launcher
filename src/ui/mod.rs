//! The view layer. Drawing never mutates the library directly: each panel
//! pushes `Action`s which `App` applies after the frame is laid out, which keeps
//! the borrow graph simple and the state transitions in one place.

pub mod chrome;
pub mod details;
pub mod grid;
pub mod sidebar;
pub mod widgets;

use std::path::PathBuf;

use crate::model::{Record, Sort, Source, SourceKind};

/// An immutable per-frame snapshot of one game, cheap to clone for a library
/// this size and much easier to pass around than borrowed library state.
#[derive(Clone)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    /// Absent when no executable was found; a store game can still be launched.
    pub exe: Option<PathBuf>,
    pub alternatives: Vec<PathBuf>,
    pub source: Source,
    /// Artwork the source already knows about.
    pub cover_hint: Option<PathBuf>,
    pub size_bytes: Option<u64>,
    pub record: Record,
    /// Our own record if we have one, else what the store reported.
    pub last_played: Option<i64>,
}

impl Card {
    /// Everything the art cache needs to find this game's cover.
    pub fn art_request(&self) -> crate::art::Request<'_> {
        crate::art::Request {
            id: &self.id,
            dir: &self.dir,
            exe: self.exe.as_deref(),
            hint: self.cover_hint.as_deref(),
        }
    }

    /// What the details panel shows as the launch target.
    pub fn launch_label(&self) -> String {
        let relative = || {
            self.exe
                .as_deref()
                .map(|exe| {
                    exe.strip_prefix(&self.dir)
                        .unwrap_or(exe)
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_else(|| "No executable found".into())
        };
        match &self.source {
            Source::Local | Source::Roblox => relative(),
            Source::Steam { app_id } => format!("Steam · app {app_id}"),
            Source::Epic { .. } => "Epic Games Launcher".into(),
            Source::Xbox { aumid } => format!("Xbox · {aumid}"),
            Source::Prism { instance } => format!("Prism Launcher · instance {instance}"),
        }
    }

    /// Only games we run ourselves, and where nothing else depends on which
    /// binary it is, offer the executable picker.
    pub fn exe_is_editable(&self) -> bool {
        self.source.exe_is_editable()
    }
}

/// Something the user asked for, applied by `App` after drawing.
pub enum Action {
    Launch(String),
    Select(Option<String>),
    ToggleFavorite(String),
    SetHidden(String, bool),
    Rename(String, String),
    SetExe(String, PathBuf),
    PickExe(String),
    OpenFolder(PathBuf),
    OpenPath(PathBuf),
    OpenUrl(String),
    ResetStats(String),
    Rescan,
    AddLibraryPath,
    RemoveLibraryPath(PathBuf),
    SetSort(Sort),
    SetSourceFilter(Option<SourceKind>),
    SettingsChanged,
}

/// Transient view state that does not belong in the saved library.
#[derive(Default)]
pub struct UiState {
    pub search: String,
    pub selected: Option<String>,
    /// Buffer for the rename field, so typing does not rewrite the record on
    /// every keystroke.
    pub rename_buffer: Option<(String, String)>,
    pub status: Option<(String, std::time::Instant)>,
    pub scanning: bool,
}

impl UiState {
    pub fn toast(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), std::time::Instant::now()));
    }
}

/// Aggregate numbers shown in the sidebar footer and the source chips.
pub struct Stats {
    pub games: usize,
    /// How many games each source contributed. A list rather than a field per
    /// source, so adding a source does not mean touching this.
    pub by_source: Vec<(SourceKind, usize)>,
    pub playtime_secs: u64,
    pub size_bytes: u64,
    pub sizes_pending: bool,
}

impl Stats {
    pub fn count(&self, kind: Option<SourceKind>) -> usize {
        match kind {
            None => self.games,
            Some(kind) => self
                .by_source
                .iter()
                .find(|(k, _)| *k == kind)
                .map(|(_, n)| *n)
                .unwrap_or(0),
        }
    }
}
