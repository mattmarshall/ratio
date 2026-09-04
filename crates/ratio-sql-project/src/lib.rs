//! ratio-sql-project — Stage E table projection of lots, positions, aggregates.
//!
//! # Why these tables exist
//!
//! Twenty million lots do not scan through a JSONL journal quickly. Reads move
//! to a projection folded once, while `journal.jsonl` stays the system of
//! record because replay and content-addressed digests are the product.
//! `//tla:projection_check` and `//tla:sql_projection_check` already named the
//! catastrophic failures; this crate is the schema those specs were waiting
//! for, not a second ledger.
//!
//! # ⛔ The journal stays authoritative
//!
//! These tables are a snapshot of one journal prefix. The watermark is that
//! prefix and its `ratio_nav::prefix_digest`. Replay rebuilds the snapshot
//! from the journal. Nothing here is appended except by folding the log.
//!
//! # ⛔ One watermark, every table
//!
//! `projAt` was a scalar on the in-memory projection. Split into tables and
//! the guarantee is gone: lots at 105 and positions at 100 both tie, and the
//! figure never existed. [`SqlProjection::replay_book`] replaces every table
//! and the watermark in one commit. There is no per-table advance.
//!
//! # ⛔ Stale is a refuse, not a figure
//!
//! Lagging is what `AsOf` makes safe on the in-memory path — the caller pins
//! what it read. A SQL read path has no such type. Asking for the journal
//! head while the snapshot is behind would answer with somebody else's day.
//! [`SqlProjection::require_caught_up`] refuses when the watermark is not
//! exactly the pin. `//tla:unpinned_projection_check`.
//!
//! # ⛔ Relief is not `ORDER BY seq`
//!
//! Seq is the acquisition ordinal. FIFO uses it. HIFO, LIFO, LOFO, and the
//! holding-period methods do not. A planner-style scan that takes the head of
//! a seq index is the silent SQL FIFO `//tla:stale_method_relief_check` exists
//! to catch. [`SqlProjection::relieve`] loads the rows and calls
//! [`ratio_project::relief::relieve_by`] under the elected method. MinTax,
//! SpecID, average cost, and wash stay elections — not a `Method` variant.
//!
//! # What this is not
//!
//! A live Postgres process, planner pushdown proved against
//! `Pg.Rel.Semantics`, or the measured 20M-lot claim. Those stay #8 / #159.
//! `Ratio.Exec` still holds: a database does not change the IO floor.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use ratio_project::{relief, AsOf, Projection, Totals};
use ratio_store::{FileBook, Journal, JournalEntry};

/// The Postgres contract this store implements. Apply it to a server when
/// #159 measures a live engine; the denotational tables below are the same
/// shape either way.
pub const SCHEMA_SQL: &str = include_str!("../schema.sql");

/// The journal prefix a figure must be folded from, content-addressed.
///
/// ⛔ THE DIGEST IS `ratio_nav::prefix_digest`. Two encoders for one prefix
/// would be two answers to "what was the book", and a close that could not
/// be replayed against a strike of the same prefix is the failure this
/// system exists to be able to make about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalPin {
    pub prefix: usize,
    pub digest: String,
}

impl JournalPin {
    /// Pin the prefix that `entries` is.
    pub fn of(entries: &[JournalEntry]) -> Result<Self> {
        Ok(Self {
            prefix: entries.len(),
            digest: ratio_nav::prefix_digest(entries)?,
        })
    }
}

/// What the snapshot claims to reflect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watermark {
    pub book_id: String,
    pub prefix: usize,
    pub digest: String,
}

/// One open lot as a table row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LotRow {
    pub view: String,
    pub dim: i64,
    pub instrument: String,
    pub lot: relief::Lot,
}

/// One position as a table row. `instrument` is `None` for the rest map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionRow {
    pub view: String,
    pub dim: i64,
    pub instrument: Option<String>,
    pub cost: i64,
    pub quantity: i64,
}

/// One (dimension, currency) aggregate as a table row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateRow {
    pub view: String,
    pub dim: i64,
    pub currency: Option<String>,
    pub debit: i128,
    pub credit: i128,
    pub postings: i64,
}

/// The Stage E store: four tables, one watermark, journal remains SoR.
///
/// ⚠ IN-PROCESS, BECAUSE THE SEMANTICS ARE WHAT THE TLA NAMED. A live
/// Postgres is the same rows behind a different engine. CI must stay able to
/// refuse a stale watermark and a silent FIFO without standing up a server.
#[derive(Clone, Debug, Default)]
pub struct SqlProjection {
    watermarks: BTreeMap<String, Watermark>,
    lots: BTreeMap<LotKey, relief::Lot>,
    positions: BTreeMap<PositionKey, (i64, i64)>,
    aggregates: BTreeMap<AggregateKey, (i128, i128, i64)>,
}

type LotKey = (String, String, i64, String, u64);
type PositionKey = (String, String, i64, Option<String>);
type AggregateKey = (String, String, i64, Option<String>);

impl SqlProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /// The schema a Postgres would apply. Same tables this store holds.
    pub fn schema_sql() -> &'static str {
        SCHEMA_SQL
    }

    /// The watermark, if this book has been replayed.
    pub fn watermark(&self, book_id: &str) -> Option<&Watermark> {
        self.watermarks.get(book_id)
    }

    /// Fold `path`'s journal through the proved projection and replace every
    /// table for `book_id` in one commit.
    ///
    /// ⭐ `Projection::of_book` READS EACH ENTRY'S PINNED CONFIG, so a fund
    /// electing HIFO is relieved HIFO. A slice-fold that took a method as a
    /// default would be `//tla:stale_method_relief_check` with a shorter
    /// signature.
    ///
    /// ⛔ REPLACE, NEVER APPEND ONTO EXISTING ROWS. Re-folding onto state
    /// already held double-counts; `//tla:rebuild_double_counts_check`.
    pub fn replay_book(&mut self, book_id: &str, path: &Path) -> Result<Watermark> {
        let book = FileBook::open(path)?;
        let entries = book.entries()?;
        let pin = JournalPin::of(&entries)?;
        if let Some(have) = self.watermarks.get(book_id) {
            if have.prefix > pin.prefix {
                bail!(
                    "projection {book_id} is at prefix {} digest {}; the journal is shorter \
                     ({}). A rewind would un-apply entries the snapshot has already folded, \
                     and there is nothing to un-apply them with. The journal is the system \
                     of record — restore it, or drop this snapshot",
                    have.prefix,
                    have.digest,
                    pin.prefix
                );
            }
            if have.prefix == pin.prefix && have.digest != pin.digest {
                bail!(
                    "projection {book_id} is at prefix {} but the journal digest is {}, not \
                     {}. The file was replaced. Replay from empty, not onto this snapshot",
                    have.prefix,
                    pin.digest,
                    have.digest
                );
            }
        }
        let projection = Projection::of_book(path)?;
        if projection.prefix() != pin.prefix {
            bail!(
                "fold prefix {} is not the journal height {} — the snapshot would pin a \
                 prefix it did not fold",
                projection.prefix(),
                pin.prefix
            );
        }
        let snapshot = Snapshot::from_projection(book_id, &projection, pin)?;
        self.commit(snapshot)
    }

    /// Fold a slice under one method. For tests that are not a `FileBook`.
    ///
    /// ⚠ NO DEFAULT METHOD. A slice does not carry configurations; the caller
    /// says which method the whole slice was relieved under.
    pub fn replay_entries(
        &mut self,
        book_id: &str,
        journal: &[JournalEntry],
        method: relief::Method,
    ) -> Result<Watermark> {
        let pin = JournalPin::of(journal)?;
        let projection = Projection::rebuild(journal, method);
        if projection.prefix() != pin.prefix {
            bail!(
                "fold prefix {} is not the journal height {}",
                projection.prefix(),
                pin.prefix
            );
        }
        let snapshot = Snapshot::from_projection(book_id, &projection, pin)?;
        self.commit(snapshot)
    }

    /// Refuse unless the snapshot is exactly `pin`.
    ///
    /// ⛔ NOT "CLOSE ENOUGH", NOT THE MINIMUM OF SEVERAL TABLES. A table at
    /// 105 has already applied 101–105. The prefix is a property of the
    /// state that produced the rows.
    pub fn require_caught_up(&self, book_id: &str, pin: &JournalPin) -> Result<&Watermark> {
        let have = self.watermarks.get(book_id).ok_or_else(|| {
            anyhow::anyhow!(
                "projection {book_id} has not been replayed; refusing a figure from an \
                 empty snapshot rather than answering with zeros that look like a fund"
            )
        })?;
        if have.prefix != pin.prefix || have.digest != pin.digest {
            bail!(
                "projection {book_id} lags or leads the journal: snapshot prefix {} \
                 digest {}, journal prefix {} digest {}. A stale projection must refuse, \
                 not answer with a figure from a different prefix. \
                 `//tla:unpinned_projection_check`",
                have.prefix,
                have.digest,
                pin.prefix,
                pin.digest
            );
        }
        Ok(have)
    }

    /// Open lots of one position, as of `pin`.
    pub fn lots_of(
        &self,
        book_id: &str,
        view: &str,
        dim: i64,
        instrument: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<relief::Lot>>> {
        let mark = self.require_caught_up(book_id, pin)?;
        let mut lots: Vec<relief::Lot> = self
            .lots
            .iter()
            .filter(|((b, v, d, i, _), _)| {
                b == book_id && v == view && *d == dim && i == instrument
            })
            .map(|(_, lot)| lot.clone())
            .collect();
        // ⚠ SORTED ON READ for a screen, same as `Holding::lots`. Relief does
        // not trust this order — `relieve_by` arranges under the elected method.
        lots.sort_by_key(|l| l.seq);
        Ok(AsOf {
            value: lots,
            prefix: mark.prefix,
            view: view.to_string(),
            through: None,
        })
    }

    /// Every position row, as of `pin`.
    pub fn positions(
        &self,
        book_id: &str,
        view: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<PositionRow>>> {
        let mark = self.require_caught_up(book_id, pin)?;
        let rows = self
            .positions
            .iter()
            .filter(|((b, v, _, _), _)| b == book_id && v == view)
            .map(|((_, _, dim, inst), (cost, qty))| PositionRow {
                view: view.to_string(),
                dim: *dim,
                instrument: inst.clone(),
                cost: *cost,
                quantity: *qty,
            })
            .collect();
        Ok(AsOf {
            value: rows,
            prefix: mark.prefix,
            view: view.to_string(),
            through: None,
        })
    }

    /// Every aggregate row, as of `pin`.
    pub fn aggregates(
        &self,
        book_id: &str,
        view: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<AggregateRow>>> {
        let mark = self.require_caught_up(book_id, pin)?;
        let rows = self
            .aggregates
            .iter()
            .filter(|((b, v, _, _), _)| b == book_id && v == view)
            .map(|((_, _, dim, cur), (debit, credit, postings))| AggregateRow {
                view: view.to_string(),
                dim: *dim,
                currency: cur.clone(),
                debit: *debit,
                credit: *credit,
                postings: *postings,
            })
            .collect();
        Ok(AsOf {
            value: rows,
            prefix: mark.prefix,
            view: view.to_string(),
            through: None,
        })
    }

    /// Relieve `want` units under the elected method, from the snapshot.
    ///
    /// ⛔ THE WALK IS `relieve_by`, NOT A SEQ SCAN. Physical storage is
    /// keyed by seq so a naive `ORDER BY seq` would perform FIFO whatever
    /// the fund elected. That is the defect this method exists to refuse.
    pub fn relieve(
        &self,
        book_id: &str,
        view: &str,
        dim: i64,
        instrument: &str,
        method: relief::Method,
        want: i64,
        pin: &JournalPin,
    ) -> Result<AsOf<relief::Relieved>> {
        let lots = self.lots_of(book_id, view, dim, instrument, pin)?;
        let relieved = relief::relieve_by(method, &lots.value, want)?;
        Ok(AsOf {
            value: relieved,
            prefix: lots.prefix,
            view: lots.view,
            through: lots.through,
        })
    }

    /// Replace every table for one book. The only write.
    fn commit(&mut self, snap: Snapshot) -> Result<Watermark> {
        self.watermarks.remove(&snap.watermark.book_id);
        self.lots.retain(|(b, _, _, _, _), _| b != snap.watermark.book_id);
        self.positions.retain(|(b, _, _, _), _| b != snap.watermark.book_id);
        self.aggregates.retain(|(b, _, _, _), _| b != snap.watermark.book_id);
        self.lots.extend(snap.lots);
        self.positions.extend(snap.positions);
        self.aggregates.extend(snap.aggregates);
        let mark = snap.watermark;
        self.watermarks.insert(mark.book_id.clone(), mark.clone());
        Ok(mark)
    }
}

/// Rows built off ONE `Projection` at ONE prefix, then committed together.
struct Snapshot {
    watermark: Watermark,
    lots: BTreeMap<LotKey, relief::Lot>,
    positions: BTreeMap<PositionKey, (i64, i64)>,
    aggregates: BTreeMap<AggregateKey, (i128, i128, i64)>,
}

impl Snapshot {
    fn from_projection(book_id: &str, projection: &Projection, pin: JournalPin) -> Result<Self> {
        if projection.prefix() != pin.prefix {
            bail!(
                "snapshot prefix {} is not the pin {}",
                projection.prefix(),
                pin.prefix
            );
        }
        let mut lots = BTreeMap::new();
        let mut positions = BTreeMap::new();
        let mut aggregates = BTreeMap::new();
        for view in projection.views() {
            let pos = projection.positions(view)?;
            if pos.prefix != pin.prefix {
                bail!(
                    "positions for {view} pinned {} but the snapshot pins {}",
                    pos.prefix,
                    pin.prefix
                );
            }
            for ((dim, inst), (cost, qty)) in &pos.value.held {
                let instrument = inst.to_string();
                positions.insert(
                    (book_id.to_string(), view.to_string(), *dim, Some(instrument.clone())),
                    (*cost, *qty),
                );
                let held = projection.lots_of(view, *dim, &instrument)?;
                if held.prefix != pin.prefix {
                    bail!(
                        "lots for {view}/{instrument} pinned {} but the snapshot pins {}",
                        held.prefix,
                        pin.prefix
                    );
                }
                for lot in held.value {
                    lots.insert(
                        (book_id.to_string(), view.to_string(), *dim, instrument.clone(), lot.seq),
                        lot,
                    );
                }
            }
            for (dim, amount) in &pos.value.rest {
                positions.insert(
                    (book_id.to_string(), view.to_string(), *dim, None),
                    (*amount, 0),
                );
            }
            let totals: AsOf<&Totals> = projection.totals(view)?;
            if totals.prefix != pin.prefix {
                bail!(
                    "aggregates for {view} pinned {} but the snapshot pins {}",
                    totals.prefix,
                    pin.prefix
                );
            }
            for ((dim, currency), row) in &totals.value.by_dim {
                aggregates.insert(
                    (
                        book_id.to_string(),
                        view.to_string(),
                        *dim,
                        currency.as_ref().map(|c| c.to_string()),
                    ),
                    (row.debit, row.credit, row.postings),
                );
            }
        }
        Ok(Self {
            watermark: Watermark {
                book_id: book_id.to_string(),
                prefix: pin.prefix,
                digest: pin.digest,
            },
            lots,
            positions,
            aggregates,
        })
    }
}

/// What a silent `ORDER BY seq` take would do. Tests only.
///
/// ⛔ NOT A RELIEF METHOD AND NOT ON THE PUBLIC SURFACE. Named so a test can
/// show the physical order is FIFO-shaped, which is why the production path
/// must not use it.
#[cfg(test)]
fn sql_fifo_take(lots: &[relief::Lot], want: i64) -> Result<(i64, u64)> {
    let mut left = want;
    let mut cost = 0i64;
    let mut seq = None;
    let mut ordered = lots.to_vec();
    ordered.sort_by_key(|l| l.seq);
    for lot in ordered {
        if left == 0 {
            break;
        }
        let take = left.min(lot.units);
        let share = if take == lot.units {
            lot.cost
        } else {
            // ⚠ THIS IS THE SILENT PATH, AND IT ROUNDS. The proved walk
            // refuses a remainder. The bug this function stands in for does
            // not — that is part of what makes it silent.
            lot.cost.saturating_mul(take) / lot.units
        };
        cost = cost.saturating_add(share);
        left -= take;
        seq = Some(lot.seq);
    }
    if left != 0 {
        bail!("sql fifo wanted {want} and the holding was short");
    }
    Ok((cost, seq.unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_rules::UNDECLARED_VIEW as B;
    use ratio_store::{Account, AccountTypeRecord as A, ConfigStore, Journal, PostingRecord};

    fn tmp_root() -> std::path::PathBuf {
        match std::env::var_os("TEST_TMPDIR") {
            Some(d) => std::path::PathBuf::from(d),
            None => std::env::temp_dir(),
        }
    }

    fn book_path(name: &str) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-sql-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn seed(
        name: &str,
        rules: &[u8],
        trades: &[(&str, i64, i64)],
    ) -> (std::path::PathBuf, ratio_store::Digest) {
        let d = book_path(name);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let c = b.put(rules).unwrap();
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
        (d, c)
    }

    fn pin_of(path: &Path) -> JournalPin {
        let entries = FileBook::open(path).unwrap().entries().unwrap();
        JournalPin::of(&entries).unwrap()
    }

    fn append_buy(path: &Path, config: &ratio_store::Digest, id: &str, inst: &str, cost: i64, qty: i64) {
        let mut b = FileBook::open(path).unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "buy".into(),
            config: config.clone(),
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
    fn the_schema_names_one_watermark_and_three_tables() {
        // ⛔ THE CONTRACT IS THE FILE, not a comment in this test. A schema
        // that lost `projection_watermark` would let a future adapter invent
        // a per-table `at`, which is the first dial `SqlProjection.tla` flips.
        let sql = SqlProjection::schema_sql();
        for needle in [
            "CREATE TABLE projection_watermark",
            "CREATE TABLE lots",
            "CREATE TABLE positions",
            "CREATE TABLE aggregates",
            "journal.jsonl STAYS THE SYSTEM OF RECORD",
            "ONE WATERMARK, NOT ONE PER TABLE",
            "ORDER BY seq IS NOT FIFO RELIEF",
        ] {
            assert!(sql.contains(needle), "schema lost {needle:?}");
        }
        assert!(
            !sql.to_ascii_lowercase().contains("lot_method"),
            "the schema must not invent a Method / Order / lot_method column"
        );
    }

    #[test]
    fn replay_from_the_journal_digest_matches_the_in_memory_fold() {
        // ⛔ AGAINST THE SYSTEM OF RECORD'S FOLD, not against itself. A table
        // store that drifted from `Projection` would be a second opinion
        // nobody can adjudicate.
        let (d, _) = seed(
            "agrees",
            b"rules = []\n",
            &[("vti", 25_000, 100), ("voo", 10_000, 40), ("vti", 5_000, 20)],
        );
        let pin = pin_of(&d);
        let memory = Projection::of_book(&d).unwrap();
        let mut store = SqlProjection::new();
        let mark = store.replay_book("agrees", &d).unwrap();
        assert_eq!(mark.prefix, pin.prefix);
        assert_eq!(mark.digest, pin.digest);
        assert_eq!(mark.prefix, 3);

        let lots = store.lots_of("agrees", B, 1, "vti", &pin).unwrap();
        assert_eq!(lots.prefix, pin.prefix);
        assert_eq!(lots.value, memory.lots_of(B, 1, "vti").unwrap().value);

        let pos = store.positions("agrees", B, &pin).unwrap();
        let held: BTreeMap<(i64, String), (i64, i64)> = memory
            .positions(B)
            .unwrap()
            .value
            .held
            .iter()
            .map(|((dim, inst), v)| ((*dim, inst.to_string()), *v))
            .collect();
        let projected: BTreeMap<(i64, String), (i64, i64)> = pos
            .value
            .iter()
            .filter_map(|r| r.instrument.as_ref().map(|i| ((r.dim, i.clone()), (r.cost, r.quantity))))
            .collect();
        assert_eq!(projected, held);

        let agg = store.aggregates("agrees", B, &pin).unwrap();
        let totals = memory.totals(B).unwrap();
        assert_eq!(agg.value.len(), totals.value.by_dim.len());
        assert_eq!(agg.prefix, totals.prefix);
    }

    #[test]
    fn a_second_replay_replaces_the_snapshot_and_does_not_double_count() {
        let (d, c) = seed("twice", b"rules = []\n", &[("vti", 10_000, 10)]);
        let mut store = SqlProjection::new();
        store.replay_book("twice", &d).unwrap();
        append_buy(&d, &c, "t1", "vti", 20_000, 10);
        let mark = store.replay_book("twice", &d).unwrap();
        let pin = pin_of(&d);
        assert_eq!(mark, Watermark { book_id: "twice".into(), prefix: pin.prefix, digest: pin.digest.clone() });
        let lots = store.lots_of("twice", B, 1, "vti", &pin).unwrap();
        assert_eq!(lots.value.len(), 2, "a rebuild that appended onto itself would hold four");
        assert_eq!(lots.value.iter().map(|l| l.units).sum::<i64>(), 20);
    }

    #[test]
    fn a_stale_projection_refuses_rather_than_answering() {
        // ⭐ THE WHOLE SAFETY ARGUMENT FOR A SQL READ PATH. Pinning the
        // journal head while reading a lagging snapshot is
        // `//tla:unpinned_projection_check`. The type cannot save us here.
        let (d, c) = seed("lag", b"rules = []\n", &[("vti", 10_000, 10)]);
        let mut store = SqlProjection::new();
        store.replay_book("lag", &d).unwrap();
        append_buy(&d, &c, "t1", "vti", 5_000, 5);
        let head = pin_of(&d);
        assert_eq!(head.prefix, 2);
        assert_eq!(store.watermark("lag").unwrap().prefix, 1);

        let err = store.lots_of("lag", B, 1, "vti", &head).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("lags or leads"), "{msg}");
        assert!(msg.contains("must refuse"), "{msg}");

        let err = store.relieve("lag", B, 1, "vti", relief::Method::Fifo, 5, &head).unwrap_err();
        assert!(format!("{err:#}").contains("lags or leads"));
    }

    #[test]
    fn a_wrong_digest_at_the_same_height_is_also_a_refuse() {
        let (d, _) = seed("digest", b"rules = []\n", &[("vti", 10_000, 10)]);
        let mut store = SqlProjection::new();
        store.replay_book("digest", &d).unwrap();
        let pin = JournalPin { prefix: 1, digest: "0".repeat(64) };
        let err = store.require_caught_up("digest", &pin).unwrap_err();
        assert!(format!("{err:#}").contains("lags or leads"));
    }

    #[test]
    fn an_empty_store_refuses_zeros_that_look_like_a_fund() {
        let store = SqlProjection::new();
        let pin = JournalPin { prefix: 0, digest: ratio_nav::prefix_digest(&[]).unwrap() };
        let err = store.require_caught_up("never", &pin).unwrap_err();
        assert!(format!("{err:#}").contains("has not been replayed"));
    }

    #[test]
    fn elected_hifo_is_not_silently_replaced_by_sql_fifo() {
        // ⭐ TWO LOTS, CHEAP THEN DEAR. Physical storage is seq order, so
        // `ORDER BY seq` takes the cheap lot. HIFO must take the dear one.
        // If this crate ever relieves by walking the seq index, this fails.
        //
        // ⚠ THE SABOTAGE IS `sql_fifo_take`. Break the production path by
        // pointing `relieve` at that function and this test goes red — that
        // is what says it covers the case, not merely the code.
        let (d, _) = seed(
            "hifo",
            b"lot_method = \"hifo\"\nrules = []\n",
            &[("vti", 1_000, 10), ("vti", 10_000, 10)],
        );
        let mut store = SqlProjection::new();
        store.replay_book("hifo", &d).unwrap();
        let pin = pin_of(&d);
        let lots = store.lots_of("hifo", B, 1, "vti", &pin).unwrap();
        assert_eq!(lots.value.len(), 2);
        assert_eq!(lots.value[0].seq, 0);
        assert_eq!(lots.value[0].cost, 1_000);
        assert_eq!(lots.value[1].cost, 10_000);

        let fifo = sql_fifo_take(&lots.value, 10).unwrap();
        assert_eq!(fifo.0, 1_000, "the seq index is FIFO-shaped — that is the trap");
        assert_eq!(fifo.1, 0);

        let got = store.relieve("hifo", B, 1, "vti", relief::Method::Hifo, 10, &pin).unwrap();
        assert_eq!(got.prefix, pin.prefix);
        assert_eq!(got.value.cost, 10_000, "HIFO takes the dear lot");
        assert_eq!(got.value.taken[0].seq, 1);
        assert_ne!(got.value.cost, fifo.0, "elected relief is not the seq scan");

        let memory = relief::relieve_by(relief::Method::Hifo, &lots.value, 10).unwrap();
        assert_eq!(got.value, memory);
    }

    #[test]
    fn fifo_still_uses_the_proved_walk_not_the_index_order() {
        // ⚠ EVEN UNDER FIFO the walk is `relieve_by`. The index happening to
        // be in the right order is not the proof — a husk, a partial that
        // will not divide, and a short holding are all refused by the walk
        // and would be silently wrong as a SQL take.
        let (d, _) = seed(
            "fifo",
            b"lot_method = \"fifo\"\nrules = []\n",
            &[("vti", 1_000, 10), ("vti", 10_000, 10)],
        );
        let mut store = SqlProjection::new();
        store.replay_book("fifo", &d).unwrap();
        let pin = pin_of(&d);
        let got = store.relieve("fifo", B, 1, "vti", relief::Method::Fifo, 10, &pin).unwrap();
        assert_eq!(got.value.cost, 1_000);
        assert_eq!(got.value.taken[0].seq, 0);
    }

    #[test]
    fn a_replaced_journal_at_the_same_height_refuses_replay_onto_the_old_snapshot() {
        let (d, _) = seed("swap", b"rules = []\n", &[("vti", 10_000, 10)]);
        let mut store = SqlProjection::new();
        store.replay_book("swap", &d).unwrap();
        let (other, _) = seed("swap-other", b"rules = []\n", &[("voo", 99_000, 3)]);
        // Same height, different bytes — the digest is the SoR, not the count.
        let err = store.replay_book("swap", &other).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("digest"), "{msg}");
        assert!(msg.contains("replaced"), "{msg}");
    }
}
