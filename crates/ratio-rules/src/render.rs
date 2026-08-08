//! Render a rule in the `rule … { }` form the website shows.
//!
//! This exists so that form can be shown to a human — in a review, a demo, or
//! an audit — **without there being a language**. Nobody writes this syntax and
//! nothing parses it: rules are TOML, and this is a one-way projection of them.
//!
//! That is the whole point. PLAN.md calls a parser the single most likely way
//! Stage 1 stalls; the readable form is worth having and the grammar behind it
//! is not.

use std::collections::BTreeMap;

use ratio_store::Account;

use crate::{DayCount, Leg, Rule, RuleKind};

/// Render a rule, naming accounts from the chart where it can.
///
/// Falls back to the bare dimension number for an account not in the chart —
/// a rule referencing one should have been rejected by `check`, and printing
/// `account 999` is more use to whoever is reading than a panic.
pub fn render(rule: &Rule, chart: &[Account]) -> String {
    let by_dim: BTreeMap<i64, &Account> = chart.iter().map(|a| (a.dim, a)).collect();
    let name = |dim: i64| match by_dim.get(&dim) {
        Some(a) => machine_name(&a.display_name),
        None => format!("account {dim}"),
    };

    let mut out = String::new();
    if !rule.description.is_empty() {
        out.push_str(&format!("-- {}\n", rule.description));
    }
    out.push_str(&format!("rule {} {{\n", rule.id));
    out.push_str(&format!("  when       {}\n", trigger(rule.kind)));

    if let Some(rate) = rule.rate_bp {
        out.push_str(&format!("  rate       {rate}bp / year\n"));
    }
    if let Some(dc) = rule.day_count {
        out.push_str(&format!("  day_count  {}\n", day_count(dc)));
    }

    // Debits first, then credits — the order a posting is read in.
    let (debits, credits): (Vec<&Leg>, Vec<&Leg>) =
        rule.legs.iter().partition(|l| l.weight > 0);
    let mut first = true;
    for leg in debits.into_iter().chain(credits) {
        let label = if first { "  posts      " } else { "             " };
        first = false;
        let side = if leg.weight > 0 { "debit " } else { "credit" };
        let mult = match leg.weight.abs() {
            1 => String::new(),
            n => format!("  × {n}"),
        };
        out.push_str(&format!("{label}{side}  {}{mult}\n", name(leg.account)));
    }

    out.push_str("}\n");
    out
}

fn trigger(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Trade => "trade t",
        RuleKind::Dividend => "dividend d",
        RuleKind::Accrual => "business_day d",
        RuleKind::Mark => "valuation v",
    }
}

fn day_count(dc: DayCount) -> &'static str {
    match dc {
        DayCount::Act365 => "ACT/365",
        DayCount::Act360 => "ACT/360",
        DayCount::Thirty360 => "30/360",
    }
}

/// "Management fee expense" → `expense.management_fee`-ish: lowercase, spaces
/// to underscores. Cosmetic only; the identity of an account is its dimension.
fn machine_name(display: &str) -> String {
    display
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuleSet;
    use ratio_store::AccountTypeRecord as A;

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

    #[test]
    fn a_rule_renders_in_the_form_the_site_shows() {
        let set = RuleSet::from_toml(
            r#"
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
"#,
        )
        .unwrap();
        let s = render(set.rule("management_fee_accrual").unwrap(), &chart());
        assert!(s.contains("rule management_fee_accrual {"));
        assert!(s.contains("when       business_day d"));
        assert!(s.contains("rate       75bp / year"));
        assert!(s.contains("day_count  ACT/365"));
        assert!(s.contains("debit   management_fee_expense"));
        assert!(s.contains("credit  management_fee_payable"));
        assert!(s.trim_end().ends_with('}'));
    }

    #[test]
    fn debits_are_listed_before_credits_whatever_the_toml_order() {
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "r"
kind = "trade"
[[rule.posting]]
account = 40
weight = -1
[[rule.posting]]
account = 10
weight = 1
"#,
        )
        .unwrap();
        let s = render(set.rule("r").unwrap(), &chart());
        let d = s.find("debit ").unwrap();
        let c = s.find("credit").unwrap();
        assert!(d < c, "{s}");
    }

    #[test]
    fn a_multiple_is_shown_and_a_unit_weight_is_not() {
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "r"
kind = "trade"
[[rule.posting]]
account = 10
weight = 3
[[rule.posting]]
account = 40
weight = -3
"#,
        )
        .unwrap();
        let s = render(set.rule("r").unwrap(), &chart());
        assert!(s.contains("× 3"), "{s}");
    }

    #[test]
    fn an_account_outside_the_chart_still_renders() {
        let set = RuleSet::from_toml(
            "[[rule]]\nid = \"r\"\nkind = \"trade\"\n[[rule.posting]]\naccount = 999\nweight = 1\n",
        )
        .unwrap();
        let s = render(set.rule("r").unwrap(), &[]);
        assert!(s.contains("account 999"), "{s}");
    }

    #[test]
    fn machine_names_are_stable_and_boring() {
        assert_eq!(machine_name("Management fee expense"), "management_fee_expense");
        assert_eq!(machine_name("Cash & equivalents"), "cash_equivalents");
        assert_eq!(machine_name("  odd   spacing "), "odd_spacing");
    }
}
