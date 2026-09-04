//! Console / API reads of the Stage E store.
//!
//! # Why this module exists
//!
//! #234 applied the schema to a live engine. The running read model was still
//! the in-memory [`ratio_project::Projection`]. This is the config-driven door
//! the console uses to serve lots, positions, and Current aggregates from the
//! same snapshot — without making Postgres the system of record.
//!
//! # ⛔ Unset is Memory, not an empty fund
//!
//! [`StoreConfig::from_env`]: a missing or empty `RATIO_PG_URL` is
//! [`None`]. The in-memory fold stays the running read model. An empty URL
//! is not localhost, and a missing watermark is not a silent `lots: []`.
//!
//! # ⛔ A figure pins the prefix it read
//!
//! [`ProjectionReads::catch_up`] pins `journal.jsonl` (`ratio_nav::prefix_digest`)
//! and refuses unless the watermark matches. A lagging store is rebuilt from
//! the journal, not answered. `//tla:projection_check` /
//! `//tla:unpinned_projection_check`.
//!
//! # What this is not
//!
//! The 140M-entry / 40GB journal fold. Planner
//! pushdown vs `Pg.Rel.Semantics` is [`crate::plan`]. The measured
//! 20M-lot projection fold is [`crate::fold_scale`].

use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use ratio_project::{relief, AsOf};
use ratio_store::{FileBook, Journal};

use crate::{
    AggregateRow, JournalPin, PgProjection, PositionRow, SqlProjection, Watermark,
};

/// How the console/API finds a live Stage E store.
///
/// ⛔ UNSET IS THE IN-MEMORY FOLD. `RATIO_PG_URL` empty or missing is
/// [`None`], not a default server. A figure served from Memory while the
/// operator thought they were on Postgres is the silent path this exists
/// to refuse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreConfig {
    pub url: String,
    pub schema: String,
}

impl StoreConfig {
    /// Schema when `RATIO_PG_SCHEMA` is unset. One name, so a laptop and CI
    /// do not invent two search_paths for the same tables.
    pub const DEFAULT_SCHEMA: &'static str = "ratio_proj";

    /// `RATIO_PG_URL` + optional `RATIO_PG_SCHEMA`. Empty is unset.
    pub fn from_env() -> Option<Self> {
        Self::from_url(
            std::env::var("RATIO_PG_URL").ok().as_deref(),
            std::env::var("RATIO_PG_SCHEMA").ok().as_deref(),
        )
    }

    /// Parse the two dials. Tests use this rather than mutating the process
    /// environment — `RATIO_PG_URL` is process-global and CI already owns it.
    pub fn from_url(url: Option<&str>, schema: Option<&str>) -> Option<Self> {
        let url = url.map(str::trim).filter(|u| !u.is_empty())?;
        let schema = schema
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(Self::DEFAULT_SCHEMA);
        Some(Self {
            url: url.to_string(),
            schema: schema.to_string(),
        })
    }
}

/// The Stage E store the console/API reads, in-process or live.
///
/// ⚠ ONE TYPE SO THE HANDLERS HAVE ONE PATH. Tests inject
/// [`Self::in_process`]; `RATIO_PG_URL` builds [`Self::connect`]. The
/// refuse / pin / replay rules are the same either way.
pub struct ProjectionReads {
    inner: Mutex<ReadsInner>,
}

enum ReadsInner {
    Sql(SqlProjection),
    Pg(PgProjection),
}

impl ProjectionReads {
    /// Empty denotational store. Console tests, and any caller that must
    /// not stand up a server.
    pub fn in_process() -> Self {
        Self {
            inner: Mutex::new(ReadsInner::Sql(SqlProjection::new())),
        }
    }

    /// Live engine at `cfg`. Applies the schema when the named schema is
    /// missing — a deliberate first-use, not a connect side-effect on an
    /// already-applied search_path.
    pub fn connect(cfg: &StoreConfig) -> Result<Self> {
        let pg = PgProjection::connect(&cfg.url, &cfg.schema)?;
        pg.ensure_schema()?;
        Ok(Self::from_pg(pg))
    }

    /// Wrap a live handle the caller already applied. Tests.
    pub fn from_pg(pg: PgProjection) -> Self {
        Self {
            inner: Mutex::new(ReadsInner::Pg(pg)),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ReadsInner>> {
        self.inner
            .lock()
            .map_err(|_| anyhow::anyhow!("the Stage E store lock was poisoned by a panic"))
    }

    /// Pin `path`'s journal — the system of record — and bring the snapshot
    /// to that pin.
    ///
    /// ⭐ REPLAY IS THE REBUILD, NOT AN ANSWER FROM A LAGGING TABLE. A store
    /// that has never been replayed is rebuilt from `journal.jsonl`. A
    /// watermark that already matches is left alone. A rewind or a
    /// same-height digest swap still refuses — there is nothing to un-apply.
    ///
    /// ⛔ NEVER RETURNS A PIN THE STORE DOES NOT HOLD. After this returns,
    /// [`Self::require_caught_up`] is true. An empty lot list from here is
    /// a holding that is empty at that prefix, not an unpinned invention.
    pub fn catch_up(&self, book_id: &str, path: &Path) -> Result<JournalPin> {
        let pin = JournalPin::of_book(path)?;
        let mut inner = self.lock()?;
        match &mut *inner {
            ReadsInner::Sql(store) => {
                let caught = store
                    .watermark(book_id)
                    .is_some_and(|w| w.prefix == pin.prefix && w.digest == pin.digest);
                if !caught {
                    store.replay_book(book_id, path)?;
                }
                store.require_caught_up(book_id, &pin)?;
            }
            ReadsInner::Pg(store) => {
                let have = store.watermark(book_id)?;
                let caught = have
                    .as_ref()
                    .is_some_and(|w| w.prefix == pin.prefix && w.digest == pin.digest);
                if !caught {
                    store.replay_book(book_id, path)?;
                }
                store.require_caught_up(book_id, &pin)?;
            }
        }
        Ok(pin)
    }

    pub fn watermark(&self, book_id: &str) -> Result<Option<Watermark>> {
        let inner = self.lock()?;
        match &*inner {
            ReadsInner::Sql(store) => Ok(store.watermark(book_id).cloned()),
            ReadsInner::Pg(store) => store.watermark(book_id),
        }
    }

    pub fn require_caught_up(&self, book_id: &str, pin: &JournalPin) -> Result<Watermark> {
        let inner = self.lock()?;
        match &*inner {
            ReadsInner::Sql(store) => store.require_caught_up(book_id, pin).cloned(),
            ReadsInner::Pg(store) => store.require_caught_up(book_id, pin),
        }
    }

    pub fn lots_of(
        &self,
        book_id: &str,
        view: &str,
        dim: i64,
        instrument: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<relief::Lot>>> {
        let inner = self.lock()?;
        match &*inner {
            ReadsInner::Sql(store) => store.lots_of(book_id, view, dim, instrument, pin),
            ReadsInner::Pg(store) => store.lots_of(book_id, view, dim, instrument, pin),
        }
    }

    pub fn positions(
        &self,
        book_id: &str,
        view: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<PositionRow>>> {
        let inner = self.lock()?;
        match &*inner {
            ReadsInner::Sql(store) => store.positions(book_id, view, pin),
            ReadsInner::Pg(store) => store.positions(book_id, view, pin),
        }
    }

    pub fn aggregates(
        &self,
        book_id: &str,
        view: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<AggregateRow>>> {
        let inner = self.lock()?;
        match &*inner {
            ReadsInner::Sql(store) => store.aggregates(book_id, view, pin),
            ReadsInner::Pg(store) => store.aggregates(book_id, view, pin),
        }
    }
}

/// Pin the journal at `path`. The digest is `ratio_nav::prefix_digest`.
impl JournalPin {
    pub fn of_book(path: &Path) -> Result<Self> {
        let entries = FileBook::open(path)?.entries()?;
        Self::of(&entries)
    }
}

/// Held vs rest split the console's position list already uses.
///
/// `instrument` None is the unattributed remainder — not a default ticker.
pub fn split_positions(
    rows: &[PositionRow],
) -> (Vec<((i64, String), (i64, i64))>, Vec<(i64, i64)>) {
    let mut held = Vec::new();
    let mut rest = Vec::new();
    for r in rows {
        match &r.instrument {
            Some(inst) => held.push(((r.dim, inst.clone()), (r.cost, r.quantity))),
            None => rest.push((r.dim, r.cost)),
        }
    }
    (held, rest)
}

/// Current-fold balances from aggregate rows. Activity windows still walk
/// the journal — the store is the inception-to-date snapshot, not a
/// time-travel table.
pub fn current_balances(
    rows: &[AggregateRow],
) -> Vec<(i64, Option<String>, i128, i128, i128, i128, i64)> {
    rows.iter()
        .map(|r| {
            (
                r.dim,
                r.currency.clone(),
                r.debit,
                r.credit,
                r.debit,
                r.credit,
                r.postings,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_project::Projection;
    use ratio_rules::UNDECLARED_VIEW as B;
    use ratio_store::{
        Account, AccountTypeRecord as A, ConfigStore, JournalEntry, PostingRecord,
    };

    fn tmp_root() -> std::path::PathBuf {
        match std::env::var_os("TEST_TMPDIR") {
            Some(d) => std::path::PathBuf::from(d),
            None => std::env::temp_dir(),
        }
    }

    fn book_path(name: &str) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-sql-reads-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn seed(name: &str, trades: &[(&str, i64, i64)]) -> std::path::PathBuf {
        let d = book_path(name);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account {
                dim: 1,
                display_name: "Investments".into(),
                account_type: A::Asset,
            },
            Account {
                dim: 2,
                display_name: "Cash".into(),
                account_type: A::Asset,
            },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        for (n, (inst, cost, qty)) in trades.iter().enumerate() {
            b.append(&JournalEntry {
                id: format!("t{n}"),
                memo: "buy".into(),
                config: c.clone(),
                postings: vec![
                    PostingRecord::of(1, *cost, inst, Some(*qty)),
                    PostingRecord::new(2, -*cost),
                ],
                trade_date: None,
                announcement: None,
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
                kind: None,
            })
            .unwrap();
        }
        d
    }

    fn append_buy(path: &Path, inst: &str, cost: i64, qty: i64, id: &str) {
        let mut b = FileBook::open(path).unwrap();
        let c = b
            .active()
            .unwrap()
            .expect("a seeded book has an active configuration");
        b.append(&JournalEntry {
            id: id.into(),
            memo: "buy".into(),
            config: c,
            postings: vec![
                PostingRecord::of(1, cost, inst, Some(qty)),
                PostingRecord::new(2, -cost),
            ],
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
            kind: None,
        })
        .unwrap();
    }

    #[test]
    fn unset_or_empty_url_is_the_in_memory_fold() {
        // ⛔ EMPTY IS UNSET, NOT LOCALHOST. A default server would make a
        // laptop without Postgres look configured and then invent refusals
        // — or worse, empty figures — for every read.
        assert!(StoreConfig::from_url(None, None).is_none());
        assert!(StoreConfig::from_url(Some(""), None).is_none());
        assert!(StoreConfig::from_url(Some("  "), Some("ratio_proj")).is_none());
        let cfg = StoreConfig::from_url(Some("postgres://ratio@127.0.0.1/ratio"), None).unwrap();
        assert_eq!(cfg.schema, StoreConfig::DEFAULT_SCHEMA);
        let cfg = StoreConfig::from_url(
            Some("postgres://ratio@127.0.0.1/ratio"),
            Some("  "),
        )
        .unwrap();
        assert_eq!(cfg.schema, StoreConfig::DEFAULT_SCHEMA);
        let cfg = StoreConfig::from_url(
            Some("postgres://ratio@127.0.0.1/ratio"),
            Some("fund_a"),
        )
        .unwrap();
        assert_eq!(cfg.schema, "fund_a");
    }

    #[test]
    fn a_store_that_has_not_been_replayed_refuses_empty_lots() {
        // ⭐ THE CONSOLE MUST NOT ANSWER `lots: []` HERE. That list looks
        // like a fund that sold everything. It is a store that was never
        // folded. `//tla:unpinned_projection_check`.
        let reads = ProjectionReads::in_process();
        let pin = JournalPin::of(&[]).unwrap();
        let err = reads
            .lots_of("never", B, 1, "vti", &pin)
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("has not been replayed"), "{msg}");
        assert!(msg.contains("zeros that look like a fund"), "{msg}");

        let err = reads.positions("never", B, &pin).unwrap_err();
        assert!(format!("{err:#}").contains("has not been replayed"));
        let err = reads.aggregates("never", B, &pin).unwrap_err();
        assert!(format!("{err:#}").contains("has not been replayed"));
    }

    #[test]
    fn catch_up_replays_from_the_journal_and_pins_the_digest() {
        let d = seed("catch", &[("vti", 25_000, 100), ("voo", 10_000, 40)]);
        let reads = ProjectionReads::in_process();
        assert!(reads.watermark("catch").unwrap().is_none());

        let pin = reads.catch_up("catch", &d).unwrap();
        let journal = JournalPin::of_book(&d).unwrap();
        assert_eq!(pin, journal);
        assert_eq!(pin.prefix, 2);
        let mark = reads.watermark("catch").unwrap().unwrap();
        assert_eq!(mark.prefix, pin.prefix);
        assert_eq!(mark.digest, pin.digest);

        let memory = Projection::of_book(&d).unwrap();
        let lots = reads.lots_of("catch", B, 1, "vti", &pin).unwrap();
        assert_eq!(lots.prefix, pin.prefix);
        assert_eq!(lots.view, B);
        assert_eq!(lots.value, memory.lots_of(B, 1, "vti").unwrap().value);
        assert!(
            lots.value.iter().all(|l| l.acquired.is_none()),
            "unset acquired stays unset — the seed carried no trade_date"
        );

        let pos = reads.positions("catch", B, &pin).unwrap();
        assert_eq!(pos.prefix, pin.prefix);
        let (held, rest) = split_positions(&pos.value);
        assert_eq!(held.len(), 2);
        assert_eq!(rest.len(), 1, "cash is the rest map, instrument unset");
        assert_eq!(rest[0].0, 2);

        let agg = reads.aggregates("catch", B, &pin).unwrap();
        assert_eq!(agg.prefix, pin.prefix);
        let balances = current_balances(&agg.value);
        assert_eq!(balances.len(), memory.balances(B).unwrap().value.len());
        assert!(
            balances.iter().any(|(_, ccy, _, _, _, _, _)| ccy.is_none()),
            "unset currency stays unset, not a silent USD"
        );
    }

    #[test]
    fn an_instrument_never_held_is_an_empty_list_at_the_pin() {
        // ⚠ THIS EMPTY IS AUTHORITATIVE. The book was replayed; VOO was
        // never bought. A missing pin would have refused above. The two
        // empties must not be confused.
        let d = seed("empty-inst", &[("vti", 10_000, 10)]);
        let reads = ProjectionReads::in_process();
        let pin = reads.catch_up("empty-inst", &d).unwrap();
        let lots = reads
            .lots_of("empty-inst", B, 1, "voo", &pin)
            .unwrap();
        assert!(lots.value.is_empty());
        assert_eq!(lots.prefix, pin.prefix);
        assert_ne!(pin.prefix, 0, "an empty holding is not an empty journal");
    }

    #[test]
    fn a_stale_pin_refuses_rather_than_answering() {
        let d = seed("stale", &[("vti", 10_000, 10)]);
        let reads = ProjectionReads::in_process();
        reads.catch_up("stale", &d).unwrap();
        append_buy(&d, "vti", 5_000, 5, "t1");
        let head = JournalPin::of_book(&d).unwrap();
        assert_eq!(head.prefix, 2);
        assert_eq!(reads.watermark("stale").unwrap().unwrap().prefix, 1);

        let err = reads
            .lots_of("stale", B, 1, "vti", &head)
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("lags or leads"), "{msg}");
        assert!(msg.contains("must refuse"), "{msg}");
    }

    #[test]
    fn catch_up_after_an_append_rebuilds_and_does_not_double_count() {
        let d = seed("again", &[("vti", 10_000, 10)]);
        let reads = ProjectionReads::in_process();
        reads.catch_up("again", &d).unwrap();
        append_buy(&d, "vti", 20_000, 10, "t1");
        let pin = reads.catch_up("again", &d).unwrap();
        assert_eq!(pin.prefix, 2);
        let lots = reads.lots_of("again", B, 1, "vti", &pin).unwrap();
        assert_eq!(
            lots.value.len(),
            2,
            "a rebuild that appended onto itself would hold four"
        );
        assert_eq!(lots.value.iter().map(|l| l.units).sum::<i64>(), 20);
        assert_eq!(lots.prefix, pin.prefix);
    }
}
