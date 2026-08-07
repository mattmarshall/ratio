//! ratio-rules — posting rules: parsed, checked, and compiled to postings.
//!
//! PLAN.md Stage 1. Three things, in order of how much they matter:
//!
//! **1. A rule is data, not a language.** Rules are TOML against the schema
//! below. The `rule … { }` syntax on the website is a *rendering*, produced by
//! [`render`] — nobody writes it, and there is no parser for it. Building one
//! would be weeks of work no customer can see, and it is the single most
//! likely way this stalls.
//!
//! **2. A rule carries weights, not amounts.** `Ratio.Chart.applyTemplate`
//! scales a template of weights by an amount, and
//! `Ratio.Chart.balanced_template_balances` proves that if the weights net to
//! zero then *every* instantiation nets to zero — at any amount, any number of
//! times. So [`check`] verifies the template **once**, at authoring time, and
//! [`compile`] can then emit postings that are balanced by theorem rather than
//! by inspection. That is what makes the control plane safe to be expressive.
//!
//! **3. The checks speak to a fund accountant.** A rejected rule says which
//! account, what the shortfall is, and what to do — not a stack trace.
//!
//! Amounts are exact integers throughout. Rates are basis points (`i64`), day
//! counts are an enum: a float cannot be *expressed*, so "no floating point"
//! is a property of the schema rather than a check that might be forgotten.

mod render;
pub use render::render;

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use ratio_kernel::{transaction_is_balanced, Posting, Transaction};
use ratio_store::{Account, PostingRecord};
use serde::{Deserialize, Serialize};

/// What a rule is triggered by, and therefore how its amount is derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// A trade: the amount is given by the event.
    Trade,
    /// A dividend: the amount is given by the event.
    Dividend,
    /// An accrual: the amount is *computed* from a basis, a rate and a period.
    Accrual,
}

/// The day-count convention an accrual is computed on.
///
/// An enum rather than a number so a rule cannot silently carry a made-up
/// convention, and so the denominator is never a float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DayCount {
    /// Actual days over 365.
    #[serde(rename = "act/365")]
    Act365,
    /// Actual days over 360.
    #[serde(rename = "act/360")]
    Act360,
    /// 30-day months over 360.
    #[serde(rename = "30/360")]
    Thirty360,
}

impl DayCount {
    /// The denominator, in days.
    pub fn denominator(self) -> i128 {
        match self {
            DayCount::Act365 => 365,
            DayCount::Act360 | DayCount::Thirty360 => 360,
        }
    }

    /// How it is written in a rule and on a report.
    pub fn as_str(self) -> &'static str {
        match self {
            DayCount::Act365 => "act/365",
            DayCount::Act360 => "act/360",
            DayCount::Thirty360 => "30/360",
        }
    }
}

/// One leg of a posting template: an account, and its share of the amount.
///
/// `weight` is a multiplier, not an amount. The weights of a rule must net to
/// zero; see [`check`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leg {
    /// The conserved dimension — the account this leg posts to.
    pub account: i64,
    /// This leg's multiple of the amount. Positive debits, negative credits.
    pub weight: i64,
}

/// A posting rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Stable identifier, used in postings and diffs.
    pub id: String,
    /// What triggers it.
    pub kind: RuleKind,
    /// What it is, in the words of whoever approved it.
    #[serde(default)]
    pub description: String,
    /// The posting template. Must net to zero.
    #[serde(rename = "posting", default)]
    pub legs: Vec<Leg>,
    /// Accrual rate in basis points per year. Required for `accrual`, and
    /// meaningless otherwise. Integer — a rate cannot be a float here.
    #[serde(default)]
    pub rate_bp: Option<i64>,
    /// Day-count convention. Required for `accrual`.
    #[serde(default)]
    pub day_count: Option<DayCount>,
}

/// A configuration: the whole rule set, as promoted to the control plane.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(rename = "rule", default)]
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Parse a configuration from TOML.
    ///
    /// A malformed rate — `rate_bp = 75.5` — fails here, with TOML's own error.
    /// That is deliberate: the schema makes a float inexpressible, so there is
    /// no separate "no floats" check to forget to run.
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).context("configuration is not valid TOML for a rule set")
    }

    /// Serialize back to TOML — the canonical form that gets content-addressed.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("serializing rule set")
    }

    /// Find a rule by id.
    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

/// The event a rule is applied to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Which rule to apply.
    pub rule: String,
    /// Event identifier, carried onto the journal entry.
    pub id: String,
    /// For `trade` / `dividend`: the amount, in minor units. For `accrual`:
    /// the basis the rate is applied to.
    pub amount: i64,
    /// Accrual only: how many days of the period this covers.
    #[serde(default)]
    pub days: Option<i64>,
    #[serde(default)]
    pub memo: String,
}

/// Why a rule was rejected, in words a fund accountant would use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The rule this concerns.
    pub rule: String,
    /// What is wrong, and what to do about it.
    pub message: String,
    /// A question for the author rather than an outright error — the rule may
    /// be right, but somebody has to say so.
    pub is_question: bool,
}

impl Finding {
    fn error(rule: &str, message: impl Into<String>) -> Self {
        Finding {
            rule: rule.to_string(),
            message: message.into(),
            is_question: false,
        }
    }
    fn question(rule: &str, message: impl Into<String>) -> Self {
        Finding {
            rule: rule.to_string(),
            message: message.into(),
            is_question: true,
        }
    }
}

/// Check a rule set against the chart of accounts.
///
/// Returns every finding rather than stopping at the first, so an author fixes
/// a configuration in one pass instead of one round trip per mistake.
///
/// The obligations, and why each is here:
///
/// * **the template balances** — the load-bearing one. Verified once here;
///   `Ratio.Chart.balanced_template_balances` then guarantees every posting the
///   rule ever emits balances, at any amount.
/// * **every account exists** — a rule referencing an account nobody has
///   defined posts into a dimension with no name on a report.
/// * **an accrual has a rate and a convention** — without both, the amount is
///   not computable and the rule is not a rule.
/// * **a non-accrual has neither** — a rate on a trade rule means somebody
///   expected it to do something it will not do.
///
/// There is deliberately **no per-leg normal-side check**. Crediting cash is
/// what every purchase does; flagging it would fire on nearly every real rule.
/// Normal side is a property of an account's *balance*, not of a posting, so
/// the indicator lives on the trial balance where it means something. A check
/// that cries wolf trains people to ignore the checks — which is the same
/// failure PLAN.md names for false breaks in a shadow run.
pub fn check(set: &RuleSet, chart: &[Account]) -> Vec<Finding> {
    let by_dim: BTreeMap<i64, &Account> = chart.iter().map(|a| (a.dim, a)).collect();
    let mut out = Vec::new();

    for rule in &set.rules {
        if rule.legs.is_empty() {
            out.push(Finding::error(
                &rule.id,
                "has no postings, so it cannot record anything. Add at least a debit and a credit.",
            ));
            continue;
        }

        // ── the template balances ──────────────────────────────────────────
        // Reuse the kernel's own check, over the weights. This is the same
        // predicate `Ratio.Core.Balanced` the ledger enforces, applied one
        // level up — so a template that passes here cannot produce a posting
        // that fails there.
        let as_txn = Transaction {
            postings: rule
                .legs
                .iter()
                .map(|l| Posting {
                    dim: l.account,
                    amount: l.weight,
                })
                .collect(),
        };
        if !transaction_is_balanced(&as_txn) {
            let net: i64 = rule.legs.iter().map(|l| l.weight).sum();
            out.push(Finding::error(
                &rule.id,
                format!(
                    "does not balance: the posting weights net to {net}, not 0. \
                     Every debit needs a matching credit — check the signs."
                ),
            ));
        }

        // ── every account exists, and sits on a sensible side ──────────────
        for leg in &rule.legs {
            match by_dim.get(&leg.account) {
                None => out.push(Finding::error(
                    &rule.id,
                    format!(
                        "posts to account {} which is not in the chart of accounts. \
                         Add it, or point the rule at an existing account.",
                        leg.account
                    ),
                )),
                Some(account) => {
                    if leg.weight == 0 {
                        out.push(Finding::error(
                            &rule.id,
                            format!(
                                "gives {} a weight of 0, so the leg never posts anything. \
                                 Remove it, or give it a weight.",
                                account.display_name
                            ),
                        ));
                    }
                }
            }
        }

        // ── accruals need a rate and a convention; others need neither ─────
        match rule.kind {
            RuleKind::Accrual => {
                if rule.rate_bp.is_none() {
                    out.push(Finding::error(
                        &rule.id,
                        "is an accrual with no rate. Set `rate_bp` — 75 basis points a year is `rate_bp = 75`.",
                    ));
                }
                if rule.day_count.is_none() {
                    out.push(Finding::question(
                        &rule.id,
                        "is an accrual with no day-count convention. Which applies — act/365, act/360 or 30/360?",
                    ));
                }
                if let Some(rate) = rule.rate_bp {
                    if rate < 0 {
                        out.push(Finding::error(
                            &rule.id,
                            format!("has a negative rate ({rate} bp). A rebate is a separate rule, not a negative fee."),
                        ));
                    }
                }
            }
            _ => {
                if rule.rate_bp.is_some() || rule.day_count.is_some() {
                    out.push(Finding::question(
                        &rule.id,
                        format!(
                            "is a {} rule but carries a rate or day-count, which only \
                             accruals use. They will be ignored — did you mean `kind = \"accrual\"`?",
                            kind_word(rule.kind)
                        ),
                    ));
                }
            }
        }
    }

    out
}


fn kind_word(k: RuleKind) -> &'static str {
    match k {
        RuleKind::Trade => "trade",
        RuleKind::Dividend => "dividend",
        RuleKind::Accrual => "accrual",
    }
}

/// Apply a rule to an event, producing postings.
///
/// The postings are balanced **by theorem** provided the rule passed [`check`]:
/// `Ratio.Chart.balanced_template_balances` says a template whose weights net
/// to zero nets to zero at every amount. This function is therefore not
/// permitted to invent a leg or adjust a total — it scales, and nothing else.
pub fn compile(rule: &Rule, event: &Event) -> Result<Vec<PostingRecord>> {
    let amount = match rule.kind {
        RuleKind::Trade | RuleKind::Dividend => event.amount,
        RuleKind::Accrual => accrual_amount(rule, event)?,
    };
    Ok(rule
        .legs
        .iter()
        .map(|leg| PostingRecord {
            dim: leg.account,
            // `Ratio.Chart.applyTemplate`: weight × amount, per leg.
            amount: leg.weight * amount,
        })
        .collect())
}

/// The accrued amount: `basis × rate × days ÷ (10 000 × denominator)`.
///
/// Computed in `i128` because the numerator overflows `i64` on a large book:
/// a 20-billion-unit basis at 75bp over 365 days is ~5.5e17 before division,
/// which is within `i64` but leaves no headroom, and a bigger book or a longer
/// period would silently wrap. Rounded half-up rather than truncated — an
/// accrual that always rounds toward zero under-accrues every single day, and
/// the error compounds across a quarter.
fn accrual_amount(rule: &Rule, event: &Event) -> Result<i64> {
    let rate = rule
        .rate_bp
        .with_context(|| format!("rule {:?} is an accrual with no rate", rule.id))?;
    let day_count = rule
        .day_count
        .with_context(|| format!("rule {:?} is an accrual with no day-count", rule.id))?;
    let days = event
        .days
        .with_context(|| format!("event {:?} accrues but does not say over how many days", event.id))?;
    if days < 0 {
        anyhow::bail!("event {:?} accrues over {days} days", event.id);
    }

    let numerator = (event.amount as i128) * (rate as i128) * (days as i128);
    let denominator = 10_000i128 * day_count.denominator();
    let rounded = round_half_up(numerator, denominator);
    i64::try_from(rounded)
        .with_context(|| format!("accrual for {:?} does not fit in i64", event.id))
}

/// Integer division rounding half away from zero.
fn round_half_up(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::AccountTypeRecord as A;

    fn chart() -> Vec<Account> {
        vec![
            acct(1, "Investments at fair value", A::Asset),
            acct(2, "Cash and equivalents", A::Asset),
            acct(10, "Management fee expense", A::Expense),
            acct(30, "Dividend income", A::Income),
            acct(40, "Management fee payable", A::Liability),
        ]
    }
    fn acct(dim: i64, name: &str, t: A) -> Account {
        Account {
            dim,
            display_name: name.into(),
            account_type: t,
        }
    }

    const FEE_TOML: &str = r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
description = "Management fee, 75bp a year on prior-day net assets"
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
    fn a_rule_set_round_trips_through_toml() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        assert_eq!(set.rules.len(), 1);
        let r = set.rule("management_fee_accrual").unwrap();
        assert_eq!(r.kind, RuleKind::Accrual);
        assert_eq!(r.rate_bp, Some(75));
        assert_eq!(r.day_count, Some(DayCount::Act365));
        let back = RuleSet::from_toml(&set.to_toml().unwrap()).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn a_fractional_rate_cannot_be_expressed() {
        // No "reject floats" check exists, because the schema makes a float
        // unrepresentable. This is the enforcement.
        let bad = FEE_TOML.replace("rate_bp = 75", "rate_bp = 75.5");
        assert!(RuleSet::from_toml(&bad).is_err());
    }

    #[test]
    fn a_good_rule_has_nothing_to_say_about_it() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        assert_eq!(check(&set, &chart()), vec![]);
    }

    #[test]
    fn an_unbalanced_template_is_rejected_with_the_shortfall() {
        let bad = FEE_TOML.replace("weight = -1", "weight = -2");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("net to -1"), "{}", f[0].message);
        assert!(!f[0].is_question);
    }

    #[test]
    fn an_unknown_account_is_rejected_by_number() {
        let bad = FEE_TOML.replace("account = 40", "account = 999");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert!(f.iter().any(|x| x.message.contains("999")
            && x.message.contains("not in the chart")));
    }

    /// A purchase credits cash, and cash is an asset. If the checker flagged
    /// that, it would fire on almost every rule a fund has — the false-positive
    /// failure PLAN.md warns about, one level up from shadow-run breaks.
    #[test]
    fn crediting_an_asset_is_not_flagged() {
        let buy = r#"
[[rule]]
id = "buy_equity"
kind = "trade"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1
"#;
        assert_eq!(check(&RuleSet::from_toml(buy).unwrap(), &chart()), vec![]);
    }

    /// …and debiting income is not flagged either, for the same reason: a
    /// reversal is ordinary. Normal side is a property of a BALANCE, and the
    /// trial balance is where it is reported.
    #[test]
    fn debiting_income_is_not_flagged_either() {
        let rev = r#"
[[rule]]
id = "dividend_reversal"
kind = "dividend"
[[rule.posting]]
account = 30
weight = 1
[[rule.posting]]
account = 2
weight = -1
"#;
        assert_eq!(check(&RuleSet::from_toml(rev).unwrap(), &chart()), vec![]);
    }

    #[test]
    fn an_accrual_without_a_rate_is_rejected_with_an_example() {
        let bad = FEE_TOML.replace("rate_bp = 75\n", "");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        let e = f.iter().find(|x| !x.is_question).unwrap();
        assert!(e.message.contains("rate_bp = 75"), "{}", e.message);
    }

    #[test]
    fn an_accrual_without_a_day_count_asks_which_one() {
        let bad = FEE_TOML.replace("day_count = \"act/365\"\n", "");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        let q = f.iter().find(|x| x.is_question).unwrap();
        assert!(q.message.contains("act/365"), "{}", q.message);
    }

    #[test]
    fn a_zero_weight_leg_is_rejected() {
        let bad = FEE_TOML.replace("weight = 1\n", "weight = 0\n");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert!(f.iter().any(|x| x.message.contains("weight of 0")));
    }

    #[test]
    fn a_rule_with_no_postings_is_rejected() {
        let set = RuleSet::from_toml(
            "[[rule]]\nid = \"empty\"\nkind = \"trade\"\n",
        )
        .unwrap();
        let f = check(&set, &chart());
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("no postings"));
    }

    #[test]
    fn compiling_a_trade_scales_the_template() {
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "buy"
kind = "trade"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1
"#,
        )
        .unwrap();
        let out = compile(
            set.rule("buy").unwrap(),
            &Event {
                rule: "buy".into(),
                id: "t1".into(),
                amount: 1_690_421_107,
                days: None,
                memo: String::new(),
            },
        )
        .unwrap();
        assert_eq!(out[0].amount, 1_690_421_107);
        assert_eq!(out[1].amount, -1_690_421_107);
    }

    /// The property `Ratio.Chart.balanced_template_balances` states, exercised
    /// over a range of amounts rather than argued about.
    #[test]
    fn a_checked_rule_balances_at_every_amount() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        assert!(check(&set, &chart()).is_empty());
        let rule = set.rule("management_fee_accrual").unwrap();
        for basis in [0i64, 1, 97, 100_000, 1_750_000_000, 20_000_000_000] {
            for days in [0i64, 1, 30, 91, 365] {
                let out = compile(
                    rule,
                    &Event {
                        rule: rule.id.clone(),
                        id: "a".into(),
                        amount: basis,
                        days: Some(days),
                        memo: String::new(),
                    },
                )
                .unwrap();
                let net: i64 = out.iter().map(|p| p.amount).sum();
                assert_eq!(net, 0, "basis={basis} days={days} did not balance");
            }
        }
    }

    #[test]
    fn an_accrual_is_computed_in_integers_and_rounds_half_up() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        let rule = set.rule("management_fee_accrual").unwrap();
        let at = |basis: i64, days: i64| {
            compile(
                rule,
                &Event {
                    rule: rule.id.clone(),
                    id: "a".into(),
                    amount: basis,
                    days: Some(days),
                    memo: String::new(),
                },
            )
            .unwrap()[0]
                .amount
        };
        // 1,750,000,000 minor units at 75bp for 1 day, act/365:
        //   1_750_000_000 * 75 * 1 / (10_000 * 365) = 35_958.9… -> 35_959
        assert_eq!(at(1_750_000_000, 1), 35_959);
        // A full year returns the whole 75bp.
        assert_eq!(at(1_750_000_000, 365), 13_125_000);
        // Zero days accrues nothing.
        assert_eq!(at(1_750_000_000, 0), 0);
    }

    #[test]
    fn a_large_book_does_not_overflow_the_accrual() {
        // The i128 intermediate is what makes this safe: the numerator here is
        // ~2.7e19, which does not fit in i64 at all.
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        let rule = set.rule("management_fee_accrual").unwrap();
        let out = compile(
            rule,
            &Event {
                rule: rule.id.clone(),
                id: "big".into(),
                amount: 1_000_000_000_000, // 10 billion units
                days: Some(365),
                memo: String::new(),
            },
        )
        .unwrap();
        assert_eq!(out[0].amount, 7_500_000_000);
        assert_eq!(out.iter().map(|p| p.amount).sum::<i64>(), 0);
    }

    #[test]
    fn rounding_is_symmetric_about_zero() {
        assert_eq!(round_half_up(5, 10), 1);
        assert_eq!(round_half_up(4, 10), 0);
        assert_eq!(round_half_up(-5, 10), -1);
        assert_eq!(round_half_up(-4, 10), 0);
    }

    #[test]
    fn every_finding_is_reported_not_just_the_first() {
        let bad = FEE_TOML
            .replace("weight = -1", "weight = -2")
            .replace("account = 10", "account = 999");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert!(f.len() >= 2, "{f:?}");
    }
}
