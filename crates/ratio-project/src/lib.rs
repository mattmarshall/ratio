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
    actions: Actions,
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
    pub fn advance(&mut self, journal: &[JournalEntry]) {
        for entry in journal.iter().skip(self.at) {
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
                match &p.instrument {
                    Some(i) => {
                        let slot =
                            self.positions.held.entry((p.dim, i.clone())).or_insert((0, 0));
                        slot.0 += p.amount;
                        slot.1 += p.quantity.unwrap_or(0);
                    }
                    None => *self.positions.rest.entry(p.dim).or_default() += p.amount,
                }
            }
        }
        self.at = journal.len();
    }

    /// Build from scratch.
    ///
    /// Discards everything first, so this is a rebuild rather than a second
    /// advance — the distinction `//tla:rebuild_double_counts_check` is about.
    pub fn rebuild(journal: &[JournalEntry]) -> Self {
        let mut p = Self::new();
        p.advance(journal);
        p
    }

    /// Open a book and build a projection of it.
    pub fn of_book(path: &std::path::Path) -> Result<Self> {
        Ok(Self::rebuild(&FileBook::open(path)?.entries()?))
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
