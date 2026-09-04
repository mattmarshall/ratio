//! Stage E store door for console / API reads.
//!
//! When `RATIO_PG_URL` is set, lots, positions, and Current aggregates are
//! served from the Postgres-backed snapshot. Unset is the in-memory fold.
//! The journal stays the system of record: replay rebuilds the snapshot;
//! a missing watermark refuses rather than answering with zeros.

use anyhow::Result;
use ratio_sql_project::{JournalPin, ProjectionReads, StoreConfig};

use crate::Console;

/// How this console serves lots / positions / Current aggregates.
///
/// ⛔ UNSET IS MEMORY. A configured store that cannot be reached is a
/// refuse at the request, not a silent fallback — a figure from the other
/// path would look like it came from this one.
pub(crate) enum StoreMode {
    Memory,
    Live(ProjectionReads),
}

impl Default for StoreMode {
    fn default() -> Self {
        Self::Memory
    }
}

impl Console {
    /// Serve lots / positions / Current aggregates from `reads`.
    ///
    /// Tests inject [`ProjectionReads::in_process`]. The network server
    /// calls [`Self::with_stage_e_from_env`].
    pub fn with_stage_e_store(mut self, reads: ProjectionReads) -> Self {
        self.store = StoreMode::Live(reads);
        self
    }

    /// `RATIO_PG_URL` set → live store. Empty or missing → Memory.
    ///
    /// ⛔ DOES NOT FALL BACK. A URL that cannot be reached fails this
    /// constructor so a request cannot look like it read the store.
    pub fn with_stage_e_from_env(self) -> Result<Self> {
        match StoreConfig::from_env() {
            None => Ok(self),
            Some(cfg) => Ok(self.with_stage_e_store(ProjectionReads::connect(&cfg)?)),
        }
    }

    pub(crate) fn stage_e(&self) -> Option<&ProjectionReads> {
        match &self.store {
            StoreMode::Memory => None,
            StoreMode::Live(reads) => Some(reads),
        }
    }

    /// Pin this fund's journal and catch the store up. `None` is Memory.
    ///
    /// ⭐ THE PIN IS THE JOURNAL'S DIGEST. Catch-up replays from
    /// `journal.jsonl` when the watermark lags; it refuses a rewind or a
    /// replaced file. After this returns, a read at `pin` is caught up.
    pub(crate) fn stage_e_pin(&self, fund: &str) -> Result<Option<(JournalPin, &ProjectionReads)>> {
        let Some(store) = self.stage_e() else {
            return Ok(None);
        };
        let path = self.book_path(fund)?;
        let pin = store.catch_up(fund, &path)?;
        Ok(Some((pin, store)))
    }
}
