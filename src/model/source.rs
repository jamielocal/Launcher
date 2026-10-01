//! Where a game came from, and the plan for starting it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where a game came from, and what it takes to start it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Found by scanning a folder; launched by running its executable.
    Local,
    /// Found in a Steam library; launched through Steam so that the overlay,
    /// cloud saves and DRM all behave as they normally would.
    Steam { app_id: String },
    /// Found in an Epic Games Launcher manifest; launched through Epic for the
    /// same reason.
    Epic {
        app_name: String,
        namespace: String,
        catalog_item_id: String,
    },
    /// Installed by the Xbox app. MSIX-packaged, so it has to be started by the
    /// shell through its Application User Model ID rather than by running the
    /// executable, which would fail without the package's identity.
    Xbox { aumid: String },
    /// Roblox, which is simply run from its versioned install folder.
    Roblox,
    /// One Prism Launcher (or MultiMC) instance, started by handing its name to
    /// the launcher.
    Prism { instance: String },
}

/// How a game is actually started.
pub enum LaunchSpec {
    /// Run this ourselves. The process is ours to watch.
    Exe { exe: PathBuf, args: Vec<String> },
    /// Ask the shell to open a URI and let the owning store take over.
    Uri(String),
    /// Run a helper that starts the game and exits immediately, so there is no
    /// process of ours to watch and the game may take a while to appear.
    Handoff { program: PathBuf, args: Vec<String> },
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::Local => "Local",
            Source::Steam { .. } => "Steam",
            Source::Epic { .. } => "Epic",
            Source::Xbox { .. } => "Xbox",
            Source::Roblox => "Roblox",
            Source::Prism { .. } => "Prism",
        }
    }

    pub fn kind(&self) -> SourceKind {
        match self {
            Source::Local => SourceKind::Local,
            Source::Steam { .. } => SourceKind::Steam,
            Source::Epic { .. } => SourceKind::Epic,
            Source::Xbox { .. } => SourceKind::Xbox,
            Source::Roblox => SourceKind::Roblox,
            Source::Prism { .. } => SourceKind::Prism,
        }
    }

    /// Works out how to start this game. `exe` is the game's launch target,
    /// which only the sources that run something themselves need.
    pub fn launch_spec(&self, exe: Option<&Path>) -> Option<LaunchSpec> {
        let run = |args: Vec<String>| {
            exe.map(|exe| LaunchSpec::Exe {
                exe: exe.to_path_buf(),
                args,
            })
        };
        match self {
            Source::Local | Source::Roblox => run(Vec::new()),
            Source::Prism { instance } => run(vec!["--launch".into(), instance.clone()]),
            Source::Steam { .. } | Source::Epic { .. } => self.launch_uri().map(LaunchSpec::Uri),
            Source::Xbox { aumid } => Some(LaunchSpec::Handoff {
                // Explorer is the documented way to activate a packaged app by
                // its AUMID without going through WinRT.
                program: PathBuf::from("explorer.exe"),
                args: vec![format!(r"shell:AppsFolder\{aumid}")],
            }),
        }
    }

    /// The URI that asks the owning store to start the game, for the sources
    /// that work that way.
    pub fn launch_uri(&self) -> Option<String> {
        match self {
            Source::Steam { app_id } => Some(format!("steam://rungameid/{app_id}")),
            Source::Epic {
                app_name,
                namespace,
                catalog_item_id,
            } => Some(format!(
                "com.epicgames.launcher://apps/{namespace}%3A{catalog_item_id}%3A{app_name}?action=launch&silent=true",
            )),
            _ => None,
        }
    }

    /// A link to the game's store page, for the context menu.
    pub fn store_page(&self) -> Option<String> {
        match self {
            Source::Steam { app_id } => {
                Some(format!("https://store.steampowered.com/app/{app_id}"))
            }
            Source::Roblox => Some("https://www.roblox.com/home".into()),
            _ => None,
        }
    }

    /// Whether the user may choose a different executable. Only meaningful where
    /// we run the game ourselves and nothing else depends on which binary it is.
    pub fn exe_is_editable(&self) -> bool {
        matches!(self, Source::Local)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Local,
    Steam,
    Epic,
    Xbox,
    Roblox,
    Prism,
}
