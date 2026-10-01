//! Minecraft instances managed by Prism Launcher (and MultiMC, which uses the
//! same layout).
//!
//! Each instance is listed as its own entry, because an instance is what you
//! actually pick when you sit down to play — a modpack at a particular Minecraft
//! version — and Prism can start one directly with `--launch <instance>`.

use std::path::{Path, PathBuf};

use crate::model::{Game, Source};
use crate::sources::shortcut;

/// Launchers sharing Prism's on-disk layout, each as (data folder, exe stem,
/// display suffix).
const FLAVOURS: &[(&str, &str)] = &[("PrismLauncher", "prismlauncher"), ("MultiMC", "multimc")];

pub fn discover() -> Vec<Game> {
    let mut games = Vec::new();
    for (data_name, exe_stem) in FLAVOURS {
        let Some(data_dir) = data_dir(data_name, exe_stem) else {
            continue;
        };
        let Some(launcher) = launcher_exe(exe_stem, &data_dir) else {
            // Without the launcher there is nothing we could start.
            continue;
        };
        games.extend(instances(&data_dir, &launcher));
    }
    games
}

/// Prism keeps its instances in roaming app data, or beside the executable when
/// it was installed as a portable folder.
fn data_dir(data_name: &str, exe_stem: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidates.push(PathBuf::from(appdata).join(data_name));
    }
    // A portable install keeps `instances` next to the executable.
    if let Some(exe) = shortcut::find_target(exe_stem) {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
        }
    }
    candidates
        .into_iter()
        .find(|p| p.join("instances").is_dir())
}

/// Finds the launcher executable. Prism is often installed as a portable folder
/// with no registry trace, so its Start Menu shortcut is the reliable record.
fn launcher_exe(exe_stem: &str, data_dir: &Path) -> Option<PathBuf> {
    let file = format!("{exe_stem}.exe");

    // A portable install sits next to its own data.
    let beside = data_dir.join(&file);
    if beside.is_file() {
        return Some(beside);
    }

    let mut candidates = Vec::new();
    for var in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var) {
            let base = PathBuf::from(base);
            candidates.push(base.join("Programs").join("PrismLauncher").join(&file));
            candidates.push(base.join("PrismLauncher").join(&file));
            candidates.push(base.join("Prism Launcher").join(&file));
            candidates.push(base.join("MultiMC").join(&file));
        }
    }
    if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
        return Some(found);
    }

    shortcut::find_target(exe_stem).filter(|p| p.is_file())
}

/// Reads every instance folder under `<data dir>/instances`.
fn instances(data_dir: &Path, launcher: &Path) -> Vec<Game> {
    let instances_dir = data_dir.join("instances");
    let Ok(entries) = std::fs::read_dir(&instances_dir) else {
        return Vec::new();
    };

    let mut games = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let config = dir.join("instance.cfg");
        if !config.is_file() {
            continue;
        }
        // The folder name is the id Prism expects on the command line; the
        // display name inside the config is what the user named it, and the two
        // drift apart as instances are renamed.
        let folder = entry.file_name().to_string_lossy().into_owned();
        let settings = read_cfg(&config);
        let name = settings
            .iter()
            .find(|(k, _)| k == "name")
            .map(|(_, v)| v.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| folder.clone());

        let icon_key = settings
            .iter()
            .find(|(k, _)| k == "iconKey")
            .map(|(_, v)| v.as_str())
            .unwrap_or("default");

        games.push(Game {
            id: format!("prism:{}", folder.to_lowercase()),
            name,
            dir: dir.clone(),
            // The launcher is what we run; the instance is an argument to it.
            exe: Some(launcher.to_path_buf()),
            alternatives: Vec::new(),
            source: Source::Prism { instance: folder },
            size_bytes: None,
            store_last_played: None,
            // Minecraft runs under Prism's bundled Java, not out of the instance
            // folder, so the whole data directory is what tells us it is up.
            watch_dir: Some(data_dir.to_path_buf()),
            cover_hint: instance_icon(data_dir, icon_key),
        });
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    games
}

/// Instance icons the user has imported live in Prism's `icons` folder. The
/// built-in ones live inside the executable, so those fall back to its own icon.
fn instance_icon(data_dir: &Path, icon_key: &str) -> Option<PathBuf> {
    if icon_key.is_empty() || icon_key == "default" {
        return None;
    }
    let icons = data_dir.join("icons");
    for ext in ["png", "jpg", "jpeg", "webp", "ico"] {
        let candidate = icons.join(format!("{icon_key}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Some icon keys already carry their extension.
    let direct = icons.join(icon_key);
    direct.is_file().then_some(direct)
}

/// Reads an INI-ish `key=value` file, ignoring section headers and comments.
/// Prism's `instance.cfg` is written by Qt's settings code, which never quotes
/// or escapes values, so a plain split is correct here.
fn read_cfg(path: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('[') && !line.starts_with('#'))
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_key_values_and_skips_section_headers() {
        let dir = std::env::temp_dir().join(format!("orbit-prism-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("instance.cfg");
        std::fs::write(
            &path,
            "[General]\nConfigVersion=1.3\niconKey=default\nname=Simply Optimized 26.1.2-Fabric\nnotes=\n",
        )
        .unwrap();

        let settings = read_cfg(&path);
        let get = |key: &str| {
            settings
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("name"), Some("Simply Optimized 26.1.2-Fabric"));
        assert_eq!(get("iconKey"), Some("default"));
        // An empty value is still a value, and `[General]` is not a key.
        assert_eq!(get("notes"), Some(""));
        assert_eq!(get("General"), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_instance_is_passed_to_the_launcher() {
        let source = Source::Prism {
            instance: "2Modded".into(),
        };
        assert!(source.launch_uri().is_none());
        assert_eq!(source.label(), "Prism");
    }

    #[test]
    fn the_built_in_icon_is_not_mistaken_for_a_file() {
        assert!(instance_icon(Path::new(r"C:\nope"), "default").is_none());
        assert!(instance_icon(Path::new(r"C:\nope"), "").is_none());
    }

    /// Dev aid: prints the real instances on this machine. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on whether Prism Launcher is installed"]
    fn dump_real_prism_instances() {
        for (data_name, exe_stem) in FLAVOURS {
            println!(
                "{data_name}: data {:?}  exe {:?}",
                data_dir(data_name, exe_stem),
                data_dir(data_name, exe_stem).and_then(|d| launcher_exe(exe_stem, &d)),
            );
        }
        for game in discover() {
            println!("{:<34} {:?}", game.name, game.cover_hint);
        }
    }
}
