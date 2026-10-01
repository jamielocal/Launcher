# Orbit

A small, fast game launcher for everything installed on one machine. Written in
Rust with [egui](https://github.com/emilk/egui) — one 7 MB executable, no
runtime, no installer, no account.

It finds games six ways: folders you point it at, Steam, the Epic Games
Launcher, the Xbox app, Roblox, and Prism Launcher's Minecraft instances. It
works out how each one has to be started, finds the best artwork available for
it, and tracks how long you play.

## Running it

```powershell
cargo run --release
```

The built binary is `target\release\orbit.exe` and can be copied or shortcut
anywhere — it keeps its state in `%APPDATA%\Orbit\library.json`, not next to
itself.

## Where games come from

Every source is read straight off disk. Nothing signs in, nothing phones home,
and there is no index to keep in sync: the files Orbit reads are the same ones
the launchers themselves rely on, so what it lists is what is installed right
now. Each source can be switched off in the sidebar.

**Folders.** Every immediate subfolder of a library folder is a candidate, and
Orbit scores the executables inside it, up to three levels deep. The score
favours, in rough order of weight:

- a filename matching the folder name once punctuation and case are stripped, so
  `My Summer Car\mysummercar.exe` matches, as does an acronym like
  `PC Building Sim 2\PCBS2.exe`
- sitting at the root of the game folder rather than buried in `Bin64` or
  `Binaries\Win64`
- being a reasonably large file
- Unreal's `-Shipping` naming

Known non-games — crash handlers, redistributables, uninstallers, mod tools,
`dowser.exe`, `elevate.exe` and friends — are rejected outright, and anything
with `launcher` in the name is demoted so the game itself wins.

**Steam.** Found through `HKCU\Software\Valve\Steam\SteamPath`. Orbit reads
`steamapps\libraryfolders.vdf` for every library folder, then one
`appmanifest_<appid>.acf` per installed app. Reading the manifests is much more
reliable than listing `steamapps\common`, which also holds shared
redistributables and leftovers from uninstalls. Entries without the
"fully installed" state flag are skipped, as are runtimes and redistributables.
Steam reports its own install sizes and last-played times, so Orbit uses those.

**Epic.** The JSON `.item` manifests in
`%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests`, keeping complete installs
that Epic itself categorises as games. DLC and add-ons point at a parent game and
are skipped.

**Xbox.** Every `XboxGames` folder on every drive. Each game's
`Content\appxmanifest.xml` gives the package identity and app id, and
`MicrosoftGame.config` gives the display name and the store artwork. These games
are MSIX-packaged, so running the executable directly fails — the process needs
the package identity the shell gives it.

**Roblox.** The newest version folder under `%LOCALAPPDATA%\Roblox\Versions`
holding `RobloxPlayerBeta.exe`, and Studio separately if it is installed. Roblox
leaves old versions behind after an update, so the newest is picked by
modification time and re-resolved on every scan.

**Prism Launcher.** Each instance under `%APPDATA%\PrismLauncher\instances` is
listed on its own, because an instance — a modpack at a particular Minecraft
version — is what you actually pick when you sit down to play. MultiMC uses the
same layout and is read the same way. Prism is often installed as a portable
folder with no registry trace, so when it is not in a standard location its Start
Menu shortcut is resolved through the shell to find it.

If a store game's folder also happens to sit inside a scanned library folder, the
store entry wins — it knows the real name, the size, and how to launch it.

## How games are launched

| Source | How |
| --- | --- |
| Folders, Roblox | The executable is run directly, with its own folder as the working directory |
| Steam | `steam://rungameid/<appid>` |
| Epic | `com.epicgames.launcher://apps/…?action=launch` |
| Xbox | `explorer.exe shell:AppsFolder\<package family>!<app id>` |
| Prism | `prismlauncher.exe --launch <instance>` |

Store games are handed to their store rather than run directly so the overlay,
cloud saves, DRM and updates all keep working. Xbox games have no alternative:
the app id is not always `App` — Minecraft for Windows uses `Game` and the
Minecraft Launcher uses `Minecraft` — which is why the manifest is read rather
than guessed at.

If a folder-scanned game launches the wrong thing, open it in the details panel
and pick a different executable, either from the dropdown of everything found in
that folder or via **Browse…**. The choice is remembered and wins over the
scanner from then on. The other sources manage their own launch target, so the
picker is hidden for them and the panel says why.

## Playtime

Watching the process Orbit started is not enough. Local games often ship a thin
bootstrapper that starts the real binary and exits; Steam may spend a minute
updating before the game appears; an Xbox game is started by the shell and a
Prism instance runs under the launcher's bundled Java. So the real signal is
"is any process running under this game's folder", and the clock runs from the
moment one is first seen until it goes away, with a short grace period in case the
game restarts itself into another binary.

Because a couple of sources do not run their game out of its own install folder,
each game can nominate a different folder to watch: Prism instances watch Prism's
data directory, and Roblox watches its install root rather than the version
folder it happened to launch, since an update moves it.

Sessions shorter than ten seconds are reported but not counted, since they almost
always mean the game failed to start. Cards show a pulsing "Starting…" while
Orbit is still waiting for the game to appear.

## Cover art

In order of preference, each game shows:

1. `<game id>.png` (or `.jpg` / `.webp`) in `%APPDATA%\Orbit\covers\` — the id is
   shown in the details panel, e.g. `teardown.png` or `steam-730.png`
2. artwork its source already pointed at: a packaged Xbox game's store logo, or a
   Prism instance's custom icon
3. a `cover` / `poster` / `grid` / `header` / `capsule` / `library` image in the
   game's own folder
4. the largest `.ico` in the game's folder
5. the icon embedded in the game's executable, extracted at up to 256×256

Covers fill the tile; icons and logos are centred on a gradient generated from
the game's name. Those gradients are confined to the violet band of the palette
and vary only in lightness and saturation, so a wall of generated tiles reads as
shades of one colour rather than a colour wheel. The details panel has a shortcut
to open the covers folder.

## Controls

| Action | How |
| --- | --- |
| Play | Double-click a card, or hover it and hit the play button |
| Details | Single-click a card; click it again or press `Esc` to close |
| Play selected | `Enter` |
| Search | `/` or `Ctrl+F`; `Esc` clears |
| Rescan | `F5` |
| Per-game menu | Right-click a card |

Favourites always sort to the front, whatever sort order is selected. Hidden
games stay out of the library until **Show hidden games** is ticked.

## Tests

```powershell
cargo test
```

Covered: the executable-scoring heuristics, the KeyValues and XML readers, each
source's filtering and launch-target construction, and the merged library's
invariants — every game has a way to launch, nothing is listed twice, and every
source that contributed games can be switched off again.

Nine further tests print what Orbit makes of *this* machine's real library. They
are ignored by default because they depend on what is installed:

```powershell
cargo test -- --ignored --nocapture
```

## Layout

```
src/
  main.rs            entry point and the module map
  app.rs             all mutable state; the only place it changes
  discovery.rs       scan the folders, fold in every enabled source
  model/             what a game is, how it launches, how the library is filtered
    source.rs          where a game came from and the plan for starting it
    game.rs            one installed game, and the state kept between runs
    settings.rs        user settings and view options
    library.rs         the collection, its filtering and its sorting
  scanner.rs         folder scanning and executable scoring
  sources/           one module per launcher Orbit reads
    steam.rs           Steam libraries and app manifests
    epic.rs            Epic install manifests
    xbox.rs            Xbox packages and their AUMIDs
    roblox.rs          Roblox's versioned install
    prism.rs           Prism Launcher / MultiMC instances
    vdf.rs             minimal reader for Valve's KeyValues format
    xml.rs             minimal attribute reader for the GDK's manifests
    shortcut.rs        resolving .lnk shortcuts through the shell
  runner.rs          launching, and measuring how long a game ran
  art.rs             cover lookup, Win32 icon extraction, texture cache
  persist.rs         debounced, atomic library.json
  theme.rs           palette, egui style, window icon, formatting
  ui/                drawing only
    chrome.rs          header and status bar
    sidebar.rs         search, filters, sources, folders, options
    grid.rs            the wall of game cards
    details.rs         the selected game's drawer
    widgets.rs         shared painting helpers
```

Two rules hold the shape together.

**Drawing never mutates.** Each panel pushes `Action`s, and every one is applied
in `App::apply` after the frame is laid out. State transitions live in one
readable list, and the borrow graph stays trivial.

**A source only has to answer three questions.** Adding one means writing a
`discover() -> Vec<Game>` function, a `Source` variant saying how to launch it,
and a line in `sources::discover`. Nothing else in the app needs to know it
exists — the sidebar chips, badges, filters and totals all derive from
`SOURCE_KINDS`.

`sources/` (the launchers) and `persist.rs` (the settings file) were once both
called "store", which was a reliable way to open the wrong file.

The palette runs from near-black to dark purple, with every surface a step along
that same ramp rather than a neutral grey, so the window reads as one material
lit from a single direction. It lives entirely in `src/theme.rs`.

Windows only, because of the icon extraction, registry reads, shortcut resolution
and process scanning. Each is behind `cfg(windows)` and stubs out cleanly, so the
rest would port.
