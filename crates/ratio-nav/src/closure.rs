//! What a period end costs, before anybody runs it.
//!
//! ⛔ THE MODEL HERE IS NOT WRITTEN HERE. `Dials`, `nav_cost`, and the four
//! terms it sums are EMITTED from `Ratio.Closure` into `generated.rs`. This
//! module supplies only the two things a proof cannot: a rate measured against
//! a real store, and a way to say the answer out loud.
//!
//! The reason to care is one theorem. `Ratio.Closure.nav_never_reads_the_lots`
//! says fragmentation is not in `nav_cost` at all — so a fund's twentieth year
//! strikes as fast as its first, and twenty million tax lots are free. Beside
//! it, `an_open_action_costs_a_whole_chart`: ONE unapplied corporate action
//! costs more than an entire quiet period end. Both are proved. What this
//! module adds is the ability to put a duration on them.
//!
//! # ⚠ Reads are proved. Seconds are measured, and measurement can go stale.
//!
//! `nav_cost` returns READS. Every claim about how long a NAV takes is that
//! number multiplied by a rate, and the rate is a property of a machine on a
//! day — not of the model, and not of anything Lean can say. Keeping the two
//! apart is the whole point of this module's shape: `Estimate::reads` carries
//! the proved part, `Estimate::nanos` the part that had to be measured, and
//! `Calibration::provenance` says where the rate came from.
//!
//! `measure` re-derives the rate from an actual book, and
//! `the_shipped_rate_is_not_fiction` is the test that fails when the shipped
//! constant has drifted away from what this machine actually does.

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Instant;

pub use crate::generated::{
    action_cost, capital_cost, factor_action_cost, factor_nav_cost, fx_cost, lots, mark_cost,
    nav_cost, Dials, PerShare,
};

/// Net asset value per unit in issue, and what the division left over.
///
/// ⛔ THE EMITTED `per_share` PANICS ON TWO INPUTS, MEASURED:
///
/// ```text
/// per_share(100, 0)     PANIC — attempt to divide by zero
/// per_share(MIN, -1)    PANIC — attempt to divide with overflow
/// ```
///
/// Neither is exotic. A fund with no units in issue is every fund on the day it
/// is established, and Rust's `div_euclid` panics on a zero divisor in EVERY
/// build profile — so this was a crash on the NAV path, not a wrong number.
///
/// `Ratio.Closure.residual_is_accounted` is stated over `Int`, where both of
/// these are ordinary questions with ordinary answers. That is the gap
/// `Ratio.Bounded` names: the theorem was always assuming the operation
/// answered at all, and nobody had written the hypothesis down.
///
/// ⚠ The emitted function is no longer re-exported. It is still the proved
/// arithmetic and still correct under its precondition — but a caller reaching
/// for `per_share` should get the one that cannot crash, and leaving both
/// visible under one name is how the wrong one gets picked.
pub fn per_share(nav: i64, units: i64) -> anyhow::Result<PerShare> {
    use ratio_common::checked;
    Ok(PerShare {
        per_unit: checked::div_euclid(nav, units, "net asset value per unit")?,
        residual: checked::rem_euclid(nav, units, "the per-unit residual")?,
    })
}

/// Nanoseconds per read.
///
/// ⛔ SEPARATE TYPE, NOT A BARE `i64`, so it cannot be passed where a count of
/// reads is wanted. The two numbers in this module are a proved quantity and a
/// measured one, and the entire honesty of the estimate rests on not confusing
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calibration {
    pub nanos_per_read: i64,
    /// Where this number came from, carried so it can be printed beside any
    /// duration derived from it. A rate with no provenance is a guess wearing
    /// a decimal point.
    pub provenance: String,
}

impl Calibration {
    /// The shipped default, for when there is no book to measure against.
    ///
    /// Taken by `measure` on 2026-08-09: an Apple M-series laptop reading a
    /// 500-entry file-backed journal warm in the page cache. That is the BEST
    /// case, named as the best case rather than passed off as typical — a
    /// network store or a cold read is orders of magnitude slower, so any
    /// estimate built on this rate is a floor.
    ///
    /// ⛔ THIS NUMBER WAS 250 UNTIL IT WAS ACTUALLY MEASURED, AND THE REAL
    /// FIGURE IS 4,436 — eighteen times larger. I wrote a plausible-looking
    /// constant, documented it as "measured", and shipped a guard whose band
    /// was ±100× so it passed without complaint. A tolerance wide enough to
    /// admit an order-of-magnitude error is not a check, and the whole module
    /// around it is about not confusing a measured number with a chosen one.
    ///
    /// The band below is now ±10×, which is loose enough for a slow CI runner
    /// and tight enough that the mistake it was written for would have failed.
    pub fn measured() -> Self {
        Self {
            nanos_per_read: 4_436,
            // ⚠ "shipped", never "measured here". The two are distinguishable
            // on sight because a reader who cannot tell a live measurement from
            // a constant will read the constant as a measurement — and the
            // whole reason this field exists is to stop that.
            provenance: "shipped rate — local file-backed journal, warm cache; \
                         a FLOOR, not a typical rate"
                .into(),
        }
    }
}

/// Render a nanosecond count in a unit a person can read.
///
/// ⛔ NOT ALWAYS MILLISECONDS. A small period end lands in microseconds, and
/// printing that as `0 ms` reads as "instant" or "broken" rather than as a
/// small number — which is how a working estimate gets mistaken for a stub.
pub fn human_nanos(n: i64) -> String {
    match n {
        n if n < 1_000 => format!("{n} ns"),
        n if n < 1_000_000 => format!("{} µs", n / 1_000),
        n if n < 1_000_000_000 => format!("{}.{} ms", n / 1_000_000, (n % 1_000_000) / 100_000),
        n => format!("{}.{} s", n / 1_000_000_000, (n % 1_000_000_000) / 100_000_000),
    }
}

/// What a period end costs, term by term.
///
/// ⛔ THE TERMS ARE KEPT APART ON PURPOSE. A single total would hide the only
/// actionable fact in the model: which dial to turn. `Ratio.Closure` proves the
/// terms have different shapes — marking is per security, fx is per CURRENCY
/// and not per position, actions are the one superlinear term, and lots appear
/// nowhere. Summing them first throws that away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estimate {
    pub dials: Dials,
    /// One price per security.
    pub marks: i64,
    /// One rate per currency — NOT one per position.
    pub fx: i64,
    /// ⛔ The superlinear term. `open_actions × securities`.
    pub actions: i64,
    /// One per subscription or redemption.
    pub capital: i64,
    /// The sum as the system works TODAY: `Ratio.Closure.navCost`, actions
    /// applied by rewriting the lots they touch.
    pub reads: i64,
    /// ⭐ The same period end once actions are factors:
    /// `Ratio.Closure.factorNavCost`.
    ///
    /// ⛔ REPORTED BESIDE `reads`, NOT INSTEAD OF IT. The flat-in-fragmentation
    /// claim holds of this one and not of the other, and quoting only the
    /// better number is the same mistake the model already made once — it
    /// charged an open action `openActions × securities` when a split touches
    /// every LOT of one instrument, understating its own worst term by eighty.
    pub factored_reads: i64,
    /// Open tax lots. ⛔ REPORTED AND NOT SUMMED — `nav_never_reads_the_lots`
    /// is precisely the claim that this number is not in `reads`. It is here so
    /// a reader can see the size of the thing that does not cost anything.
    pub open_lots: i64,
    /// `reads × nanos_per_read`. The measured part.
    pub nanos: i64,
    /// The same, for `factored_reads`.
    pub factored_nanos: i64,
    pub provenance: String,
}

impl Estimate {
    /// The share of the work that is corporate actions, in percent.
    ///
    /// The headline of the whole model: on an S&P tracker carrying twenty
    /// million tax lots, ONE unapplied split is 49.85% of the entire period
    /// end — 500 reads of 1,003 — while the twenty million lots are 0%.
    ///
    /// ⚠ I first wrote "over half" here and the test caught it at 49. Just
    /// under half is the true number and it makes the same point, which is
    /// exactly why rounding it up was tempting and not worth it.
    pub fn actions_share(&self) -> i64 {
        if self.reads == 0 {
            0
        } else {
            self.actions * 100 / self.reads
        }
    }
}

/// Cost a period end.
///
/// ⚠ THE DIALS ARE BOUNDS-CHECKED, and this is the mildest of the overflow
/// findings — deliberately noted as such. `lots` is `securities × lots_per` and
/// `action_cost` is `open_actions × lots_per`, so a caller who types enough
/// zeroes into `ratio closure` gets a wrapped product. A wrong answer here is
/// an embarrassing ESTIMATE, not a wrong book, which is why it is guarded last
/// and why the guard refuses rather than saturating: an estimate that silently
/// clamps is one somebody quotes.
pub fn estimate(d: Dials, c: &Calibration) -> Result<Estimate> {
    use ratio_common::checked;
    checked::mul(d.securities, d.lots_per, "open tax lots")?;
    checked::mul(d.open_actions, d.lots_per, "the cost of the open actions")?;
    checked::add(
        checked::add(d.securities, d.currencies, "the chart and the currencies")?,
        checked::add(
            checked::mul(d.open_actions, d.lots_per, "the cost of the open actions")?,
            d.capital_txns,
            "the actions and the capital",
        )?,
        "what a period end reads",
    )?;

    let reads = nav_cost(d);
    let factored = factor_nav_cost(d);
    Ok(Estimate {
        dials: d,
        factored_reads: factored,
        factored_nanos: factored.saturating_mul(c.nanos_per_read),
        marks: mark_cost(d),
        fx: fx_cost(d),
        actions: action_cost(d),
        capital: capital_cost(d),
        reads,
        open_lots: lots(d),
        nanos: reads.saturating_mul(c.nanos_per_read),
        provenance: c.provenance.clone(),
    })
}

/// Measure this machine's read rate against a real book.
///
/// ⛔ AGAINST THE ACTUAL STORE, not a synthetic loop. The model counts reads of
/// facts out of a journal, so the thing timed here is `Journal::entries` over
/// the book at `path` — the same call the strike path makes.
///
/// ⛔ AND IT REFUSES A JOURNAL TOO SMALL TO AMORTIZE THE FIXED COST, which is a
/// bigger trap than the empty one. Each `entries()` call pays to open and parse
/// the file whether it holds one record or ten thousand, so a naive
/// elapsed/records divides that fixed cost across however few reads there were.
/// Measured on one machine:
///
///     entries      1       10       50      100      500     2000
///     ns/read  227000    26736     7411     5296     4460     3462
///
/// ⚠ A 51× SPREAD, and every one of those numbers looks like a plausible
/// measurement. `nav_cost` counts MARGINAL reads — the model has no per-call
/// term — so a rate taken from a small book is not the quantity the model is
/// asking for, and multiplying by it produces a confident, wrong duration. The
/// floor is where the curve flattens, not a round number.
///
/// ⚠ It also errors on an EMPTY journal rather than returning a rate. Reporting
/// a nanosecond figure derived from having read nothing is the failure that
/// matters; a division by zero is not.
/// Below this the fixed per-call cost swamps the marginal one. See `measure`
/// for the measured curve this is read off.
pub const MIN_ENTRIES: usize = 100;

pub fn measure(path: &Path) -> Result<Calibration> {
    let b = FileBook::open(path).context("opening the book to measure it")?;

    // One pass to warm, then timed passes. An unwarmed first read measures the
    // filesystem, not the store.
    let n = b.entries()?.len();
    anyhow::ensure!(
        n > 0,
        "the journal at {} has no entries — there is nothing here to measure, and a \
         rate derived from zero reads would be a fabrication rather than a measurement",
        path.display()
    );
    anyhow::ensure!(
        n >= MIN_ENTRIES,
        "the journal at {} has {n} entries, and below {MIN_ENTRIES} the fixed cost of \
         opening and parsing the file dominates: the same code measures 227,000 ns/read \
         over one entry and 4,460 over five hundred. `nav_cost` counts MARGINAL reads, \
         so a rate taken here would not be the quantity the model multiplies",
        path.display()
    );

    const PASSES: usize = 20;
    let start = Instant::now();
    let mut read = 0usize;
    for _ in 0..PASSES {
        read += b.entries()?.len();
    }
    let elapsed = start.elapsed().as_nanos() as i64;

    Ok(Calibration {
        nanos_per_read: (elapsed / read.max(1) as i64).max(1),
        provenance: format!(
            "measured here — {read} reads over {PASSES} passes of {}",
            path.display()
        ),
    })
}

use ratio_store::{FileBook, Journal};

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::{Account, AccountTypeRecord as A, ConfigStore, PostingRecord};

    /// An S&P tracker: five hundred names, three currencies, twenty years of
    /// fragmentation behind it.
    const TRACKER: Dials = Dials {
        securities: 500,
        currencies: 3,
        lots_per: 40_000,
        open_actions: 0,
        capital_txns: 0,
    };

    #[test]
    fn twenty_million_tax_lots_are_free() {
        // ⛔ THE CLAIM, IN THE RUNNING CODE. `Ratio.Closure.
        // nav_never_reads_the_lots` proves the cost does not depend on
        // fragmentation; this is the same statement where somebody can run it.
        let year_one = Dials { lots_per: 1, ..TRACKER };
        let year_twenty = TRACKER;

        assert_eq!(lots(year_twenty), 20_000_000, "twenty million open lots");
        assert_eq!(lots(year_one), 500);
        assert_eq!(
            nav_cost(year_one),
            nav_cost(year_twenty),
            "a fund's twentieth year strikes as fast as its first"
        );
        assert_eq!(nav_cost(TRACKER), 503, "the chart plus the currencies");
    }

    #[test]
    fn one_open_action_costs_eighty_period_ends() {
        // ⛔ THE CLIFF, and the number that used to be here was 500.
        //
        // `action_cost` charged `open_actions × securities` until the model was
        // corrected: a split reaches into ONE instrument and touches every LOT
        // of it. Forty thousand, not five hundred — the model understated its
        // own worst term by eighty, in the direction that flattered.
        let quiet = TRACKER;
        let one = Dials { open_actions: 1, ..TRACKER };

        assert_eq!(nav_cost(quiet), 503, "the chart plus the currencies");
        assert_eq!(nav_cost(one), 40_503);
        assert_eq!(nav_cost(one) - nav_cost(quiet), 40_000, "the lots of one name");
        assert!(
            nav_cost(one) > nav_cost(quiet) * 80,
            "eighty period ends, not one"
        );
    }

    #[test]
    fn a_rewrite_makes_the_nav_read_the_lots_and_a_factor_does_not() {
        // ⭐ THE CLAIM THE PRODUCT RESTS ON, IN BOTH POSITIONS.
        //
        // Two funds identical but for fragmentation, one split outstanding.
        // Under a rewrite the cost moves with the lots — which is the
        // flat-in-fragmentation promise simply not holding.
        let young = Dials { lots_per: 1, open_actions: 1, ..TRACKER };
        let old = Dials { open_actions: 1, ..TRACKER };

        assert_ne!(
            nav_cost(young),
            nav_cost(old),
            "a rewrite walks the lots, so fragmentation is in the cost"
        );
        assert_eq!(
            factor_nav_cost(young),
            factor_nav_cost(old),
            "under a factor it is not — for ANY number of open actions"
        );

        // And the size of what the redesign removes.
        assert_eq!((nav_cost(old), factor_nav_cost(old)), (40_503, 503));
    }

    #[test]
    fn dials_that_would_wrap_are_refused_rather_than_estimated() {
        // ⚠ The mildest of the overflow findings, and still worth refusing: an
        // estimate that silently clamps is one somebody quotes.
        let absurd = Dials { securities: i64::MAX / 2, lots_per: 4, ..TRACKER };
        let err = estimate(absurd, &Calibration::measured()).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }

    #[test]
    fn on_a_quiet_day_the_two_models_agree_exactly() {
        // ⚠ The redesign removes a CLIFF, and a cliff is not a slope. Anyone
        // quoting it as "the NAV got faster" has it backwards.
        assert_eq!(nav_cost(TRACKER), factor_nav_cost(TRACKER));

        let e = estimate(TRACKER, &Calibration::measured()).unwrap();
        assert_eq!(e.reads, e.factored_reads);
        assert_eq!(e.nanos, e.factored_nanos);
    }

    #[test]
    fn fx_does_not_grow_with_the_chart() {
        let small = Dials { securities: 5, ..TRACKER };
        assert_eq!(
            fx_cost(small),
            fx_cost(TRACKER),
            "three currencies is three translations at five names or five hundred"
        );
    }

    #[test]
    fn the_residual_is_euclidean_the_way_lean_divides() {
        // ⛔ THE ONE PLACE RUST AND LEAN DISAGREE BY DEFAULT, pinned on both
        // sides. `Ratio.Closure` has `perShare (-7) 3 = (-3, 2)`; Rust's own
        // `/` and `%` would give `(-2, -1)`.
        let p = per_share(-7, 3).unwrap();
        assert_eq!((p.per_unit, p.residual), (-3, 2), "Euclidean, as Lean divides");
        assert_ne!((-7i64 / 3, -7i64 % 3), (p.per_unit, p.residual), "Rust's own operators differ");

        // `residual_is_accounted` holds of BOTH, which is why the theorem is
        // not what protects this and the example is.
        assert_eq!(3 * p.per_unit + p.residual, -7);
        assert_eq!(3 * (-7i64 / 3) + (-7i64 % 3), -7);

        // A negative NAV is not hypothetical, and units in issue stay positive.
        let ordinary = per_share(1000, 3).unwrap();
        assert_eq!((ordinary.per_unit, ordinary.residual), (333, 1));

        // ⛔ AND THE TWO THAT USED TO PANIC. `Ratio.Bounded.a_zero_divisor_is_
        // refused` and `the_one_division_that_does_not_fit`.
        let err = per_share(100, 0).unwrap_err();
        assert!(format!("{err:#}").contains("no answer, not a large one"), "{err:#}");
        assert!(per_share(i64::MIN, -1).is_err(), "the one division that does not fit");
    }

    pub(super) fn book_with(n: usize, name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ratio-closure-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        for i in 0..n {
            b.append(&crate::JournalEntry {
                id: format!("t{i}"),
                memo: "measured".into(),
                config: c.clone(),
                postings: vec![PostingRecord::new(1, 100), PostingRecord::new(2, -100)],
            
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    #[test]
    fn the_shipped_rate_is_not_fiction() {
        // The shipped constant is a measurement, and a measurement rots. This
        // does not assert a number — it asserts that the shipped one is still
        // in the same league as what this machine does, which is all a
        // calibration can honestly claim.
        let d = book_with(500, "rate");
        let m = measure(&d).unwrap();
        let shipped = Calibration::measured().nanos_per_read;

        // Printed, not just asserted: the shipped constant has to come from
        // somewhere, and this is where. `--test_output=all` to read it.
        println!("MEASURED HERE: {} ns/read", m.nanos_per_read);
        assert!(m.nanos_per_read > 0, "a measurement of zero is not a measurement");
        // ⛔ ±10×, NOT ±100×. The first version of this used 100 and happily
        // passed a shipped constant that was 18× wrong. A band wide enough to
        // admit an order-of-magnitude error does not check anything.
        assert!(
            m.nanos_per_read < shipped * 10 && m.nanos_per_read * 10 > shipped,
            "shipped {shipped}ns/read vs measured {}ns/read — the constant has drifted \
             out of range and every duration derived from it is now fiction",
            m.nanos_per_read
        );
        assert!(m.provenance.contains("measured here"), "{}", m.provenance);
        assert!(
            !Calibration::measured().provenance.contains("measured here"),
            "the shipped constant must not read like a live measurement"
        );
    }

    #[test]
    fn a_journal_too_small_to_amortize_refuses() {
        // ⛔ THE TRAP THE EMPTY CASE DOES NOT COVER. A one-entry book produces
        // 227,000 ns/read — a number that looks exactly like a measurement and
        // is 51× the marginal cost the model actually multiplies.
        let d = book_with(3, "tiny");
        let err = measure(&d).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("MARGINAL"), "{msg}");
        assert!(msg.contains("227,000"), "the spread is named, not just asserted: {msg}");

        // And at the floor it goes through.
        assert!(measure(&book_with(MIN_ENTRIES, "atfloor")).is_ok());
    }

    #[test]
    fn an_empty_journal_refuses_to_produce_a_rate() {
        // ⛔ THE FAILURE THAT MATTERS. A rate derived from zero reads would be
        // a confident nanosecond figure built on nothing, and it would flow
        // straight into an estimate that looks exactly like a real one.
        let d = book_with(0, "empty");
        let err = measure(&d).unwrap_err();
        assert!(format!("{err:#}").contains("fabrication"), "{err:#}");
    }

    #[test]
    fn an_estimate_carries_where_its_rate_came_from() {
        let e = estimate(TRACKER, &Calibration::measured()).unwrap();
        assert!(e.provenance.contains("FLOOR"), "a floor is named as a floor");
        // ⛔ 503 reads at 250ns is 126 µs. As `0 ms` it reads as broken.
        assert_eq!(e.nanos, 503 * 4_436);
        // ⛔ 503 reads at the measured rate is 2.2 ms. At the invented 250ns it
        // was 126 µs, and "126 µs" is exactly the kind of number that gets
        // quoted at a prospect.
        assert_eq!(human_nanos(e.nanos), "2.2 ms");
        assert_eq!(human_nanos(999), "999 ns");
        assert_eq!(human_nanos(1_500_000), "1.5 ms");
        assert_eq!(human_nanos(90_000_000_000), "90.0 s");
        assert_eq!(human_nanos(999), "999 ns");
        assert_eq!(human_nanos(1_500_000), "1.5 ms");
        assert_eq!(human_nanos(90_000_000_000), "90.0 s");
        assert_eq!(e.reads, 503);

    }
}

