//! One installed game, and the state we keep about it between runs.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Source;

/// Everything we know about an installed game. Rebuilt on every scan.
#[derive(Clone, Debug)]
pub struct Game {
    pub id: String,
    /// Display name: the user's override, else the name the store or folder gives.
    pub name: String,
    pub dir: PathBuf,
    /// The executable to launch, for local games, and the one we read the icon
    /// from for store games. `None` when no plausible binary was found, which a
    /// store game can still survive because it launches through a URI.
    pub exe: Option<PathBuf>,
    /// Other plausible executables, offered in the details panel as alternatives.
    pub alternatives: Vec<PathBuf>,
    pub source: Source,
    /// Bytes on disk. Stores report this themselves; local games get a
    /// background folder walk.
    pub size_bytes: Option<u64>,
    /// "Last played" according to the store, used until we have our own record.
    pub store_last_played: Option<i64>,
    /// Where to look for the game's processes, when that is not `dir`. Prism
    /// instances run under the launcher's bundled Java, and Roblox moves itself
    /// into a new folder on every update.
    pub watch_dir: Option<PathBuf>,
    /// Artwork the source already knows about, preferred over anything found by
    /// searching — a packaged game's store logo, or a Prism instance's icon.
    pub cover_hint: Option<PathBuf>,
}

impl Game {
    /// The folder whose running processes mean "this game is up".
    pub fn watch_root(&self) -> &Path {
        self.watch_dir.as_deref().unwrap_or(&self.dir)
    }
}

/// The per-game state worth keeping between runs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// Overrides the name the scanner or store gave us.
    pub custom_name: Option<String>,
    /// Overrides the launch target. Only meaningful for local games.
    pub custom_exe: Option<PathBuf>,
    pub favorite: bool,
    pub hidden: bool,
    pub playtime_secs: u64,
    pub launches: u32,
    /// Unix seconds.
    pub last_played: Option<i64>,
}

impl Record {
    pub fn is_default(&self) -> bool {
        self.custom_name.is_none()
            && self.custom_exe.is_none()
            && !self.favorite
            && !self.hidden
            && self.playtime_secs == 0
            && self.launches == 0
            && self.last_played.is_none()
    }
}
