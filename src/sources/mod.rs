//! The launchers Orbit reads, one module each.
//!
//! Every one of these is read straight off disk rather than through any API:
//! Steam's app manifests, Epic's install manifests, the Xbox app's package
//! manifests, Roblox's versioned install folder and Prism's instance configs are
//! the same files the launchers themselves rely on. So what Orbit lists is
//! exactly what is installed, with no sign-in and nothing to keep in sync.

pub mod epic;
pub mod prism;
pub mod roblox;
pub mod shortcut;
pub mod steam;
pub mod vdf;
pub mod xbox;
pub mod xml;

use crate::model::{Game, Settings};

/// Collects games from every enabled source.
pub fn discover(settings: &Settings) -> Vec<Game> {
    let mut games = Vec::new();
    if settings.scan_steam {
        games.extend(steam::discover());
    }
    if settings.scan_epic {
        games.extend(epic::discover());
    }
    if settings.scan_xbox {
        games.extend(xbox::discover());
    }
    if settings.scan_roblox {
        games.extend(roblox::discover());
    }
    if settings.scan_prism {
        games.extend(prism::discover());
    }
    games
}
