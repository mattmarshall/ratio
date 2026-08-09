//! Reading a holding through its corporate actions, instead of rewriting it.
//!
//! `Ratio.Closure.an_open_action_makes_the_nav_read_the_lots`: applying a split
//! by rewriting is a walk over every lot of the instrument — forty thousand for
//! an S&P name, eighty times the entire quiet period end. Reading through the
//! actions instead costs O(splits) on the lots actually touched, and a NAV
//! touches none.
//!
//! ⛔ ONE STEP AT A TIME, NEVER A COMPOSED RATIO.
//! `Ratio.Actions.Factor.a_factor_can_succeed_where_the_rewrite_refuses`:
//! 3-for-2 then 2-for-1 composes to ⟨6,2⟩, which divides five units cleanly and
//! returns fifteen — while applying them in order refuses, because the
//! intermediate seven-and-a-half shares were never held and the half was paid
//! out as cash in lieu. Both are correct arithmetic. One is wrong about the
//! world.
//!
//! ⚠ AND THE TWO WAYS A DIVISION HERE CAN FAIL MEAN OPPOSITE THINGS, which is
//! the trap this module is shaped around. Reading forward and hitting a
//! fraction means a fraction was really paid away — a posting the configuration
//! must declare. Relieving backward and hitting a fraction means nothing of the
//! sort: the client owns the shares and is selling them, and the fraction is an
//! artifact of storing pre-split units. The first refuses; the second
//! subdivides.

use anyhow::{bail, Result};

use crate::generated_actions::{relief_divides, relief_units, step_divides, step_units};

/// A split, as one announcement declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub num: i64,
    pub den: i64,
}

/// What a lot holds today, reading through the splits since it was acquired.
///
/// `steps` must be in EX-DATE ORDER, oldest first.
/// `Ratio.Actions.actions_do_not_commute` — a 3-for-2 then a 2-for-1 is not a
/// 2-for-1 then a 3-for-2: one order divides and the other refuses, so the
/// order is part of the answer rather than an implementation detail.
pub fn units_as_of(stored_units: i64, steps: &[Step]) -> Result<i64> {
    let mut units = stored_units;
    for (n, s) in steps.iter().enumerate() {
        if !step_divides(units, s.num, s.den) {
            bail!(
                "{units} units {}-for-{} (split {} of {}) is not a whole number of units — \
                 the fraction is cash in lieu, which realizes a gain, and this configuration \
                 has not declared how it is handled",
                s.num,
                s.den,
                n + 1,
                steps.len()
            );
        }
        units = step_units(units, s.num, s.den);
    }
    Ok(units)
}

/// How a sale quoted in today's units lands on a lot stored in its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relief {
    /// It lands on whole stored units. Relieve this many.
    Whole(i64),
    /// ⛔ IT STRADDLES. The sale is perfectly ordinary — the client owns the
    /// shares and is selling them — and the fraction is an artifact of storing
    /// pre-split units. The lot must be SUBDIVIDED:
    /// `Ratio.Lots.partition_sums_to_whole` says that changes nothing held, and
    /// `Ratio.Lots.partial_relief_is_exactly_pro_rata` says the cost follows
    /// exactly.
    ///
    /// Carries the whole part, and the numerator/denominator of what is left,
    /// so the caller subdivides rather than rounds. Nothing here may round:
    /// `Ratio.Lots` is a standing reminder that conservation holds while the
    /// wrong amount moves.
    Subdivide { whole: i64, part_num: i64, part_den: i64 },
}

/// Convert a sale quoted in today's units into stored units.
///
/// ⚠ THIS NEVER REFUSES, and that is the difference from [`units_as_of`]. A
/// sale that does not land on a stored unit is not an error — it is a lot that
/// needs splitting in two, which is one write instead of forty thousand.
pub fn relief_in_stored_units(wanted: i64, steps: &[Step]) -> Result<Relief> {
    // Compose for the reverse direction. Safe here in a way it is not going
    // forward: the guard going forward is about intermediate holdings that
    // really existed, and there are no intermediates in a single conversion of
    // one quantity.
    let (mut num, mut den) = (1i64, 1i64);
    for s in steps {
        if s.den == 0 {
            bail!("a split cannot be for zero units");
        }
        num = num.checked_mul(s.num).ok_or_else(|| anyhow::anyhow!("split overflows"))?;
        den = den.checked_mul(s.den).ok_or_else(|| anyhow::anyhow!("split overflows"))?;
    }
    if num == 0 {
        bail!("a split cannot be for zero units");
    }
    if relief_divides(wanted, num, den) {
        return Ok(Relief::Whole(relief_units(wanted, num, den)));
    }
    let scaled = wanted.checked_mul(den).ok_or_else(|| anyhow::anyhow!("relief overflows"))?;
    Ok(Relief::Subdivide {
        whole: scaled.div_euclid(num),
        part_num: scaled.rem_euclid(num),
        part_den: num,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(num: i64, den: i64) -> Step {
        Step { num, den }
    }

    #[test]
    fn no_history_reads_straight_through() {
        // `Ratio.Actions.Factor.no_announcement_is_the_identity`. Most
        // instruments, most of the time — and it costs nothing.
        assert_eq!(units_as_of(1_000, &[]).unwrap(), 1_000);
    }

    #[test]
    fn a_split_scales_what_the_lot_holds() {
        assert_eq!(units_as_of(100, &[s(2, 1)]).unwrap(), 200);
        assert_eq!(units_as_of(1_000, &[s(1, 10)]).unwrap(), 100, "a reverse split");
        assert_eq!(units_as_of(100, &[s(2, 1), s(3, 2)]).unwrap(), 300);
    }

    #[test]
    fn reading_step_by_step_refuses_where_a_composed_factor_would_not() {
        // ⛔ THE COUNTEREXAMPLE, IN THE RUNNING CODE.
        // `Ratio.Actions.Factor.a_factor_can_succeed_where_the_rewrite_refuses`.
        //
        // Five units, 3-for-2 then 2-for-1. Composed that is ⟨6,2⟩ and five
        // units divide cleanly into fifteen. Step by step the first split gives
        // 7.5, which is not a share — the holder held seven and was paid cash
        // for the half.
        let err = units_as_of(5, &[s(3, 2), s(2, 1)]).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("cash in lieu"), "{msg}");
        assert!(msg.contains("split 1 of 2"), "names WHICH step: {msg}");

        // And the composed factor really would have accepted it, which is why
        // the step-by-step fold is not an implementation preference.
        assert!(step_divides(5, 6, 2), "⟨6,2⟩ divides five units");
        assert_eq!(step_units(5, 6, 2), 15, "silently, into fifteen");
    }

    #[test]
    fn where_every_step_divides_the_two_agree() {
        // `Ratio.Actions.Factor.they_agree_when_every_step_divides`.
        assert_eq!(units_as_of(4, &[s(3, 2), s(2, 1)]).unwrap(), 12);
        assert_eq!(step_units(4, 6, 2), 12, "and the composed factor agrees here");
    }

    #[test]
    fn order_changes_whether_it_is_readable_at_all() {
        // `Ratio.Actions.actions_do_not_commute`. Not a performance question —
        // it decides whether the holding can be read.
        assert!(units_as_of(5, &[s(3, 2), s(2, 1)]).is_err());
        assert_eq!(units_as_of(5, &[s(2, 1), s(3, 2)]).unwrap(), 15);
    }

    #[test]
    fn an_even_sale_lands_on_whole_stored_units() {
        // `Ratio.Actions.Factor.an_even_sale_lands_exactly`.
        assert_eq!(relief_in_stored_units(100, &[s(2, 1)]).unwrap(), Relief::Whole(50));
        assert_eq!(relief_in_stored_units(7, &[]).unwrap(), Relief::Whole(7));
    }

    #[test]
    fn an_odd_sale_subdivides_rather_than_refusing() {
        // ⛔ THE DISTINCTION THIS MODULE EXISTS FOR.
        // `Ratio.Actions.Factor.an_odd_sale_after_a_split_does_not_land_on_a_
        // stored_lot`. 101 shares at 2-for-1 is fifty and a half stored units.
        //
        // Nothing fractional is being paid away — the client owns 101 shares
        // and is selling all of them. Refusing would be refusing an ordinary
        // instruction, and rounding would lose half a stored unit while the
        // books went on tying.
        match relief_in_stored_units(101, &[s(2, 1)]).unwrap() {
            Relief::Subdivide { whole, part_num, part_den } => {
                assert_eq!((whole, part_num, part_den), (50, 1, 2), "fifty and one half");
                // ⛔ EXACT, not rounded: the part is carried as a ratio so the
                // caller subdivides. `Ratio.Lots.partial_relief_is_exactly_pro_rata`.
                assert_eq!(whole * 2 + part_num, 101, "and it accounts for all 101");
            }
            Relief::Whole(_) => panic!("101 at 2-for-1 does not land on a stored unit"),
        }
    }

    #[test]
    fn the_two_failures_are_not_the_same_failure() {
        // Reading forward refuses; relieving backward subdivides. Same
        // division, opposite meanings, and reusing one guard for both is how a
        // perfectly ordinary sale would come to be rejected as cash in lieu.
        assert!(units_as_of(5, &[s(3, 2)]).is_err(), "forward: refuses");
        assert!(
            matches!(
                relief_in_stored_units(5, &[s(3, 2)]).unwrap(),
                Relief::Subdivide { .. }
            ),
            "backward: subdivides"
        );
    }

    #[test]
    fn a_short_position_reads_the_way_lean_divides() {
        // ⛔ Euclidean, as `Ratio.Closure` pins for `perShare`. Rust's own `/`
        // and `%` truncate toward zero and would disagree here; units go
        // negative on a short.
        assert_eq!(units_as_of(-100, &[s(2, 1)]).unwrap(), -200);
        assert!(step_divides(-100, 2, 1));
    }

    #[test]
    fn zero_denominators_are_refused_rather_than_dividing() {
        assert!(units_as_of(100, &[s(2, 0)]).is_err());
        assert!(relief_in_stored_units(100, &[s(2, 0)]).is_err());
        assert!(relief_in_stored_units(100, &[s(0, 1)]).is_err(), "and a zero numerator");
    }
}
