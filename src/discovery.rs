//! Finding what is installed.
//!
//! One pass over the library folders, then one pass per enabled source, merged
//! into the single list the rest of the app works from.

use std::collections::BTreeMap;

use crate::model::{Game, Record, Settings};
use crate::{scanner, sources};

/// The whole discovery pipeline: scan the library folders, then fold in every
/// enabled store. Pure apart from reading the filesystem, so it can run on a
/// worker thread and be exercised on its own.
pub fn collect_library(settings: &Settings, records: &BTreeMap<String, Record>) -> Vec<Game> {
    let mut games = scanner::scan(&settings.library_paths, records);
    // A store game whose folder also sits inside a scanned library folder would
    // otherwise appear twice. The store entry knows more about it — its real
    // name, its size, how to launch it — so it replaces the local one.
    for game in sources::discover(settings) {
        games.retain(|existing| existing.dir != game.dir);
        games.push(game);
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    games
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{self, Source};
    use crate::theme as th;

    #[test]
    fn the_merged_library_lists_each_game_once() {
        let settings = Settings::default();
        let games = collect_library(&settings, &BTreeMap::new());

        let mut ids: Vec<&str> = games.iter().map(|g| g.id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "a game was listed twice");

        // Two entries pointing at one folder would mean the store/local merge
        // failed to collapse them.
        let mut dirs: Vec<String> = games
            .iter()
            .map(|g| g.dir.to_string_lossy().to_lowercase())
            .collect();
        let before = dirs.len();
        dirs.sort_unstable();
        dirs.dedup();
        assert_eq!(before, dirs.len(), "two games shared an install folder");
    }

    #[test]
    fn every_game_can_say_how_it_starts() {
        for game in collect_library(&Settings::default(), &BTreeMap::new()) {
            assert!(
                game.source.launch_uri().is_some() || game.exe.is_some(),
                "{} has no way to launch",
                game.name
            );
        }
    }

    #[test]
    fn disabling_every_source_leaves_only_local_games() {
        let settings = Settings {
            scan_steam: false,
            scan_epic: false,
            scan_xbox: false,
            scan_roblox: false,
            scan_prism: false,
            ..Settings::default()
        };
        for game in collect_library(&settings, &BTreeMap::new()) {
            assert_eq!(game.source, Source::Local, "{} is not local", game.name);
        }
    }

    /// Guards against a source being added without a way to turn it off, which
    /// would leave its games unfilterable from the sidebar.
    #[test]
    fn every_source_present_in_the_library_can_be_switched_off() {
        let all_on = collect_library(&Settings::default(), &BTreeMap::new());
        for kind in model::SOURCE_KINDS {
            if kind == model::SourceKind::Local {
                continue;
            }
            if !all_on.iter().any(|g| g.source.kind() == kind) {
                continue;
            }
            let mut settings = Settings::default();
            match kind {
                model::SourceKind::Steam => settings.scan_steam = false,
                model::SourceKind::Epic => settings.scan_epic = false,
                model::SourceKind::Xbox => settings.scan_xbox = false,
                model::SourceKind::Roblox => settings.scan_roblox = false,
                model::SourceKind::Prism => settings.scan_prism = false,
                model::SourceKind::Local => unreachable!(),
            }
            let remaining = collect_library(&settings, &BTreeMap::new());
            assert!(
                !remaining.iter().any(|g| g.source.kind() == kind),
                "{kind:?} games survived their source being turned off"
            );
        }
    }

    /// Dev aid: prints the whole merged library. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the machine's own game library"]
    fn dump_merged_library() {
        for game in collect_library(&Settings::default(), &BTreeMap::new()) {
            println!(
                "{:<8} {:<34} {:>9}  {}",
                game.source.label(),
                game.name,
                game.size_bytes.map(th::format_size).unwrap_or_default(),
                game.exe
                    .as_deref()
                    .and_then(|e| e.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "(launcher only)".into())
            );
        }
    }
}
