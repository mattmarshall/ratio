//! ratio-chart — the chart-of-accounts layer over the conservation kernel.
//!
//! The proved pieces are **authored in Lean** (`//lean:Ratio/Chart/Emit.lean`)
//! and emitted into `generated.rs`: the normal-balance classification
//! (`normal_side`, one proved theorem per account type in `Ratio.Core`) and the
//! debit/credit split of a signed amount (`pos_part` / `neg_part`, whose
//! `posPart a - negPart a = a` is `Ratio.Chart.part_split`).
//!
//! What is written here is the aggregation over a list of postings, mirroring
//! `Ratio.Chart.debits` / `Ratio.Chart.credits` — the same division of labour as
//! `ratio-kernel`, which emits its records and writes the fold against them.
//!
//! The theorem that makes this layer worth having is
//! `Ratio.Chart.trial_balance_ties`: for a conserving transaction, total debits
//! equal total credits. So [`trial_balance`] cannot report an out-of-balance
//! book that the kernel admitted — the accountant's check and the kernel's
//! invariant are the same arithmetic, not two things that have to agree.

mod generated;
pub use generated::*;
pub use ratio_kernel::*;

/// Total debits over a transaction's postings (`Ratio.Chart.debits`).
///
/// ⛔ ACCUMULATED IN `i128`. Two things wrap here and neither is exotic. The
/// SUM adds one magnitude per posting, and `neg_part` is `-a`, which overflows
/// at `i64::MIN` — `Ratio.Bounded.the_range_is_not_symmetric`, and there is
/// exactly one input at which negation is not negation.
///
/// ⚠ The emitted `pos_part`/`neg_part` are still the proved functions and are
/// still correct over `Int`. What is widened is the ACCUMULATION, which no
/// theorem was ever about.
pub fn debits(postings: &[Posting]) -> i128 {
    postings.iter().map(|p| (p.amount as i128).max(0)).sum()
}

/// Total credits over a transaction's postings, as a positive number
/// (`Ratio.Chart.credits`).
pub fn credits(postings: &[Posting]) -> i128 {
    postings.iter().map(|p| -(p.amount as i128).min(0)).sum()
}

/// The trial balance of a transaction's postings.
///
/// ⚠ REFUSES A FIGURE IT CANNOT REPORT rather than truncating one. A trial
/// balance is read to decide whether a book ties; a truncated one could tie
/// when the book does not, which is the same failure the kernel's balance check
/// had.
pub fn trial_balance(postings: &[Posting]) -> anyhow::Result<TrialBalance> {
    Ok(TrialBalance {
        debits: i64::try_from(debits(postings))
            .map_err(|_| anyhow::anyhow!("total debits do not fit in 64 bits"))?,
        credits: i64::try_from(credits(postings))
            .map_err(|_| anyhow::anyhow!("total credits do not fit in 64 bits"))?,
    })
}

/// The trial balance of a whole ledger — the monoidal fold of the log, summed
/// per side. Conservation is closed under the fold (`Ratio.Core.ledger_conserves`),
/// so if every transaction balances then so does this.
///
/// ⛔ THIS IS THE ONE THAT ACTUALLY GOES. A per-transaction trial balance is
/// bounded by one entry; this adds every posting in the LOG, so it grows with
/// history rather than with the fund. Folding `i64` here wraps on a long-lived
/// book, and a wrapped trial balance can TIE when the book does not — which is
/// the same failure the kernel's balance check had, one level up.
pub fn ledger_trial_balance(log: &[Transaction]) -> anyhow::Result<TrialBalance> {
    let mut d: i128 = 0;
    let mut c: i128 = 0;
    for txn in log {
        d += debits(&txn.postings);
        c += credits(&txn.postings);
    }
    Ok(TrialBalance {
        debits: i64::try_from(d)
            .map_err(|_| anyhow::anyhow!("this ledger's total debits do not fit in 64 bits"))?,
        credits: i64::try_from(c)
            .map_err(|_| anyhow::anyhow!("this ledger's total credits do not fit in 64 bits"))?,
    })
}

/// Whether a posting sits on the side its account type calls normal.
/// A positive amount is a debit; a negative one is a credit.
pub fn is_normal_side(account: AccountType, amount: i64) -> bool {
    if amount == 0 {
        return true;
    }
    let side = if amount > 0 { Side::Debit } else { Side::Credit };
    normal_side(account) == side
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(dim: i64, amount: i64) -> Posting {
        Posting { dim, amount }
    }

    #[test]
    fn normal_sides_match_the_proved_classification() {
        assert_eq!(normal_side(AccountType::Asset), Side::Debit);
        assert_eq!(normal_side(AccountType::Expense), Side::Debit);
        assert_eq!(normal_side(AccountType::Liability), Side::Credit);
        assert_eq!(normal_side(AccountType::Equity), Side::Credit);
        assert_eq!(normal_side(AccountType::Income), Side::Credit);
    }

    #[test]
    fn parts_split_every_amount() {
        // Ratio.Chart.part_split: pos_part a - neg_part a = a, for every a.
        for a in [-9_999_i64, -1, 0, 1, 7, 1_204_880_11] {
            assert_eq!(pos_part(a) - neg_part(a), a, "failed for {a}");
        }
    }

    #[test]
    fn a_balanced_transaction_ties() {
        let postings = vec![p(0, 500), p(1, -500)];
        assert!(transaction_is_balanced(&Transaction {
            postings: postings.clone()
        }));
        let tb = trial_balance(&postings).unwrap();
        assert_eq!(tb.debits, 500);
        assert_eq!(tb.credits, 500);
        assert!(trial_balance_ties(tb));
    }

    #[test]
    fn an_empty_book_ties() {
        assert!(trial_balance_ties(trial_balance(&[]).unwrap()));
    }

    #[test]
    fn a_multi_leg_transaction_ties() {
        // A fee accrual: expense debit, liability credit, split across two dims.
        let postings = vec![p(10, 61_240_18), p(20, -61_240_00), p(21, -18)];
        assert!(transaction_is_balanced(&Transaction {
            postings: postings.clone()
        }));
        assert!(trial_balance_ties(trial_balance(&postings).unwrap()));
    }

    #[test]
    fn an_unbalanced_transaction_does_not_tie() {
        // The kernel would refuse this; if it ever reached the report, the
        // report says so rather than quietly showing a plug.
        let postings = vec![p(0, 500), p(1, -499)];
        assert!(!transaction_is_balanced(&Transaction {
            postings: postings.clone()
        }));
        assert!(!trial_balance_ties(trial_balance(&postings).unwrap()));
    }

    #[test]
    fn the_ledger_fold_ties_when_every_transaction_does() {
        // Ratio.Core.ledger_conserves + Ratio.Chart.trial_balance_ties.
        let log = vec![
            Transaction {
                postings: vec![p(0, 1_000), p(1, -1_000)],
            },
            Transaction {
                postings: vec![p(2, 250), p(0, -250)],
            },
            Transaction {
                postings: vec![p(3, 7), p(4, -3), p(5, -4)],
            },
        ];
        let tb = ledger_trial_balance(&log).unwrap();
        assert_eq!(tb.debits, 1_257);
        assert_eq!(tb.credits, 1_257);
        assert!(trial_balance_ties(tb));
    }

    #[test]
    fn normal_side_reads_the_sign() {
        assert!(is_normal_side(AccountType::Asset, 100));
        assert!(!is_normal_side(AccountType::Asset, -100));
        assert!(is_normal_side(AccountType::Income, -100));
        assert!(!is_normal_side(AccountType::Income, 100));
        // A zero posting is on nobody's abnormal side.
        assert!(is_normal_side(AccountType::Equity, 0));
    }
}
