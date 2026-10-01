//! Discovers games by walking each library folder's immediate subfolders and
//! scoring the executables inside, because a game folder typically contains
//! several `.exe` files and only one of them is the game.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{game_id, Game, Record, Source};

/// How deep below a game folder we look for executables. Unreal titles bury
/// theirs in `Binaries/Win64`, which is depth 3.
const MAX_DEPTH: usize = 3;

/// Substrings that mean "this is never the game". Matched against the
/// lowercased file stem.
const BLOCKED: &[&str] = &[
    "crashhandler",
    "crashreport",
    "crashpad",
    "crashsender",
    "crashreporter",
    "vc_redist",
    "vcredist",
    "dxsetup",
    "directx",
    "dotnetfx",
    "oalinst",
    "redist",
    "uninstall",
    "unins000",
    "setup",
    "installer",
    "support",
    "elevate",
    "dowser",
    "notification_helper",
    "squirrel",
    "modtest",
    "benchmark",
    "editor",
    "dedicated",
    "epicwebhelper",
    "steamservice",
    "touchup",
    "perf_graph",
    "console",
    "python",
    "ffmpeg",
    "7z",
];

/// Folder names that are never games.
const SKIP_DIRS: &[&str] = &[
    "redist",
    "_commonredist",
    "directx",
    "$recycle.bin",
    "system volume information",
];

/// Strip everything but letters and digits so "My Summer Car" and
/// "mysummercar.exe" compare equal.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn is_blocked(stem_lower: &str) -> bool {
    BLOCKED.iter().any(|bad| stem_lower.contains(bad))
}

/// Uppercase initials plus any digits: "PC Building Sim 2" -> "pcbs2".
fn initials(name: &str) -> String {
    let mut out = String::new();
    for word in name.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        if word.chars().all(|c| c.is_numeric()) {
            out.push_str(word);
        } else if let Some(c) = word.chars().next() {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// Scores one executable as "probably the game". Higher is better; `None`
/// rejects the candidate outright.
fn score(exe: &Path, game_dir: &Path, depth: usize, size: u64) -> Option<i64> {
    let stem = exe.file_stem()?.to_string_lossy().to_lowercase();
    if is_blocked(&stem) {
        return None;
    }

    let folder = game_dir.file_name()?.to_string_lossy().into_owned();
    let folder_norm = normalize(&folder);
    let stem_norm = normalize(&stem);
    let mut points: i64 = 0;

    // Name agreement with the folder is by far the strongest signal.
    if stem_norm == folder_norm {
        points += 1000;
    } else if folder_norm.starts_with(&stem_norm) || stem_norm.starts_with(&folder_norm) {
        points += 600;
    } else if initials(&folder) == stem_norm {
        points += 500;
    } else if !stem_norm.is_empty() && folder_norm.contains(&stem_norm) {
        points += 400;
    }

    // Executables at the root of the game folder are usually the entry point;
    // deeply nested ones are usually engine plumbing.
    points += match depth {
        0 => 300,
        1 => 120,
        _ => 0,
    };

    // A real game binary is rarely tiny.
    points += match size {
        s if s > 64 * 1024 * 1024 => 140,
        s if s > 8 * 1024 * 1024 => 90,
        s if s > 1024 * 1024 => 40,
        _ => 0,
    };

    // Common engine entry-point naming.
    if stem.ends_with("-shipping") {
        points += 220;
    }
    if stem.contains("steam") {
        points += 60;
    }
    // A launcher is a plausible fallback, but loses to the game itself.
    if stem.contains("launcher") {
        points -= 250;
    }

    Some(points)
}

fn collect_exes(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, usize, u64)>) {
    if depth > MAX_DEPTH || out.len() > 400 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            let lower = entry.file_name().to_string_lossy().to_lowercase();
            if !SKIP_DIRS.contains(&lower.as_str()) {
                collect_exes(&path, depth + 1, out);
            }
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            out.push((path, depth, meta.len()));
        }
    }
}

/// Scans every library path and returns the games found, name-sorted.
pub fn scan(library_paths: &[PathBuf], records: &BTreeMap<String, Record>) -> Vec<Game> {
    let mut games: Vec<Game> = Vec::new();

    for root in library_paths {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                continue;
            }
            let lower = entry.file_name().to_string_lossy().to_lowercase();
            if SKIP_DIRS.contains(&lower.as_str()) || lower.starts_with('.') {
                continue;
            }
            // Don't list the launcher's own source folder as a game.
            if dir.join("Cargo.toml").is_file() {
                continue;
            }
            if let Some(game) = inspect(&dir, records) {
                // A folder reachable from two library paths should appear once.
                if !games.iter().any(|g| g.id == game.id) {
                    games.push(game);
                }
            }
        }
    }

    games.sort_by_key(|g| g.name.to_lowercase());
    games
}

/// Picks the most likely game executable in a folder, plus everything else
/// found there as alternatives. Used both for local games and to give store
/// games an icon to display.
pub fn best_executable(dir: &Path) -> (Option<PathBuf>, Vec<PathBuf>) {
    let mut found = Vec::new();
    collect_exes(dir, 0, &mut found);
    if found.is_empty() {
        return (None, Vec::new());
    }

    let mut ranked: Vec<(i64, PathBuf)> = found
        .iter()
        .filter_map(|(path, depth, size)| {
            score(path, dir, *depth, *size).map(|s| (s, path.clone()))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    // Everything was blocked: fall back to the largest executable rather than
    // pretending the folder holds no game.
    let best = match ranked.first() {
        Some((_, path)) => path.clone(),
        None => match found.iter().max_by_key(|(_, _, size)| *size) {
            Some((path, _, _)) => path.clone(),
            None => return (None, Vec::new()),
        },
    };

    let mut alternatives: Vec<PathBuf> = found
        .iter()
        .map(|(p, _, _)| p.clone())
        .filter(|p| *p != best)
        .collect();
    alternatives.sort();
    (Some(best), alternatives)
}

/// Turns one folder into a `Game`, or `None` if it holds no plausible game.
fn inspect(dir: &Path, records: &BTreeMap<String, Record>) -> Option<Game> {
    let (best, mut alternatives) = best_executable(dir);
    let best = best?;

    let id = game_id(dir);
    let record = records.get(&id);
    let folder_name = dir.file_name()?.to_string_lossy().into_owned();

    // An override always wins, but only while it still exists on disk.
    let exe = record
        .and_then(|r| r.custom_exe.clone())
        .filter(|p| p.is_file())
        .unwrap_or(best.clone());
    if exe != best {
        // The scanner's own pick becomes an alternative so it can be chosen back.
        alternatives.retain(|p| *p != exe);
        if !alternatives.contains(&best) {
            alternatives.push(best);
            alternatives.sort();
        }
    }

    Some(Game {
        id,
        name: record
            .and_then(|r| r.custom_name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(folder_name),
        dir: dir.to_path_buf(),
        exe: Some(exe),
        alternatives,
        source: Source::Local,
        size_bytes: None,
        store_last_played: None,
        watch_dir: None,
        cover_hint: None,
    })
}

/// Recursive byte count for a folder, used by the background size walk.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => stack.push(entry.path()),
                Ok(meta) => total += meta.len(),
                Err(_) => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Builds a throwaway folder tree of empty files for the scanner to chew on.
    fn fixture(name: &str, files: &[(&str, usize)]) -> PathBuf {
        let root = std::env::temp_dir()
            .join("orbit-scanner-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (relative, size) in files {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, vec![0u8; *size]).unwrap();
        }
        root
    }

    fn pick(dir: &Path) -> String {
        inspect(dir, &BTreeMap::new())
            .expect("a game should have been detected")
            .exe
            .expect("an executable should have been chosen")
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn prefers_the_name_matching_executable_over_helpers() {
        let root = fixture(
            "teardown",
            &[
                ("Teardown/teardown.exe", 4096),
                ("Teardown/crashpad_handler.exe", 8192),
                ("Teardown/teardown_modtest.exe", 8192),
            ],
        );
        assert_eq!(pick(&root.join("Teardown")), "teardown.exe");
    }

    #[test]
    fn matches_a_name_with_spaces_against_a_squashed_filename() {
        let root = fixture("msc", &[("My Summer Car/mysummercar.exe", 4096)]);
        assert_eq!(pick(&root.join("My Summer Car")), "mysummercar.exe");
    }

    #[test]
    fn matches_initials_when_the_filename_is_an_acronym() {
        let root = fixture(
            "pcbs",
            &[
                ("PC Building Sim 2/PCBS2.exe", 4096),
                ("PC Building Sim 2/UnityCrashHandler64.exe", 999_999),
            ],
        );
        assert_eq!(pick(&root.join("PC Building Sim 2")), "PCBS2.exe");
    }

    #[test]
    fn prefers_a_root_executable_over_a_nested_duplicate() {
        let root = fixture(
            "beam",
            &[
                ("BeamNG.drive/BeamNG.drive.exe", 4096),
                ("BeamNG.drive/Bin64/BeamNG.drive.x64.exe", 900_000),
                ("BeamNG.drive/BeamMP-Launcher.exe", 500_000),
                ("BeamNG.drive/support.exe", 4096),
            ],
        );
        assert_eq!(pick(&root.join("BeamNG.drive")), "BeamNG.drive.exe");
    }

    #[test]
    fn finds_unreal_shipping_binaries_nested_under_binaries() {
        let root = fixture(
            "ue",
            &[(
                "Some Game/Binaries/Win64/SomeGame-Win64-Shipping.exe",
                80 * 1024 * 1024,
            )],
        );
        assert_eq!(pick(&root.join("Some Game")), "SomeGame-Win64-Shipping.exe");
    }

    #[test]
    fn falls_back_to_the_largest_executable_when_every_name_is_blocked() {
        let root = fixture(
            "blocked",
            &[("Odd/setup.exe", 1024), ("Odd/installer.exe", 90_000)],
        );
        assert_eq!(pick(&root.join("Odd")), "installer.exe");
    }

    /// Dev aid: prints what the scanner makes of the real library on this
    /// machine. Run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the machine's own game library"]
    fn dump_real_library() {
        let paths = crate::model::Settings::default().library_paths;
        println!("scanning {paths:?}");
        for game in scan(&paths, &BTreeMap::new()) {
            println!(
                "{:<22} -> {}",
                game.name,
                game.exe
                    .as_deref()
                    .map(|e| e.strip_prefix(&game.dir).unwrap_or(e).display().to_string())
                    .unwrap_or_else(|| "(none)".into())
            );
        }
    }

    #[test]
    fn ignores_folders_without_executables() {
        let root = fixture("docs", &[("Notes/readme.txt", 10)]);
        assert!(inspect(&root.join("Notes"), &BTreeMap::new()).is_none());
    }
}
