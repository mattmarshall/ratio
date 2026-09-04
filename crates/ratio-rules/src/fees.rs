//! Management-fee accrual — elected terms, conserved receivable/expense.
//!
//! `Ratio.Fees.Accrual`. Stage 1 already computes an accrual amount
//! from a rate, a day-count and a period. This is the books half: the
//! elected `management_fee_accrual` rule posts expense / receivable,
//! and the citeable receivable stays **unset** when fee terms are
//! absent.
//!
//! ⛔ `None` IS UNSET, NOT A SILENT 0. A book that never elected a fee
//! must not report a receivable of nothing. A zero rate is not an
//! election — omit the rule. `Ratio.Fees.no_terms_leaves_receivable_unset`.
//!
//! ⛔ THE POSTING CONSERVES. Expense debit and receivable credit are
//! opposite signs of one amount. Same-sign legs are not an accrual.
//! `Ratio.Fees.a_posting_conserves`.
//!
//! The amount itself is Stage 1's integer formula (half-up). This
//! module does not invent a second one. A zero computed amount is
//! not a posting — appending it would increment a count and print a
//! silent 0.
//!
//! Invoice PDF / LP statements / payment collection stay Connect.

use anyhow::Result;

use crate::{DayCount, RuleKind, RuleSet};

/// The elected management-fee rule. A different id is a generic
/// Stage 1 accrual, not this figure.
pub const FEE_RULE_ID: &str = "management_fee_accrual";

/// Elected fee terms, read from the named accrual rule.
///
/// `None` (the rule is missing, the kind is wrong, the rate is not
/// positive, a convention is missing, or the legs do not name a
/// distinct expense and receivable) is unset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeTerms {
    pub rate_bp: i64,
    pub day_count: DayCount,
    /// Debit-weight account — management fee expense.
    pub expense: i64,
    /// Credit-weight account — management fee payable / receivable.
    pub receivable: i64,
}

impl RuleSet {
    /// The elected management-fee terms, or silence.
    ///
    /// ⛔ ABSENCE IS UNSET. CreateBook writes no fee rule, so a new
    /// Investment book has no terms and no receivable. Writing the
    /// rule is the election. A silent 75 bp on every fund would be
    /// the same defect as a silent FIFO.
    pub fn fee_terms(&self) -> Option<FeeTerms> {
        let rule = self.rule(FEE_RULE_ID)?;
        if rule.kind != RuleKind::Accrual {
            return None;
        }
        let rate_bp = rule.rate_bp.filter(|n| *n > 0)?;
        let day_count = rule.day_count?;
        let expense = rule.legs.iter().find(|l| l.weight > 0).map(|l| l.account)?;
        let receivable = rule.legs.iter().find(|l| l.weight < 0).map(|l| l.account)?;
        if expense == receivable {
            // ⛔ A COLLIDED CHART HIDES THE FEE. Two roles on one
            // dimension net to zero in place; the trial balance ties
            // and the receivable is somebody else's.
            return None;
        }
        Some(FeeTerms {
            rate_bp,
            day_count,
            expense,
            receivable,
        })
    }
}

/// Accrue from elected terms.
///
/// `None` terms stay unset. A zero computed amount is not a posting.
/// The pair is (expense debit, receivable credit) — they conserve.
/// `Ratio.Fees.no_terms_is_unset`, `Ratio.Fees.a_posting_conserves`.
pub fn accrue(
    terms: Option<&FeeTerms>,
    basis: i64,
    days: i64,
) -> Result<Option<(i64, i64)>> {
    let Some(t) = terms else {
        return Ok(None);
    };
    if t.rate_bp <= 0 {
        return Ok(None);
    }
    let amount = crate::computed_accrual(t.rate_bp, t.day_count, basis, days)?;
    if amount == 0 {
        return Ok(None);
    }
    Ok(Some((amount, -amount)))
}

/// The citeable receivable.
///
/// `None` terms stay unset — even if `posted` is non-empty (some other
/// rule moved the payable). An empty slice stays unset, not a measured
/// 0. A posted then reversed list that sums to 0 is a real zero.
/// `Ratio.Fees.no_terms_leaves_receivable_unset`.
pub fn fee_receivable(terms: Option<&FeeTerms>, posted: &[i64]) -> Option<i64> {
    let Some(t) = terms else {
        return None;
    };
    if t.rate_bp <= 0 {
        return None;
    }
    match posted {
        [] => None,
        xs => Some(xs.iter().copied().sum()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, Event, RuleSet};
    use ratio_store::{Account, AccountTypeRecord as A};

    fn chart() -> Vec<Account> {
        vec![
            Account {
                dim: 10,
                display_name: "Management fee expense".into(),
                account_type: A::Expense,
            },
            Account {
                dim: 40,
                display_name: "Management fee payable".into(),
                account_type: A::Liability,
            },
        ]
    }

    const FEE: &str = r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
rate_bp = 75
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1
"#;

    #[test]
    fn no_fee_rule_is_unset_rather_than_a_silent_zero() {
        // `Ratio.Fees.no_terms_is_unset`.
        assert!(RuleSet::default().fee_terms().is_none());
        assert!(RuleSet::from_toml("rules = []\n").unwrap().fee_terms().is_none());
        assert_eq!(accrue(None, 365_000_000, 1).unwrap(), None);
        assert_eq!(fee_receivable(None, &[]), None);
        assert_eq!(
            fee_receivable(None, &[100]),
            None,
            "posted under some other rule is not a fee receivable"
        );
    }

    #[test]
    fn a_zero_rate_is_not_an_election() {
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
rate_bp = 0
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1
"#,
        )
        .unwrap();
        assert!(set.fee_terms().is_none(), "0 bp is silence wearing a rule");
        let findings = check(&set, &chart());
        assert!(
            findings.iter().any(|f| f.message.contains("silent zero receivable")),
            "{findings:?}"
        );
    }

    #[test]
    fn same_sign_legs_are_not_an_accrual() {
        // `Ratio.Fees` wellFormedAccrual ⟨100, 100⟩ = false.
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
rate_bp = 75
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = 1
"#,
        )
        .unwrap();
        let findings = check(&set, &chart());
        assert!(
            findings.iter().any(|f| f.message.contains("does not balance")),
            "same-sign must fail the template, not post a hidden fee: {findings:?}"
        );
        // The credit leg is missing, so this is not elected terms either.
        assert!(set.fee_terms().is_none());
    }

    #[test]
    fn a_collided_chart_is_not_fee_terms() {
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
rate_bp = 75
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 10
weight = -1
"#,
        )
        .unwrap();
        assert!(
            set.fee_terms().is_none(),
            "expense and receivable on one dim hide the fee"
        );
    }

    #[test]
    fn elected_terms_accrue_a_conserved_receivable_and_expense() {
        let set = RuleSet::from_toml(FEE).unwrap();
        assert!(check(&set, &chart()).is_empty());
        let terms = set.fee_terms().expect("75 bp act/365 is an election");
        assert_eq!(terms.rate_bp, 75);
        assert_eq!(terms.day_count, DayCount::Act365);
        assert_eq!(terms.expense, 10);
        assert_eq!(terms.receivable, 40);

        // 365_000_000 × 75 × 1 / (10_000 × 365) = 7_500 exactly.
        let pair = accrue(Some(&terms), 365_000_000, 1)
            .unwrap()
            .expect("a dividing year-fraction is an accrual");
        assert_eq!(pair, (7_500, -7_500));
        assert_eq!(pair.0 + pair.1, 0, "Ratio.Fees.a_posting_conserves");

        let compiled = crate::compile(
            set.rule(FEE_RULE_ID).unwrap(),
            &Event {
                rule: FEE_RULE_ID.into(),
                id: "a1".into(),
                amount: 365_000_000,
                days: Some(1),
                memo: String::new(),
                instrument: None,
                quantity: None,
            },
        )
        .unwrap();
        assert_eq!(compiled.len(), 2);
        let expense = compiled.iter().find(|p| p.dim == 10).unwrap();
        let recv = compiled.iter().find(|p| p.dim == 40).unwrap();
        assert_eq!(expense.amount, 7_500);
        assert_eq!(recv.amount, -7_500);
        assert_eq!(expense.amount + recv.amount, 0);
        assert_eq!(
            (expense.amount, recv.amount),
            pair,
            "the compiled legs ARE the proved pair — not a second amount"
        );
    }

    #[test]
    fn a_zero_day_accrual_is_not_a_posting() {
        let set = RuleSet::from_toml(FEE).unwrap();
        let terms = set.fee_terms().unwrap();
        assert_eq!(
            accrue(Some(&terms), 365_000_000, 0).unwrap(),
            None,
            "Ratio.Fees.a_zero_amount_is_not_an_accrual"
        );
    }

    #[test]
    fn receivable_stays_unset_until_an_accrual_posts() {
        let set = RuleSet::from_toml(FEE).unwrap();
        let terms = set.fee_terms();
        assert_eq!(fee_receivable(terms.as_ref(), &[]), None);
        assert_eq!(fee_receivable(terms.as_ref(), &[7_500]), Some(7_500));
        assert_eq!(
            fee_receivable(terms.as_ref(), &[7_500, -7_500]),
            Some(0),
            "paid in full is a real zero, not unset"
        );
    }
}
