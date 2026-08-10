//! Relieving tax lots — the walk, over decisions made in Lean.
//!
//! `Ratio.Lots` proves what one relief computes and `Ratio.Lots.Relief` proves
//! what it produces; `//tla:relief_engine_check` proves what a run of them must
//! not do. This is the fold those describe, and it deliberately owns nothing
//! they decide: `takes_whole_lot`, `partial_divides`, `partial_cost` and
//! `lot_is_sound` are emitted.
//!
//! # ⛔ What conservation cannot see
//!
//! Every failure this module guards against leaves the books tying:
//!
//! * a **husk** — a lot holding zero units and carrying cost — is consumed by
//!   `takes_whole_lot`, since `0 <= want`, and hands its whole basis to a sale
//!   that received no units for it. `Ratio.Lots.Edges.a_husk_gives_away_its_cost`
//! * a **partial** relief that rounds gives up the wrong basis by a minor unit.
//!   `Ratio.Lots.partial_relief_is_exactly_pro_rata`
//! * an **unsorted** list makes "FIFO" mean whatever order storage returned.
//!   `Ratio.Lots.Edges.sorting_is_the_callers_job`
//!
//! In all three the entry balances, the units are right, and only the realized
//! gain is wrong — which is the figure nobody reconciles because it has no
//! counterparty.

use anyhow::{bail, Result};

use crate::generated_lots::{lot_is_sound, partial_cost, partial_divides, takes_whole_lot};

/// A tax lot: when it was acquired, what it holds, what it cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lot {
    /// Acquisition ordinal. ⛔ FIFO IS THIS FIELD, not the order of the vector —
    /// `relieve` sorts by it rather than trusting what it was handed.
    pub seq: u64,
    pub units: i64,
    pub cost: i64,
}

/// What a sale took out of one lot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Taken {
    pub seq: u64,
    pub units: i64,
    pub cost: i64,
}

/// What a relief produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relieved {
    pub taken: Vec<Taken>,
    pub left: Vec<Lot>,
    /// What the relieved lots cost. `Ratio.Lots.takenCost`.
    pub cost: i64,
}

impl Relieved {
    /// Proceeds less the cost given up. `Ratio.Lots.Relief.gain`.
    ///
    /// ⚠ THE ONE FIGURE HERE WITH NO COUNTERPARTY. A wrong NAV is caught by a
    /// reconciliation; a wrong realized gain is caught by nobody until a tax
    /// authority asks, which is why every guard in this module is about it
    /// rather than about conservation.
    pub fn gain(&self, proceeds: i64) -> Result<i64> {
        ratio_common::checked::add(proceeds, -self.cost, "the realized gain")
    }
}

/// Which lots a sale gives up.
///
/// ⛔ A TERM OF AN ADMINISTRATION AGREEMENT, NOT AN IMPLEMENTATION CHOICE. The
/// method decides the REALIZED GAIN — the same holding and the same trade
/// produce different taxable income under each, with no figure on the balance
/// sheet moving. `Ratio.Lots.Methods.the_method_decides_the_taxable_gain`.
///
/// ⚠ AND THE SPACE IS NOT ALL ORDERINGS. These four sort and walk. SPECIFIC
/// IDENTIFICATION is a selection that may take from the middle of a holding, and
/// AVERAGE COST pools the holding so there is no lot to give up at all — both
/// are modelled in `Ratio.Lots.Methods` and neither belongs in this enum.
/// Adding them here as variants is the mistake the Lean file exists to prevent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Method {
    /// Oldest acquisition first.
    #[default]
    Fifo,
    /// Newest first.
    Lifo,
    /// ⛔ Dearest PER UNIT first — the method chosen to reduce a gain.
    /// `Ratio.Lots.Methods.hifo_is_per_unit_not_per_lot`: by TOTAL cost a large
    /// cheap lot outranks a small dear one, and taking it shelters less. Getting
    /// this wrong does not fail; it overstates the taxable gain, which is the
    /// opposite of what the method was chosen for.
    Hifo,
    /// Cheapest per unit first — chosen to REALIZE a gain deliberately, which a
    /// fund does against a capital-loss carryforward.
    Lofo,
}

impl Method {
    /// Order a holding for this method.
    ///
    /// ⛔ THE TIEBREAK APPLIES ONLY ON A TIE. `dearer(a,b) || a.seq <= b.seq` is
    /// true whenever the sequence ascends, whatever the costs — so the tiebreak
    /// overrides the method and HIFO quietly performs FIFO. Lean's `decide`
    /// reported that theorem FALSE, which is the only reason it was caught: a
    /// test asserting "HIFO differs from LOFO" would have PASSED, because both
    /// were broken the same way in opposite directions.
    fn arrange(self, lots: &mut [Lot]) {
        // Dearer per unit, cross-multiplied. ⚠ Not `cost / units`: integer
        // division ties lots whose per-unit costs differ by less than a minor
        // unit, and a method that ties lots which are not tied picks by whatever
        // the sort does next.
        fn dearer(a: &Lot, b: &Lot) -> std::cmp::Ordering {
            (b.units as i128 * a.cost as i128).cmp(&(a.units as i128 * b.cost as i128))
        }
        match self {
            Method::Fifo => lots.sort_by_key(|l| l.seq),
            Method::Lifo => lots.sort_by_key(|l| std::cmp::Reverse(l.seq)),
            Method::Hifo => lots.sort_by(|a, b| dearer(b, a).then(a.seq.cmp(&b.seq))),
            Method::Lofo => lots.sort_by(|a, b| dearer(a, b).then(a.seq.cmp(&b.seq))),
        }
    }
}

/// Relieve `want` units, oldest lot first.
pub fn relieve(lots: &[Lot], want: i64) -> Result<Relieved> {
    relieve_by(Method::Fifo, lots, want)
}

/// Relieve `want` units under a declared method.
///
/// ⛔ ORDERS THE HOLDING ITSELF. `Ratio.Lots.relieveFifo` takes the head of the
/// list, so it is FIFO exactly when the caller handed it acquisition order — and
/// a projection keyed by instrument does not. Naming a method and then trusting
/// the caller to have implemented it is how a fund's tax position quietly
/// becomes whatever the storage layer returned.
pub fn relieve_by(method: Method, lots: &[Lot], want: i64) -> Result<Relieved> {
    if want < 0 {
        bail!("a relief of {want} units is not a relief; a negative sale is a purchase");
    }
    for l in lots {
        // ⛔ `Ratio.Lots.Edges.a_husk_is_refused`. A lot holding nothing and
        // carrying cost would be CONSUMED by the walk below and hand over its
        // whole basis for no units, understating the gain by exactly that
        // amount — with every invariant satisfied.
        if !lot_is_sound(l.units, l.cost) {
            bail!(
                "lot {} holds {} units and carries {} — a holding of nothing that owes \
                 something is not a lot, and relieving it would give away its basis for \
                 no units at all",
                l.seq,
                l.units,
                l.cost
            );
        }
    }

    let mut ordered: Vec<Lot> = lots.to_vec();
    method.arrange(&mut ordered);

    let mut taken = Vec::new();
    let mut cost = 0i64;
    let mut remaining = want;
    let mut left: Vec<Lot> = Vec::new();
    let mut it = ordered.into_iter();

    for lot in it.by_ref() {
        if remaining == 0 {
            left.push(lot);
            break;
        }
        if takes_whole_lot(lot.units, remaining) {
            taken.push(Taken { seq: lot.seq, units: lot.units, cost: lot.cost });
            cost = ratio_common::checked::add(cost, lot.cost, "the relieved cost")?;
            remaining -= lot.units;
            continue;
        }
        // Part of this lot, and none of the ones behind it.
        ratio_common::checked::mul(lot.cost, remaining, "a pro-rata lot split")?;
        if !partial_divides(lot.cost, remaining, lot.units) {
            bail!(
                "relieving {remaining} of lot {}'s {} units does not divide its cost of {} \
                 into whole minor units — which way to round is a term of an administration \
                 agreement, not a property of arithmetic",
                lot.seq,
                lot.units,
                lot.cost
            );
        }
        let part = partial_cost(lot.cost, remaining, lot.units);
        taken.push(Taken { seq: lot.seq, units: remaining, cost: part });
        cost = ratio_common::checked::add(cost, part, "the relieved cost")?;
        left.push(Lot { seq: lot.seq, units: lot.units - remaining, cost: lot.cost - part });
        remaining = 0;
    }
    left.extend(it);

    if remaining > 0 {
        bail!(
            "this holding is {remaining} units short of the {want} being sold — \
             `Ratio.Lots.short_sales_are_refused`, and going negative would create \
             a position nobody opened"
        );
    }
    Ok(Relieved { taken, left, cost })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(seq: u64, units: i64, cost: i64) -> Lot {
        Lot { seq, units, cost }
    }

    #[test]
    fn a_sale_costs_what_it_consumes() {
        // `Ratio.Lots.relief_touches_only_what_it_takes`.
        let r = relieve(&[l(1, 10, 200), l(2, 5, 50)], 10).unwrap();
        assert_eq!(r.cost, 200);
        assert_eq!(r.left, vec![l(2, 5, 50)], "and leaves the rest alone");
    }

    #[test]
    fn what_was_taken_plus_what_remains_is_what_was_there() {
        // `Ratio.Lots.cost_is_conserved` and `units_are_conserved`.
        let lots = [l(1, 10, 200), l(2, 5, 50), l(3, 8, 400)];
        for want in 0..=23i64 {
            let r = relieve(&lots, want).unwrap();
            assert_eq!(
                r.cost + r.left.iter().map(|x| x.cost).sum::<i64>(),
                650,
                "cost, at want={want}"
            );
            assert_eq!(
                r.taken.iter().map(|t| t.units).sum::<i64>() + r.left.iter().map(|x| x.units).sum::<i64>(),
                23,
                "units, at want={want}"
            );
            assert_eq!(r.taken.iter().map(|t| t.units).sum::<i64>(), want, "sold exactly {want}");
        }
    }

    #[test]
    fn a_partial_relief_is_exactly_pro_rata() {
        // `Ratio.Lots.partial_relief_is_exactly_pro_rata`. Four of ten units at
        // 200 is 80 — not 79, not 81.
        let r = relieve(&[l(1, 10, 200)], 4).unwrap();
        assert_eq!(r.cost, 80);
        assert_eq!(r.left, vec![l(1, 6, 120)]);
    }

    #[test]
    fn a_partial_relief_that_would_round_is_refused() {
        // ⛔ Conservation holds of a ROUNDED answer, so nothing downstream would
        // catch it. Three of seven units at 100 is 42.857…
        let err = relieve(&[l(1, 7, 100)], 3).unwrap_err();
        assert!(format!("{err:#}").contains("term of an administration agreement"), "{err:#}");
    }

    #[test]
    fn a_husk_is_refused_rather_than_consumed() {
        // ⛔ `Ratio.Lots.Edges.a_husk_gives_away_its_cost`. Unguarded, this
        // relieves 200 where the answer is 100 — the husk hands over its whole
        // basis for no units — and cost is conserved, units are conserved, and
        // the realized gain is understated by exactly 100.
        let err = relieve(&[l(1, 0, 100), l(2, 10, 200)], 5).unwrap_err();
        assert!(format!("{err:#}").contains("give away its basis"), "{err:#}");

        // And the sound holding alone relieves half as much.
        assert_eq!(relieve(&[l(2, 10, 200)], 5).unwrap().cost, 100);
    }

    #[test]
    fn a_fully_relieved_lot_is_not_a_husk() {
        // Nothing held and nothing owed passes, and behaves as if absent.
        let r = relieve(&[l(1, 0, 0), l(2, 10, 200)], 5).unwrap();
        assert_eq!(r.cost, 100);
    }

    #[test]
    fn fifo_is_the_sequence_not_the_vector_order() {
        // ⛔ `Ratio.Lots.Edges.sorting_is_the_callers_job` — the Lean function
        // takes the head, so this one SORTS. A projection keyed by instrument
        // does not return acquisition order, and trusting it would make a
        // fund's tax position whatever the storage layer felt like.
        let jumbled = [l(9, 1, 90), l(1, 1, 10), l(5, 1, 50)];
        let r = relieve(&jumbled, 1).unwrap();
        assert_eq!(r.taken, vec![Taken { seq: 1, units: 1, cost: 10 }], "the OLDEST lot");
        assert_eq!(r.cost, 10, "not 90");
    }

    #[test]
    fn the_method_decides_the_taxable_gain() {
        // ⭐ `Ratio.Lots.Methods.the_method_decides_the_taxable_gain`. Two
        // one-unit lots at 10 and 40, a sale of one at 50. Four times the
        // taxable income between HIFO and LOFO, and nothing on the balance
        // sheet moves.
        let lots = [l(1, 1, 10), l(2, 1, 40)];
        let g = |m| relieve_by(m, &lots, 1).unwrap().gain(50).unwrap();
        assert_eq!(g(Method::Fifo), 40, "gives up the OLD lot");
        assert_eq!(g(Method::Lifo), 10, "gives up the NEW lot");
        assert_eq!(g(Method::Hifo), 10, "gives up the DEAR lot — shelters the gain");
        assert_eq!(g(Method::Lofo), 40, "gives up the CHEAP lot — realizes it");
    }

    #[test]
    fn hifo_is_per_unit_not_per_lot() {
        // ⛔ `Ratio.Lots.Methods.hifo_is_per_unit_not_per_lot`. A lot of 100
        // units costing 1,000 is 10 each; a lot of 1 unit costing 50 is 50.
        // By TOTAL cost the first is dearer; per UNIT the second is, by five
        // times, and it is the one that shelters the most gain.
        let lots = [l(1, 100, 1_000), l(2, 1, 50)];
        let r = relieve_by(Method::Hifo, &lots, 1).unwrap();
        assert_eq!(r.cost, 50, "the small DEAR lot, not the large cheap one");
        assert_eq!(r.taken[0].seq, 2);
    }

    #[test]
    fn the_tiebreak_does_not_override_the_method() {
        // ⛔ THE BUG LEAN CAUGHT. `dearer(a,b) || a.seq <= b.seq` is true
        // whenever the sequence ascends, so the tiebreak overrode the method
        // and HIFO performed FIFO.
        //
        // ⚠ A test asserting "HIFO differs from LOFO" would have PASSED — both
        // were broken the same way in opposite directions. This asserts the
        // VALUE, which is the only thing that catches it.
        let lots = [l(1, 1, 10), l(2, 1, 40)];
        assert_eq!(relieve_by(Method::Hifo, &lots, 1).unwrap().cost, 40, "the dear lot");
        assert_eq!(relieve_by(Method::Fifo, &lots, 1).unwrap().cost, 10, "the old lot");
    }

    #[test]
    fn every_method_conserves_whatever_it_chooses() {
        // `Ratio.Lots.Methods.every_ordering_method_conserves`. The invariant a
        // new method must preserve — and the one whoever adds it will not think
        // to check, because the gain is what the method is FOR.
        let lots = [l(1, 10, 200), l(2, 5, 50), l(3, 8, 400)];
        for m in [Method::Fifo, Method::Lifo, Method::Hifo, Method::Lofo] {
            for want in 0..=23i64 {
                let r = relieve_by(m, &lots, want).unwrap();
                assert_eq!(
                    r.cost + r.left.iter().map(|x| x.cost).sum::<i64>(),
                    650,
                    "{m:?} at want={want}"
                );
                assert_eq!(r.taken.iter().map(|t| t.units).sum::<i64>(), want, "{m:?}");
            }
        }
    }

    #[test]
    fn the_gain_is_proceeds_less_what_it_cost() {
        // `Ratio.Lots.Relief.the_gain_and_the_relieved_cost_are_the_proceeds`.
        let r = relieve(&[l(1, 1, 10), l(2, 1, 40)], 1).unwrap();
        assert_eq!(r.gain(50).unwrap(), 40, "FIFO gives up the cheap lot");
        assert_eq!(r.gain(50).unwrap() + r.cost, 50, "and the two are the proceeds");
    }

    #[test]
    fn selling_more_than_the_holding_is_refused() {
        // `Ratio.Lots.short_sales_are_refused`. Not "relieve what there is and
        // report a smaller sale" — that would silently fill part of an order.
        let err = relieve(&[l(1, 10, 200)], 11).unwrap_err();
        assert!(format!("{err:#}").contains("1 units short"), "{err:#}");
        assert!(relieve(&[l(1, 10, 200)], -1).is_err(), "and a negative sale is not a sale");
    }

    #[test]
    fn selling_everything_leaves_nothing() {
        // `Ratio.Lots.Edges.selling_everything_leaves_nothing` — an empty list,
        // not a list of husks, which the NEXT sale would have consumed.
        let r = relieve(&[l(1, 10, 200), l(2, 5, 50)], 15).unwrap();
        assert!(r.left.is_empty());
        assert_eq!(r.cost, 250);
    }

    #[test]
    fn a_sale_of_nothing_disturbs_nothing() {
        let lots = [l(1, 10, 200), l(2, 5, 50)];
        let r = relieve(&lots, 0).unwrap();
        assert!(r.taken.is_empty());
        assert_eq!(r.left, lots.to_vec());
        assert_eq!(r.gain(0).unwrap(), 0);
    }

    #[test]
    fn an_overflowing_pro_rata_split_is_refused() {
        // ⛔ `Ratio.Bounded`. `cost * want` at a large basis wraps, and the
        // wrapped product PASSES `partial_divides` — the proof cannot see it
        // because in `Int` the multiplication simply happened.
        let err = relieve(&[l(1, 1_000_000, i64::MAX / 2)], 999_999).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }
}
