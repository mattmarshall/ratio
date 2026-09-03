//! Book identity beside the journal: kind, optional fund, optional org.
//!
//! ⭐ THE KERNEL ALREADY TREATS A DIRECTORY AS A BOOK. `FileBook` is a journal
//! plus content-addressed config. This sidecar is the control-plane fact the
//! directory never had to carry: whether anyone filed the book under a fund
//! or a WorkOS organization. Absence is independence, not an error.

use std::path::Path;

use anyhow::{bail, Context, Result};
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

/// The chart a new book starts with. Personal and project are different
/// partitions of the same conserved quantities. Investment used to be the
/// nine accounts `ratio init` writes; CreateBook adds distribution and
/// partner-capital equity so a fund book is not only a trading blotter.
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
            // ⭐ THE COUNTERPART THAT WAS MISSING. Contributions without
            // distributions made every return of capital an invented account.
            acct(22, "Distributions", AccountTypeRecord::Equity),
            acct(23, "Allocations", AccountTypeRecord::Equity),
            acct(24, "Capital transfers", AccountTypeRecord::Equity),
            acct(30, "Dividend income", AccountTypeRecord::Income),
            acct(31, "Realized gain on investments", AccountTypeRecord::Income),
            acct(40, "Management fee payable", AccountTypeRecord::Liability),
            // ⭐ PARTNER-SCOPED CAPITAL IS MORE EQUITY DIMS, NOT A SECOND
            // LEDGER. LP and GP partition where capital sits; they do not
            // net to zero, and they roll up to book capital. Conservation
            // is untouched — `Ratio.Ingest.partition_preserves_conservation`.
            acct(50, "Partner capital — LP", AccountTypeRecord::Equity),
            acct(51, "Partner capital — GP", AccountTypeRecord::Equity),
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

/// Equity that is capital activity — not unrealized gain.
///
/// ⛔ UNREALIZED GAIN IS VALUATION, NOT WHO PUT MONEY IN. Folding it into
/// partner capital would make a mark-to-market look like a contribution.
/// Matching is by display name so a book that kept the old nine-account
/// chart still cites "Capital contributions", and a book that added
/// partner dims cites those too.
pub fn is_capital_account(display_name: &str) -> bool {
    matches!(
        display_name,
        "Capital contributions" | "Distributions" | "Allocations" | "Capital transfers"
    ) || display_name.starts_with("Partner capital")
}

/// The configuration CreateBook activates. Kind selects the rules the
/// operator can apply; an empty file is still a configuration.
///
/// ⚠ `kind = "trade"` HERE IS THE RULE SCHEMA, NOT A SALE. RuleKind has
/// no "transfer" / "contribute" variant. These templates carry no
/// `per_instrument` leg, so the lot walk skips them — same door Personal
/// transfers use.
pub fn config_for(kind: BookKind) -> &'static str {
    match kind {
        BookKind::Investment => INVESTMENT_CONFIG,
        BookKind::Personal | BookKind::Project => "# ratio configuration\nrules = []\n",
    }
}

/// Partner-capital and book-level contribute / distribute / allocate /
/// transfer templates against `chart_for(Investment)`.
///
/// Amounts are exact minor units. An allocation across partners is two
/// (or more) events with integer shares — a percentage that will not
/// divide is a misstatement, not a rounding error.
const INVESTMENT_CONFIG: &str = r#"# ratio configuration
# Capital activity. Not a return, not IRR, not attribution.

[[rule]]
id = "contribute"
kind = "trade"
description = "Book-level contribution: cash in, capital contributions up"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 20
weight = -1

[[rule]]
id = "distribute"
kind = "trade"
description = "Book-level distribution: distributions up, cash out"

[[rule.posting]]
account = 22
weight = 1

[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "allocate_gain"
kind = "trade"
description = "Close realized gain into allocations (book-level)"

[[rule.posting]]
account = 31
weight = 1

[[rule.posting]]
account = 23
weight = -1

[[rule]]
id = "allocate_fee"
kind = "trade"
description = "Close management fee expense into allocations (book-level)"

[[rule.posting]]
account = 23
weight = 1

[[rule.posting]]
account = 10
weight = -1

[[rule]]
id = "contribute_lp"
kind = "trade"
description = "LP contribution: cash in, partner capital up"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 50
weight = -1

[[rule]]
id = "contribute_gp"
kind = "trade"
description = "GP contribution: cash in, partner capital up"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 51
weight = -1

[[rule]]
id = "distribute_lp"
kind = "trade"
description = "LP distribution: partner capital down, cash out"

[[rule.posting]]
account = 50
weight = 1

[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "distribute_gp"
kind = "trade"
description = "GP distribution: partner capital down, cash out"

[[rule.posting]]
account = 51
weight = 1

[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "transfer_lp_gp"
kind = "trade"
description = "Transfer capital LP → GP"

[[rule.posting]]
account = 50
weight = 1

[[rule.posting]]
account = 51
weight = -1

[[rule]]
id = "transfer_gp_lp"
kind = "trade"
description = "Transfer capital GP → LP"

[[rule.posting]]
account = 51
weight = 1

[[rule.posting]]
account = 50
weight = -1

[[rule]]
id = "allocate_gain_lp"
kind = "trade"
description = "Allocate realized gain to LP (exact amount, not a percentage)"

[[rule.posting]]
account = 31
weight = 1

[[rule.posting]]
account = 50
weight = -1

[[rule]]
id = "allocate_gain_gp"
kind = "trade"
description = "Allocate realized gain to GP (exact amount, not a percentage)"

[[rule.posting]]
account = 31
weight = 1

[[rule.posting]]
account = 51
weight = -1

[[rule]]
id = "allocate_fee_lp"
kind = "trade"
description = "Allocate management fee to LP by closing the expense into capital"

[[rule.posting]]
account = 50
weight = 1

[[rule.posting]]
account = 10
weight = -1

[[rule]]
id = "allocate_fee_gp"
kind = "trade"
description = "Allocate management fee to GP by closing the expense into capital"

[[rule.posting]]
account = 51
weight = 1

[[rule.posting]]
account = 10
weight = -1
"#;

/// Create the directory, the chart, the kind's configuration, and the sidecar.
///
/// ⛔ NO FUND AND NO ORG ARE WRITTEN. A caller that wants either files the
/// book afterwards. Create is the independent book.
pub fn initialize(path: &Path, id: &str, display: &str, kind: BookKind) -> Result<()> {
    if path.join("accounts.json").is_file() || path.join("book.toml").is_file() {
        bail!("book {id:?} already exists");
    }
    let mut b = FileBook::open(path)?;
    b.put_accounts(&chart_for(kind))?;
    let digest = b.put(config_for(kind).as_bytes())?;
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
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Distributions"));
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Partner capital — LP"));
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Partner capital — GP"));
        assert!(
            !investment
                .iter()
                .any(|a| a.display_name == "Unrealized gain" && is_capital_account(&a.display_name)),
            "unrealized gain is valuation, not capital activity"
        );
        assert!(is_capital_account("Capital contributions"));
        assert!(is_capital_account("Distributions"));
        assert!(is_capital_account("Partner capital — LP"));
        assert!(!is_capital_account("Unrealized gain"));
        assert!(!is_capital_account("Investments at fair value"));
        assert!(project.iter().any(|a| a.display_name == "Work in progress"));
    }

    #[test]
    fn initialize_investment_seeds_contribute_and_distribute_and_partners() {
        let dir = std::env::temp_dir().join("ratio-book-init-investment-capital");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "partners", "Partners", BookKind::Investment).unwrap();
        let chart = FileBook::open(&dir).unwrap().accounts().unwrap();
        assert_eq!(chart, chart_for(BookKind::Investment));
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().expect("CreateBook activates a config");
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = ratio_rules::RuleSet::from_toml(&text).unwrap();
        for id in [
            "contribute",
            "distribute",
            "contribute_lp",
            "contribute_gp",
            "distribute_lp",
            "transfer_lp_gp",
            "allocate_gain_lp",
            "allocate_fee_lp",
        ] {
            assert!(set.rule(id).is_some(), "missing {id} in {text}");
        }
        assert!(
            set.rules.iter().all(|r| r.legs.iter().all(|l| !l.per_instrument)),
            "capital rules must not open lots"
        );
        let findings = ratio_rules::check(&set, &chart);
        assert!(
            findings.iter().all(|f| f.is_question),
            "seeded capital rules must balance against the chart: {findings:?}"
        );
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
}
