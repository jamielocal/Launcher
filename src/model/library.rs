//! The scanned games plus their saved records, and the filtering and sorting
//! that turns them into what the grid shows.

use std::collections::BTreeMap;

use super::{Filter, Game, Record, Settings, Sort, SourceKind};

/// Every source kind, in the order the sidebar lists them.
pub const SOURCE_KINDS: [SourceKind; 6] = [
    SourceKind::Local,
    SourceKind::Steam,
    SourceKind::Epic,
    SourceKind::Xbox,
    SourceKind::Roblox,
    SourceKind::Prism,
];

/// The scanned games plus their persisted records.
#[derive(Default)]
pub struct Library {
    pub games: Vec<Game>,
    pub records: BTreeMap<String, Record>,
}

impl Library {
    pub fn record(&self, id: &str) -> Record {
        self.records.get(id).cloned().unwrap_or_default()
    }

    pub fn record_mut(&mut self, id: &str) -> &mut Record {
        self.records.entry(id.to_string()).or_default()
    }

    pub fn game(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }

    /// Our own record wins; otherwise fall back to what the store told us.
    pub fn last_played(&self, game: &Game) -> Option<i64> {
        self.record(&game.id).last_played.or(game.store_last_played)
    }

    pub fn total_playtime(&self) -> u64 {
        self.games
            .iter()
            .map(|g| self.record(&g.id).playtime_secs)
            .sum()
    }

    pub fn total_size(&self) -> u64 {
        self.games.iter().filter_map(|g| g.size_bytes).sum()
    }

    /// How many games each source contributed, for the sidebar chips.
    pub fn count_by_source(&self) -> Vec<(SourceKind, usize)> {
        SOURCE_KINDS
            .iter()
            .map(|kind| {
                let count = self
                    .games
                    .iter()
                    .filter(|g| g.source.kind() == *kind)
                    .count();
                (*kind, count)
            })
            .collect()
    }

    /// Applies the search box, the filter chips and the sort order.
    pub fn view(&self, query: &str, settings: &Settings) -> Vec<&Game> {
        let needle = query.trim().to_lowercase();
        let mut out: Vec<&Game> = self
            .games
            .iter()
            .filter(|g| {
                let rec = self.record(&g.id);
                if rec.hidden && !settings.show_hidden {
                    return false;
                }
                if let Some(kind) = settings.source_filter {
                    if g.source.kind() != kind {
                        return false;
                    }
                }
                match settings.filter {
                    Filter::All => {}
                    Filter::Favorites if !rec.favorite => return false,
                    Filter::Unplayed if rec.launches > 0 => return false,
                    _ => {}
                }
                needle.is_empty() || g.name.to_lowercase().contains(&needle)
            })
            .collect();

        out.sort_by(|a, b| {
            let (ra, rb) = (self.record(&a.id), self.record(&b.id));
            // Favorites float to the top of every ordering.
            rb.favorite
                .cmp(&ra.favorite)
                .then_with(|| match settings.sort {
                    Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    Sort::LastPlayed => self.last_played(b).cmp(&self.last_played(a)),
                    Sort::Playtime => rb.playtime_secs.cmp(&ra.playtime_secs),
                    Sort::Size => b.size_bytes.unwrap_or(0).cmp(&a.size_bytes.unwrap_or(0)),
                })
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out
    }
}
