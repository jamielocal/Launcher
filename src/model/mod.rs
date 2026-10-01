//! What a game is, where it came from, how it is launched, and the library that
//! holds them.

mod game;
mod library;
mod settings;
mod source;

pub use game::{Game, Record};
pub use library::{Library, SOURCE_KINDS};
pub use settings::{Filter, Settings, Sort};
pub use source::{LaunchSpec, Source, SourceKind};

use std::path::Path;

/// Stable identity for a local game: the lowercased folder name. Survives the
/// library being moved to another drive, which a full-path key would not.
pub fn game_id(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| dir.to_string_lossy().to_lowercase())
}

/// A filename-safe form of a game id, for naming cover images.
///
/// Store ids look like `steam:730`, and Windows cannot put a colon in a
/// filename — it would be read as an alternate data stream — so every character
/// that is not safe in a name becomes a dash.
pub fn file_key(id: &str) -> String {
    id.chars()
        .map(|c| match c {
            c if c.is_alphanumeric() => c,
            '-' | '_' | '.' | ' ' => c,
            _ => '-',
        })
        .collect()
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_filenames_drop_characters_windows_rejects() {
        assert_eq!(file_key("steam:730"), "steam-730");
        assert_eq!(file_key("epic:0f0f0f0f"), "epic-0f0f0f0f");
        // Local ids are folder names and usually already safe.
        assert_eq!(file_key("beamng.drive"), "beamng.drive");
        assert_eq!(file_key("pc building sim 2"), "pc building sim 2");
    }
}
