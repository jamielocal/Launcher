//! Launching games and measuring how long they were played.
//!
//! Watching the process we spawned is not enough. Local games often ship a thin
//! bootstrapper that starts the real binary and exits, and store games are not
//! started by us at all — we hand a URI to Steam or Epic, which may need to
//! start up, patch, and show a dialog before the game appears. So the real
//! signal is "is any process running out of this game's folder", and the clock
//! runs from the moment we first see one.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::model::LaunchSpec;

/// How long we wait for a game we started directly to show up.
const DIRECT_STARTUP_GRACE: Duration = Duration::from_secs(20);
/// How long we wait for a store to get around to starting the game. Steam can
/// easily spend a minute or two updating first.
const STORE_STARTUP_GRACE: Duration = Duration::from_secs(300);
/// Once the game has been seen, how long it may vanish for before we call it
/// quits. Covers a game restarting itself into a different binary.
const HANDOFF_GRACE: Duration = Duration::from_secs(15);
/// Process scans are not free, so rate-limit them.
const PROBE_INTERVAL: Duration = Duration::from_secs(2);
/// Sessions shorter than this are assumed to be a failed start, not play time.
const MIN_SESSION: Duration = Duration::from_secs(10);

struct Session {
    id: String,
    /// Any process running under here counts as "the game is up".
    dir: PathBuf,
    /// The process we spawned, while it is still alive. `None` for URI launches.
    child: Option<Child>,
    requested: Instant,
    /// When we first saw the game actually running.
    play_started: Option<Instant>,
    /// When the game stopped being visible, if it has.
    vanished_at: Option<Instant>,
    startup_grace: Duration,
}

/// A session that just ended.
pub struct Finished {
    pub id: String,
    pub secs: u64,
    /// The game never appeared, or appeared only briefly — usually a failed
    /// start rather than something that was played.
    pub suspicious: bool,
}

#[derive(Default)]
pub struct Runner {
    sessions: Vec<Session>,
    last_probe: Option<Instant>,
}

impl Runner {
    /// Starts a game.
    ///
    /// `watch_dir` is the folder whose running processes mean the game is up,
    /// which is not always the folder it was installed into.
    pub fn launch(&mut self, id: &str, watch_dir: &Path, launch: LaunchSpec) -> Result<(), String> {
        if self.is_running(id) {
            return Err("That game is already running.".into());
        }

        let (child, startup_grace) = match launch {
            LaunchSpec::Exe { exe, args } => {
                if !exe.is_file() {
                    return Err(format!("Executable not found: {}", exe.display()));
                }
                // Most games assume their own folder is the working directory.
                let workdir = exe.parent().unwrap_or(watch_dir).to_path_buf();
                let child = Command::new(&exe)
                    .args(&args)
                    .current_dir(workdir)
                    .spawn()
                    .map_err(|e| format!("Could not start {}: {e}", exe.display()))?;
                (Some(child), DIRECT_STARTUP_GRACE)
            }
            LaunchSpec::Uri(uri) => {
                open::that_detached(&uri)
                    .map_err(|e| format!("Could not hand off to the store: {e}"))?;
                (None, STORE_STARTUP_GRACE)
            }
            LaunchSpec::Handoff { program, args } => {
                // The helper exits as soon as it has asked for the game, so
                // there is nothing of ours to watch and the game itself may be
                // slow to appear.
                Command::new(&program)
                    .args(&args)
                    .spawn()
                    .map_err(|e| format!("Could not start {}: {e}", program.display()))?;
                (None, STORE_STARTUP_GRACE)
            }
        };

        self.sessions.push(Session {
            id: id.to_string(),
            dir: watch_dir.to_path_buf(),
            child,
            requested: Instant::now(),
            play_started: None,
            vanished_at: None,
            startup_grace,
        });
        Ok(())
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.sessions.iter().any(|s| s.id == id)
    }

    /// True while we are still waiting for a game to appear after launching it.
    pub fn is_starting(&self, id: &str) -> bool {
        self.sessions
            .iter()
            .any(|s| s.id == id && s.play_started.is_none())
    }

    pub fn any_running(&self) -> bool {
        !self.sessions.is_empty()
    }

    /// Seconds the game has actually been up, once it has been seen.
    pub fn elapsed(&self, id: &str) -> Option<u64> {
        self.sessions
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.play_started)
            .map(|t| t.elapsed().as_secs())
    }

    /// Reaps finished sessions. Call once per frame; the expensive process scan
    /// is internally throttled.
    pub fn poll(&mut self) -> Vec<Finished> {
        let mut finished = Vec::new();
        if self.sessions.is_empty() {
            return finished;
        }

        // Cheap pass: a child we spawned and that is still alive tells us the
        // game is up without needing a scan.
        for session in &mut self.sessions {
            if let Some(child) = &mut session.child {
                match child.try_wait() {
                    Ok(None) => {
                        session.play_started.get_or_insert_with(Instant::now);
                        session.vanished_at = None;
                    }
                    // Exited, or we can no longer ask: fall back to scanning.
                    Ok(Some(_)) | Err(_) => session.child = None,
                }
            }
        }

        let due = self
            .last_probe
            .map(|t| t.elapsed() >= PROBE_INTERVAL)
            .unwrap_or(true);
        if !due {
            return finished;
        }
        self.last_probe = Some(Instant::now());

        let running = running_process_paths();
        let now = Instant::now();

        self.sessions.retain_mut(|session| {
            let visible =
                session.child.is_some() || running.iter().any(|p| p.starts_with(&session.dir));

            if visible {
                session.play_started.get_or_insert(now);
                session.vanished_at = None;
                return true;
            }

            match session.play_started {
                // Seen before, gone now: allow a short gap for a handoff.
                Some(started) => {
                    let since = *session.vanished_at.get_or_insert(now);
                    if now.duration_since(since) < HANDOFF_GRACE {
                        return true;
                    }
                    // Don't count the grace period itself as play time.
                    let played = since.duration_since(started);
                    finished.push(Finished {
                        id: session.id.clone(),
                        secs: played.as_secs(),
                        suspicious: played < MIN_SESSION,
                    });
                    false
                }
                // Never seen: keep waiting until the startup grace runs out.
                None => {
                    if session.requested.elapsed() < session.startup_grace {
                        return true;
                    }
                    finished.push(Finished {
                        id: session.id.clone(),
                        secs: 0,
                        suspicious: true,
                    });
                    false
                }
            }
        });

        finished
    }
}

/// Full image paths of every process we are allowed to inspect.
#[cfg(windows)]
fn running_process_paths() -> Vec<PathBuf> {
    use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut paths = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return paths;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                // Most processes are not ours; asking only for limited info
                // keeps this working without elevation.
                if let Ok(handle) = OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    entry.th32ProcessID,
                ) {
                    let mut buf = [0u16; MAX_PATH as usize];
                    let mut len = buf.len() as u32;
                    if QueryFullProcessImageNameW(
                        handle,
                        PROCESS_NAME_FORMAT(0),
                        windows::core::PWSTR(buf.as_mut_ptr()),
                        &mut len,
                    )
                    .is_ok()
                    {
                        paths.push(PathBuf::from(String::from_utf16_lossy(
                            &buf[..len as usize],
                        )));
                    }
                    let _ = CloseHandle(handle);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    paths
}

#[cfg(not(windows))]
fn running_process_paths() -> Vec<PathBuf> {
    Vec::new()
}
