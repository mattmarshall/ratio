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

/// The ingest templates CreateBook writes for this kind, in the order
/// [`config_for`] lists them.
///
/// ⭐ KIND-AWARE, NOT A SHARED MENU. A Personal book that offered
/// `custodian-positions` would be asking a household to pick a fund feed.
/// The live list is the book's own configuration; these ids are what
/// [`config_for`] puts there, and what the console catalog filters on.
///
/// Investment is two mappings on purpose: the holdings snapshot (recorded,
/// never booked) and the trade column contract that posts. One without the
/// other is a file you can read and a loop you cannot run.
pub fn ingest_template_ids(kind: BookKind) -> &'static [&'static str] {
    match kind {
        BookKind::Personal => &["bank-statement"],
        BookKind::Investment => &["custodian-positions", "prime_equity_trades"],
        BookKind::Project => &["project-invoices"],
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
            acct(3, "Accounts receivable", AccountTypeRecord::Asset),
            acct(4, "Retainage receivable", AccountTypeRecord::Asset),
            acct(5, "Unbilled receivables", AccountTypeRecord::Asset),
            acct(10, "Project costs", AccountTypeRecord::Expense),
            acct(11, "Site and mobilization", AccountTypeRecord::Expense),
            acct(12, "Structure", AccountTypeRecord::Expense),
            acct(13, "Finishes and closeout", AccountTypeRecord::Expense),
            acct(20, "Funding", AccountTypeRecord::Equity),
            acct(30, "Project revenue", AccountTypeRecord::Income),
            acct(40, "Payables", AccountTypeRecord::Liability),
            acct(41, "Progress billings", AccountTypeRecord::Liability),
            acct(42, "Retainage payable", AccountTypeRecord::Liability),
        ],
    }
}

/// The opening configuration CreateBook writes: posting rules that hit
/// [`chart_for`] and the ingest template(s) for this kind.
///
/// ⭐ INVESTMENT SEEDS THE TRADE COLUMN CONTRACT, NOT A JOURNAL. The mapping
/// is the same `prime_equity_trades` file `deploy/seed-demo-book.sh` delivers;
/// a blank book has no rows until someone reads a file. The demo script still
/// posts recon history so the blocked-NAV story has a break — that is a
/// different book, and inventing those entries here would lie about this one's.
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

/// Custodian holdings snapshot plus the trade file that posts.
///
/// The snapshot is REFERENCE DATA: no `posts` block, so a row is recorded
/// and citable and never touches the journal. The trade mapping is the same
/// column contract the demo delivers (`TradeRef,ISIN,Symbol,Exch,Broker,B/S,
/// Quantity,Price,Ccy,TradeDate`) — not a second vendor catalog. Amounts
/// post as `consideration` (quantity × price); `dated` is the trade date, or
/// every lot this file opens is refused by every holding-period method.
///
/// ⚠ THE JOURNAL STAYS EMPTY UNTIL A FILE IS ADMITTED. Seeding counterparties
/// or opening balances here would be the fake history #76 refuses.
const INVESTMENT_CONFIG: &str = r#"
[[rule]]
id = "equity_purchase"
kind = "trade"
description = "Buy: investments up, cash down"
[[rule.posting]]
account = 1
weight = 1
per_instrument = true
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "disposal_proceeds"
kind = "trade"
description = "Sale, proceeds half: cash in against realized gain"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 31
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

[[template]]
id = "prime_equity_trades"
reads = "csv"

  [[template.entity]]
  name = "security"
  kind = "instrument"
  absent = "pend"
  by = [
    { attribute = "isin", column = "ISIN" },
    { attribute = "ticker", column = "Symbol", within = { attribute = "exchange", column = "Exch" } },
  ]

  [[template.entity]]
  name = "broker"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "code", column = "Broker" }]

  [template.fact]
  kind = "trade"
  reference = "TradeRef"
  entities = { security = "security", broker = "broker" }

  [[template.fact.value]]
  field = "side"
  as = "enum"
  column = "B/S"
  map = { B = "buy", S = "sell" }

  [[template.fact.value]]
  field = "quantity"
  as = "decimal"
  column = "Quantity"

  [[template.fact.value]]
  field = "price"
  as = "money"
  column = "Price"
  currency = "Ccy"

  [[template.fact.value]]
  field = "traded"
  as = "date"
  column = "TradeDate"
  format = "MM/DD/YYYY"

  [template.fact.posts]
  by = "side"
  amount = "consideration"
  rules = { buy = "equity_purchase", sell = "disposal_proceeds" }
  dated = "traded"
"#;

/// Project posting rules plus the vendor-invoice ingest template.
///
/// ⭐ THE ACCOUNT NUMBERS ARE `chart_for(Project)`'S. `initialize` runs
/// `check` against that chart before the digest is activated, so a drift
/// between the two is a refused create rather than a book that cannot post.
///
/// ⭐ WORK PACKAGES ARE ACCOUNTS, NOT INSTRUMENTS. Site / structure /
/// finishes partition project costs the way the chart partitions anything
/// else. Tagging a cost with an instrument would open a tax lot, and a
/// phase is not a security.
///
/// Progress-bill credits billings; earn-progress credits revenue. The two
/// are independent, so billed-to-date and earned-to-date can diverge while
/// every entry still conserves. Retainage is a transfer off AR (or onto
/// payables) — not a percentage baked into the bill — so a contract with
/// no holdback never posts it, and the figure stays unset rather than 0%.
///
/// ⚠ WIP CAPITALIZATION RULES ARE #66 / PR #80. This template does not
/// seed them, so the two PRs do not each own the same `[[rule]]` ids.
/// The WIP *account* stays on the chart; the transfer pattern lands with
/// that issue.
///
/// Phase budget: `[[project.phase]] account = <dim> budget = <minor units>`.
/// Omitting the row means no baseline, not a budget of zero.
///
/// The `project-invoices` template still maps `cost`/`invoice` onto the
/// unpartitioned `project_cost` / `vendor_invoice` rules. Per-phase mapping
/// is a later operator choice, not a CreateBook invention.
const PROJECT_CONFIG: &str = r#"# Project posting rules. Amount given; no instrument, so no lot.
# Work packages are accounts 11–13, not instruments.
# Progress-bill and earn-progress are independent: billed and earned can diverge.
# Retainage is a transfer, not a baked-in split — omit it and the figure stays unset.
# Phase budget: [[project.phase]] account = <dim> budget = <minor units>.

[[rule]]
id = "project_cost"
kind = "trade"
description = "Unpartitioned project costs paid from cash"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "project_cost_site"
kind = "trade"
description = "Site and mobilization paid from cash"
[[rule.posting]]
account = 11
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "project_cost_structure"
kind = "trade"
description = "Structure paid from cash"
[[rule.posting]]
account = 12
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "project_cost_finishes"
kind = "trade"
description = "Finishes and closeout paid from cash"
[[rule.posting]]
account = 13
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "vendor_invoice"
kind = "trade"
description = "Unpartitioned project costs on a vendor invoice"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "vendor_invoice_site"
kind = "trade"
description = "Site and mobilization on a vendor invoice"
[[rule.posting]]
account = 11
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "vendor_invoice_structure"
kind = "trade"
description = "Structure on a vendor invoice"
[[rule.posting]]
account = 12
weight = 1
[[rule.posting]]
account = 40
weight = -1

[[rule]]
id = "vendor_invoice_finishes"
kind = "trade"
description = "Finishes and closeout on a vendor invoice"
[[rule.posting]]
account = 13
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

[[rule]]
id = "progress_bill"
kind = "trade"
description = "Progress bill: receivable against billings on account"
[[rule.posting]]
account = 3
weight = 1
[[rule.posting]]
account = 41
weight = -1

[[rule]]
id = "hold_retainage"
kind = "trade"
description = "Hold retainage from a receivable until a milestone clears"
[[rule.posting]]
account = 4
weight = 1
[[rule.posting]]
account = 3
weight = -1

[[rule]]
id = "release_retainage"
kind = "trade"
description = "Release retainage onto the receivable"
[[rule.posting]]
account = 3
weight = 1
[[rule.posting]]
account = 4
weight = -1

[[rule]]
id = "collect_receivable"
kind = "trade"
description = "Collect a billed receivable into cash"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 3
weight = -1

[[rule]]
id = "earn_progress"
kind = "trade"
description = "Recognize earned progress against unbilled receivables"
[[rule.posting]]
account = 5
weight = 1
[[rule.posting]]
account = 30
weight = -1

[[rule]]
id = "hold_vendor_retainage"
kind = "trade"
description = "Hold retainage from a vendor payable"
[[rule.posting]]
account = 40
weight = 1
[[rule.posting]]
account = 42
weight = -1

[[rule]]
id = "release_vendor_retainage"
kind = "trade"
description = "Release vendor retainage back onto payables"
[[rule.posting]]
account = 42
weight = 1
[[rule.posting]]
account = 40
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
    use ratio_store::Journal;

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
        assert!(project.iter().any(|a| a.display_name == "Progress billings"));
        assert!(project.iter().any(|a| a.display_name == "Retainage receivable"));
        assert!(project.iter().any(|a| a.display_name == "Site and mobilization"));
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
            assert_eq!(ids, ingest_template_ids(kind), "{kind:?}: {ids:?}");
            for id in ingest_template_ids(kind) {
                let t = set.template(id).unwrap();
                assert!(t.check().is_empty(), "{kind:?} {id}: {:?}", t.check());
            }
        }
        let personal = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Personal)).unwrap();
        assert!(personal.template("custodian-positions").is_none());
        assert!(personal.template("prime_equity_trades").is_none());
        assert!(personal.template("project-invoices").is_none());
        let project = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        assert!(project.template("custodian-positions").is_none());
        assert!(project.template("prime_equity_trades").is_none());
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
    fn a_prime_equity_trade_row_posts_consideration_and_picks_the_buy_rule() {
        // ⭐ THE COLUMN CONTRACT #76 ASKS FOR. Same headers the demo delivers;
        // money is minor units; quantity × price is refused rather than rounded.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Investment)).unwrap();
        let t = set.template("prime_equity_trades").unwrap();
        assert!(t.fact.posts.is_some());
        let rows = ratio_ingest::extract_csv(PRIME_TRADES_CSV).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        assert_eq!(
            p.facts[0].values.get("price"),
            Some(&ratio_ingest::Value::Money {
                minor: 25_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "equity_purchase");
        // 1000 × 250.00 → 25_000_000 minor units, not a float and not a guess.
        assert_eq!(minor, 25_000_000);
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-02-24"));
        // VWRL is in the file and not in the master — LEAVE_ONE_PENDING's shape.
        let resolved = ratio_ingest::resolve_all(
            &p.facts,
            &[
                sample_entity(
                    "cp-prime",
                    ratio_ingest::EntityKind::Counterparty,
                    &[("code", "PRME")],
                ),
                sample_entity(
                    "inst-vti",
                    ratio_ingest::EntityKind::Instrument,
                    &[("isin", "US9229087690")],
                ),
                sample_entity(
                    "inst-voo",
                    ratio_ingest::EntityKind::Instrument,
                    &[("ticker", "VOO"), ("exchange", "ARCX")],
                ),
            ],
        );
        let pending: Vec<_> = resolved.iter().filter(|r| !r.is_admissible()).collect();
        assert_eq!(pending.len(), 1, "{:?}", pending.iter().map(|r| &r.fact.reference).collect::<Vec<_>>());
        assert_eq!(pending[0].fact.reference, "PB-0043");
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
        assert!(set.template("prime_equity_trades").is_none());
        // ⛔ AND THE JOURNAL IS EMPTY. A CreateBook book that arrived with
        // recon-posted history would be the fake past #76 is about.
        let mut n = 0usize;
        b.for_each_entry_since(0, &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(n, 0);
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
        assert!(set.rule("progress_bill").is_some());
        assert!(set.rule("hold_retainage").is_some());
        assert!(set.rule("earn_progress").is_some());
        assert!(set.rule("project_cost_site").is_some());
        assert!(
            set.rules.iter().all(|r| r.legs.iter().all(|l| !l.per_instrument)),
            "a project phase is an account, not an instrument — a per_instrument \
             leg would open a lot"
        );
        assert!(
            set.project.is_none(),
            "a new project has no phase baseline until someone sets [[project.phase]]"
        );
        let against_empty = check(&set, &[]);
        assert!(
            against_empty.iter().any(|f| !f.is_question),
            "project rules must not check against an empty chart: {against_empty:?}"
        );
        // Wave 2 (#75) ingest mapping is additive: still one template, still
        // unpartitioned cost/invoice rules. Not a per-phase ingest menu.
        let ingest = ratio_ingest::TemplateSet::from_toml(PROJECT_CONFIG).unwrap();
        assert_eq!(ingest.templates.len(), 1);
        assert_eq!(ingest.templates[0].id, "project-invoices");
    }

    #[test]
    fn initialize_seeds_progress_billing_and_no_phase_budget() {
        let dir = std::env::temp_dir().join("ratio-book-init-project-billing");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "bridge", "Bridge", BookKind::Project).unwrap();
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().unwrap();
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = RuleSet::from_toml(&text).unwrap();
        assert!(set.rule("progress_bill").is_some(), "{text}");
        assert!(set.rule("hold_retainage").is_some(), "{text}");
        assert!(set.project.is_none());
        let ingest = ratio_ingest::TemplateSet::from_toml(&text).unwrap();
        assert_eq!(ingest.templates[0].id, "project-invoices");
    }

    fn sample_delivery() -> ratio_ingest::Delivery {
        ratio_ingest::Delivery {
            digest: "a".repeat(64),
            origin: "fixture.csv".into(),
            received: 1,
            bytes: 1,
        }
    }

    fn sample_entity(
        id: &str,
        kind: ratio_ingest::EntityKind,
        attrs: &[(&str, &str)],
    ) -> ratio_ingest::Entity {
        ratio_ingest::Entity {
            id: id.into(),
            kind,
            display_name: id.into(),
            attributes: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    /// Same rows `console/fixtures/samples/prime_equity_trades.csv` and
    /// `deploy/seed-demo-book.sh` deliver. VWRL is the one the master lacks.
    const PRIME_TRADES_CSV: &str = "\
TradeRef,ISIN,Symbol,Exch,Broker,B/S,Quantity,Price,Ccy,TradeDate
PB-0041,US9229087690,VTI,ARCX,PRME,B,1000,250.00,USD,02/24/2026
PB-0042,,VOO,ARCX,PRME,B,400,450.00,USD,02/25/2026
PB-0043,IE00B3RBWM25,VWRL,XAMS,PRME,B,250,112.40,EUR,02/26/2026
";
}
