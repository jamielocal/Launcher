//! Finds Steam's libraries and reads the app manifests inside them.
//!
//! Steam records one `appmanifest_<appid>.acf` per installed app in each
//! library's `steamapps` folder, and lists the libraries themselves in
//! `steamapps/libraryfolders.vdf`. Reading those is far more reliable than
//! looking at what is in `steamapps/common`, which also holds shared
//! redistributables and leftovers from uninstalls.

use std::path::{Path, PathBuf};

use crate::model::{Game, Source};
use crate::sources::vdf;

/// `StateFlags` bit meaning "fully installed". Anything without it is still
/// downloading, needs an update, or is otherwise not ready to run.
const STATE_FULLY_INSTALLED: u64 = 4;

/// Appids that are tooling rather than games and would only clutter the grid.
const TOOL_APP_IDS: &[&str] = &[
    "228980",  // Steamworks Common Redistributables
    "1070560", // Steam Linux Runtime 1.0
    "1391110", // Steam Linux Runtime 2.0 (soldier)
    "1628350", // Steam Linux Runtime 3.0 (sniper)
    "1826330", // Proton Experimental tooling
    "858280",  // Proton 3.7
    "961940",  // Proton 3.16
    "1054830", // Proton 4.11
    "1113280", // Proton 4.2
    "1245040", // Proton 5.0
    "1420170", // Proton 5.13
    "1580130", // Proton 6.3
    "1887720", // Proton 7.0
    "2180100", // Proton 8.0
    "2805730", // Proton 9.0
];

/// Name fragments that mark an entry as tooling.
const TOOL_NAME_FRAGMENTS: &[&str] = &[
    "redistributable",
    "steam linux runtime",
    "proton",
    "steamworks shared",
];

/// Reads every installed Steam game across every library folder.
pub fn discover() -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let Some(root) = steam_root() else {
        return games;
    };
    for library in library_folders(&root) {
        let steamapps = library.join("steamapps");
        let Ok(entries) = std::fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_manifest = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("appmanifest_") && n.ends_with(".acf"));
            if is_manifest {
                if let Some(game) = read_manifest(&path, &steamapps) {
                    // A library reachable by two spellings would otherwise give
                    // us the same app twice.
                    if seen_ids.insert(game.id.clone()) {
                        games.push(game);
                    }
                }
            }
        }
    }
    games
}

/// Steam's own install folder, from the registry with sensible fallbacks.
pub fn steam_root() -> Option<PathBuf> {
    if let Some(path) = registry_steam_path() {
        if path.is_dir() {
            return Some(path);
        }
    }
    let candidates = [
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("steamapps").is_dir())
}

/// Steam writes `SteamPath` with forward slashes, so normalise separators.
#[cfg(windows)]
fn registry_steam_path() -> Option<PathBuf> {
    let raw = read_registry_string(
        windows::Win32::System::Registry::HKEY_CURRENT_USER,
        r"Software\Valve\Steam",
        "SteamPath",
    )?;
    Some(PathBuf::from(raw.replace('/', "\\")))
}

#[cfg(not(windows))]
fn registry_steam_path() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
pub(crate) fn read_registry_string(
    hive: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    value: &str,
) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};

    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let (subkey, value) = (wide(subkey), wide(value));

    // Ask for the size first, then read into a buffer of exactly that size.
    let mut bytes: u32 = 0;
    let probe = unsafe {
        RegGetValueW(
            hive,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        )
    };
    if probe.is_err() || bytes == 0 {
        return None;
    }

    let mut buf = vec![0u16; (bytes as usize).div_ceil(2)];
    let read = unsafe {
        RegGetValueW(
            hive,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
            Some(&mut bytes),
        )
    };
    if read.is_err() {
        return None;
    }
    let text: String = String::from_utf16_lossy(&buf);
    Some(text.trim_end_matches('\0').to_string())
}

/// Every library folder Steam knows about, including its own install folder.
///
/// The registry spells Steam's own path in lower case while
/// `libraryfolders.vdf` uses the real casing, so the two must be compared
/// case-insensitively or the primary library is visited twice.
fn library_folders(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let push = |path: PathBuf, out: &mut Vec<PathBuf>, seen: &mut Vec<String>| {
        let key = path.to_string_lossy().to_lowercase().replace('/', "\\");
        let key = key.trim_end_matches('\\').to_string();
        if path.is_dir() && !seen.contains(&key) {
            seen.push(key);
            out.push(path);
        }
    };

    push(root.to_path_buf(), &mut out, &mut seen);
    let manifest = root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(&manifest) {
        for path in vdf::all(&vdf::parse(&text), "path") {
            push(PathBuf::from(path), &mut out, &mut seen);
        }
    }
    out
}

/// Turns one `appmanifest_*.acf` into a `Game`, skipping anything that is not a
/// ready-to-run game.
fn read_manifest(manifest: &Path, steamapps: &Path) -> Option<Game> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let entries = vdf::parse(&text);

    let app_id = vdf::first(&entries, "appid")?;
    let name = vdf::first(&entries, "name")?;
    let install_dir = vdf::first(&entries, "installdir")?;

    let state: u64 = vdf::first(&entries, "StateFlags")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if state & STATE_FULLY_INSTALLED == 0 {
        return None;
    }
    if is_tool(&app_id, &name) {
        return None;
    }

    let dir = steamapps.join("common").join(&install_dir);
    if !dir.is_dir() {
        return None;
    }

    // Steam already knows the size, so no folder walk is needed.
    let size_bytes = vdf::first(&entries, "SizeOnDisk").and_then(|s| s.parse().ok());
    let store_last_played = vdf::first(&entries, "LastPlayed")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|t| *t > 0);

    // The executable is only needed for its icon, so a miss is not fatal.
    let (exe, alternatives) = crate::scanner::best_executable(&dir);

    Some(Game {
        id: format!("steam:{app_id}"),
        name,
        dir,
        exe,
        alternatives,
        source: Source::Steam { app_id },
        size_bytes,
        store_last_played,
        watch_dir: None,
        cover_hint: None,
    })
}

fn is_tool(app_id: &str, name: &str) -> bool {
    if TOOL_APP_IDS.contains(&app_id) {
        return true;
    }
    let lower = name.to_lowercase();
    TOOL_NAME_FRAGMENTS
        .iter()
        .any(|fragment| lower.contains(fragment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_redistributables_and_runtimes() {
        assert!(is_tool("228980", "Steamworks Common Redistributables"));
        assert!(is_tool("999999", "Proton 8.0"));
        assert!(is_tool("999999", "Steam Linux Runtime 3.0 (sniper)"));
        assert!(!is_tool("730", "Counter-Strike 2"));
        assert!(!is_tool("431960", "Wallpaper Engine"));
    }

    #[test]
    fn builds_the_expected_launch_uri() {
        let source = Source::Steam {
            app_id: "730".into(),
        };
        assert_eq!(
            source.launch_uri().as_deref(),
            Some("steam://rungameid/730")
        );
    }

    #[test]
    fn library_folders_are_deduplicated_across_spellings() {
        // Steam's own folder, however it is spelled, must be visited once.
        let Some(root) = steam_root() else {
            return;
        };
        let folders = library_folders(&root);
        let mut keys: Vec<String> = folders
            .iter()
            .map(|p| p.to_string_lossy().to_lowercase())
            .collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(before, keys.len(), "duplicate library folders: {folders:?}");
    }

    #[test]
    fn every_discovered_app_is_listed_once() {
        let mut ids: Vec<String> = discover().into_iter().map(|g| g.id).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(before, ids.len(), "the same app was listed twice");
    }

    /// Dev aid: prints the real Steam library on this machine. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the machine's own Steam install"]
    fn dump_real_steam_library() {
        println!("steam root: {:?}", steam_root());
        for game in discover() {
            println!(
                "{:<40} {:>8} GB  {:?}",
                game.name,
                game.size_bytes.unwrap_or(0) / 1_073_741_824,
                game.exe.as_ref().and_then(|e| e.file_name())
            );
        }
    }
}
