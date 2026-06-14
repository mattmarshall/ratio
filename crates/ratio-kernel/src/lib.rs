//! ratio-kernel — the double-entry accounting engine.
//!
//! Phase 1 (this increment) wires the Lean-authored Money layer; the
//! Account/Split/Transaction model + the balance/normal-side invariant proofs
//! land in Phase 1b (authored + proven in Lean, emitted here like ratio-common).

pub use ratio_common::*;

/// A zero USD amount — smoke that the Lean-emitted Money API links + composes.
pub fn usd_zero() -> Money {
    money_zero(Currency::Usd)
}

#[cfg(test)]
mod tests {
    #[test]
    fn usd_zero_is_zero() {
        assert!(super::money_is_zero(super::usd_zero()));
    }
}
