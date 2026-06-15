//! ratio-api — the gRPC service surface over the kernel.
//!
//! The generated `ratio.v1` wire messages (//proto:ratio_rust_proto, crate
//! `ratio_proto`) are converted to the Lean-authored kernel records; a
//! transaction is *admissible* only if it conserves value — its postings net to
//! zero (`ratio_kernel::transaction_is_balanced`), the kernel's sole invariant
//! (research.tex). `CreateTransaction` rejects the rest with FAILED_PRECONDITION.
//!
//! The tonic `impl Ledger` + server transport wrap this logic; that wiring is
//! deferred (it needs the rules_rust_prost tonic-consumer pattern). This module
//! is the transport-agnostic core, fully tested.

use ratio_kernel::{Posting, Transaction};
use ratio_proto::ratio::v1::{Posting as WirePosting, Transaction as WireTransaction};

/// Convert a wire posting to the kernel record.
pub fn posting_from_wire(p: &WirePosting) -> Posting {
    Posting { dim: p.dim, amount: p.amount }
}

/// Convert a wire transaction to the kernel record (the vector of postings).
pub fn transaction_from_wire(t: &WireTransaction) -> Transaction {
    Transaction {
        postings: t.postings.iter().map(posting_from_wire).collect(),
    }
}

/// A transaction is admissible iff it conserves value — its postings net to zero
/// across every dimension (research.tex). The Ledger service admits only these.
pub fn is_admissible(t: &WireTransaction) -> bool {
    ratio_kernel::transaction_is_balanced(&transaction_from_wire(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(postings: Vec<WirePosting>) -> WireTransaction {
        WireTransaction { postings, ..Default::default() }
    }

    #[test]
    fn balanced_two_leg_is_admissible() {
        let t = wire(vec![
            WirePosting { dim: 0, amount: 500 },
            WirePosting { dim: 0, amount: -500 },
        ]);
        assert!(is_admissible(&t));
    }

    #[test]
    fn unbalanced_is_rejected() {
        assert!(!is_admissible(&wire(vec![WirePosting { dim: 0, amount: 500 }])));
    }

    #[test]
    fn empty_is_admissible() {
        assert!(is_admissible(&wire(vec![])));
    }

    #[test]
    fn conversion_preserves_postings() {
        let t = transaction_from_wire(&wire(vec![WirePosting { dim: 7, amount: 42 }]));
        assert_eq!(t.postings.len(), 1);
        assert_eq!(t.postings[0].dim, 7);
        assert_eq!(t.postings[0].amount, 42);
    }
}
