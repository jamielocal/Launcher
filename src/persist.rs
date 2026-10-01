//! Loading and saving `library.json` under %APPDATA%\Orbit.
//!
//! Writes are debounced: the UI marks the store dirty and the app flushes at
//! most every couple of seconds, plus once on exit.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::model::{Record, Settings};

const FLUSH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Persisted {
    settings: Settings,
    records: BTreeMap<String, Record>,
}

pub struct Store {
    path: PathBuf,
    dirty: bool,
    last_flush: Instant,
}

impl Store {
    /// Reads saved state, falling back to defaults if the file is missing or
    /// unreadable — a corrupt config should never stop the launcher opening.
    pub fn load() -> (Self, Settings, BTreeMap<String, Record>) {
        let path = config_path();
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Persisted>(&s).ok())
            .unwrap_or_default();

        let store = Self {
            path,
            dirty: false,
            last_flush: Instant::now(),
        };
        (store, data.settings, data.records)
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Writes if dirty and the debounce window has elapsed.
    pub fn maybe_flush(&mut self, settings: &Settings, records: &BTreeMap<String, Record>) {
        if self.dirty && self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush(settings, records);
        }
    }

    pub fn flush(&mut self, settings: &Settings, records: &BTreeMap<String, Record>) {
        // Drop records that carry no information so the file stays readable.
        let records: BTreeMap<String, Record> = records
            .iter()
            .filter(|(_, r)| !r.is_default())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let payload = Persisted {
            settings: settings.clone(),
            records,
        };

        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&payload) {
            // Write to a sibling then rename, so an interrupted write cannot
            // leave a half-truncated config behind.
            let tmp = self.path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
        self.dirty = false;
        self.last_flush = Instant::now();
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Orbit").join("library.json")
}
