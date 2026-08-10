//! Arithmetic that agrees with the proof or declines to answer.
//!
//! ⛔ THE HYPOTHESIS EVERY THEOREM IN THIS REPO WAS ASSUMING. Every proof is
//! stated over Lean's `Int`, which is unbounded. Every emitted function runs on
//! `i64`, which is not — and the failure is not a crash, it is a wrong answer
//! that passes the guard:
//!
//! ```text
//! 4_000_000_000_000_000_000 * 3        wraps to  -6446744073709551616
//! (-6446744073709551616).rem_euclid(2)  ==       0
//! ```
//!
//! So a divisibility check says YES about a product that never happened. The
//! proof cannot see it, because in `Int` the multiplication simply happened.
//!
//! `Ratio.Bounded` is the specification these functions implement, and the
//! boundaries below are the ones it proves. This does not make arithmetic safe —
//! nothing can, at a fixed width. It makes the REFUSAL exact: an operation
//! either agrees with the theorem or declines, with no third case where it
//! answers differently.
//!
//! ⚠ THREE OF THESE ARE LIVE ON THE EMITTED SURFACE, measured rather than
//! feared. `per_share(x, 0)` panics on divide-by-zero — a fund with no units in
//! issue, which every fund is on the day it is established. `per_share(MIN, -1)`
//! panics on divide-with-overflow. And `money_neg(MIN)` returns its argument
//! unchanged, in a kernel whose whole subject is that debits and credits cancel.

use anyhow::{bail, Result};

/// `Ratio.Bounded.add`. The sum, or an error naming what would not fit.
pub fn add(a: i64, b: i64, what: &str) -> Result<i64> {
    a.checked_add(b)
        .ok_or_else(|| anyhow::anyhow!("{what}: {a} + {b} does not fit in 64 bits"))
}

/// `Ratio.Bounded.mul`.
///
/// ⛔ THE ONE THAT FAILS SILENTLY IF UNCHECKED. An overflowing sum tends to look
/// wrong; an overflowing product wraps to a plausible-looking number of the
/// wrong sign, and divisibility guards accept it.
pub fn mul(a: i64, b: i64, what: &str) -> Result<i64> {
    a.checked_mul(b).ok_or_else(|| {
        anyhow::anyhow!(
            "{what}: {a} × {b} does not fit in 64 bits. The proof behind this is stated over \
             unbounded integers and does not rule it out — and the wrapped value would pass a \
             divisibility check"
        )
    })
}

/// `Ratio.Bounded.neg`.
///
/// ⛔ NEGATION CAN FAIL, at exactly one input, because the range is not
/// symmetric: there is one more negative value than positive.
/// `Ratio.Bounded.the_range_is_not_symmetric`.
pub fn neg(a: i64, what: &str) -> Result<i64> {
    a.checked_neg().ok_or_else(|| {
        anyhow::anyhow!("{what}: -({a}) does not fit in 64 bits — the range is not symmetric")
    })
}

/// `Ratio.Bounded.div`, Euclidean to match Lean's `Int`.
///
/// ⛔ TWO WAYS TO FAIL, AND THEY MEAN DIFFERENT THINGS. A zero divisor is a
/// question with no answer — a per-share figure for a fund with no units in
/// issue. `MIN / -1` is a question whose answer will not fit. Both panic in
/// Rust, in every build profile, so neither is something a release build
/// survives.
pub fn div_euclid(a: i64, b: i64, what: &str) -> Result<i64> {
    if b == 0 {
        bail!("{what}: division by zero — there is no answer, not a large one");
    }
    a.checked_div_euclid(b)
        .ok_or_else(|| anyhow::anyhow!("{what}: {a} ÷ {b} does not fit in 64 bits"))
}

/// The remainder that goes with [`div_euclid`]. Non-negative, as Lean's `%` is.
pub fn rem_euclid(a: i64, b: i64, what: &str) -> Result<i64> {
    if b == 0 {
        bail!("{what}: remainder by zero");
    }
    a.checked_rem_euclid(b)
        .ok_or_else(|| anyhow::anyhow!("{what}: {a} mod {b} does not fit in 64 bits"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_arithmetic_is_untouched() {
        // `Ratio.Bounded.ordinary_arithmetic_is_untouched`, the same values.
        // A check that refused real work would be a system that stops working
        // on large funds, which is worse than the bug it prevents.
        assert_eq!(add(1_000_000, 2_000_000, "x").unwrap(), 3_000_000);
        assert_eq!(mul(1_000_000, 1_000_000, "x").unwrap(), 1_000_000_000_000);
        assert_eq!(neg(42, "x").unwrap(), -42);
        assert_eq!(div_euclid(1000, 3, "x").unwrap(), 333);
    }

    #[test]
    fn the_ends_of_the_range_fit_and_one_step_past_does_not() {
        // `Ratio.Bounded.the_ends_of_the_range_fit` and
        // `one_step_past_either_end_does_not_fit` — the boundary itself, which
        // is exactly where an off-by-one would hide.
        assert_eq!(add(i64::MAX, 0, "x").unwrap(), i64::MAX);
        assert_eq!(add(i64::MIN, 0, "x").unwrap(), i64::MIN);
        assert!(add(i64::MAX, 1, "x").is_err());
        assert!(add(i64::MIN, -1, "x").is_err());
    }

    #[test]
    fn a_product_that_would_wrap_is_refused_not_wrapped() {
        // ⛔ The measured case. Unchecked this wraps NEGATIVE and then passes a
        // divisibility guard, which is how a proof about `Int` comes to bless a
        // number the machine never computed.
        let err = mul(4_000_000_000_000_000_000, 3, "reading a holding").unwrap_err();
        assert!(format!("{err:#}").contains("divisibility check"));

        let wrapped = 4_000_000_000_000_000_000i64.wrapping_mul(3);
        assert!(wrapped < 0, "it wraps negative");
        assert_eq!(wrapped.rem_euclid(2), 0, "and looks perfectly divisible");
    }

    #[test]
    fn negating_the_floor_is_refused() {
        // `Ratio.Bounded.negating_the_floor_is_refused`. Unchecked, `0 - MIN`
        // IS `MIN` — the sign does not flip, silently, in a kernel whose whole
        // subject is that debits and credits cancel.
        assert!(neg(i64::MIN, "x").is_err());
        assert_eq!(0i64.wrapping_sub(i64::MIN), i64::MIN, "unchecked, it does not move");
    }

    #[test]
    fn a_zero_divisor_is_refused_rather_than_panicking() {
        // ⛔ `Ratio.Bounded.a_zero_divisor_is_refused`. A fund with no units in
        // issue is not exotic — it is every fund on the day it is established —
        // and `div_euclid` panics on it in EVERY build profile.
        let err = div_euclid(100, 0, "per-share").unwrap_err();
        assert!(format!("{err:#}").contains("no answer, not a large one"), "{err:#}");
        assert!(rem_euclid(100, 0, "per-share").is_err());
    }

    #[test]
    fn the_one_division_that_does_not_fit_is_refused() {
        // `Ratio.Bounded.the_one_division_that_does_not_fit`. Nothing about the
        // inputs looks unusual, and it panics with "divide with overflow".
        assert!(div_euclid(i64::MIN, -1, "x").is_err());
    }
}
