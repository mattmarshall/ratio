//! Book identity beside the journal: kind, optional fund, optional org.
//!
//! ⭐ THE KERNEL ALREADY TREATS A DIRECTORY AS A BOOK. `FileBook` is a journal
//! plus content-addressed config. This sidecar is the control-plane fact the
//! directory never had to carry: whether anyone filed the book under a fund
//! or a WorkOS organization. Absence is independence, not an error.

use std::path::Path;

use anyhow::{bail, Context, Result};
use ratio_store::{Account, AccountTypeRecord, ConfigStore, FileBook};

/// The ingest template CreateBook writes for this kind.
///
/// ⭐ KIND-AWARE, NOT A SHARED MENU. A Personal book that offered
/// `custodian-positions` would be asking a household to pick a fund feed.
/// The live list is the book's own configuration; this id is what
/// [`config_for`] puts there, and what the console catalog filters on.
pub fn ingest_template_id(kind: BookKind) -> &'static str {
    match kind {
        BookKind::Personal => "bank-statement",
        BookKind::Investment => "custodian-positions",
        BookKind::Project => "project-invoices",
    }
}

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

/// The opening configuration CreateBook writes: posting rules that hit
/// [`chart_for`] and one ingest template for this kind.
///
/// ⛔ NOT THE DEMO FUND'S TRADE FILE. `deploy/seed-demo-book.sh` still owns
/// `prime_equity_trades` — delivery → resolve → admit, with VWRL left pending
/// so an operator has a fact they can open. CreateBook seeds the *kind's*
/// column contract so a blank book can read a file; it does not invent a
/// journal to reconcile against.
pub fn config_for(kind: BookKind) -> &'static str {
    match kind {
        BookKind::Personal => PERSONAL_CONFIG,
        BookKind::Investment => INVESTMENT_CONFIG,
        BookKind::Project => PROJECT_CONFIG,
    }
}

/// Bank / card CSV → cash and expense claims. Amounts are a money column
/// (never a float); `Kind` picks the rule so a signed-amount inference
/// cannot silently flip income and a card charge.
const PERSONAL_CONFIG: &str = r#"
[[rule]]
id = "living_expense"
kind = "trade"
description = "Living expenses up, cash and bank down"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "household_income"
kind = "trade"
description = "Cash and bank up, income down"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 30
weight = -1

[[rule]]
id = "card_charge"
kind = "trade"
description = "Living expenses up, credit cards and loans up"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1

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

[[template]]
id = "bank-statement"
reads = "csv"

  [[template.entity]]
  name = "payee"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "name", column = "Memo" }]

  [template.fact]
  kind = "statement"
  reference = "Ref"
  entities = { payee = "payee" }

  [[template.fact.value]]
  field = "dated"
  as = "date"
  column = "Date"
  format = "MM/DD/YYYY"

  [[template.fact.value]]
  field = "amount"
  as = "money"
  column = "Amount"
  currency = "Ccy"

  [[template.fact.value]]
  field = "memo"
  as = "text"
  column = "Memo"

  [[template.fact.value]]
  field = "account"
  as = "text"
  column = "Account"

  [[template.fact.value]]
  field = "kind"
  as = "enum"
  column = "Kind"
  map = { expense = "expense", income = "income", card = "card" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { expense = "living_expense", income = "household_income", card = "card_charge" }
  dated = "dated"
"#;

/// Custodian positions snapshot. REFERENCE DATA: no `posts` block, so a row
/// is recorded and citable and never touches the journal. The column
/// contract is one real file: LineRef, AsOf, ISIN, Ticker, Exch, Quantity,
/// MarketValue, Ccy.
///
/// ⚠ THE CLOSED LOOP IS `prime_equity_trades` IN `deploy/seed-demo-book.sh`.
/// That path delivers, resolves, admits, and leaves VWRL pending — a break
/// an operator can open. A CreateBook investment book has no journal yet,
/// so seeding the trade file here would be a mapping with nothing to post.
const INVESTMENT_CONFIG: &str = r#"
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
[[template]]
id = "custodian-positions"
reads = "csv"

  [[template.entity]]
  name = "holding"
  kind = "instrument"
  absent = "pend"
  by = [
    { attribute = "isin", column = "ISIN" },
    { attribute = "ticker", column = "Ticker", within = { attribute = "exchange", column = "Exch" } },
  ]

  [template.fact]
  kind = "position"
  reference = "LineRef"
  entities = { holding = "holding" }

  [[template.fact.value]]
  field = "asOf"
  as = "date"
  column = "AsOf"
  format = "YYYY-MM-DD"

  [[template.fact.value]]
  field = "quantity"
  as = "decimal"
  column = "Quantity"

  [[template.fact.value]]
  field = "marketValue"
  as = "money"
  column = "MarketValue"
  currency = "Ccy"
"#;

/// Vendor invoice / cost CSV → project costs and payables claims.
const PROJECT_CONFIG: &str = r#"
[[rule]]
id = "project_cost"
kind = "trade"
description = "Project costs up, cash down"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "vendor_invoice"
kind = "trade"
description = "Project costs up, payables up"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "pay_vendor"
kind = "trade"
description = "Pay a vendor from cash"
[[rule.posting]]
account = 40
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "capitalize_wip"
kind = "trade"
description = "Move project costs into work in progress"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 10
weight = -1

[[rule]]
id = "recognize_wip"
kind = "trade"
description = "Recognize capitalized WIP as project cost"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "receive_funding"
kind = "trade"
description = "Funding received to cash"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 20
weight = -1

[[rule]]
id = "recognize_revenue"
kind = "trade"
description = "Project revenue received to cash"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 30
weight = -1

[[template]]
id = "project-invoices"
reads = "csv"

  [[template.entity]]
  name = "vendor"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "name", column = "Vendor" }]

  [template.fact]
  kind = "invoice"
  reference = "InvoiceRef"
  entities = { vendor = "vendor" }

  [[template.fact.value]]
  field = "dated"
  as = "date"
  column = "Date"
  format = "YYYY-MM-DD"

  [[template.fact.value]]
  field = "amount"
  as = "money"
  column = "Amount"
  currency = "Ccy"

  [[template.fact.value]]
  field = "memo"
  as = "text"
  column = "Memo"
  optional = true

  [[template.fact.value]]
  field = "kind"
  as = "enum"
  column = "Kind"
  map = { cost = "cost", invoice = "invoice" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { cost = "project_cost", invoice = "vendor_invoice" }
  dated = "dated"
"#;

/// Create the directory, the chart, the kind's opening ingest configuration,
/// and the sidecar.
///
/// ⛔ NO FUND AND NO ORG ARE WRITTEN. A caller that wants either files the
/// book afterwards. Create is the independent book.
pub fn initialize(path: &Path, id: &str, display: &str, kind: BookKind) -> Result<()> {
    if path.join("accounts.json").is_file() || path.join("book.toml").is_file() {
        bail!("book {id:?} already exists");
    }
    let chart = chart_for(kind);
    let cfg = config_for(kind);
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
    fn each_kind_seeds_its_own_ingest_template_and_not_the_others() {
        // ⭐ THE CLAIM #72 IS ABOUT. A Personal book that listed
        // `custodian-positions` would force a household to pick a fund feed.
        // CreateBook writes the kind's mapping into the opening configuration,
        // so ListTemplates is kind-aware because the book only has its own.
        for kind in [BookKind::Personal, BookKind::Investment, BookKind::Project] {
            let set = ratio_ingest::TemplateSet::from_toml(config_for(kind)).unwrap();
            let ids: Vec<&str> = set.templates.iter().map(|t| t.id.as_str()).collect();
            assert_eq!(ids, vec![ingest_template_id(kind)], "{kind:?}: {ids:?}");
            let t = set.template(ingest_template_id(kind)).unwrap();
            assert!(t.check().is_empty(), "{kind:?}: {:?}", t.check());
        }
        let personal = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Personal)).unwrap();
        assert!(personal.template("custodian-positions").is_none());
        assert!(personal.template("project-invoices").is_none());
        let project = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        assert!(project.template("custodian-positions").is_none());
        assert!(project.template("bank-statement").is_none());
    }

    #[test]
    fn seeded_rules_balance_against_the_kind_chart() {
        // ⛔ A TEMPLATE THAT POSTS AT A RULE THE CHART CANNOT EXPRESS would
        // admit a fact and then refuse the entry. The opening configuration
        // is checked the same way an approval is.
        for kind in [BookKind::Personal, BookKind::Investment, BookKind::Project] {
            let set = ratio_rules::RuleSet::from_toml(config_for(kind)).unwrap();
            let findings = ratio_rules::check(&set, &chart_for(kind));
            assert!(
                findings.is_empty(),
                "{kind:?} opening rules do not check: {findings:?}"
            );
        }
    }

    #[test]
    fn a_bank_statement_row_becomes_a_money_claim_not_a_float() {
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Personal)).unwrap();
        let t = set.template("bank-statement").unwrap();
        let csv = "\
Ref,Date,Amount,Ccy,Memo,Account,Kind
T-1,03/01/2026,45.20,USD,GROCERY STORE,checking,expense
T-2,03/02/2026,3200.00,USD,ACME PAYROLL,checking,income
T-3,03/03/2026,89.00,USD,ELECTRIC CO,card,card
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        assert_eq!(
            p.facts[0].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 4520,
                currency: "USD".into()
            }),
        );
        assert_eq!(
            p.facts[1].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 320_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "living_expense");
        assert_eq!(minor, 4520);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "household_income");
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[2]).unwrap();
        assert_eq!(rule, "card_charge");
        // A payee nobody has added pends — the same shape as a missing
        // instrument on the fund path. Adding it later clears without
        // re-ingesting.
        let resolved = ratio_ingest::resolve_all(&p.facts, &[]);
        assert!(resolved.iter().all(|r| !r.is_admissible()));
    }

    #[test]
    fn a_project_invoice_row_claims_the_vendor_and_picks_cost_or_payable() {
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        let t = set.template("project-invoices").unwrap();
        let csv = "\
InvoiceRef,Date,Amount,Ccy,Vendor,Memo,Kind
INV-1,2026-03-01,1200.00,USD,ACME STEEL,steel delivery,invoice
INV-2,2026-03-02,450.00,USD,CITY POWER,,cost
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 2);
        assert_eq!(
            p.facts[0].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 120_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "vendor_invoice");
        assert_eq!(minor, 120_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "project_cost");
        // Optional memo: the cost row left it blank and still mapped.
        assert!(p.facts[1].values.get("memo").is_none());
    }

    #[test]
    fn custodian_positions_record_and_never_post() {
        // ⭐ A MODE, NOT A GAP. The snapshot is what a recon reads against.
        // Seeding a `posts` block would invent journal entries a blank
        // investment book does not have counterparties for.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Investment)).unwrap();
        let t = set.template("custodian-positions").unwrap();
        assert!(t.fact.posts.is_none());
        let csv = "\
LineRef,AsOf,ISIN,Ticker,Exch,Quantity,MarketValue,Ccy
P-1,2026-02-26,US9229087690,VTI,ARCX,1000,262500.00,USD
P-2,2026-02-26,,VOO,ARCX,400,176700.00,USD
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 2);
        assert_eq!(
            p.facts[0].values.get("marketValue"),
            Some(&ratio_ingest::Value::Money {
                minor: 26_250_000,
                currency: "USD".into()
            }),
        );
        assert!(ratio_ingest::posting_for(t, &p.facts[0]).is_err());
        // Blank ISIN drops that rung; ticker within exchange remains.
        let holding = p.facts[1].entities.get("holding").unwrap();
        assert_eq!(holding.rungs.len(), 1);
        assert_eq!(holding.rungs[0][0].attr, "ticker");
    }

    #[test]
    fn initialize_writes_the_kind_template_into_the_opening_config() {
        let dir = std::env::temp_dir().join("ratio-book-init-personal-ingest");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "household", "Household", BookKind::Personal).unwrap();
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().unwrap();
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = ratio_ingest::TemplateSet::from_toml(&text).unwrap();
        assert_eq!(set.templates.len(), 1);
        assert_eq!(set.templates[0].id, "bank-statement");
        assert!(set.template("custodian-positions").is_none());
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
    fn personal_rules_check_against_the_personal_chart() {
        // ⭐ THE TEMPLATE IS NOT A LABEL. Create writes these rules, and a
        // rule that named a dimension the chart does not have would be a
        // book that cannot post — the stored-but-unread defect wearing a
        // configuration.
        let set = RuleSet::from_toml(PERSONAL_CONFIG).unwrap();
        let findings = check(&set, &chart_for(BookKind::Personal));
        assert!(
            findings.iter().all(|f| f.is_question),
            "personal rules must check: {findings:?}"
        );
        assert!(set.rule("xfer_cash_investments").is_some());
        assert!(set.rule("xfer_cash_cards").is_some());
        assert!(
            set.rules.iter().any(|r| r.id.starts_with("xfer_")),
            "a personal book must be able to transfer without a trade"
        );
        assert!(
            set.rules.iter().all(|r| r.legs.iter().all(|l| !l.per_instrument)),
            "a household transfer that is per-instrument would open a lot"
        );
        assert!(
            set.personal.is_none(),
            "a new household has no baseline until someone sets [personal] budget"
        );
    }

    #[test]
    fn initialize_personal_activates_the_household_rules() {
        let dir = std::env::temp_dir().join("ratio-book-init-personal");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "household", "Household", BookKind::Personal).unwrap();
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().expect("a new book has a configuration");
        let set = RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest).unwrap())).unwrap();
        assert!(set.rule("xfer_cash_investments").is_some());
        assert_eq!(b.accounts().unwrap(), chart_for(BookKind::Personal));
    }

    #[test]
    fn project_template_rules_check_against_its_chart() {
        // ⭐ A RULE THAT NAMES AN ACCOUNT THE CHART DOES NOT HAVE WOULD CREATE
        // a book that cannot post. initialize refuses that; this test is what
        // notices the two drifting apart before a create is attempted.
        let set = RuleSet::from_toml(PROJECT_CONFIG).unwrap();
        let findings = check(&set, &chart_for(BookKind::Project));
        assert!(
            findings.iter().all(|f| f.is_question),
            "project rules must check against chart_for(Project): {findings:?}"
        );
        assert!(set.rule("capitalize_wip").is_some());
        assert!(set.rule("recognize_wip").is_some());
        assert!(set.rule("project_cost").is_some());
        assert!(set.rule("vendor_invoice").is_some());
        assert!(
            set.project.is_none(),
            "a new project has no baseline until someone sets [project] budget"
        );
        let against_empty = check(&set, &[]);
        assert!(
            against_empty.iter().any(|f| !f.is_question),
            "project rules must not check against an empty chart: {against_empty:?}"
        );
    }

    #[test]
    fn initialize_seeds_the_wip_rules_and_no_project_budget() {
        let dir = std::env::temp_dir().join("ratio-book-init-project-rules");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "bridge", "Bridge", BookKind::Project).unwrap();
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().unwrap();
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = RuleSet::from_toml(&text).unwrap();
        assert!(set.rule("capitalize_wip").is_some(), "{text}");
        assert!(set.project.is_none());
    }

    fn sample_delivery() -> ratio_ingest::Delivery {
        ratio_ingest::Delivery {
            digest: "a".repeat(64),
            origin: "fixture.csv".into(),
            received: 1,
            bytes: 1,
        }
    }
}
