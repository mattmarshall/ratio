//! Recognising an entry, under one book of record.
//!
//! ⭐ THE WALK IS HERE AND THE DECISION IS EMITTED. `Ratio.Views` proves what a
//! view IS — a recognition predicate, so every view's books tie and two views
//! differ by exactly the entries in flight between them — and this is the Rust
//! that asks it, entry by entry. The division is `DEVELOPING.md`'s: the fold
//! stays in Rust, the per-entry decision comes from Lean.
//!
//! ⚠ UNTIL `Ratio.Views.Emit` LANDS, THE DECISIONS BELOW ARE HAND-WRITTEN
//! MIRRORS of the Lean rather than emitted from it, and they are marked. That
//! is the same state `relief.rs` was in before `Ratio.Lots.Emit`, and it is a
//! gap rather than a design: two spellings of one decision is exactly the
//! failure `every_method_round_trips_through_its_declared_name` exists for.
//!
//! # What a view refuses, and why it is not an omission
//!
//! ⛔ AN ENTRY A VIEW CANNOT DATE IS REPORTED, NEVER DROPPED. Three things
//! produce one: the record carries no `trade_date`, the date will not parse, or
//! the entry's pinned configuration does not declare this view at all. In every
//! case the honest answer is a refusal a person can read — the same judgement
//! `Ratio.Valuation.strike_refuses_exactly_when_something_is_unpriced` makes
//! about an unpriced position. Folding it anyway would be trade-date basis
//! wearing a settlement label; excluding it silently would remove value from a
//! book whose trial balance went on tying, which is the shape of every defect in
//! HANDOFF.md's table.

use std::collections::BTreeSet;

use ratio_rules::{Basis, Calendar, RuleSet, UNDECLARED_VIEW};

use crate::relief::Day;

/// How far a roll will look for an open day before giving up.
///
/// ⛔ A BOUND, NOT A TUNING KNOB. A calendar that closes the market on every
/// weekday is one typo in a generated file, and a search for the next open day
/// over it does not return. `Ratio.Views.a_calendar_that_never_opens_refuses_
/// rather_than_looping` makes the refusal the answer; this is the number that
/// makes it reachable. Ten weeks is longer than any real closure and short
/// enough that a broken calendar fails in microseconds.
const ROLL_FUEL: u32 = 70;

/// One book of record, resolved from the configuration an entry pinned.
///
/// ⚠ RESOLVED PER CONFIGURATION AND CACHED BY DIGEST, exactly as `Terms` is.
/// How an entry is recognised is a term of the agreement in force when it was
/// posted, not of whichever is active now — `//tla:calendar_in_side_file_check`
/// is what happens when a settlement date is re-derived over a calendar amended
/// since.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewDef {
    pub id: String,
    pub display_name: String,
    pub basis: Basis,
    pub settles_in: i64,
    /// The days the market is shut, as days since the epoch. Empty on a basis
    /// that consults no calendar.
    pub holidays: BTreeSet<Day>,
    /// Weekday numbers the market is shut on, 0 = Sunday.
    pub weekend: Vec<i64>,
    /// Whether the configuration declared this view, or whether it is the one
    /// every book has.
    pub declared: bool,
}

/// Where a view puts an entry, as three answers rather than two.
///
/// ⛔ `Unplaceable` IS NOT "NOT YET", AND COLLAPSING THEM IS THE WHOLE FAILURE.
/// An entry the view will recognise on Tuesday is out of scope of a Monday cut
/// and reconciles; an entry it can never date is a refusal somebody has to see.
/// `Ratio.Views.Placement` is the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The journal's own order — no date consulted, so it is always in scope.
    Always,
    /// Recognised on this day.
    On(Day),
    /// The record does not support this view. Carries the reason, for reporting.
    Unplaceable(String),
}

impl ViewDef {
    /// The single view a book that declares none has.
    ///
    /// ⛔ `recorded`, NOT `settlement 0`, AND THE DIFFERENCE IS THE MIGRATION.
    /// This consults no date, so it answers over every book ever written —
    /// including the ones whose entries carry no `trade_date`, which is most of
    /// them. A same-day settlement convention would refuse every one of those,
    /// and reporting it as an election nobody made is the `lot_method: None`
    /// mistake one layer out.
    pub fn undeclared() -> Self {
        ViewDef {
            id: UNDECLARED_VIEW.to_string(),
            display_name: "Journal order".to_string(),
            basis: Basis::Recorded,
            settles_in: 0,
            holidays: BTreeSet::new(),
            weekend: Vec::new(),
            declared: false,
        }
    }

    /// Every view a configuration declares, or the one every book has.
    ///
    /// ⚠ A HOLIDAY THAT WILL NOT PARSE IS DROPPED HERE AND NOT REFUSED, because
    /// `RuleSet::from_toml` already refuses it — this is downstream of a
    /// configuration that has been read, so an unparseable date at this point
    /// cannot happen and defensively refusing would be dead code claiming to be
    /// a guard.
    pub fn of(set: &RuleSet) -> Vec<ViewDef> {
        let declared = set.views_declared();
        set.effective_views()
            .into_iter()
            .map(|v| {
                let cal: Option<&Calendar> =
                    v.calendar.as_deref().and_then(|id| set.calendar(id));
                ViewDef {
                    display_name: v.label().to_string(),
                    id: v.id,
                    basis: v.basis,
                    settles_in: v.settles_in.unwrap_or(0),
                    holidays: cal
                        .map(|c| {
                            c.holidays
                                .iter()
                                .filter_map(|d| ratio_common::days_from_iso_date(d).ok())
                                .map(|n| n as Day)
                                .collect()
                        })
                        .unwrap_or_default(),
                    weekend: cal.map(|c| c.weekend.clone()).unwrap_or_default(),
                    declared,
                }
            })
            .collect()
    }

    /// Whether the market is open on this day.
    ///
    /// ⚠ MIRRORS `Ratio.Views.nextOpenDay`'s `holiday` PREDICATE, and the
    /// weekday arithmetic is `(day + 4).rem_euclid(7)` because 1970-01-01 was a
    /// Thursday. Getting that constant wrong shifts every settlement date by a
    /// fixed number of days, which reads as an ordinary date and sorts
    /// correctly.
    fn is_open(&self, d: Day) -> bool {
        let dow = i64::from((d as i64 + 4).rem_euclid(7) as i32);
        !self.weekend.contains(&dow) && !self.holidays.contains(&d)
    }

    /// `n` open days after `d`, or nothing if the calendar never opens far
    /// enough. `Ratio.Views.openDaysAfter`.
    fn open_days_after(&self, d: Day, n: i64) -> Option<Day> {
        let mut at = d;
        for _ in 0..n {
            let mut fuel = ROLL_FUEL;
            loop {
                if fuel == 0 {
                    return None;
                }
                fuel -= 1;
                // ⛔ CHECKED, BECAUSE A DAY IS AN `i32` AND THE PROOF IS OVER
                // `Int`. A trade date read out of a delivered file is whatever
                // the file said; `days_from_iso_date` accepts any year that
                // parses, so this can be asked about a day near the end of the
                // range and wrap into the past.
                at = at.checked_add(1)?;
                if self.is_open(at) {
                    break;
                }
            }
        }
        Some(at)
    }

    /// Where this view puts an entry. `Ratio.Views.placement`.
    ///
    /// `traded` is the entry's trade date already parsed, `None` when the record
    /// carries none or it would not parse — both of which the caller has already
    /// reported as a break.
    pub fn placement(&self, entry_id: &str, traded: Option<Day>) -> Placement {
        match self.basis {
            Basis::Recorded => Placement::Always,
            Basis::Trade => match traded {
                Some(t) => Placement::On(t),
                None => Placement::Unplaceable(format!(
                    "{entry_id}: view {:?} recognises on the trade date and this entry \
                     carries none. The epoch would recognise it before every cut and today \
                     after all of them; neither is what the record says",
                    self.id
                )),
            },
            Basis::Settlement => match traded {
                None => Placement::Unplaceable(format!(
                    "{entry_id}: view {:?} settles {} days after the trade and this entry \
                     carries no trade date, so there is nothing to roll forward from",
                    self.id, self.settles_in
                )),
                Some(t) => match self.open_days_after(t, self.settles_in) {
                    Some(s) => Placement::On(s),
                    None => Placement::Unplaceable(format!(
                        "{entry_id}: view {:?} could not find {} open days after the trade \
                         date — its calendar closes the market for longer than any roll \
                         looks",
                        self.id, self.settles_in
                    )),
                },
            },
        }
    }

    /// Whether this view has this entry, as of a cut.
    ///
    /// ⚠ `as_of = None` IS "THE WHOLE JOURNAL", AND IT IS WHY THE VIEWS CAN
    /// AGREE. Folded to the end of history every view recognises everything it
    /// can place, because everything eventually settles. The difference between
    /// two views lives entirely at a CUT —
    /// `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_gap`, which is also
    /// why a differential test that does not cut is worth nothing.
    pub fn recognises(&self, placement: &Placement, as_of: Option<Day>) -> bool {
        match placement {
            Placement::Always => true,
            Placement::On(d) => match as_of {
                None => true,
                // ⚠ `<=`, AND THE BOUNDARY IS ON THE DAY. A trade settling
                // exactly on the valuation date is IN. Off by one moves an
                // entry between periods, which reads as an ordinary figure.
                Some(cut) => *d <= cut,
            },
            Placement::Unplaceable(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso(s: &str) -> Day {
        ratio_common::days_from_iso_date(s).unwrap() as Day
    }

    fn settled(days: i64, weekend: Vec<i64>, holidays: &[&str]) -> ViewDef {
        ViewDef {
            id: "ibor".into(),
            display_name: "IBOR".into(),
            basis: Basis::Settlement,
            settles_in: days,
            holidays: holidays.iter().map(|d| iso(d)).collect(),
            weekend,
            declared: true,
        }
    }

    #[test]
    fn a_settlement_view_skips_the_weekend() {
        // Friday 2026-02-27 + 2 open days, weekend Sat/Sun → Tuesday 2026-03-03.
        // ⚠ WRITTEN OUT AS DATES RATHER THAN OFFSETS. A test asserting
        // `t + 4` agrees with whatever the walk does, including a walk that
        // counts calendar days and calls them open ones.
        let v = settled(2, vec![0, 6], &[]);
        assert_eq!(
            v.placement("e1", Some(iso("2026-02-27"))),
            Placement::On(iso("2026-03-03"))
        );
    }

    #[test]
    fn a_holiday_pushes_the_settlement_a_day_further() {
        // The same trade, with the Monday shut: Wednesday rather than Tuesday.
        let v = settled(2, vec![0, 6], &["2026-03-02"]);
        assert_eq!(
            v.placement("e1", Some(iso("2026-02-27"))),
            Placement::On(iso("2026-03-04"))
        );
    }

    #[test]
    fn a_calendar_that_never_opens_refuses_rather_than_looping() {
        // The Rust half of `Ratio.Views.a_calendar_that_never_opens_refuses_
        // rather_than_looping`. `RuleSet::from_toml` refuses a seven-day weekend
        // before it gets here; this is the guard for the case it cannot see — a
        // holiday list long enough to outrun the roll.
        let v = settled(1, vec![0, 1, 2, 3, 4, 5], &[]);
        // Six weekend days plus a holiday on every remaining Saturday for longer
        // than the fuel: the walk gives up rather than spinning.
        let all_saturdays: Vec<String> = (0..80)
            .map(|i| ratio_common::iso_date_from_days((iso("2026-02-27") + i) as i64))
            .collect();
        let refs: Vec<&str> = all_saturdays.iter().map(|s| s.as_str()).collect();
        let v = settled(1, v.weekend.clone(), &refs);
        assert!(matches!(
            v.placement("e1", Some(iso("2026-02-27"))),
            Placement::Unplaceable(_)
        ));
    }

    #[test]
    fn an_undated_entry_is_refused_by_an_election_and_placed_by_the_default() {
        // ⛔ THE MIGRATION, AS A TEST. `recorded` consults no date, so it
        // answers over the entries every existing book is full of; both
        // elections refuse them and say why.
        let recorded = ViewDef::undeclared();
        assert_eq!(recorded.placement("e1", None), Placement::Always);
        assert!(!recorded.declared, "nobody said");

        for basis in [Basis::Trade, Basis::Settlement] {
            let mut v = settled(2, vec![0, 6], &[]);
            v.basis = basis;
            match v.placement("e1", None) {
                Placement::Unplaceable(why) => {
                    assert!(why.contains("e1"), "the refusal must name the entry: {why}")
                }
                other => panic!("{basis:?} placed an undated entry as {other:?}"),
            }
        }
    }

    #[test]
    fn the_settlement_day_itself_is_recognised() {
        // ⚠ THE BOUNDARY IS ON THE DAY, like `the_threshold_day_is_long_term`.
        // Off by one moves an entry between periods and looks entirely ordinary.
        let v = settled(2, vec![0, 6], &[]);
        let p = v.placement("e1", Some(iso("2026-02-27")));
        assert!(v.recognises(&p, Some(iso("2026-03-03"))), "the day itself is in");
        assert!(!v.recognises(&p, Some(iso("2026-03-02"))), "the day before is not");
    }

    #[test]
    fn a_fold_with_no_cut_recognises_everything_either_view_can_place() {
        // ⛔ THE VACUITY TRAP, AS A TEST THAT ASSERTS IT RATHER THAN FALLING
        // INTO IT. Two views over the whole journal agree — so any differential
        // test between them that does not CUT is green under a broken
        // convention as readily as a working one.
        let trade = {
            let mut v = settled(0, vec![], &[]);
            v.basis = Basis::Trade;
            v
        };
        let settle = settled(2, vec![0, 6], &[]);
        let d = Some(iso("2026-02-27"));
        assert!(trade.recognises(&trade.placement("e1", d), None));
        assert!(settle.recognises(&settle.placement("e1", d), None));
    }

    #[test]
    fn a_settlement_date_is_never_before_the_trade() {
        // `Ratio.Views.settlement_is_never_before_the_trade`. A roll that
        // stepped backwards over a holiday would settle a trade before it was
        // struck, which sorts correctly and lands it in the wrong period.
        let v = settled(3, vec![0, 6], &["2026-03-02", "2026-03-03"]);
        for offset in 0..40 {
            let t = iso("2026-02-01") + offset;
            match v.placement("e1", Some(t)) {
                Placement::On(s) => assert!(s > t, "settled {s} on a trade of {t}"),
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}
