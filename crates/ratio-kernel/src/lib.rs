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

/// Net of a transaction's postings — the 1-D conserved total (`Ratio.Core.total`).
pub fn transaction_total(txn: &Transaction) -> i64 {
    txn.postings.iter().map(|p| p.amount).sum()
}

/// A transaction conserves value iff its postings net to zero
/// (`Ratio.Core.Balanced`; the kernel's sole invariant).
pub fn transaction_is_balanced(txn: &Transaction) -> bool {
    transaction_total(txn) == 0
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
        assert_eq!(transaction_total(&t), 0);
        assert!(transaction_is_balanced(&t));
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
