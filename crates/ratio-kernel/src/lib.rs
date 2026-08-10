//! ratio-kernel — the conservation kernel (per `research.tex`).
//!
//! The records (`Posting`, `Transaction`) are **authored in Lean**
//! (`//lean:Ratio/Kernel/Emit.lean`) and emitted to Rust into `generated.rs`,
//! gated by a `diff_test`. A transaction is an integer vector over a conserved
//! basis; it is *valid* iff its postings net to zero. The conservation check
//! below mirrors the proven `Ratio.Core` (`Balanced` / `ledger_conserves`); it
//! is branch-minimal integer aggregation (no floats), per the paper. Business
//! rules (accounts, normal-balance) are control-plane, not kernel.

mod generated;
pub use generated::*;
pub use ratio_common::*;

/// Net of a transaction's postings, EXACTLY — the 1-D conserved total
/// (`Ratio.Core.total`).
///
/// ⛔ `i128`, AND THAT IS NOT DEFENSIVENESS. Summed as `i64` this wraps, and a
/// transaction that plainly does not balance can wrap to zero:
///
/// ```text
/// postings   [i64::MAX, i64::MAX, 2]
/// true sum   18446744073709551616
/// i64 .sum() 0            ← and `transaction_is_balanced` said TRUE
/// ```
///
/// Measured, not feared. In a debug build `.sum()` panics; in release it wraps
/// and an unbalanced entry passes the door of a ledger whose entire claim is
/// that it cannot go out of balance.
///
/// ⭐ AND BALANCE IS DECIDABLE WITHOUT OVERFLOW, which is why this returns a
/// number rather than an `Option`. The sum of any realistic number of `i64`s is
/// exact in `i128` — it would take 2^64 postings to trouble it — so the door
/// check stays infallible AND becomes correct. A guard that could ERROR would
/// be a door that fails closed on a valid entry, which is its own kind of wrong.
pub fn transaction_total_exact(txn: &Transaction) -> i128 {
    txn.postings.iter().map(|p| p.amount as i128).sum()
}

/// The net as an `i64`, when it is representable.
///
/// ⚠ Separate from [`transaction_total_exact`] on purpose: a caller that wants
/// a number to POST needs it to fit, and a caller that wants to know whether the
/// entry balances does not. Collapsing the two is how the balance check came to
/// depend on the total being representable.
pub fn transaction_total(txn: &Transaction) -> Option<i64> {
    i64::try_from(transaction_total_exact(txn)).ok()
}

/// A transaction conserves value iff its postings net to zero
/// (`Ratio.Core.Balanced`; the kernel's sole invariant).
pub fn transaction_is_balanced(txn: &Transaction) -> bool {
    transaction_total_exact(txn) == 0
}

/// A zero USD amount — smoke that the Lean-emitted Money API links + composes.
pub fn usd_zero() -> Money {
    money_zero(Currency::Usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_transaction_balances() {
        assert!(transaction_is_balanced(&Transaction { postings: vec![] }));
    }

    #[test]
    fn two_leg_posting_balances() {
        let t = Transaction {
            postings: vec![
                Posting { dim: 0, amount: 500 },
                Posting { dim: 0, amount: -500 },
            ],
        };
        assert_eq!(transaction_total(&t), Some(0));
        assert!(transaction_is_balanced(&t));
    }

    #[test]
    fn an_unbalanced_transaction_cannot_wrap_to_balanced() {
        // ⛔ THE KERNEL'S SOLE INVARIANT, FAILING. Summed as `i64` these wrap to
        // zero and `transaction_is_balanced` returned TRUE — in a ledger whose
        // entire claim is that it cannot go out of balance.
        //
        //     true sum   18446744073709551616
        //     i64 .sum() 0
        //
        // Debug builds panic; release builds wrap and the entry passes the door.
        let t = Transaction {
            postings: vec![
                Posting { dim: 0, amount: i64::MAX },
                Posting { dim: 0, amount: i64::MAX },
                Posting { dim: 0, amount: 2 },
            ],
        };
        assert_eq!(transaction_total_exact(&t), 18_446_744_073_709_551_616i128);
        assert!(!transaction_is_balanced(&t), "it does not balance and must not say it does");
        assert_eq!(transaction_total(&t), None, "and the total is not representable");

        // And the wrapped arithmetic really would have accepted it.
        let wrapped = t.postings.iter().fold(0i64, |a, p| a.wrapping_add(p.amount));
        assert_eq!(wrapped, 0, "unchecked, it nets to zero");
    }

    #[test]
    fn a_balance_at_the_extremes_is_still_a_balance() {
        // ⚠ The other direction: the fix must not refuse a transaction that
        // genuinely balances using extreme values. `is_balanced` is a DOOR, and
        // a door that fails closed on a valid entry is its own kind of wrong.
        let t = Transaction {
            postings: vec![
                Posting { dim: 0, amount: i64::MAX },
                Posting { dim: 1, amount: i64::MIN },
                Posting { dim: 2, amount: 1 },
            ],
        };
        assert!(transaction_is_balanced(&t), "MAX + MIN + 1 == 0");
        assert_eq!(transaction_total(&t), Some(0));
    }

    #[test]
    fn unbalanced_is_rejected() {
        let t = Transaction {
            postings: vec![Posting { dim: 0, amount: 500 }],
        };
        assert!(!transaction_is_balanced(&t));
    }

    #[test]
    fn usd_zero_is_zero() {
        assert!(money_is_zero(usd_zero()));
    }
}
