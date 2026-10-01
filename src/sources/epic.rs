//! Reads the Epic Games Launcher's install manifests.
//!
//! Epic writes one JSON `.item` file per installed artifact into its manifests
//! folder. Those files describe DLC and plugins as well as games, so entries are
//! filtered down to complete installs that Epic itself categorises as games.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::{Game, Source};

/// Epic's manifest schema, narrowed to the fields we use. Unknown fields are
/// ignored, so launcher updates that add keys cannot break parsing.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    display_name: Option<String>,
    install_location: Option<String>,
    launch_executable: Option<String>,
    app_name: Option<String>,
    catalog_namespace: Option<String>,
    catalog_item_id: Option<String>,
    /// Set on DLC and add-ons, naming the game they belong to.
    main_game_app_name: Option<String>,
    install_size: Option<u64>,
    #[serde(rename = "bIsIncompleteInstall")]
    is_incomplete_install: Option<bool>,
    app_categories: Option<Vec<String>>,
}

/// Reads every installed Epic game.
pub fn discover() -> Vec<Game> {
    let Some(dir) = manifests_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut games = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("item"))
        {
            if let Some(game) = read_manifest(&path) {
                games.push(game);
            }
        }
    }
    games
}

/// Epic's manifests folder, from the registry with the standard path as fallback.
pub fn manifests_dir() -> Option<PathBuf> {
    if let Some(base) = registry_app_data_path() {
        let candidate = base.join("Manifests");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    let base = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let candidate = base
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    candidate.is_dir().then_some(candidate)
}

#[cfg(windows)]
fn registry_app_data_path() -> Option<PathBuf> {
    use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
    crate::sources::steam::read_registry_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher",
        "AppDataPath",
    )
    .map(PathBuf::from)
}

#[cfg(not(windows))]
fn registry_app_data_path() -> Option<PathBuf> {
    None
}

fn read_manifest(path: &Path) -> Option<Game> {
    let bytes = std::fs::read(path).ok()?;
    // Epic writes these with a UTF-8 byte order mark, which serde_json rejects.
    let text = std::str::from_utf8(&bytes)
        .ok()?
        .trim_start_matches('\u{feff}');
    let manifest: Manifest = serde_json::from_str(text).ok()?;

    if manifest.is_incomplete_install.unwrap_or(false) {
        return None;
    }
    // Add-ons point at a parent game; only list the parent itself.
    if manifest
        .main_game_app_name
        .as_deref()
        .is_some_and(|parent| !parent.is_empty() && Some(parent) != manifest.app_name.as_deref())
    {
        return None;
    }
    // Epic also tracks engines and plugins through the same manifests.
    let categories = manifest.app_categories.unwrap_or_default();
    if !categories.is_empty() && !categories.iter().any(|c| c.eq_ignore_ascii_case("games")) {
        return None;
    }

    let dir = PathBuf::from(manifest.install_location?);
    if !dir.is_dir() {
        return None;
    }

    let app_name = manifest.app_name?;
    let exe = manifest
        .launch_executable
        .map(|relative| dir.join(relative))
        .filter(|p| p.is_file());
    // Fall back to the scanner when the manifest's executable is missing, so the
    // tile still gets an icon.
    let (exe, alternatives) = match exe {
        Some(exe) => {
            let (_, mut others) = crate::scanner::best_executable(&dir);
            others.retain(|p| *p != exe);
            (Some(exe), others)
        }
        None => crate::scanner::best_executable(&dir),
    };

    Some(Game {
        id: format!("epic:{app_name}"),
        name: manifest
            .display_name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| app_name.clone()),
        dir,
        exe,
        alternatives,
        source: Source::Epic {
            app_name,
            namespace: manifest.catalog_namespace.unwrap_or_default(),
            catalog_item_id: manifest.catalog_item_id.unwrap_or_default(),
        },
        size_bytes: manifest.install_size,
        store_last_played: None,
        watch_dir: None,
        cover_hint: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "AppName": "0123456789abcdef0123456789abcdef",
        "DisplayName": "Example Game",
        "InstallLocation": "C:\\Program Files\\Epic Games\\ExampleGame",
        "LaunchExecutable": "ExampleGame.exe",
        "CatalogNamespace": "fedcba9876543210fedcba9876543210",
        "CatalogItemId": "0f0f0f0f1e1e1e1e2d2d2d2d3c3c3c3c",
        "MainGameAppName": "",
        "InstallSize": 14705188927,
        "bIsIncompleteInstall": false,
        "AppCategories": ["games", "applications"],
        "SomeFutureKeyWeDoNotKnowAbout": 42
    }"#;

    #[test]
    fn parses_a_manifest_and_ignores_unknown_keys() {
        let manifest: Manifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(manifest.display_name.as_deref(), Some("Example Game"));
        assert_eq!(manifest.install_size, Some(14_705_188_927));
    }

    #[test]
    fn tolerates_a_byte_order_mark() {
        let with_bom = format!("\u{feff}{SAMPLE}");
        let trimmed = with_bom.trim_start_matches('\u{feff}');
        assert!(serde_json::from_str::<Manifest>(trimmed).is_ok());
    }

    #[test]
    fn builds_the_expected_launch_uri() {
        let source = Source::Epic {
            app_name: "APP".into(),
            namespace: "NS".into(),
            catalog_item_id: "ID".into(),
        };
        assert_eq!(
            source.launch_uri().as_deref(),
            Some("com.epicgames.launcher://apps/NS%3AID%3AAPP?action=launch&silent=true")
        );
    }

    /// Dev aid: prints the real Epic library on this machine. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the machine's own Epic install"]
    fn dump_real_epic_library() {
        println!("manifests: {:?}", manifests_dir());
        for game in discover() {
            println!("{:<40} {:?}", game.name, game.exe);
        }
    }
}
