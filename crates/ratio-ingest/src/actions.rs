//! Corporate actions.
//!
//! ⛔ EVERY DECISION HERE IS PROVED IN `Ratio.Actions`, and the names match so
//! the two read against each other:
//!
//! | here          | there                              |
//! |---------------|------------------------------------|
//! | `split`       | `split`                            |
//! | `spin_off`    | `spinOff`                          |
//!
//! And the three things that make an action unlike everything else in this
//! crate, each with a theorem behind it:
//!
//! * it CONSERVES COST and deliberately CHANGES UNITS (`split_conserves_cost`,
//!   `split_scales_units`) — so a check that units did not move is a check that
//!   the action did not happen;
//! * ⛔ IT IS NOT IDEMPOTENT (`applying_twice_is_not_applying_once`). Marking
//!   twice posts nothing, admitting twice writes nothing, ingesting twice reads
//!   nothing — every other write here is safe to retry and the console retries
//!   them freely. A split applied twice quadruples the position and the trial
//!   balance still ties. The record of having applied it IS the idempotence;
//!   there is no arithmetic underneath to catch the second one.
//! * it is RETROACTIVE, which `//tla:actions_check` is about.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// A ratio as an action declares it: `2 for 1`, `3 for 2`, `1 for 10`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Split {
    /// Units received.
    pub num: i64,
    /// Units given up.
    pub den: i64,
}

/// A corporate action as announced — before anybody applies it.
///
/// ⛔ THE EX-DATE IS WHY THIS IS RECORDED SEPARATELY FROM APPLYING IT. An action
/// is effective on a day, and a NAV struck on or after that day should have
/// included it. Whether it did is answerable from the journal — the strike pins
/// a position and the applied action is an entry — but only if the ex-date was
/// written down when the action arrived.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announced {
    pub id: String,
    pub instrument: String,
    pub split: Split,
    /// `YYYY-MM-DD`. ISO so it compares in date order as a string.
    pub ex_date: String,
    /// When we were told, which is not when it took effect.
    pub announced: i64,
}

/// A holding of one instrument, as a split sees it: units and what they cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holding {
    pub units: i64,
    pub cost: i64,
}

/// Apply a split to a holding.
///
/// ⛔ REFUSES WHEN THE UNITS DO NOT DIVIDE. A 3-for-2 of five units is seven
/// and a half, and a fractional share is not a share. What happens to the
/// fraction is CASH IN LIEU — which realizes a gain, and is therefore a posting
/// rather than a rounding, and belongs to the configuration that declares how
/// this fund handles it.
///
/// Flooring would lose half a share while the cost stayed put, so the books
/// would tie and the holding would be short. That is the same shape as marking
/// from cost and as rounding a partial lot relief: a balanced entry of the
/// wrong size. `Ratio.Actions.an_indivisible_split_is_refused`.
pub fn split(s: Split, h: &Holding) -> Result<Holding> {
    if s.den == 0 {
        bail!("a split cannot be for zero units");
    }
    let scaled = h.units.checked_mul(s.num).ok_or_else(|| anyhow::anyhow!("split overflows"))?;
    if scaled % s.den != 0 {
        bail!(
            "{} units {}-for-{} is not a whole number of units — the fraction is cash in \
             lieu, which realizes a gain, and this configuration has not declared how it \
             is handled",
            h.units,
            s.num,
            s.den
        );
    }
    Ok(Holding {
        // `Ratio.Actions.split_scales_units`.
        units: scaled / s.den,
        // `Ratio.Actions.split_conserves_cost` — the same money now buys a
        // different number of units. A book that revalued here would invent a
        // gain the market did not give it.
        cost: h.cost,
    })
}

/// Allocate a cost basis between a parent and what was spun out of it.
///
/// `basis_points` is the fraction that moves to the new instrument.
/// `Ratio.Actions.spin_off_allocates_the_whole_basis`: what stays plus what
/// leaves is what was there, or a phantom loss appears on the eventual sale.
pub fn spin_off(basis_points: i64, h: &Holding) -> Result<(Holding, Holding)> {
    if !(0..=10_000).contains(&basis_points) {
        bail!("{basis_points} is not a fraction in basis points");
    }
    let scaled = h
        .cost
        .checked_mul(basis_points)
        .ok_or_else(|| anyhow::anyhow!("allocation overflows"))?;
    if scaled % 10_000 != 0 {
        bail!(
            "allocating {basis_points}bp of {} does not land on a whole minor unit, and \
             which way to round is a term of an agreement rather than a property of \
             arithmetic",
            h.cost
        );
    }
    let moved = scaled / 10_000;
    Ok((
        Holding { units: h.units, cost: h.cost - moved },
        Holding { units: h.units, cost: moved },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_split_conserves_cost_and_changes_units() {
        let before = Holding { units: 100, cost: 25_000_00 };
        let after = split(Split { num: 2, den: 1 }, &before).unwrap();
        assert_eq!(after.units, 200, "two for one doubles the units");
        assert_eq!(after.cost, before.cost, "and does not touch what they cost");

        // ⚠ Which means a check that units did not move is a check that the
        // action did NOT happen — the opposite of every other operation here.
        assert_ne!(after.units, before.units);
    }

    #[test]
    fn an_indivisible_split_is_refused_not_floored() {
        // ⛔ `Ratio.Actions.an_indivisible_split_is_refused`. Five units, three
        // for two. Flooring gives seven and loses half a share; the cost is
        // unchanged either way, so the books TIE and the holding is short.
        let h = Holding { units: 5, cost: 100_00 };
        let err = split(Split { num: 3, den: 2 }, &h).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("cash in lieu"), "{msg}");
        assert!(msg.contains("realizes a gain"), "{msg}");

        // …and an even holding is fine.
        assert_eq!(split(Split { num: 3, den: 2 }, &Holding { units: 4, cost: 100_00 })
            .unwrap()
            .units, 6);
    }

    #[test]
    fn applying_a_split_twice_is_not_applying_it_once() {
        // ⛔ THE ONE THAT BREAKS THE HABIT. Every other write in this crate is
        // safe to retry. This one quadruples the position, and the cost is
        // unchanged so nothing in the arithmetic notices.
        let h = Holding { units: 100, cost: 5_000_00 };
        let once = split(Split { num: 2, den: 1 }, &h).unwrap();
        let twice = split(Split { num: 2, den: 1 }, &once).unwrap();

        assert_eq!(once.units, 200);
        assert_eq!(twice.units, 400, "a retry doubles it again");
        assert_eq!(twice.cost, h.cost, "and the trial balance still ties");

        // Which is why whatever applies an action has to record that it did:
        // `//tla:reapply_action_check` is this, one interface layer up.
    }

    #[test]
    fn a_spin_off_allocates_the_basis_and_creates_none() {
        let h = Holding { units: 100, cost: 10_000_00 };
        let (parent, child) = spin_off(2500, &h).unwrap();
        assert_eq!(parent.cost + child.cost, h.cost, "the basis is allocated, not created");
        assert_eq!(child.cost, 2_500_00);
        // Units are unchanged on the parent; what the child holds is the
        // distribution ratio's business, not the allocation's.
        assert_eq!(parent.units, h.units);
    }

    #[test]
    fn an_indivisible_allocation_is_refused() {
        let err = spin_off(3333, &Holding { units: 10, cost: 100 }).unwrap_err();
        assert!(format!("{err:#}").contains("term of an agreement"));
    }

    #[test]
    fn a_reverse_split_is_the_same_rule_backwards() {
        // 1-for-10. The units divide, so it goes through; the cost does not
        // move, so the per-unit cost rises by ten.
        let h = Holding { units: 1000, cost: 5_000_00 };
        let after = split(Split { num: 1, den: 10 }, &h).unwrap();
        assert_eq!(after.units, 100);
        assert_eq!(after.cost, h.cost);

        // …and a holding that does not divide is refused, which is the case a
        // reverse split hits constantly — odd lots are the norm.
        assert!(split(Split { num: 1, den: 10 }, &Holding { units: 105, cost: 100 }).is_err());
    }
}
