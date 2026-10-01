//! Games installed by the Xbox app / Microsoft Store into an `XboxGames` folder.
//!
//! These are MSIX-packaged, so running the executable directly fails — the
//! process needs the package identity the shell gives it. Instead each game is
//! started through its Application User Model ID, which is the package family
//! name and the app id from the package manifest joined by `!`.

use std::path::{Path, PathBuf};

use crate::model::{Game, Source};
use crate::sources::xml;

/// Folders inside an `XboxGames` root that are not games.
const NOT_GAMES: &[&str] = &["gamesave", "temp", "$windows.~ws"];

/// Reads every Xbox game across every drive.
pub fn discover() -> Vec<Game> {
    let mut games = Vec::new();
    for root in roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let lower = entry.file_name().to_string_lossy().to_lowercase();
            if NOT_GAMES.contains(&lower.as_str()) {
                continue;
            }
            if let Some(game) = read_install(&dir) {
                if !games.iter().any(|g: &Game| g.id == game.id) {
                    games.push(game);
                }
            }
        }
    }
    games
}

/// `XboxGames` can sit on any fixed drive, and the Xbox app lets you pick which.
fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for letter in 'A'..='Z' {
        let path = PathBuf::from(format!("{letter}:\\XboxGames"));
        if path.is_dir() {
            roots.push(path);
        }
    }
    roots
}

/// Turns one `XboxGames\<Game>` folder into a `Game`.
fn read_install(dir: &Path) -> Option<Game> {
    let content = dir.join("Content");
    let manifest = std::fs::read_to_string(content.join("appxmanifest.xml")).ok()?;

    // The package manifest is authoritative for how the game is started.
    let identity = xml::attribute(&manifest, "Identity", "Name")?;
    // Not always "App": Minecraft's is "Game", its launcher's is "Minecraft".
    let app_id = xml::attribute(&manifest, "Application", "Id")?;
    let family = package_family_name(&identity)?;
    let aumid = format!("{family}!{app_id}");

    // `MicrosoftGame.config` carries the display name and the store artwork.
    let config = ["MicrosoftGame.config", "MicrosoftGame.Config"]
        .iter()
        .find_map(|name| std::fs::read_to_string(content.join(name)).ok())
        .unwrap_or_default();

    let name = xml::attribute(&config, "ShellVisuals", "DefaultDisplayName")
        .or_else(|| xml::attribute(&manifest, "DisplayName", "Name"))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });

    // The exe is only ever used for an icon here, and store artwork beats it.
    let exe = xml::attribute(&config, "Executable", "Name")
        .map(|name| content.join(name))
        .filter(|p| p.is_file());

    Some(Game {
        id: format!("xbox:{identity}"),
        name,
        dir: dir.to_path_buf(),
        exe,
        alternatives: Vec::new(),
        source: Source::Xbox { aumid },
        size_bytes: None,
        store_last_played: None,
        // The game runs out of its own Content folder, so the default watch
        // directory is right.
        watch_dir: None,
        cover_hint: store_logo(&content, &config),
    })
}

/// The largest logo the package ships, which makes a far better tile than an
/// icon prised out of a UWP executable.
fn store_logo(content: &Path, config: &str) -> Option<PathBuf> {
    // Biggest first.
    const KEYS: &[&str] = &[
        "Square480x480Logo",
        "Square150x150Logo",
        "StoreLogo",
        "SplashScreenImage",
        "Square44x44Logo",
    ];
    KEYS.iter()
        .filter_map(|key| xml::attribute(config, "ShellVisuals", key))
        .map(|name| content.join(name))
        .find(|path| path.is_file())
}

/// Resolves a package's family name, which is its identity name plus a hash of
/// the publisher.
///
/// The hash is derivable from the publisher string, but Windows has already done
/// that work: every installed package has a folder named after its family name
/// under `%LOCALAPPDATA%\Packages`, so this matches against those instead of
/// reimplementing the hash.
#[cfg(windows)]
fn package_family_name(identity: &str) -> Option<String> {
    let packages = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Packages");
    let prefix = format!("{identity}_");
    std::fs::read_dir(packages)
        .ok()?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .find(|name| name.starts_with(&prefix))
}

#[cfg(not(windows))]
fn package_family_name(_identity: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_target_is_the_app_user_model_id() {
        let source = Source::Xbox {
            aumid: "Microsoft.MinecraftUWP_8wekyb3d8bbwe!Game".into(),
        };
        // Xbox games are started by the shell, not by us running a file.
        assert!(source.launch_uri().is_none());
        assert_eq!(source.label(), "Xbox");
    }

    /// Dev aid: prints the real Xbox library on this machine. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the machine's own Xbox installs"]
    fn dump_real_xbox_library() {
        println!("roots: {:?}", roots());
        for game in discover() {
            println!(
                "{:<26} {:?}\n   art {:?}",
                game.name,
                match &game.source {
                    Source::Xbox { aumid } => aumid.clone(),
                    _ => String::new(),
                },
                game.cover_hint.as_ref().and_then(|p| p.file_name()),
            );
        }
    }
}
