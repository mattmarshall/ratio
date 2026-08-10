//! ratio-project — the derived read model, and the type that makes its one
//! catastrophic failure unrepresentable.
//!
//! # Why there is a projection at all
//!
//! `FileBook::positions` folds the whole journal on every call. At the rate
//! `ratio_nav::closure::measure` reports, twenty million lots is 89 seconds —
//! and `Ratio.Exec.no_partition_beats_the_io_floor` says no number of workers
//! touches that, because it is IO. So reads move to a projection folded once
//! and advanced incrementally, while the journal stays the system of record
//! because replay and content-addressed digests are the product.
//!
//! # ⛔ The one way this goes catastrophically wrong
//!
//! `//tla:projection_check` proves the safety condition and
//! `//tla:unpinned_projection_check` shows it failing: a figure PINS one
//! journal position and READS a projection built from another. Nothing
//! downstream can notice. The trial balance ties on whatever it is handed, the
//! digest is well-formed, and `ratio replay` recomputes from the pinned prefix
//! and disagrees — by which time the first number is what somebody was paid on
//! and `Ratio.Period.one_answer_per_day` refuses to restate it.
//!
//! ⭐ SO THE POSITION IS NOT A FIELD A CALLER LOOKS UP. Every read returns
//! [`AsOf`], which carries the prefix it was folded from, and there is no other
//! way to get a number out of this crate. A caller cannot pin the journal head
//! while reading a lagging projection because it never has the head to hand —
//! it has only what it read. The TLA property is `StrikeFoldsItsOwnPrefix`;
//! here it is the type.
//!
//! ⚠ That is a stronger guarantee than a test, and a weaker one than it looks:
//! nothing stops a caller writing `.value` and pairing it with a position from
//! somewhere else. What the type buys is that doing so requires saying so.

use std::collections::BTreeMap;

/// The per-lot relief decisions, authored in Lean.
mod generated_lots;

/// Relieving tax lots — the walk, over decisions made in Lean.
pub mod relief;

use anyhow::Result;
use ratio_ingest::factor::Step;
use ratio_store::{FileBook, Journal, JournalEntry};

/// A value read from the projection, carrying the journal prefix it was folded
/// from.
///
/// ⛔ THE PREFIX TRAVELS WITH THE VALUE. A strike records `prefix`, not the
/// journal's length, and that is the whole safety argument — see the module
/// docs and `//tla:unpinned_projection_check`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsOf<T> {
    pub value: T,
    /// Entries folded. Also the position a figure built from this must pin.
    pub prefix: usize,
}

impl<T> AsOf<T> {
    /// Transform the value, keeping the prefix. The prefix cannot be changed by
    /// this or any other method — it is set once, by the fold that produced it.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AsOf<U> {
        AsOf { value: f(self.value), prefix: self.prefix }
    }
}

/// Positions, folded from a journal prefix.
///
/// Shaped like `FileBook::positions` returns them so the two can be compared
/// directly — which `the_projection_agrees_with_a_full_fold` does, because a
/// read model that drifts from the record it derives from is worse than no read
/// model at all.
/// Running totals a NAV needs, accumulated as the journal is folded.
///
/// ⛔ ACCUMULATED, NOT RECOMPUTED. `ratio_nav::fold_nav` walks every entry on
/// every strike — O(journal), and the journal holds every trade ever made. These
/// move by exactly what each new entry contributes, so a strike off a maintained
/// projection is O(positions): `Ratio.Plan.aggregate_agrees_with_scan`, and
/// `Ratio.Plan.a_stale_total_makes_the_plans_disagree` is what goes wrong if
/// they ever drift.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// Postings by dimension, whatever the account type. The NAV picks out
    /// assets and liabilities; the projection does not know the chart.
    ///
    /// ⛔ `i128`, LIKE `ratio_nav::fold_nav`. These accumulate over the whole
    /// journal, so `debits` in particular grows with HISTORY rather than with
    /// the fund — it adds the magnitude of every posting ever made. An `i64`
    /// accumulator wraps, and a wrapped total does not look wrong; it looks
    /// like a NAV.
    pub by_dim: BTreeMap<i64, i128>,
    pub debits: i128,
    pub credits: i128,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Positions {
    /// `(dim, instrument) -> (cost, quantity)`.
    pub held: BTreeMap<(i64, String), (i64, i64)>,
    /// `dim -> amount`, for postings naming no instrument.
    pub rest: BTreeMap<i64, i64>,
}

/// The corporate actions a projection has seen, and whether each was applied by
/// REWRITING the lots or is still to be read through as a factor.
///
/// ⛔ AN ACTION IS ONE OR THE OTHER, NEVER BOTH, and the journal says which.
/// Every book written before `Ratio.Actions.Factor` has `action-{id}` entries
/// that already walked the lots — the units in the projection include those
/// splits. Applying a factor on top would square them, silently, while the
/// trial balance went on tying: `Ratio.Actions.applying_twice_is_not_applying_
/// once` in a new costume.
///
/// So the rule is derived rather than configured: an announcement whose
/// `action-{id}` entry is in the prefix has ALREADY been rewritten and must not
/// be read through; one without is read through and costs nothing to leave
/// open. That is the whole win — an outstanding action stops being a cliff.
#[derive(Clone, Debug, Default)]
struct Actions {
    /// `instrument -> (ex_date, id, num, den)`, in journal order.
    announced: Vec<(String, String, String, i64, i64)>,
    /// Ids whose rewrite entry is in the prefix.
    rewritten: std::collections::BTreeSet<String>,
}

/// The read model.
#[derive(Clone, Debug, Default)]
pub struct Projection {
    positions: Positions,
    totals: Totals,
    actions: Actions,
    /// How far into the journal FILE this has read.
    ///
    /// ⛔ BYTES, NOT ENTRIES, and the two are not interchangeable. `at` says how
    /// many entries were folded; this says where to resume reading without
    /// parsing what came before. A projection that tracked only `at` would have
    /// to read and discard the whole journal to find entry `at + 1`.
    read_to: u64,
    /// Entries folded so far.
    ///
    /// ⛔ PRIVATE, and there is no setter. `advance` moves it by exactly what
    /// it folded. `//tla:rebuild_double_counts_check` is the failure this
    /// prevents: a projection whose claimed position stays honest while its
    /// contents are folded twice is not detectably wrong — the number is simply
    /// too big.
    at: usize,
}

impl Projection {
    /// An empty projection, at position zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many entries this has folded.
    pub fn prefix(&self) -> usize {
        self.at
    }

    /// Fold the entries this has not seen.
    ///
    /// ⛔ FROM `self.at`, NEVER FROM ZERO. Re-folding onto state already held
    /// double-counts, and the result carries an honest position over doubled
    /// contents. `advance` takes the WHOLE journal and skips what it has,
    /// rather than taking a delta the caller computed — a delta is one more
    /// thing a caller can get wrong, and getting it wrong is silent.
    /// Returns how many entries it folded.
    ///
    /// ⛔ RETURNED SO THE INCREMENTAL PROPERTY IS OBSERVABLE. "It advances
    /// rather than rebuilding" is otherwise only checkable by timing, and a
    /// timing test that passes on a rebuild fast enough to look incremental
    /// proves nothing. A maintained projection folds the DELTA; this is the
    /// number that says so.
    pub fn advance(&mut self, journal: &[JournalEntry]) -> usize {
        for entry in journal.iter().skip(self.at) {
            self.fold(entry);
        }
        let folded = journal.len().saturating_sub(self.at);
        // ⛔ `max`, NOT `= journal.len()`. A SHORTER journal must not rewind the
        // prefix — the entries were folded and the totals still hold them.
        // Assigning the length outright let a truncated or sliced read move the
        // position BACKWARD, after which the next advance re-folds everything
        // between and double-counts it: `//tla:rebuild_double_counts_check`,
        // reachable without any rebuild at all.
        //
        // ⚠ Found by `a_maintained_projection_folds_only_the_delta`, which was
        // written to check the incremental property and caught this instead.
        // Every other test in this file passed — none of them ever handed
        // `advance` a journal shorter than one it had already seen.
        self.at = journal.len().max(self.at);
        folded
    }

    /// Fold ONE entry into the totals, the positions, and the action index.
    ///
    /// ⛔ THE ONLY PLACE AN ENTRY IS FOLDED. `advance` takes a slice and
    /// `follow` reads bytes off disk, and both go through here — two folds
    /// would be two chances to disagree about what an entry means, and the
    /// disagreement would be a NAV that changed depending on how the projection
    /// happened to be brought up to date.
    fn fold(&mut self, entry: &JournalEntry) {
        if let Some(a) = &entry.announcement {
            self.actions.announced.push((
                a.instrument.clone(),
                a.ex_date.clone(),
                a.id.clone(),
                a.numerator,
                a.denominator,
            ));
        }
        if let Some(id) = entry.id.strip_prefix("action-") {
            self.actions.rewritten.insert(id.to_string());
        }
        for p in &entry.postings {
            // ⚠ `-p.amount` is not the magnitude at the floor: `-i64::MIN`
            // overflows. Widened first, it does not.
            let amount = p.amount as i128;
            *self.totals.by_dim.entry(p.dim).or_default() += amount;
            if amount >= 0 {
                self.totals.debits += amount;
            } else {
                self.totals.credits += -amount;
            }
            match &p.instrument {
                Some(i) => {
                    let slot = self.positions.held.entry((p.dim, i.clone())).or_insert((0, 0));
                    slot.0 += p.amount;
                    slot.1 += p.quantity.unwrap_or(0);
                }
                None => *self.positions.rest.entry(p.dim).or_default() += p.amount,
            }
        }
    }

    /// Build from scratch.
    ///
    /// Discards everything first, so this is a rebuild rather than a second
    /// advance — the distinction `//tla:rebuild_double_counts_check` is about.
    pub fn rebuild(journal: &[JournalEntry]) -> Self {
        let mut p = Self::new();
        let _ = p.advance(journal);
        p
    }

    /// Open a book and build a projection of it.
    pub fn of_book(path: &std::path::Path) -> Result<Self> {
        let mut p = Self::new();
        p.follow(path)?;
        Ok(p)
    }

    /// Fold whatever has been appended since this last read. Returns how many.
    ///
    /// ⭐ THIS IS WHAT MAKES THE FLAT CURVE TRUE IN A RUNNING PROCESS. A
    /// projection that called `entries()` would parse the whole journal on
    /// every call just to learn nothing had changed; this seeks to where it
    /// stopped and reads forward. Calling it on an unchanged book costs a
    /// `stat` and a seek.
    ///
    /// ⚠ It can FAIL, and the failure is worth having. A journal shorter than
    /// the offset means the file was replaced — a different book at the same
    /// path — and resuming from a stale offset would splice two histories and
    /// fold the result as one.
    pub fn follow(&mut self, path: &std::path::Path) -> Result<usize> {
        let (fresh, now) = FileBook::open(path)?.entries_since(self.read_to)?;
        let n = fresh.len();
        for entry in &fresh {
            self.fold(entry);
        }
        self.at += n;
        self.read_to = now;
        Ok(n)
    }

    /// The positions, as of the prefix folded.
    ///
    /// ⛔ THE ONLY WAY OUT OF THIS TYPE, and it hands back the prefix with the
    /// value. There is deliberately no `fn positions(&self) -> &Positions`.
    pub fn positions(&self) -> AsOf<&Positions> {
        AsOf { value: &self.positions, prefix: self.at }
    }

    /// Total cost held in one instrument, across every account.
    pub fn cost_of(&self, instrument: &str) -> AsOf<i64> {
        AsOf {
            value: self
                .positions
                .held
                .iter()
                .filter(|((_, i), _)| i == instrument)
                .map(|(_, (cost, _))| *cost)
                .sum(),
            prefix: self.at,
        }
    }

    /// Net asset value and the trial-balance difference, off the maintained
    /// totals rather than a walk over the journal.
    ///
    /// ⭐ THIS IS THE POINT OF THE WHOLE CRATE. `ratio_nav::fold_nav` is
    /// O(journal) and this is O(dimensions) — a chart, not a history. The figure
    /// must be IDENTICAL, which `the_projection_strikes_the_same_nav_as_a_full_
    /// fold` checks against the existing path rather than against itself.
    ///
    /// A liability nets negative because it is credit-normal, so summing assets
    /// and liabilities subtracts without a special case — the same fold as
    /// `ratio_nav`, and a sign error here is invisible in a screenshot and wrong
    /// by twice the liability.
    pub fn nav(&self, is_asset_or_liability: &dyn Fn(i64) -> bool) -> Result<AsOf<(i64, i64)>> {
        let nav: i128 = self
            .totals
            .by_dim
            .iter()
            .filter(|(dim, _)| is_asset_or_liability(**dim))
            .map(|(_, amount)| *amount)
            .sum();
        // ⛔ A figure that cannot be represented is REFUSED rather than
        // truncated. `Ratio.Bounded`: an operation agrees with the theorem or
        // declines, and there is no third answer.
        Ok(AsOf {
            value: (
                i64::try_from(nav).map_err(|_| {
                    anyhow::anyhow!("this fund's net asset value does not fit in 64 bits")
                })?,
                i64::try_from(self.totals.debits - self.totals.credits).map_err(|_| {
                    anyhow::anyhow!("this fund's trial-balance difference does not fit in 64 bits")
                })?,
            ),
            prefix: self.at,
        })
    }

    /// The splits an instrument's stored units must be read through, on a day.
    ///
    /// Announced on or before the day, inside this prefix, and NOT already
    /// rewritten. In journal order, which is ex-date order for anything
    /// announced before it took effect — and
    /// `Ratio.Actions.actions_do_not_commute` means that order is part of the
    /// answer rather than an implementation detail.
    pub fn steps_for(&self, instrument: &str, day: &str) -> Vec<Step> {
        self.actions
            .announced
            .iter()
            .filter(|(i, ex, id, _, _)| {
                i == instrument && ex.as_str() <= day && !self.actions.rewritten.contains(id)
            })
            .map(|(_, _, _, num, den)| Step { num: *num, den: *den })
            .collect()
    }

    /// Units held in one instrument on a day, read through its open actions.
    ///
    /// ⭐ THIS IS WHAT MAKES AN OUTSTANDING ACTION FREE. Nothing is rewritten,
    /// so `Ratio.Closure.factored_nav_never_reads_the_lots` holds — the cost is
    /// O(splits) on the one instrument rather than O(lots) over all of them.
    ///
    /// ⚠ It can REFUSE, and that is not a bug in the read path. A step that
    /// does not divide means the holder was paid cash in lieu, which realizes a
    /// gain and is a posting the configuration must declare:
    /// `Ratio.Actions.Factor.a_factor_can_succeed_where_the_rewrite_refuses`.
    pub fn units_as_of(&self, dim: i64, instrument: &str, day: &str) -> Result<AsOf<i64>> {
        let stored = self
            .positions
            .held
            .get(&(dim, instrument.to_string()))
            .map(|(_, q)| *q)
            .unwrap_or(0);
        Ok(AsOf {
            value: ratio_ingest::factor::units_as_of(stored, &self.steps_for(instrument, day))?,
            prefix: self.at,
        })
    }

    /// Whether this has caught up with a journal of the given length.
    ///
    /// ⚠ Lagging is SAFE — `//tla:projection_check` proves it, because a figure
    /// pins what it read. This exists so a caller can wait for freshness when
    /// it wants freshness, not because a stale read would be wrong.
    pub fn is_current_with(&self, journal_len: usize) -> bool {
        self.at == journal_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::{Account, AccountTypeRecord as A, ConfigStore, PostingRecord};

    fn book(name: &str, trades: &[(&str, i64, i64)]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
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
                    PostingRecord {
                        dim: 1,
                        amount: *cost,
                        instrument: Some((*inst).into()),
                        quantity: Some(*qty),
                    },
                    PostingRecord::new(2, -*cost),
                ],
            
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    fn entries(d: &std::path::Path) -> Vec<JournalEntry> {
        FileBook::open(d).unwrap().entries().unwrap()
    }

    #[test]
    fn the_projection_agrees_with_a_full_fold() {
        // ⛔ AGAINST THE SYSTEM OF RECORD, not against itself. A read model
        // that drifts from the journal it derives from is worse than none —
        // it is a second opinion nobody asked for and nobody can adjudicate.
        let d = book("agrees", &[("vti", 25_000, 100), ("voo", 10_000, 40), ("vti", 5_000, 20)]);
        let js = entries(&d);
        let p = Projection::rebuild(&js);

        let (held, rest) = FileBook::open(&d).unwrap().positions().unwrap();
        assert_eq!(p.positions().value.held, held);
        assert_eq!(p.positions().value.rest, rest);
        assert_eq!(p.prefix(), 3);
    }

    #[test]
    fn the_projection_strikes_the_same_nav_as_a_full_fold() {
        // ⭐ AGAINST THE EXISTING PATH, NOT AGAINST ITSELF. `ratio_nav::strike`
        // walks every entry; this reads maintained totals. The whole value of
        // the projection is that the figures are the same number, and a test
        // that compared the projection to another projection would prove only
        // that it is consistent with its own mistake.
        let d = book(
            "navsame",
            &[("vti", 25_000, 100), ("voo", 10_000, 40), ("vti", 5_000, 20)],
        );
        let js = entries(&d);
        let p = Projection::rebuild(&js);

        // dims 1 and 2 are assets in `book()`; nothing else is.
        let got = p.nav(&|dim| dim == 1 || dim == 2).unwrap();
        let want = ratio_nav::strike(&d, 1_782_662_400, "e.marsh").unwrap();

        assert_eq!(got.value.0, want.net_asset_value, "the same NAV");
        assert_eq!(got.value.1, want.trial_balance_difference, "and the same difference");
        assert_eq!(got.prefix, want.journal_position, "over the same prefix");
    }

    #[test]
    fn the_nav_ignores_dimensions_that_are_not_assets_or_liabilities() {
        // Capital is equity. Including it would net the NAV to zero — the
        // figure would look "balanced" and be worthless, which is the sign
        // error this fold exists to avoid.
        let d = book("navdims", &[("vti", 25_000, 100)]);
        let p = Projection::rebuild(&entries(&d));
        assert_eq!(p.nav(&|dim| dim == 1 || dim == 2).unwrap().value.0, 0, "buy: asset in, cash out");
        assert_eq!(p.nav(&|dim| dim == 1).unwrap().value.0, 25_000, "investments alone");
    }

    #[test]
    fn totals_advance_rather_than_being_recomputed() {
        // The incremental property: catching up in pieces lands where folding
        // the lot from scratch would. `Ratio.Plan.a_stale_total_makes_the_plans
        // _disagree` is what a drifted total would cause.
        let d = book("navincr", &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3)]);
        let js = entries(&d);
        let mut piecemeal = Projection::new();
        for n in 1..=js.len() {
            piecemeal.advance(&js[..n]);
        }
        let assets = |dim: i64| dim == 1 || dim == 2;
        assert_eq!(piecemeal.nav(&assets).unwrap(), Projection::rebuild(&js).nav(&assets).unwrap());
    }

    #[test]
    fn advancing_twice_folds_each_entry_once() {
        // ⛔ `//tla:rebuild_double_counts_check`. A second advance over the same
        // journal must be a no-op. If it re-folded, the position would stay
        // honest and the contents would double — and nothing about the number
        // would look wrong.
        let d = book("twice", &[("vti", 25_000, 100), ("vti", 5_000, 20)]);
        let js = entries(&d);

        let mut p = Projection::new();
        p.advance(&js);
        let once = p.cost_of("vti");
        p.advance(&js);
        let twice = p.cost_of("vti");

        assert_eq!(once.value, 30_000);
        assert_eq!(twice, once, "a second advance over the same journal folds nothing");
        assert_eq!(p.prefix(), 2);
    }

    #[test]
    fn advancing_incrementally_equals_rebuilding() {
        // The whole reason `advance` exists: catching up in pieces must land
        // exactly where folding the lot from scratch would.
        let d = book("incr", &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3), ("bnd", 40, 4)]);
        let js = entries(&d);

        let mut piecemeal = Projection::new();
        for n in 1..=js.len() {
            piecemeal.advance(&js[..n]);
        }
        assert_eq!(piecemeal.positions().value, &Projection::rebuild(&js).positions().value.clone());
        assert_eq!(piecemeal.prefix(), js.len());
    }

    #[test]
    fn a_maintained_projection_folds_only_the_delta() {
        // ⭐ STEP 3 OF THE SEAM, AND THE ONE THAT MAKES THE FLAT CURVE REAL IN
        // PRODUCTION. `of_book` rebuilds from zero — O(journal) — which is the
        // cost the benchmark reports as COLD BUILD. A process that keeps a
        // projection pays it once and then folds only what arrived.
        //
        // ⚠ Asserted by COUNT, not by timing. A rebuild fast enough to look
        // incremental would pass a stopwatch and fail this.
        let d = book("delta", &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3)]);
        let js = entries(&d);

        let mut p = Projection::new();
        assert_eq!(p.advance(&js), 3, "the first pass folds everything");
        assert_eq!(p.advance(&js), 0, "and a second folds nothing at all");
        assert_eq!(p.advance(&js[..2]), 0, "a SHORTER journal folds nothing either");

        // One more arrives.
        let mut grown = js.clone();
        grown.push(js[0].clone());
        assert_eq!(p.advance(&grown), 1, "only the new entry");
        assert_eq!(p.prefix(), 4);
    }

    fn append_one(d: &std::path::Path, id: &str) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "later".into(),
            config: c,
            postings: vec![
                PostingRecord { dim: 1, amount: 7, instrument: Some("vti".into()), quantity: Some(1) },
                PostingRecord::new(2, -7),
            ],
            announcement: None,
        })
        .unwrap();
    }

    #[test]
    fn following_a_book_reads_only_what_was_appended() {
        // ⭐ THE PIECE THAT MAKES MAINTENANCE REAL. `entries()` parses the whole
        // journal, so a cached projection built on it pays O(journal) just to
        // learn nothing changed — a rebuild with a cache in front of it.
        // `follow` seeks to where it stopped.
        let d = book("follow", &[("vti", 10, 1), ("voo", 20, 2)]);
        let mut p = Projection::new();
        assert_eq!(p.follow(&d).unwrap(), 2, "first pass folds both");
        assert_eq!(p.follow(&d).unwrap(), 0, "an unchanged book folds nothing");

        append_one(&d, "later-1");
        assert_eq!(p.follow(&d).unwrap(), 1, "only the new entry");
        assert_eq!(p.prefix(), 3);

        // And it lands exactly where a cold build would.
        assert_eq!(p.positions().value, &Projection::of_book(&d).unwrap().positions().value.clone());
        let assets = |dim: i64| dim == 1 || dim == 2;
        assert_eq!(p.nav(&assets).unwrap().value, Projection::of_book(&d).unwrap().nav(&assets).unwrap().value);
    }

    #[test]
    fn a_journal_that_shrank_is_refused_rather_than_spliced() {
        // ⛔ An append-only log does not shrink, so a shorter file at the same
        // path is a DIFFERENT BOOK. Resuming from the stale offset would splice
        // two histories and fold the result as one — every figure downstream
        // would be built from a mixture nothing could reproduce.
        let d = book("shrank", &[("vti", 10, 1), ("voo", 20, 2), ("bnd", 30, 3)]);
        let mut p = Projection::new();
        p.follow(&d).unwrap();

        // A different, shorter book at the same path.
        let _ = book("shrank", &[("vti", 10, 1)]);
        let err = p.follow(&d).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("does not shrink"), "{msg}");
    }

    #[test]
    fn a_read_carries_the_prefix_it_was_folded_from() {
        // ⭐ `//tla:unpinned_projection_check` as a type rather than a test.
        //
        // The projection lags the journal by two entries. Anything built from
        // this read has ONLY `prefix` to pin — the journal's length is not
        // reachable from an `AsOf`, so pinning the head while reading stale
        // data takes a deliberate act rather than an oversight.
        let d = book("pinned", &[("vti", 10, 1), ("vti", 20, 2), ("vti", 30, 3)]);
        let js = entries(&d);

        let mut p = Projection::new();
        p.advance(&js[..1]);

        let read = p.cost_of("vti");
        assert_eq!(read.prefix, 1, "what it folded");
        assert_eq!(read.value, 10, "and the value agrees with that prefix, not the journal");
        assert_ne!(read.prefix, js.len(), "the journal has moved on, and the read has not");
        assert!(!p.is_current_with(js.len()));
    }

    #[test]
    fn map_cannot_change_the_prefix() {
        // The prefix is set by the fold and by nothing else. `map` exists so a
        // caller can shape the value without ever getting the chance to restate
        // where it came from.
        let d = book("map", &[("vti", 10, 1)]);
        let p = Projection::rebuild(&entries(&d));
        let doubled = p.cost_of("vti").map(|v| v * 2);
        assert_eq!(doubled, AsOf { value: 20, prefix: 1 });
    }

    fn announce(id: &str, inst: &str, num: i64, den: i64, ex: &str, cfg: &ratio_store::Digest) -> JournalEntry {
        JournalEntry {
            id: format!("announce-{id}"),
            memo: String::new(),
            config: cfg.clone(),
            postings: Vec::new(),
            announcement: Some(ratio_store::AnnouncementRecord {
                id: id.into(),
                instrument: inst.into(),
                numerator: num,
                denominator: den,
                ex_date: ex.into(),
                announced: 0,
            }),
        }
    }

    #[test]
    fn an_outstanding_action_is_read_through_and_costs_nothing() {
        // ⭐ THE WHOLE POINT. A 2-for-1 announced and never applied: the stored
        // units are untouched, nothing was rewritten, and the holding reads
        // correctly on any day at or after the ex-date.
        let d = book("open", &[("vti", 25_000, 100)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js);

        assert_eq!(p.units_as_of(1, "vti", "2026-01-14").unwrap().value, 100, "before the ex-date");
        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().value, 200, "on and after it");
        assert_eq!(
            p.positions().value.held[&(1, "vti".into())].1,
            100,
            "and the STORED units were never rewritten — that is the saving"
        );
    }

    #[test]
    fn the_ex_date_itself_is_included() {
        // ⛔ THE BOUNDARY, AND MY TESTS DID NOT REACH IT. Two mutation probes
        // survived this file — `<= day` weakened to `< day` passed, because
        // every case used a day strictly before or strictly after. A NAV struck
        // ON the ex-date must include the action: that is what "effective" means
        // and it is the day the market re-priced.
        let d = book("exday", &[("vti", 100, 50)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js);

        assert_eq!(p.units_as_of(1, "vti", "2026-01-14").unwrap().value, 50, "the day before");
        assert_eq!(p.units_as_of(1, "vti", "2026-01-15").unwrap().value, 100, "ON the ex-date");
    }

    #[test]
    fn a_split_scales_only_its_own_instrument() {
        // ⛔ ALSO A SURVIVING MUTATION. Dropping the instrument filter entirely
        // left every test green, because no book here held two names — so
        // "steps are per instrument" was asserted nowhere and a split on one
        // holding would have scaled every other.
        let d = book("two", &[("vti", 100, 50), ("voo", 200, 80)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js);

        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().value, 100, "split");
        assert_eq!(p.units_as_of(1, "voo", "2026-02-01").unwrap().value, 80, "untouched");
        assert!(p.steps_for("voo", "2026-02-01").is_empty());
    }

    #[test]
    fn an_action_already_rewritten_is_not_read_through_as_well() {
        // ⛔ THE MIGRATION HAZARD. Every book written before the factor path has
        // `action-{id}` entries that already walked the lots. Reading the same
        // split through on top would SQUARE it — 400 units where there are 200 —
        // while the cost stayed put and the trial balance went on tying.
        // `Ratio.Actions.applying_twice_is_not_applying_once`, in a new costume.
        let d = book("rewritten", &[("vti", 25_000, 100)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        // the rewrite: units doubled in the stored figures
        js.push(JournalEntry {
            id: "action-ca-1".into(),
            memo: "applied".into(),
            config: cfg,
            postings: vec![ratio_store::PostingRecord {
                dim: 1,
                amount: 0,
                instrument: Some("vti".into()),
                quantity: Some(100),
            }],
            announcement: None,
        });
        let p = Projection::rebuild(&js);

        assert_eq!(
            p.positions().value.held[&(1, "vti".into())].1,
            200,
            "the rewrite is in the stored units"
        );
        assert!(p.steps_for("vti", "2026-02-01").is_empty(), "so it is NOT read through");
        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().value, 200, "not 400");
    }

    #[test]
    fn a_read_that_would_owe_cash_in_lieu_refuses() {
        // ⚠ Not a bug in the read path. A step that does not divide means the
        // holder was paid cash for a fraction, which realizes a gain and is a
        // posting the configuration has to declare.
        let d = book("lieu", &[("vti", 100, 5)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 3, 2, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js);

        assert_eq!(p.units_as_of(1, "vti", "2026-01-14").unwrap().value, 5, "before: fine");
        let err = p.units_as_of(1, "vti", "2026-02-01").unwrap_err();
        assert!(format!("{err:#}").contains("cash in lieu"), "{err:#}");
    }

    #[test]
    fn a_factor_read_still_carries_its_prefix() {
        // The safety property survives the new read path — `AsOf`, not a bare
        // number, because a figure built from this must pin what it folded.
        let d = book("prefixed", &[("vti", 10, 1)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js);
        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().prefix, 2);
    }

    #[test]
    fn an_empty_journal_projects_to_nothing_at_position_zero() {
        let p = Projection::rebuild(&[]);
        assert_eq!(p.prefix(), 0);
        assert_eq!(p.cost_of("vti"), AsOf { value: 0, prefix: 0 });
        assert!(p.is_current_with(0), "current with an empty journal, not stale");
    }
}
