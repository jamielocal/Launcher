//! Roblox, which installs itself per user into a versioned folder and replaces
//! that folder on every update.
//!
//! The player and Studio are listed separately because they are separate things
//! to open. Both are launched by running their executable: Roblox opens its own
//! home screen from there, the same as the Start Menu shortcut does.

use std::path::{Path, PathBuf};

use crate::model::{Game, Source};

/// The executables worth listing, with the name to show for each.
const TARGETS: &[(&str, &str, bool)] = &[
    ("RobloxPlayerBeta.exe", "Roblox", false),
    ("RobloxStudioBeta.exe", "Roblox Studio", true),
];

pub fn discover() -> Vec<Game> {
    let mut games = Vec::new();
    for root in roots() {
        for (exe_name, display, is_studio) in TARGETS {
            if games.iter().any(|g: &Game| g.name == *display) {
                continue;
            }
            if let Some(exe) = newest_version_with(&root, exe_name) {
                games.push(Game {
                    id: format!("roblox:{}", if *is_studio { "studio" } else { "player" }),
                    name: (*display).to_string(),
                    dir: exe.parent().unwrap_or(&root).to_path_buf(),
                    exe: Some(exe),
                    alternatives: Vec::new(),
                    source: Source::Roblox,
                    size_bytes: None,
                    store_last_played: None,
                    // Updates move the game into a new version folder, so watch
                    // the install root rather than the version we launched.
                    watch_dir: Some(root.clone()),
                    cover_hint: None,
                });
            }
        }
    }
    games
}

/// Where Roblox may have installed itself, per user and machine-wide.
fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Roblox"));
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Roblox"));
    roots.push(PathBuf::from(r"C:\Program Files\Roblox"));
    roots.retain(|p| p.is_dir());
    roots
}

/// Finds the most recently installed version folder holding `exe_name`.
///
/// Roblox leaves old versions behind after an update, so picking by modification
/// time avoids launching a stale build.
fn newest_version_with(root: &Path, exe_name: &str) -> Option<PathBuf> {
    let versions = root.join("Versions");
    let search_in = if versions.is_dir() {
        versions
    } else {
        root.to_path_buf()
    };

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&search_in).ok()?.flatten() {
        let candidate = entry.path().join(exe_name);
        if !candidate.is_file() {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(best, _)| modified > *best) {
            best = Some((modified, candidate));
        }
    }

    // A flat install keeps the executable directly in the root.
    best.map(|(_, path)| path).or_else(|| {
        let direct = root.join(exe_name);
        direct.is_file().then_some(direct)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roblox_is_launched_by_running_its_executable() {
        assert!(Source::Roblox.launch_uri().is_none());
        assert_eq!(Source::Roblox.label(), "Roblox");
    }

    /// Dev aid: prints what was found on this machine. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on whether Roblox is installed"]
    fn dump_real_roblox() {
        println!("roots: {:?}", roots());
        for game in discover() {
            println!("{:<14} {:?}", game.name, game.exe);
        }
    }
}
