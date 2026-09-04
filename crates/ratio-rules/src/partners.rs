//! Partner allocation cut — named weights, not a partner count.
//!
//! `Ratio.Partners.Cut`. `/capital` allocated income / expense /
//! unrealized stay unset without a cut. A silent 1/N of book NAV, or a
//! fabricated 0.00 share of a figure that moved, is the defect.
//!
//! ⛔ EVERY PRODUCT IS CHECKED BEFORE THE DIVISIBILITY GUARD IS ASKED.
//! The theorems are over Lean's `Int`. Asking `rem_euclid` about a
//! wrapped `i64` product gets an answer about a number that never
//! happened. `ratio_common::checked` is the Rust side of
//! `Ratio.Bounded`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};
use ratio_common::checked;
use serde::{Deserialize, Serialize};

/// One named weight in a cut.
///
/// `partner` is the grain on `Partner capital — LP` (`LP`), not a
/// chart dimension. `weight` is a positive integer share of the whole
/// — 80 and 20, not 0.80. The total is the sum, not 100 and not the
/// partner count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartnerShare {
    pub partner: String,
    pub weight: i64,
}

/// The book figure a special or a default cut applies to.
///
/// ⛔ THESE THREE, AND NO OTHER. A kind nobody named is not a silent
/// income row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationKind {
    Income,
    Expense,
    Unrealized,
}

impl AllocationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Income => "income",
            Self::Expense => "expense",
            Self::Unrealized => "unrealized",
        }
    }
}

/// A standing special: this partner's weight of this kind, replacing
/// the default cut for that kind.
///
/// ⛔ A WEIGHT, NOT AN AMOUNT. Period figures move. A standing 100% of
/// expense to the GP is `weight = 1` as the sole share of that kind.
/// Exact amounts are journal facts (`JournalEntry.special_allocations`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialAllocation {
    pub partner: String,
    pub kind: AllocationKind,
    pub weight: i64,
}

/// An exact amount named on a journal entry. Not a weight.
///
/// `None` on the entry means this sale/close carries no special.
/// `Some([])` is elected and unnamed and refuses — the SpecID shape.
/// `Ratio.Partners.unnamed_facts_refuse`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationFact {
    pub partner: String,
    /// `income` / `expense` / `unrealized`.
    pub kind: String,
    pub amount: i64,
}

/// Check a cut at read time.
///
/// Empty is silence — nobody said, allocated plugs stay unset. A
/// zero or negative weight is not a weight. Two rows for one partner
/// are two answers under one name.
pub fn check_cut(cut: &[PartnerShare]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for s in cut {
        if s.partner.trim().is_empty() {
            bail!(
                "a partner_cut row without a partner is not a cut — the grain \
                 is the suffix on Partner capital (LP, GP), not a blank"
            );
        }
        if s.weight <= 0 {
            bail!(
                "partner_cut {:?} weight is {}, and a non-positive weight is not \
                 a weight — that row would take nothing or the opposite of what \
                 the election says. Ratio.Partners.wellFormed",
                s.partner,
                s.weight
            );
        }
        if !seen.insert(s.partner.clone()) {
            bail!(
                "this configuration names partner_cut {:?} twice. Two rows for \
                 one partner are two answers under one name. \
                 Ratio.Partners.wellFormed",
                s.partner
            );
        }
    }
    Ok(())
}

/// Check standing specials at read time.
pub fn check_specials(specials: &[SpecialAllocation]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for s in specials {
        if s.partner.trim().is_empty() {
            bail!(
                "a special_allocation row without a partner is not a cut — \
                 omit the row"
            );
        }
        if s.weight <= 0 {
            bail!(
                "special_allocation {:?} {:?} weight is {}, and a non-positive \
                 weight is not a weight. Ratio.Partners.wellFormed",
                s.partner,
                s.kind.as_str(),
                s.weight
            );
        }
        if !seen.insert((s.kind, s.partner.clone())) {
            bail!(
                "this configuration names special_allocation {:?} {:?} twice. \
                 Two rows for one partner and kind are two answers under one name",
                s.partner,
                s.kind.as_str()
            );
        }
    }
    Ok(())
}

/// The cut that applies to a kind: the specials if any were named,
/// otherwise the default.
///
/// `Ratio.Partners.cutFor`. Empty specials are silence, not a
/// zero-share cut of that kind.
pub fn cut_for<'a>(
    kind: AllocationKind,
    default: &'a [PartnerShare],
    specials: &'a [SpecialAllocation],
) -> Vec<PartnerShare> {
    let named: Vec<PartnerShare> = specials
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| PartnerShare {
            partner: s.partner.clone(),
            weight: s.weight,
        })
        .collect();
    if named.is_empty() {
        default.to_vec()
    } else {
        named
    }
}

/// Apply a cut to a figure.
///
/// ⭐ `Ok(None)` IS UNSET. An empty cut, or a `None` the caller already
/// decided, is not a silent 1/N. `Ratio.Partners.no_cut_is_unset`.
///
/// ⛔ A FIGURE THAT WILL NOT DIVIDE IS `Err`, NOT A ROUNDED `Ok`.
/// Flooring would leave a remainder nowhere. The books would still
/// tie. `Ratio.Partners.a_slice_is_exactly_pro_rata`.
///
/// ⛔ THE PRODUCT IS CHECKED FIRST. An overflowing `figure × weight`
/// that wrapped would pass the divisibility guard about a number the
/// machine never computed.
pub fn allocate(figure: i64, cut: &[PartnerShare]) -> Result<Option<BTreeMap<String, i64>>> {
    if cut.is_empty() {
        return Ok(None);
    }
    check_cut(cut)?;
    let mut total: i64 = 0;
    for s in cut {
        total = checked::add(total, s.weight, "partner_cut total")?;
    }
    if total <= 0 {
        bail!(
            "partner_cut weights sum to {total}, and a non-positive total is \
             not a cut. Ratio.Partners.wellFormed"
        );
    }
    let mut out = BTreeMap::new();
    for s in cut {
        let prod = checked::mul(figure, s.weight, "partner_cut slice")?;
        let rem = checked::rem_euclid(prod, total, "partner_cut slice")?;
        if rem != 0 {
            bail!(
                "partner_cut cannot allocate {figure} across the named weights \
                 ({} / {total} for {:?}): the figure does not divide exactly, \
                 and a remainder would be a misstatement of who owns the \
                 income, not a rounding error. \
                 Ratio.Partners.a_slice_is_exactly_pro_rata",
                s.weight,
                s.partner
            );
        }
        let share = checked::div_euclid(prod, total, "partner_cut slice")?;
        out.insert(s.partner.clone(), share);
    }
    Ok(Some(out))
}

/// Apply journal specials of one kind, then the remainder cut.
///
/// `None` facts fall through to the cut. `Some([])` refuses.
/// Facts that cover the figure are the allocation. A remainder
/// needs a cut that divides. An overshoot refuses.
/// `Ratio.Partners.applyFacts`.
pub fn apply_facts(
    figure: i64,
    facts: Option<&[AllocationFact]>,
    kind: AllocationKind,
    remainder: &[PartnerShare],
) -> Result<Option<BTreeMap<String, i64>>> {
    let Some(all) = facts else {
        return allocate(figure, remainder);
    };
    if all.is_empty() {
        bail!(
            "special_allocations is present and empty — that is an unnamed \
             election, not silence. Omit the field. A silent 1/N is the \
             defect. Ratio.Partners.unnamed_facts_refuse"
        );
    }
    let mut taken: i64 = 0;
    let mut out = BTreeMap::new();
    for f in all.iter().filter(|f| f.kind == kind.as_str()) {
        if f.partner.trim().is_empty() {
            bail!("a special allocation without a partner is not a cut");
        }
        taken = checked::add(taken, f.amount, "special allocation")?;
        let slot = out.entry(f.partner.clone()).or_insert(0);
        *slot = checked::add(*slot, f.amount, "special allocation")?;
    }
    if taken == figure {
        return Ok(Some(out));
    }
    if taken > figure {
        bail!(
            "special allocations of {} sum to {taken}, which overshoots \
             the book figure {figure}. Ratio.Partners.an_overshoot_refuses",
            kind.as_str()
        );
    }
    // taken < figure, including taken == 0 when no facts of this kind.
    if taken == 0 && all.iter().all(|f| f.kind != kind.as_str()) {
        return allocate(figure, remainder);
    }
    let left = checked::sub(figure, taken, "special allocation remainder")?;
    let Some(rest) = allocate(left, remainder)? else {
        bail!(
            "special allocations of {} leave remainder {left} and there is \
             no partner cut for what is left — unset, not a silent split. \
             Ratio.Partners.no_facts_and_no_cut_is_unset",
            kind.as_str()
        );
    };
    for (partner, amount) in rest {
        let slot = out.entry(partner).or_insert(0);
        *slot = checked::add(*slot, amount, "special allocation remainder")?;
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(partner: &str, weight: i64) -> PartnerShare {
        PartnerShare {
            partner: partner.into(),
            weight,
        }
    }

    #[test]
    fn no_cut_is_unset_not_an_equal_split() {
        // `Ratio.Partners.no_cut_is_unset`. 30.00 / 2 is 15.00.
        assert_eq!(allocate(3000, &[]).unwrap(), None);
        assert_ne!(
            allocate(3000, &[]).unwrap(),
            Some(BTreeMap::from([
                ("LP".into(), 1500),
                ("GP".into(), 1500)
            ]))
        );
    }

    #[test]
    fn eighty_twenty_of_a_hundred_is_eighty_and_twenty() {
        let cut = [share("LP", 80), share("GP", 20)];
        let got = allocate(10000, &cut).unwrap().expect("divides");
        assert_eq!(got.get("LP"), Some(&8000));
        assert_eq!(got.get("GP"), Some(&2000));
        assert_eq!(got.values().sum::<i64>(), 10000);
        // ⛔ NOT 1/N. 50/50 of 100.00 is 50.00.
        assert_ne!(got.get("LP"), Some(&5000));
    }

    #[test]
    fn a_figure_that_will_not_divide_is_refused() {
        let cut = [share("LP", 80), share("GP", 20)];
        let err = allocate(101, &cut).unwrap_err().to_string();
        assert!(err.contains("does not divide"), "{err}");
        assert!(err.contains("pro_rata") || err.contains("misstatement"), "{err}");
    }

    #[test]
    fn a_zero_weight_is_not_a_cut() {
        let err = allocate(100, &[share("LP", 80), share("GP", 0)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a weight"), "{err}");
    }

    #[test]
    fn a_duplicate_partner_is_two_answers() {
        let err = allocate(100, &[share("LP", 50), share("LP", 50)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("twice"), "{err}");
    }

    #[test]
    fn an_overflowing_slice_is_refused_not_wrapped() {
        // ⛔ THE MEASURED CASE. Unchecked, a huge figure × weight wraps
        // and can pass a divisibility guard.
        let cut = [share("LP", 3), share("GP", 1)];
        let err = allocate(4_000_000_000_000_000_000, &cut)
            .unwrap_err()
            .to_string();
        assert!(err.contains("64 bits") || err.contains("divisibility"), "{err}");
    }

    #[test]
    fn a_kind_without_specials_uses_the_default() {
        let default = [share("LP", 80), share("GP", 20)];
        let specials = [SpecialAllocation {
            partner: "GP".into(),
            kind: AllocationKind::Expense,
            weight: 1,
        }];
        let income = cut_for(AllocationKind::Income, &default, &specials);
        assert_eq!(income, default);
        let expense = cut_for(AllocationKind::Expense, &default, &specials);
        assert_eq!(expense, vec![share("GP", 1)]);
        let whole = allocate(5000, &expense).unwrap().expect("sole taker");
        assert_eq!(whole.get("GP"), Some(&5000));
        assert!(whole.get("LP").is_none());
    }

    #[test]
    fn unnamed_facts_refuse_rather_than_splitting() {
        let err = apply_facts(100, Some(&[]), AllocationKind::Income, &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unnamed"), "{err}");
    }

    #[test]
    fn facts_that_cover_the_figure_are_the_allocation() {
        let facts = [AllocationFact {
            partner: "GP".into(),
            kind: "income".into(),
            amount: 3000,
        }];
        let got = apply_facts(3000, Some(&facts), AllocationKind::Income, &[])
            .unwrap()
            .expect("covered");
        assert_eq!(got.get("GP"), Some(&3000));
    }

    #[test]
    fn an_overshoot_refuses() {
        let facts = [AllocationFact {
            partner: "GP".into(),
            kind: "income".into(),
            amount: 12,
        }];
        let err = apply_facts(10, Some(&facts), AllocationKind::Income, &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("overshoots"), "{err}");
    }

    #[test]
    fn a_remainder_without_a_cut_is_unset() {
        let facts = [AllocationFact {
            partner: "GP".into(),
            kind: "income".into(),
            amount: 40,
        }];
        let err = apply_facts(100, Some(&facts), AllocationKind::Income, &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("no partner cut"), "{err}");
    }

    #[test]
    fn facts_plus_a_remainder_that_divides_conserve() {
        // `Ratio.Partners` example: 40 named + 80/20 of leftover 60.
        let facts = [AllocationFact {
            partner: "SP".into(),
            kind: "income".into(),
            amount: 40,
        }];
        let cut = [share("LP", 80), share("GP", 20)];
        let got = apply_facts(100, Some(&facts), AllocationKind::Income, &cut)
            .unwrap()
            .expect("divides");
        assert_eq!(got.get("SP"), Some(&40));
        assert_eq!(got.get("LP"), Some(&48));
        assert_eq!(got.get("GP"), Some(&12));
        assert_eq!(got.values().sum::<i64>(), 100);
    }

    #[test]
    fn no_unit_movement_is_unset_not_a_fake_zero() {
        // `Ratio.Partners.no_movement_is_unset`.
        assert_eq!(units_in_issue(&[]), None);
        assert_ne!(units_in_issue(&[]), Some(0));
    }

    #[test]
    fn a_subscription_then_a_redemption_leaves_the_difference() {
        // 10 issued, 4 retired → 6. `Ratio.Partners` example.
        assert_eq!(units_in_issue(&[10, -4]), Some(6));
        assert_eq!(redeem(Some(10), 4).unwrap(), Some(6));
        assert_eq!(redeem(Some(10), 10).unwrap(), Some(0));
        // Issued and redeemed stay apart — the net is not the plug.
        // `Ratio.Partners.periodIssued` / `periodRedeemed`.
        assert_eq!(period_issued(&[]), None);
        assert_eq!(period_redeemed(&[]), None);
        assert_eq!(period_issued(&[10]), Some(10));
        assert_eq!(period_redeemed(&[10]), None);
        assert_eq!(period_issued(&[-4]), None);
        assert_eq!(period_redeemed(&[-4]), Some(4));
        assert_eq!(period_issued(&[10, -4]), Some(10));
        assert_eq!(period_redeemed(&[10, -4]), Some(4));
        assert_eq!(
            period_issued(&[10, -4]).unwrap() - period_redeemed(&[10, -4]).unwrap(),
            units_in_issue(&[10, -4]).unwrap()
        );
    }

    #[test]
    fn a_zero_unit_movement_is_refused() {
        assert!(!well_formed_move(100, 0));
        assert!(well_formed_move(100, 10));
        assert!(well_formed_move(-40, -4));
        assert!(!well_formed_move(100, -10));
    }

    #[test]
    fn cannot_redeem_when_unset_or_more_than_issued() {
        let err = redeem(None, 4).unwrap_err().to_string();
        assert!(err.contains("unset") || err.contains("nobody issued"), "{err}");
        let err = redeem(Some(10), 11).unwrap_err().to_string();
        assert!(err.contains("11") && err.contains("10"), "{err}");
        let err = redeem(Some(10), 0).unwrap_err().to_string();
        assert!(err.contains("zero") || err.contains("not a redemption"), "{err}");
    }

    #[test]
    fn allocating_units_without_a_cut_is_unset() {
        // `Ratio.Partners.allocating_units_without_a_cut_is_unset`.
        assert_eq!(allocate(30, &[]).unwrap(), None);
    }
}

/// A partner-unit movement is well-formed when cash and units are
/// non-zero and the same sign. `Ratio.Partners.wellFormedMove`.
///
/// Zero units is a contribution, not a subscription. Opposite signs
/// would issue units while paying cash out.
pub fn well_formed_move(cash: i64, units: i64) -> bool {
    if units == 0 || cash == 0 {
        return false;
    }
    match checked::mul(cash, units, "unit movement") {
        Ok(p) => p > 0,
        Err(_) => false,
    }
}

/// Units in issue from signed movements (positive issued, negative
/// retired). Empty is unset — not a measured zero.
/// `Ratio.Partners.no_movement_is_unset`.
pub fn units_in_issue(units: &[i64]) -> Option<i64> {
    if units.is_empty() {
        return None;
    }
    let mut n: i64 = 0;
    for &u in units {
        n = match checked::add(n, u, "units in issue") {
            Ok(v) => v,
            Err(_) => return None,
        };
    }
    Some(n)
}

/// Period units issued. Empty, or a window with only redemptions, is
/// unset — not a silent zero issue. `Ratio.Partners.no_issue_is_unset`.
pub fn period_issued(units: &[i64]) -> Option<i64> {
    if !units.iter().any(|&u| u > 0) {
        return None;
    }
    let mut n: i64 = 0;
    for &u in units.iter().filter(|&&u| u > 0) {
        n = match checked::add(n, u, "units issued") {
            Ok(v) => v,
            Err(_) => return None,
        };
    }
    Some(n)
}

/// Period units redeemed (absolute). Empty, or a window with only
/// subscriptions, is unset. `Ratio.Partners.no_redeem_is_unset`.
pub fn period_redeemed(units: &[i64]) -> Option<i64> {
    if !units.iter().any(|&u| u < 0) {
        return None;
    }
    let mut n: i64 = 0;
    for &u in units.iter().filter(|&&u| u < 0) {
        n = match checked::add(n, -u, "units redeemed") {
            Ok(v) => v,
            Err(_) => return None,
        };
    }
    Some(n)
}

/// Redeem `units` from an outstanding figure.
///
/// Unset outstanding cannot redeem. Over-redemption refuses. Zero
/// units is not a redemption. `Ratio.Partners.cannot_redeem_when_unset`.
pub fn redeem(outstanding: Option<i64>, units: i64) -> Result<Option<i64>> {
    let Some(n) = outstanding else {
        bail!(
            "cannot redeem {units} units when units in issue are unset — \
             nobody issued them. A contribution is not a subscription. \
             Ratio.Partners.cannot_redeem_when_unset"
        );
    };
    if units <= 0 {
        bail!(
            "zero units is not a redemption — omit the field for a \
             distribution, or name the units. Ratio.Partners.a_zero_redeem_is_refused"
        );
    }
    if units > n {
        bail!(
            "cannot redeem {units} units when {n} are in issue. \
             Over-redemption refuses rather than going negative. \
             Ratio.Partners.cannot_redeem_more_than_issued"
        );
    }
    Ok(Some(checked::sub(n, units, "redeem")?))
}
