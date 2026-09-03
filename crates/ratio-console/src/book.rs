//! Book identity beside the journal: kind, optional fund, optional org.
//!
//! ⭐ THE KERNEL ALREADY TREATS A DIRECTORY AS A BOOK. `FileBook` is a journal
//! plus content-addressed config. This sidecar is the control-plane fact the
//! directory never had to carry: whether anyone filed the book under a fund
//! or a WorkOS organization. Absence is independence, not an error.

use std::path::Path;

use anyhow::{bail, Context, Result};
use ratio_rules::{check, RuleSet};
use ratio_store::{Account, AccountTypeRecord, ConfigStore, FileBook};

/// What a book is used for. Same kernel; different chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookKind {
    Personal,
    Investment,
    Project,
}

impl BookKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BookKind::Personal => "personal",
            BookKind::Investment => "investment",
            BookKind::Project => "project",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "personal" | "PERSONAL" | "KIND_PERSONAL" => Ok(BookKind::Personal),
            "investment" | "INVESTMENT" | "KIND_INVESTMENT" => Ok(BookKind::Investment),
            "project" | "PROJECT" | "KIND_PROJECT" => Ok(BookKind::Project),
            other => bail!("{other:?} is not a book kind"),
        }
    }

    pub fn proto(self) -> i32 {
        match self {
            BookKind::Personal => 1,
            BookKind::Investment => 2,
            BookKind::Project => 3,
        }
    }

    pub fn from_proto(v: i32) -> Result<Self> {
        match v {
            1 => Ok(BookKind::Personal),
            2 => Ok(BookKind::Investment),
            3 => Ok(BookKind::Project),
            0 => bail!("a book kind is required"),
            other => bail!("{other} is not a book kind"),
        }
    }
}

/// What `book.toml` records, and what a missing file means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookMeta {
    pub kind: BookKind,
    pub display_name: String,
    /// Fund id this book is filed under. `None` = independent.
    pub fund: Option<String>,
    /// WorkOS organization id. `None` = no org layer.
    pub organization: Option<String>,
}

impl BookMeta {
    /// A directory with no sidecar is a legacy investment book filed as itself.
    ///
    /// ⛔ THAT IS THE COMPATIBILITY CONTRACT, NOT A DEFAULT FOR NEW BOOKS.
    /// `CreateBook` always writes a sidecar, so a personal book cannot be
    /// mistaken for a fund. Seeded demo funds have no sidecar and keep
    /// appearing in ListFunds.
    pub fn load(path: &Path, id: &str) -> Self {
        let text = std::fs::read_to_string(path.join("book.toml")).unwrap_or_default();
        if text.trim().is_empty() {
            return BookMeta {
                kind: BookKind::Investment,
                display_name: crate::display_name(id),
                fund: Some(id.to_string()),
                organization: None,
            };
        }
        let kind = kv(&text, "kind")
            .and_then(|s| BookKind::parse(&s).ok())
            .unwrap_or(BookKind::Investment);
        let display_name = kv(&text, "display_name").unwrap_or_else(|| crate::display_name(id));
        let fund = kv(&text, "fund").filter(|s| !s.is_empty());
        let organization = kv(&text, "organization").filter(|s| !s.is_empty());
        BookMeta { kind, display_name, fund, organization }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let mut body = format!(
            "kind = {:?}\ndisplay_name = {:?}\n",
            self.kind.as_str(),
            self.display_name
        );
        if let Some(f) = &self.fund {
            body.push_str(&format!("fund = {f:?}\n"));
        }
        if let Some(o) = &self.organization {
            body.push_str(&format!("organization = {o:?}\n"));
        }
        std::fs::write(path.join("book.toml"), body).context("writing book.toml")
    }
}

fn kv(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            continue;
        }
        let raw = v.trim().trim_matches('"').trim_matches('\'').to_string();
        if raw.is_empty() {
            return None;
        }
        return Some(raw);
    }
    None
}

/// The chart a new book starts with. Same accounts `ratio init` writes for
/// investment; personal and project are different partitions of the same
/// conserved quantities.
pub fn chart_for(kind: BookKind) -> Vec<Account> {
    let acct = |dim: i64, name: &str, t: AccountTypeRecord| Account {
        dim,
        display_name: name.to_string(),
        account_type: t,
    };
    match kind {
        BookKind::Investment => vec![
            acct(1, "Investments at fair value", AccountTypeRecord::Asset),
            acct(2, "Cash and equivalents", AccountTypeRecord::Asset),
            acct(3, "Dividends receivable", AccountTypeRecord::Asset),
            acct(10, "Management fee expense", AccountTypeRecord::Expense),
            acct(20, "Capital contributions", AccountTypeRecord::Equity),
            acct(21, "Unrealized gain", AccountTypeRecord::Equity),
            acct(30, "Dividend income", AccountTypeRecord::Income),
            acct(31, "Realized gain on investments", AccountTypeRecord::Income),
            acct(40, "Management fee payable", AccountTypeRecord::Liability),
        ],
        BookKind::Personal => vec![
            acct(1, "Cash and bank", AccountTypeRecord::Asset),
            acct(2, "Investments", AccountTypeRecord::Asset),
            acct(10, "Living expenses", AccountTypeRecord::Expense),
            acct(11, "Taxes", AccountTypeRecord::Expense),
            acct(20, "Opening equity", AccountTypeRecord::Equity),
            acct(30, "Income", AccountTypeRecord::Income),
            acct(40, "Credit cards and loans", AccountTypeRecord::Liability),
        ],
        BookKind::Project => vec![
            acct(1, "Cash", AccountTypeRecord::Asset),
            acct(2, "Work in progress", AccountTypeRecord::Asset),
            acct(10, "Project costs", AccountTypeRecord::Expense),
            acct(20, "Funding", AccountTypeRecord::Equity),
            acct(30, "Project revenue", AccountTypeRecord::Income),
            acct(40, "Payables", AccountTypeRecord::Liability),
        ],
    }
}

/// The configuration a new book starts with.
///
/// Investment and project still write an empty rule set — those charts wait
/// on rules an operator approves (project WIP is #66). Personal writes the
/// household spend and transfer rules so a book created from the template
/// can post living expenses without inventing a fund trade.
fn configuration_for(kind: BookKind) -> &'static str {
    match kind {
        BookKind::Personal => PERSONAL_CONFIG,
        BookKind::Investment | BookKind::Project => "# ratio configuration\nrules = []\n",
    }
}

/// Household rules. `kind = "trade"` because the amount is given; none of
/// the legs is `per_instrument`, so a posting opens no lot and relieves
/// none. Cash → investments is a personal transfer, not a lot-relieving
/// sale; living expenses are a conserved spend, not a NAV mark.
///
/// ⭐ THE ACCOUNT NUMBERS ARE `chart_for(Personal)`'S. `initialize` runs
/// `check` against that chart before the digest is activated, so a drift
/// between the two is a refused create rather than a book that cannot post.
///
/// Budget vs actual: set `[personal] budget = <minor units>` on this
/// configuration, and optionally `[personal.envelope]` keyed by chart
/// dimension (10 living expenses, 11 taxes). Actuals are the journal.
/// Omitting the table means no baseline has been set — not a budget of
/// zero.
const PERSONAL_CONFIG: &str = r#"# Household posting rules. Amount given; no instrument, so no lot.
# Budget vs actual: set `[personal] budget = <minor units>` here.
# Optional `[personal.envelope]` is keyed by chart dimension (10, 11).
# Actuals are living expenses and taxes on this chart — not a second ledger.

[[rule]]
id = "xfer_cash_investments"
kind = "trade"
description = "Move cash to investments"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "xfer_investments_cash"
kind = "trade"
description = "Move investments to cash"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "xfer_cash_cards"
kind = "trade"
description = "Pay a credit card from cash"
[[rule.posting]]
account = 40
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "xfer_cards_cash"
kind = "trade"
description = "Draw cash on a credit card"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "xfer_investments_cards"
kind = "trade"
description = "Pay a credit card from investments"
[[rule.posting]]
account = 40
weight = 1
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "xfer_cards_investments"
kind = "trade"
description = "Invest with a credit card"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "spend_cash"
kind = "trade"
description = "Living expenses paid from cash"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "spend_card"
kind = "trade"
description = "Living expenses put on a card"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "pay_tax"
kind = "trade"
description = "Taxes paid from cash"
[[rule.posting]]
account = 11
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "receive_income"
kind = "trade"
description = "Income received to cash"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 30
weight = -1
"#;

/// Create the directory, the chart, a kind-appropriate configuration, and the sidecar.
///
/// ⛔ NO FUND AND NO ORG ARE WRITTEN. A caller that wants either files the
/// book afterwards. Create is the independent book.
pub fn initialize(path: &Path, id: &str, display: &str, kind: BookKind) -> Result<()> {
    if path.join("accounts.json").is_file() || path.join("book.toml").is_file() {
        bail!("book {id:?} already exists");
    }
    let chart = chart_for(kind);
    let cfg = configuration_for(kind);
    let set = RuleSet::from_toml(cfg).context("the template configuration is not TOML")?;
    let errors: Vec<_> = check(&set, &chart)
        .into_iter()
        .filter(|f| !f.is_question)
        .collect();
    if !errors.is_empty() {
        bail!(
            "the {:?} template's rules do not check against its chart: {}",
            kind.as_str(),
            errors
                .iter()
                .map(|f| format!("{}: {}", f.rule, f.message))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    let mut b = FileBook::open(path)?;
    b.put_accounts(&chart)?;
    let digest = b.put(cfg.as_bytes())?;
    b.set_active(&digest)?;
    BookMeta {
        kind,
        display_name: if display.is_empty() {
            crate::display_name(id)
        } else {
            display.to_string()
        },
        fund: None,
        organization: None,
    }
    .write(path)?;
    Ok(())
}

/// Grant `who` this book in `MEMBERSHIP.tsv`, so the next request can open it.
///
/// ⛔ THE CREATOR'S `sub`, NOT AN ORG. A personal book must not become visible
/// to every member of a WorkOS organization the creator happens to sit in.
/// Org grants are a separate line, written by an operator. An empty `who`
/// (the local CLI) writes nothing — Local is unrestricted already.
pub fn grant(root: &Path, who: &str, book_id: &str) -> Result<()> {
    if who.is_empty() {
        return Ok(());
    }
    let actor = who;
    let path = root.join("MEMBERSHIP.tsv");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    let line = format!("{actor}\t{book_id}\n");
    if text.lines().any(|l| l == line.trim_end()) {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&line);
    std::fs::write(&path, text).context("granting membership")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_sidecar_is_a_legacy_fund_book() {
        let dir = std::env::temp_dir().join("ratio-book-meta-legacy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let m = BookMeta::load(&dir, "ashcombe");
        assert_eq!(m.kind, BookKind::Investment);
        assert_eq!(m.fund.as_deref(), Some("ashcombe"));
        assert!(m.organization.is_none());
    }

    #[test]
    fn a_written_sidecar_is_independent_when_it_names_no_fund() {
        let dir = std::env::temp_dir().join("ratio-book-meta-personal");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        BookMeta {
            kind: BookKind::Personal,
            display_name: "Household".into(),
            fund: None,
            organization: None,
        }
        .write(&dir)
        .unwrap();
        let m = BookMeta::load(&dir, "household");
        assert_eq!(m.kind, BookKind::Personal);
        assert_eq!(m.display_name, "Household");
        assert!(m.fund.is_none());
        assert!(m.organization.is_none());
    }

    #[test]
    fn each_kind_gets_a_different_chart() {
        let personal = chart_for(BookKind::Personal);
        let investment = chart_for(BookKind::Investment);
        let project = chart_for(BookKind::Project);
        assert_ne!(personal, investment);
        assert_ne!(personal, project);
        assert_ne!(investment, project);
        assert!(personal.iter().any(|a| a.display_name == "Cash and bank"));
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Investments at fair value"));
        assert!(project.iter().any(|a| a.display_name == "Work in progress"));
    }

    #[test]
    fn initialize_writes_the_kind_chart_and_files_no_fund() {
        let dir = std::env::temp_dir().join("ratio-book-init-project");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "bridge", "Bridge", BookKind::Project).unwrap();
        let m = BookMeta::load(&dir, "bridge");
        assert_eq!(m.kind, BookKind::Project);
        assert!(m.fund.is_none());
        assert!(m.organization.is_none());
        let chart = FileBook::open(&dir).unwrap().accounts().unwrap();
        assert_eq!(chart, chart_for(BookKind::Project));
    }

    #[test]
    fn personal_template_rules_check_against_its_chart_and_seed_no_budget() {
        // ⭐ A RULE THAT NAMES AN ACCOUNT THE CHART DOES NOT HAVE WOULD CREATE
        // a book that cannot post. initialize refuses that; this test is what
        // notices the two drifting apart before a create is attempted.
        let set = RuleSet::from_toml(PERSONAL_CONFIG).unwrap();
        let findings = check(&set, &chart_for(BookKind::Personal));
        assert!(
            findings.iter().all(|f| f.is_question),
            "personal rules must check against chart_for(Personal): {findings:?}"
        );
        assert!(set.rule("spend_cash").is_some());
        assert!(set.rule("pay_tax").is_some());
        assert!(
            set.personal.is_none(),
            "a new household has no baseline until someone sets [personal] budget"
        );
        let against_empty = check(&set, &[]);
        assert!(
            against_empty.iter().any(|f| !f.is_question),
            "personal rules must not check against an empty chart: {against_empty:?}"
        );
    }

    #[test]
    fn initialize_seeds_the_household_rules_and_no_personal_budget() {
        let dir = std::env::temp_dir().join("ratio-book-init-personal-rules");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "household", "Household", BookKind::Personal).unwrap();
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().unwrap();
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = RuleSet::from_toml(&text).unwrap();
        assert!(set.rule("spend_cash").is_some(), "{text}");
        assert!(set.personal.is_none());
    }
}
