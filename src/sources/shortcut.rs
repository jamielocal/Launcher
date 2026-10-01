//! Resolving Windows `.lnk` shortcuts to the program they point at.
//!
//! Some launchers are installed as a portable folder with no registry trace at
//! all, which leaves their Start Menu shortcut as the only reliable record of
//! where they went. Reading that is far better than guessing at install paths or
//! walking the whole disk.

use std::path::{Path, PathBuf};

/// The folders Windows keeps program shortcuts in.
fn shortcut_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut push = |base: Option<std::ffi::OsString>, tail: &[&str]| {
        if let Some(base) = base {
            let mut path = PathBuf::from(base);
            for part in tail {
                path.push(part);
            }
            if path.is_dir() {
                roots.push(path);
            }
        }
    };
    push(
        std::env::var_os("ProgramData"),
        &["Microsoft", "Windows", "Start Menu", "Programs"],
    );
    push(
        std::env::var_os("APPDATA"),
        &["Microsoft", "Windows", "Start Menu", "Programs"],
    );
    push(std::env::var_os("USERPROFILE"), &["Desktop"]);
    roots
}

/// Finds the program a shortcut named like `stem` points at.
///
/// Matching is loose — "Prism Launcher.lnk" is found by `prismlauncher` as well
/// as by `prism launcher` — because a shortcut's name is chosen by whoever made
/// the installer and does not have to match the executable.
pub fn find_target(stem: &str) -> Option<PathBuf> {
    let needle: String = stem
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();

    for root in shortcut_roots() {
        if let Some(found) = search(&root, &needle, 0) {
            return Some(found);
        }
    }
    None
}

/// Start Menu folders are shallow, so two levels is plenty.
fn search(dir: &Path, needle: &str, depth: usize) -> Option<PathBuf> {
    if depth > 2 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
        {
            continue;
        }
        let name: String = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if name == needle {
            if let Some(target) = resolve(&path) {
                if target.is_file() {
                    return Some(target);
                }
            }
        }
    }

    // Exact matches anywhere in this tree beat a near miss, so recurse only
    // after checking every shortcut at this level.
    subdirs
        .into_iter()
        .find_map(|sub| search(&sub, needle, depth + 1))
}

/// Reads a shortcut's target path through the shell's own shortcut object,
/// which handles every `.lnk` variant Windows produces.
#[cfg(windows)]
pub fn resolve(lnk: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let wide: Vec<u16> = lnk
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        // The scan runs on its own thread, so this is the apartment's first
        // initialisation. A failure here usually means it was already set up
        // with a different model, which is still fine for us.
        let initialised = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();

        let target = (|| {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let file: IPersistFile = link.cast().ok()?;
            file.Load(PCWSTR(wide.as_ptr()), STGM_READ).ok()?;

            let mut buf = [0u16; 1024];
            let mut find = WIN32_FIND_DATAW::default();
            link.GetPath(&mut buf, &mut find, 0).ok()?;

            let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            (end > 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..end])))
        })();

        // Everything above is dropped by now, so the apartment can go.
        if initialised {
            CoUninitialize();
        }
        target
    }
}

#[cfg(not(windows))]
pub fn resolve(_lnk: &Path) -> Option<PathBuf> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_missing_shortcut_resolves_to_nothing() {
        assert!(resolve(Path::new(r"C:\nope\nothing-here.lnk")).is_none());
        assert!(find_target("a launcher that is not installed").is_none());
    }

    /// Dev aid: prints what the Start Menu says about a few launchers. Run with
    /// `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on what is installed on this machine"]
    fn dump_shortcut_lookups() {
        for name in ["Prism Launcher", "MultiMC", "Roblox Player", "Steam"] {
            println!("{name:<16} -> {:?}", find_target(name));
        }
    }
}
