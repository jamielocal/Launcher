//! User settings, and the view options that go with them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::SourceKind;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Folders scanned for games. Each immediate subfolder is a candidate.
    pub library_paths: Vec<PathBuf>,
    /// Pick up games installed through Steam.
    pub scan_steam: bool,
    /// Pick up games installed through the Epic Games Launcher.
    pub scan_epic: bool,
    /// Pick up games installed by the Xbox app.
    pub scan_xbox: bool,
    /// Pick up Roblox and Roblox Studio.
    pub scan_roblox: bool,
    /// Pick up Prism Launcher / MultiMC Minecraft instances.
    pub scan_prism: bool,
    pub sort: Sort,
    pub filter: Filter,
    pub source_filter: Option<SourceKind>,
    pub show_hidden: bool,
    /// Minimise the launcher window while a game is running.
    pub minimize_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_paths: default_library_paths(),
            scan_steam: true,
            scan_epic: true,
            scan_xbox: true,
            scan_roblox: true,
            scan_prism: true,
            sort: Sort::LastPlayed,
            filter: Filter::All,
            source_filter: None,
            show_hidden: false,
            minimize_on_launch: false,
        }
    }
}

/// Guess where the games live: the launcher normally sits inside the library
/// folder, so its grandparent-of-exe is the natural first guess.
fn default_library_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        // target/<profile>/orbit.exe -> walk up to the project, then its parent.
        for ancestor in exe.ancestors().skip(1).take(4) {
            if ancestor.join("Cargo.toml").is_file() {
                if let Some(parent) = ancestor.parent() {
                    out.push(parent.to_path_buf());
                }
                break;
            }
        }
        if out.is_empty() {
            if let Some(parent) = exe.parent().and_then(|p| p.parent()) {
                out.push(parent.to_path_buf());
            }
        }
    }
    out.retain(|p| p.is_dir());
    if out.is_empty() {
        out.push(PathBuf::from(r"C:\Games"));
    }
    out
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    Name,
    LastPlayed,
    Playtime,
    Size,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::LastPlayed, Sort::Name, Sort::Playtime, Sort::Size];
    pub fn label(self) -> &'static str {
        match self {
            Sort::Name => "Name",
            Sort::LastPlayed => "Recently played",
            Sort::Playtime => "Most played",
            Sort::Size => "Size on disk",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Filter {
    All,
    Favorites,
    Unplayed,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Favorites, Filter::Unplayed];
    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All games",
            Filter::Favorites => "Favorites",
            Filter::Unplayed => "Never played",
        }
    }
}
