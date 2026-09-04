//! Book identity beside the journal: kind, optional fund, optional org.
//!
//! ⭐ THE KERNEL ALREADY TREATS A DIRECTORY AS A BOOK. `FileBook` is a journal
//! plus content-addressed config. This sidecar is the control-plane fact the
//! directory never had to carry: whether anyone filed the book under a fund
//! or a WorkOS organization. Absence is independence, not an error.

use std::path::Path;

use anyhow::{bail, Context, Result};
use ratio_rules::{check, RuleSet};
use ratio_store::{Account, AccountTypeRecord, ConfigStore, Digest, FileBook};

/// The ingest templates CreateBook writes for this kind, in the order
/// [`config_for`] lists them.
///
/// ⭐ KIND-AWARE, NOT A SHARED MENU. A Personal book that offered
/// `custodian-positions` would be asking a household to pick a fund feed.
/// The live list is the book's own configuration; these ids are what
/// [`config_for`] puts there, and what the console catalog filters on.
///
/// Personal is four mappings on purpose: bank / card, named-loan
/// payments, the brokerage trade column contract (household transfers,
/// not lots), and the holdings snapshot (recorded, never booked).
/// Investment is four mappings on purpose: the holdings snapshot (recorded,
/// never booked), the trade column contract that posts, the capital-call
/// contract (`commit_*` / `call_*`), and subscriptions / redemptions
/// (`subscribe_*` / `redeem_*`). One without the others is a file you
/// can read and a loop you cannot run.
pub fn ingest_template_ids(kind: BookKind) -> &'static [&'static str] {
    match kind {
        BookKind::Personal => &[
            "bank-statement",
            "loan-payment",
            "brokerage-statement",
            "brokerage-positions",
        ],
        BookKind::Investment => &[
            "custodian-positions",
            "prime_equity_trades",
            "capital-calls",
            "subscriptions",
        ],
        BookKind::Project => &["project-invoices", "change-orders", "purchase-orders"],
        BookKind::Operating => &["customer-invoices", "vendor-bills"],
    }
}

/// What a book is used for. Same kernel; different chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookKind {
    Personal,
    Investment,
    Project,
    Operating,
}

impl BookKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BookKind::Personal => "personal",
            BookKind::Investment => "investment",
            BookKind::Project => "project",
            BookKind::Operating => "operating",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "personal" | "PERSONAL" | "KIND_PERSONAL" => Ok(BookKind::Personal),
            "investment" | "INVESTMENT" | "KIND_INVESTMENT" => Ok(BookKind::Investment),
            "project" | "PROJECT" | "KIND_PROJECT" => Ok(BookKind::Project),
            "operating" | "OPERATING" | "KIND_OPERATING" => Ok(BookKind::Operating),
            other => bail!("{other:?} is not a book kind"),
        }
    }

    pub fn proto(self) -> i32 {
        match self {
            BookKind::Personal => 1,
            BookKind::Investment => 2,
            BookKind::Project => 3,
            BookKind::Operating => 4,
        }
    }

    pub fn from_proto(v: i32) -> Result<Self> {
        match v {
            1 => Ok(BookKind::Personal),
            2 => Ok(BookKind::Investment),
            3 => Ok(BookKind::Project),
            4 => Ok(BookKind::Operating),
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
            // ⭐ PERIOD-CLOSE EQUITY, NOT PARTNER CAPITAL. Allocations are
            // a personed close-into-capital; retained earnings is the
            // residual a period close carries. `[close] equity_destination
            // = 25`. Closing into Capital contributions would make a
            // year's surplus look like a subscription.
            acct(25, "Retained earnings", AccountTypeRecord::Equity),
            acct(30, "Dividend income", AccountTypeRecord::Income),
            acct(31, "Realized gain on investments", AccountTypeRecord::Income),
            acct(40, "Management fee payable", AccountTypeRecord::Liability),
            // ⭐ PARTNER-SCOPED CAPITAL IS MORE EQUITY DIMS, NOT A SECOND
            // LEDGER. LP and GP partition where capital sits; they do not
            // net to zero, and they roll up to book capital. Conservation
            // is untouched — `Ratio.Ingest.partition_preserves_conservation`.
            acct(50, "Partner capital — LP", AccountTypeRecord::Equity),
            acct(51, "Partner capital — GP", AccountTypeRecord::Equity),
            // ⭐ COMMITMENT / UNDRAWN ARE EQUITY, NOT AN ASSET. An undrawn
            // receivable would put unfunded capital into NAV — money that
            // has not arrived. Both sides of the pair are equity so they
            // cancel in the NAV filter (`Ratio.Chart.Dimensions`: equity
            // is partitioning, not conserved) and still conserve: a
            // commitment is Dr undrawn / Cr commitments; a call draws
            // that pair while cash and partner capital move. Grain is
            // the partner dim, not one fund-level bucket.
            acct(52, "Commitments — LP", AccountTypeRecord::Equity),
            acct(53, "Commitments — GP", AccountTypeRecord::Equity),
            acct(54, "Undrawn commitments — LP", AccountTypeRecord::Equity),
            acct(55, "Undrawn commitments — GP", AccountTypeRecord::Equity),
        ],
        BookKind::Personal => vec![
            acct(1, "Cash and bank", AccountTypeRecord::Asset),
            acct(2, "Investments", AccountTypeRecord::Asset),
            acct(10, "Living expenses", AccountTypeRecord::Expense),
            acct(11, "Taxes", AccountTypeRecord::Expense),
            acct(12, "Mortgage interest", AccountTypeRecord::Expense),
            acct(13, "Auto loan interest", AccountTypeRecord::Expense),
            acct(14, "Student loan interest", AccountTypeRecord::Expense),
            acct(20, "Opening equity", AccountTypeRecord::Equity),
            // ⭐ WHERE A PERIOD CLOSE ROLLS SURPLUS. Opening equity is
            // beginning capital, not the residual. `[close] equity_destination
            // = 25` names this; a book without that key refuses the close
            // rather than defaulting here. `Ratio.Close.missing_destination_
            // refuses_the_close`.
            acct(25, "Retained earnings", AccountTypeRecord::Equity),
            acct(30, "Income", AccountTypeRecord::Income),
            acct(40, "Credit cards", AccountTypeRecord::Liability),
            acct(41, "Mortgage", AccountTypeRecord::Liability),
            acct(42, "Auto loan", AccountTypeRecord::Liability),
            acct(43, "Student loan", AccountTypeRecord::Liability),
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
            // ⭐ CHANGE ORDERS ARE EQUITY, NOT A COST AND NOT A MUTATION OF
            // `[project] budget`. An approved CO is Dr authorization / Cr
            // approved change orders — the same conserved pair as a
            // commitment. Both sides are equity so they cancel if folded
            // into funding, and `/budget` still cites the original
            // baseline. Grain is the work package (site / structure /
            // finishes), not one lump CO bucket that fights cost-by-phase.
            acct(21, "Change-order authorization", AccountTypeRecord::Equity),
            acct(22, "Change-order authorization — Site and mobilization", AccountTypeRecord::Equity),
            acct(23, "Change-order authorization — Structure", AccountTypeRecord::Equity),
            acct(24, "Change-order authorization — Finishes and closeout", AccountTypeRecord::Equity),
            acct(25, "Approved change orders", AccountTypeRecord::Equity),
            acct(26, "Approved change orders — Site and mobilization", AccountTypeRecord::Equity),
            acct(27, "Approved change orders — Structure", AccountTypeRecord::Equity),
            acct(28, "Approved change orders — Finishes and closeout", AccountTypeRecord::Equity),
            // ⭐ PERIOD-CLOSE EQUITY, NOT FUNDING AND NOT A CHANGE ORDER.
            // Funding is who put money in; a CO pair is authorization.
            // `[close] equity_destination = 29`. Dim 25 is already
            // Approved change orders.
            acct(29, "Retained earnings", AccountTypeRecord::Equity),
            acct(30, "Project revenue", AccountTypeRecord::Income),
            acct(40, "Payables", AccountTypeRecord::Liability),
            acct(41, "Progress billings", AccountTypeRecord::Liability),
            acct(42, "Retainage payable", AccountTypeRecord::Liability),
            // ⭐ AWARDED COMMITMENTS ARE EQUITY, NOT A COST AND NOT A
            // PAYABLE. An open subcontract is cost the job has agreed to
            // and has not yet incurred. Putting it on the expense side
            // would inflate actual; putting remaining as an asset would
            // make an un-incurred PO look like money that arrived. Both
            // sides of the pair are equity so they cancel if folded into
            // funding, and `/budget` cites the credit-normal awarded
            // side. Grain is the work package — same accounts cost-by-
            // package already uses — not a second WBS vocabulary.
            acct(60, "Commitment authorization", AccountTypeRecord::Equity),
            acct(61, "Commitment authorization — Site and mobilization", AccountTypeRecord::Equity),
            acct(62, "Commitment authorization — Structure", AccountTypeRecord::Equity),
            acct(63, "Commitment authorization — Finishes and closeout", AccountTypeRecord::Equity),
            acct(64, "Awarded commitments", AccountTypeRecord::Equity),
            acct(65, "Awarded commitments — Site and mobilization", AccountTypeRecord::Equity),
            acct(66, "Awarded commitments — Structure", AccountTypeRecord::Equity),
            acct(67, "Awarded commitments — Finishes and closeout", AccountTypeRecord::Equity),
        ],
        // ⭐ BUSINESS-NORMAL GROUPINGS, NOT FUND NAV AND NOT HOUSEHOLD NET
        // WORTH. Cash, AR, AP, operating revenue/expense, and owner equity
        // are the partitions an ordinary service company or studio cites.
        // Inventory, payroll, tax, and prepaid/accrued are not invented
        // here — a silent zero on those would be a QuickBooks clone.
        // AR and AP are control accounts the sheet cites. Aging them by
        // due date needs a due date on the invoice/bill and application
        // of collections/payments to those items — optional on the
        // journal, unset on `/aging` when either cut is missing.
        BookKind::Operating => vec![
            acct(1, "Cash", AccountTypeRecord::Asset),
            acct(2, "Accounts receivable", AccountTypeRecord::Asset),
            acct(10, "Operating expenses", AccountTypeRecord::Expense),
            acct(20, "Owner equity", AccountTypeRecord::Equity),
            // ⭐ PERIOD-CLOSE EQUITY, NOT OWNER EQUITY. Owner equity is
            // who put money in and drew it out. `[close] equity_destination
            // = 25`. Closing into Owner equity would make a year's surplus
            // look like a contribution.
            acct(25, "Retained earnings", AccountTypeRecord::Equity),
            acct(30, "Operating revenue", AccountTypeRecord::Income),
            acct(40, "Accounts payable", AccountTypeRecord::Liability),
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

/// Commitment and undrawn equity — not funded capital.
///
/// ⛔ THESE ARE NOT `is_capital_account`. Folding them into ending capital
/// would make a commitment look like money that arrived. They appear on
/// the capital fold so `/capital` can cite them; `bookCapital` still
/// excludes them.
pub fn is_commitment_account(display_name: &str) -> bool {
    display_name.starts_with("Commitments") || display_name.starts_with("Undrawn commitments")
}

/// Approved change orders and their authorizing contra — not funding, not cost.
///
/// ⛔ THESE ARE NOT `[project] budget`. Folding an approved CO into the
/// configuration total would rewrite the baseline the way a spreadsheet
/// does. Citing them as project costs would mix authorization with spend.
/// Both sides of the pair are equity so they conserve; `/budget` and
/// `/billing` cite the credit-normal approved-change-order side.
pub fn is_change_order_account(display_name: &str) -> bool {
    display_name == "Approved change orders"
        || display_name == "Change-order authorization"
        || display_name.starts_with("Approved change orders — ")
        || display_name.starts_with("Change-order authorization — ")
}

/// Awarded purchase orders / subcontracts and their authorizing contra.
///
/// ⛔ THESE ARE NOT PROJECT COSTS AND NOT VENDOR PAYABLES. Folding an
/// award into incurred would treat an open PO as spend. Citing it as a
/// payable would mix a memorandum with an invoice. Both sides are equity
/// so they conserve; `/budget` cites the credit-normal awarded side.
/// ⛔ NOT `is_commitment_account`. That name is the partner-capital pair
/// on an Investment book (`Commitments — LP`). "Awarded commitments"
/// starts with Awarded, so the two classifiers do not collide.
pub fn is_awarded_commitment_account(display_name: &str) -> bool {
    display_name == "Awarded commitments"
        || display_name == "Commitment authorization"
        || display_name.starts_with("Awarded commitments — ")
        || display_name.starts_with("Commitment authorization — ")
}

/// The opening configuration CreateBook writes: posting rules that hit
/// [`chart_for`] and the ingest template(s) for this kind.
///
/// Personal seeds four mappings (`bank-statement`, `loan-payment`,
/// `brokerage-statement`, `brokerage-positions`); Investment and Project
/// seed their own. The live list is the book's configuration.
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
        BookKind::Operating => OPERATING_CONFIG,
    }
}

/// Bank / card CSV → cash and expense claims. Amounts are a money column
/// (never a float); `Kind` picks the rule so a signed-amount inference
/// cannot silently flip income and a card charge.
///
/// Loan payments: a second template reads principal and interest as two
/// money columns and posts two balanced rules into one entry. Set
/// `[personal.loan]` keyed by liability dimension (value = interest
/// expense dimension) to name a schedule; omitting the table means no
/// loan has been named — not a roll-forward of zeros.
///
/// Brokerage / custodian CSV (#167): the same column contract the fund
/// trade loop reads (`B/S`, ISIN / ticker, consideration), posted as
/// household transfers onto Investments — not `equity_purchase`.
/// Holdings are reference data (`brokerage-positions`); live recon
/// reuses the fund refuse paths. Lot relief stays unset (#187).
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


[[rule]]
id = "mortgage_interest"
kind = "trade"
description = "Mortgage interest up, cash and bank down"
[[rule.posting]]
account = 12
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "mortgage_principal"
kind = "trade"
description = "Mortgage liability down, cash and bank down"
[[rule.posting]]
account = 41
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "auto_interest"
kind = "trade"
description = "Auto loan interest up, cash and bank down"
[[rule.posting]]
account = 13
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "auto_principal"
kind = "trade"
description = "Auto loan liability down, cash and bank down"
[[rule.posting]]
account = 42
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "student_interest"
kind = "trade"
description = "Student loan interest up, cash and bank down"
[[rule.posting]]
account = 14
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "student_principal"
kind = "trade"
description = "Student loan liability down, cash and bank down"
[[rule.posting]]
account = 43
weight = 1
[[rule.posting]]
account = 1
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

[[template]]
id = "loan-payment"
reads = "csv"

  [[template.entity]]
  name = "lender"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "name", column = "Lender" }]

  [template.fact]
  kind = "payment"
  reference = "Ref"
  entities = { lender = "lender" }

  [[template.fact.value]]
  field = "dated"
  as = "date"
  column = "Date"
  format = "YYYY-MM-DD"

  [[template.fact.value]]
  field = "principal"
  as = "money"
  column = "Principal"
  currency = "Ccy"

  [[template.fact.value]]
  field = "interest"
  as = "money"
  column = "Interest"
  currency = "Ccy"
  optional = true

  [[template.fact.value]]
  field = "loan"
  as = "enum"
  column = "Loan"
  map = { mortgage = "mortgage", auto = "auto", student = "student" }

  [[template.fact.value]]
  field = "memo"
  as = "text"
  column = "Memo"
  optional = true

  [template.fact.posts]
  by = "loan"
  amount = "principal"
  rules = { mortgage = "mortgage_principal", auto = "auto_principal", student = "student_principal" }
  dated = "dated"

    [[template.fact.posts.also]]
    amount = "interest"
    rules = { mortgage = "mortgage_interest", auto = "auto_interest", student = "student_interest" }

# Brokerage / custodian CSV. Same columns as the fund trade file; the
# rules are household transfers, not lot-opening purchases. A per-instrument
# leg here would be #187 wearing an ingest sticker.
[[template]]
id = "brokerage-statement"
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
  rules = { buy = "xfer_cash_investments", sell = "xfer_investments_cash" }
  dated = "traded"

# Holdings snapshot. Recorded, never posted — the same mode as
# Investment `custodian-positions`. Live recon compares market value
# to the journal's Investments carrying value.
[[template]]
id = "brokerage-positions"
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

# Where a period close rolls surplus. Absent is unset — not Opening equity.
[close]
equity_destination = 25
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
/// or opening balances here would be the fake history #76 refuses. The live
/// feed (#155) is ingest + admit + `recon --from-ingest` on this seed.
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

# Unitization. A subscription issues units; a redemption retires them.
# Quantity is MEASURED — it does not enter conservation, and it opens
# no lot (`measured`, not `per_instrument`). A contribution without
# units is still contribute_lp; a silent 0 on that path is the defect.
# Book-level subscribe/redeem do NOT split units 1/N across partners.
# `Ratio.Partners.allocating_units_without_a_cut_is_unset`.

[[rule]]
id = "subscribe"
kind = "trade"
description = "Book-level subscription: cash in, capital contributions up, units issued"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 20
weight = -1
measured = true

[[rule]]
id = "redeem"
kind = "trade"
description = "Book-level redemption: distributions up, cash out, units retired"

[[rule.posting]]
account = 22
weight = 1
measured = true

[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "subscribe_lp"
kind = "trade"
description = "LP subscription: cash in, partner capital up, units issued"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 50
weight = -1
measured = true

[[rule]]
id = "subscribe_gp"
kind = "trade"
description = "GP subscription: cash in, partner capital up, units issued"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 51
weight = -1
measured = true

[[rule]]
id = "redeem_lp"
kind = "trade"
description = "LP redemption: partner capital down, cash out, units retired"

[[rule.posting]]
account = 50
weight = 1
measured = true

[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "redeem_gp"
kind = "trade"
description = "GP redemption: partner capital down, cash out, units retired"

[[rule.posting]]
account = 51
weight = 1
measured = true

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

# Commitments. Not a schedule, not IRR, not a waterfall.
# A commitment is the pair: undrawn up, commitments up. A call is cash
# and partner capital PLUS drawing that pair. contribute_lp is still
# funded capital without a draw — using it when you meant a call leaves
# undrawn stale, which is the operator's claim, not a silent draw.

[[rule]]
id = "commit_lp"
kind = "trade"
description = "LP commitment: undrawn up, commitments up"

[[rule.posting]]
account = 54
weight = 1

[[rule.posting]]
account = 52
weight = -1

[[rule]]
id = "commit_gp"
kind = "trade"
description = "GP commitment: undrawn up, commitments up"

[[rule.posting]]
account = 55
weight = 1

[[rule.posting]]
account = 53
weight = -1

[[rule]]
id = "call_lp"
kind = "trade"
description = "LP capital call: cash in, partner capital up, draw the commitment"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 50
weight = -1

[[rule.posting]]
account = 52
weight = 1

[[rule.posting]]
account = 54
weight = -1

[[rule]]
id = "call_gp"
kind = "trade"
description = "GP capital call: cash in, partner capital up, draw the commitment"

[[rule.posting]]
account = 2
weight = 1

[[rule.posting]]
account = 51
weight = -1

[[rule.posting]]
account = 53
weight = 1

[[rule.posting]]
account = 55
weight = -1

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

[[template]]
id = "capital-calls"
reads = "csv"

  [template.fact]
  kind = "capital"
  reference = "CallRef"

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
  field = "kind"
  as = "enum"
  column = "Kind"
  map = { commit_lp = "commit_lp", commit_gp = "commit_gp", call_lp = "call_lp", call_gp = "call_gp" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { commit_lp = "commit_lp", commit_gp = "commit_gp", call_lp = "call_lp", call_gp = "call_gp" }
  dated = "dated"

# Unitization. A row names cash AND a whole-unit quantity. Kind picks
# subscribe / redeem (book-level or partner). Quantity is MEASURED — it
# does not enter conservation. A contribution without units stays on
# capital-calls / contribute_*; a silent 0 here is the defect.
[[template]]
id = "subscriptions"
reads = "csv"

  [template.fact]
  kind = "capital"
  reference = "Ref"

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
  field = "quantity"
  as = "decimal"
  column = "Quantity"

  [[template.fact.value]]
  field = "kind"
  as = "enum"
  column = "Kind"
  map = { subscribe = "subscribe", subscribe_lp = "subscribe_lp", subscribe_gp = "subscribe_gp", redeem = "redeem", redeem_lp = "redeem_lp", redeem_gp = "redeem_gp" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { subscribe = "subscribe", subscribe_lp = "subscribe_lp", subscribe_gp = "subscribe_gp", redeem = "redeem", redeem_lp = "redeem_lp", redeem_gp = "redeem_gp" }
  dated = "dated"

# Partner allocation cut. Named weights, not a partner count.
# Empty is unset — allocated plugs stay unset, not a silent 1/N.
# CreateBook(Investment) writes LP 80 / GP 20 so the live demo can
# cite a dividing figure. Writing equal weights is an election;
# inventing them from the partner count is not.
# `Ratio.Partners.no_cut_is_unset`.
[[partner_cut]]
partner = "LP"
weight = 80

[[partner_cut]]
partner = "GP"
weight = 20

# Where a period close rolls surplus. Absent is unset — not Capital contributions.
[close]
equity_destination = 25
"#;

/// Project posting rules plus the job-cost / AP / progress-bill ingest template.
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
/// WIP capitalization (`capitalize_wip` / `recognize_wip`) is #66 / PR #80.
/// Progress billing is #85 / PR #88. Change orders are #91. Remaining to
/// bill and collections vs billed (#100) compose onto `/billing` from the
/// same journal — they do not add a rule. Awarded commitments and remaining
/// to spend (#104) compose onto `/budget` the same way: `award_commitment_*`
/// / `release_commitment_*` are the pair; remaining to spend is revised −
/// incurred − awarded and stays unset until those inputs can support it.
/// `/wip` and `/billing` stay two URLs; change orders, remaining-to-bill,
/// collections, and committed cost compose onto `/budget` and `/billing`
/// rather than a third chrome list.
///
/// Phase budget: `[[project.phase]] account = <dim> budget = <minor units>`.
/// Omitting the row means no baseline, not a budget of zero.
/// Book-level `[project] budget` is the original contract `/budget` cites.
/// An approved change order posts; it does not rewrite that key.
///
/// The `project-invoices` template is the job-cost / AP / progress-bill
/// statement: Kind picks `project_cost*` / `vendor_invoice*` / `progress_bill`
/// / `pay_vendor` / `earn_progress`. Per-phase mapping is the Kind suffix
/// (`invoice_site`), the same grain change-orders already use — not a
/// second WBS and not a CreateBook invention of a phase the chart lacks.
/// Retainage and WIP kinds are absent on purpose: a holdback is a transfer
/// already on `/record`, and ingesting an invoice must not invent one.
/// `hold_retainage` / `capitalize_wip` in Kind are refused, not posted.
/// `collect_receivable` stays on `/billing` (#173) — this file is vendor
/// cost / AP / owner progress-bill, not customer cash. `change-orders`
/// maps `approve_co_*` / `deduct_co_*` onto the work-package pair.
/// `purchase-orders` maps `award_commitment_*` / `release_commitment_*`
/// onto the awarded-commitment pair.
const PROJECT_CONFIG: &str = r#"# Project posting rules. Amount given; no instrument, so no lot.
# Work packages are accounts 11–13, not instruments.
# Progress-bill and earn-progress are independent: billed and earned can diverge.
# Retainage is a transfer, not a baked-in split — omit it and the figure stays unset.
# project-invoices maps cost / invoice / progress_bill / pay_vendor / earn_progress.
# hold_retainage and capitalize_wip are not kinds on that file — ingesting an
# invoice must not invent a holdback or a WIP transfer.
# Change orders are a conserved equity pair keyed by work package. They do not
# rewrite [project] budget — that key is the original baseline.
# Awarded commitments are a second conserved equity pair on the same grain:
# an open subcontract is not incurred cost and not a vendor payable.
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

# Change orders. Not a mutation of [project] budget, not a cost, not AIA G702.
# An approval is the pair: authorization up, approved change orders up.
# A deduction reverses that pair. Grain is the work package — site /
# structure / finishes — matching cost-by-phase, not one CO bucket.
# Unpartitioned approve_co / deduct_co hit the unpartitioned pair.

[[rule]]
id = "approve_co"
kind = "trade"
description = "Approve an unpartitioned change order: authorization up, approved change orders up"
[[rule.posting]]
account = 21
weight = 1
[[rule.posting]]
account = 25
weight = -1

[[rule]]
id = "approve_co_site"
kind = "trade"
description = "Approve a site-and-mobilization change order"
[[rule.posting]]
account = 22
weight = 1
[[rule.posting]]
account = 26
weight = -1

[[rule]]
id = "approve_co_structure"
kind = "trade"
description = "Approve a structure change order"
[[rule.posting]]
account = 23
weight = 1
[[rule.posting]]
account = 27
weight = -1

[[rule]]
id = "approve_co_finishes"
kind = "trade"
description = "Approve a finishes-and-closeout change order"
[[rule.posting]]
account = 24
weight = 1
[[rule.posting]]
account = 28
weight = -1

[[rule]]
id = "deduct_co"
kind = "trade"
description = "Deduct an unpartitioned change order: reverse the approval pair"
[[rule.posting]]
account = 25
weight = 1
[[rule.posting]]
account = 21
weight = -1

[[rule]]
id = "deduct_co_site"
kind = "trade"
description = "Deduct a site-and-mobilization change order"
[[rule.posting]]
account = 26
weight = 1
[[rule.posting]]
account = 22
weight = -1

[[rule]]
id = "deduct_co_structure"
kind = "trade"
description = "Deduct a structure change order"
[[rule.posting]]
account = 27
weight = 1
[[rule.posting]]
account = 23
weight = -1

[[rule]]
id = "deduct_co_finishes"
kind = "trade"
description = "Deduct a finishes-and-closeout change order"
[[rule.posting]]
account = 28
weight = 1
[[rule.posting]]
account = 24
weight = -1

# Awarded commitments. Not a cost, not a payable, not a purchasing product.
# An award is the pair: authorization up, awarded commitments up. A release
# reverses that pair as the cost is incurred (vendor_invoice / project_cost
# stay the actual). Grain is the work package — site / structure / finishes
# — matching cost-by-phase, not a second breakdown. Unpartitioned
# award_commitment / release_commitment hit the unpartitioned pair.
# Remaining to spend is composed on /budget: revised − incurred − awarded.
# Unset awarded is not a fake zero; treating it as zero would print
# budget − actual as headroom.

[[rule]]
id = "award_commitment"
kind = "trade"
description = "Award an unpartitioned purchase order: authorization up, awarded commitments up"
[[rule.posting]]
account = 60
weight = 1
[[rule.posting]]
account = 64
weight = -1

[[rule]]
id = "award_commitment_site"
kind = "trade"
description = "Award a site-and-mobilization purchase order"
[[rule.posting]]
account = 61
weight = 1
[[rule.posting]]
account = 65
weight = -1

[[rule]]
id = "award_commitment_structure"
kind = "trade"
description = "Award a structure purchase order"
[[rule.posting]]
account = 62
weight = 1
[[rule.posting]]
account = 66
weight = -1

[[rule]]
id = "award_commitment_finishes"
kind = "trade"
description = "Award a finishes-and-closeout purchase order"
[[rule.posting]]
account = 63
weight = 1
[[rule.posting]]
account = 67
weight = -1

[[rule]]
id = "release_commitment"
kind = "trade"
description = "Release an unpartitioned purchase order: reverse the award pair as cost is incurred"
[[rule.posting]]
account = 64
weight = 1
[[rule.posting]]
account = 60
weight = -1

[[rule]]
id = "release_commitment_site"
kind = "trade"
description = "Release a site-and-mobilization purchase order"
[[rule.posting]]
account = 65
weight = 1
[[rule.posting]]
account = 61
weight = -1

[[rule]]
id = "release_commitment_structure"
kind = "trade"
description = "Release a structure purchase order"
[[rule.posting]]
account = 66
weight = 1
[[rule.posting]]
account = 62
weight = -1

[[rule]]
id = "release_commitment_finishes"
kind = "trade"
description = "Release a finishes-and-closeout purchase order"
[[rule.posting]]
account = 67
weight = 1
[[rule.posting]]
account = 63
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
  map = { cost = "cost", cost_site = "cost_site", cost_structure = "cost_structure", cost_finishes = "cost_finishes", invoice = "invoice", invoice_site = "invoice_site", invoice_structure = "invoice_structure", invoice_finishes = "invoice_finishes", progress_bill = "progress_bill", pay_vendor = "pay_vendor", earn_progress = "earn_progress" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { cost = "project_cost", cost_site = "project_cost_site", cost_structure = "project_cost_structure", cost_finishes = "project_cost_finishes", invoice = "vendor_invoice", invoice_site = "vendor_invoice_site", invoice_structure = "vendor_invoice_structure", invoice_finishes = "vendor_invoice_finishes", progress_bill = "progress_bill", pay_vendor = "pay_vendor", earn_progress = "earn_progress" }
  dated = "dated"

[[template]]
id = "change-orders"
reads = "csv"

  [template.fact]
  kind = "change"
  reference = "ChangeRef"

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
  map = { approve_co = "approve_co", approve_co_site = "approve_co_site", approve_co_structure = "approve_co_structure", approve_co_finishes = "approve_co_finishes", deduct_co = "deduct_co", deduct_co_site = "deduct_co_site", deduct_co_structure = "deduct_co_structure", deduct_co_finishes = "deduct_co_finishes" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { approve_co = "approve_co", approve_co_site = "approve_co_site", approve_co_structure = "approve_co_structure", approve_co_finishes = "approve_co_finishes", deduct_co = "deduct_co", deduct_co_site = "deduct_co_site", deduct_co_structure = "deduct_co_structure", deduct_co_finishes = "deduct_co_finishes" }
  dated = "dated"

[[template]]
id = "purchase-orders"
reads = "csv"

  [template.fact]
  kind = "purchase"
  reference = "PurchaseRef"

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
  map = { award_commitment = "award_commitment", award_commitment_site = "award_commitment_site", award_commitment_structure = "award_commitment_structure", award_commitment_finishes = "award_commitment_finishes", release_commitment = "release_commitment", release_commitment_site = "release_commitment_site", release_commitment_structure = "release_commitment_structure", release_commitment_finishes = "release_commitment_finishes" }

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { award_commitment = "award_commitment", award_commitment_site = "award_commitment_site", award_commitment_structure = "award_commitment_structure", award_commitment_finishes = "award_commitment_finishes", release_commitment = "release_commitment", release_commitment_site = "release_commitment_site", release_commitment_structure = "release_commitment_structure", release_commitment_finishes = "release_commitment_finishes" }
  dated = "dated"

# Where a period close rolls surplus. Dim 25 is Approved change orders.
[close]
equity_destination = 29
"#;

/// Operating-company posting rules plus customer-invoice and vendor-bill ingest.
///
/// ⭐ THE ACCOUNT NUMBERS ARE `chart_for(Operating)`'S. `initialize` runs
/// `check` against that chart before the digest is activated.
///
/// ⛔ NOT A PROJECT JOB AND NOT A HOUSEHOLD. `invoice_customer` is entity-wide
/// AR against operating revenue — not progress billings, not retainage, not
/// billed-vs-earned. `vendor_bill` is entity-wide AP against operating
/// expense — not a work-package cost. Due date and open-item application
/// are optional on the journal; `/aging` stays unset when either is
/// missing rather than inventing current or an equal split.
///
/// Cash sales and cash expenses stay available so a studio that never
/// invoices can still cite a period income statement. Owner contribution
/// and draw are equity, not revenue or expense.
const OPERATING_CONFIG: &str = r#"# Operating-company posting rules. Amount given; no instrument, so no lot.
# AR and AP are control accounts. DueDate and AppliesTo are optional:
# aging stays unset when a remaining item has no due date or a reduction
# does not name the invoice/bill it applies to — no silent "current" bucket.

[[rule]]
id = "invoice_customer"
kind = "trade"
description = "Bill a customer: receivable against operating revenue"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 30
weight = -1

[[rule]]
id = "collect_receivable"
kind = "trade"
description = "Collect a receivable into cash"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "receive_revenue"
kind = "trade"
description = "Cash sale: cash against operating revenue"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 30
weight = -1

[[rule]]
id = "vendor_bill"
kind = "trade"
description = "Vendor bill: operating expenses on account"
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
id = "pay_expense"
kind = "trade"
description = "Operating expenses paid from cash"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "contribute_equity"
kind = "trade"
description = "Owner contribution: cash in, owner equity up"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 20
weight = -1

[[rule]]
id = "draw_equity"
kind = "trade"
description = "Owner draw: owner equity down, cash out"
[[rule.posting]]
account = 20
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[template]]
id = "customer-invoices"
reads = "csv"

  [[template.entity]]
  name = "customer"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "name", column = "Customer" }]

  [template.fact]
  kind = "invoice"
  reference = "InvoiceRef"
  entities = { customer = "customer" }

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
  map = { invoice = "invoice", collect = "collect" }

  [[template.fact.value]]
  field = "dueDate"
  as = "date"
  column = "DueDate"
  format = "YYYY-MM-DD"
  optional = true

  [[template.fact.value]]
  field = "application"
  as = "text"
  column = "AppliesTo"
  optional = true

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { invoice = "invoice_customer", collect = "collect_receivable" }
  dated = "dated"

[[template]]
id = "vendor-bills"
reads = "csv"

  [[template.entity]]
  name = "vendor"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "name", column = "Vendor" }]

  [template.fact]
  kind = "bill"
  reference = "BillRef"
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
  map = { bill = "bill", pay = "pay" }

  [[template.fact.value]]
  field = "dueDate"
  as = "date"
  column = "DueDate"
  format = "YYYY-MM-DD"
  optional = true

  [[template.fact.value]]
  field = "application"
  as = "text"
  column = "AppliesTo"
  optional = true

  [template.fact.posts]
  by = "kind"
  amount = "amount"
  rules = { bill = "vendor_bill", pay = "pay_vendor" }
  dated = "dated"

# Where a period close rolls surplus. Dim 20 is Owner equity — who put
# money in. Closing into it would make a year's surplus look like a
# contribution. `Ratio.Close.missing_destination_refuses_the_close`.
[close]
equity_destination = 25
"#;

/// Create the directory, the chart, the kind's opening ingest configuration,
/// and the sidecar.
///
/// ⛔ NO FUND AND NO ORG ARE WRITTEN. A caller that wants either files the
/// book afterwards. Create is the independent book.
pub fn initialize(path: &Path, id: &str, display: &str, kind: BookKind) -> Result<Digest> {
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
    Ok(digest)
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
    fn grant_writes_the_subject_and_not_an_org_and_is_idempotent() {
        let dir = std::env::temp_dir().join("ratio-book-grant");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        grant(&dir, "", "household").unwrap();
        assert!(
            !dir.join("MEMBERSHIP.tsv").is_file(),
            "Local (empty who) writes nothing"
        );
        grant(&dir, "user_1", "household").unwrap();
        grant(&dir, "user_1", "household").unwrap();
        grant(&dir, "user_2", "household").unwrap();
        let text = std::fs::read_to_string(dir.join("MEMBERSHIP.tsv")).unwrap();
        assert_eq!(text, "user_1\thousehold\nuser_2\thousehold\n");
        assert!(!text.contains("org:"), "grant is the subject's id, not an org");
    }

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
        let operating = chart_for(BookKind::Operating);
        assert_ne!(personal, investment);
        assert_ne!(personal, project);
        assert_ne!(investment, project);
        assert_ne!(operating, personal);
        assert_ne!(operating, project);
        assert_ne!(operating, investment);
        assert!(personal.iter().any(|a| a.display_name == "Cash and bank"));
        assert!(
            personal.iter().any(|a| a.display_name == "Retained earnings"),
            "period close needs a named equity destination: {personal:?}"
        );
        assert!(
            investment.iter().any(|a| a.display_name == "Retained earnings"),
            "investment close destination: {investment:?}"
        );
        assert!(
            project.iter().any(|a| a.display_name == "Retained earnings"),
            "project close destination: {project:?}"
        );
        assert!(
            operating.iter().any(|a| a.display_name == "Retained earnings"),
            "operating close destination is not Owner equity: {operating:?}"
        );
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Investments at fair value"));
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Commitments — LP"));
        assert!(investment
            .iter()
            .any(|a| a.display_name == "Undrawn commitments — GP"));
        assert!(project.iter().any(|a| a.display_name == "Work in progress"));
        assert!(project.iter().any(|a| a.display_name == "Progress billings"));
        assert!(project.iter().any(|a| a.display_name == "Retainage receivable"));
        assert!(project.iter().any(|a| a.display_name == "Site and mobilization"));
        assert!(project
            .iter()
            .any(|a| a.display_name == "Approved change orders — Site and mobilization"));
        assert!(project
            .iter()
            .any(|a| a.display_name == "Change-order authorization — Structure"));
        assert!(project
            .iter()
            .any(|a| a.display_name == "Awarded commitments — Site and mobilization"));
        assert!(project
            .iter()
            .any(|a| a.display_name == "Commitment authorization — Structure"));
        assert!(operating.iter().any(|a| a.display_name == "Cash"));
        assert!(operating.iter().any(|a| a.display_name == "Accounts receivable"));
        assert!(operating.iter().any(|a| a.display_name == "Accounts payable"));
        assert!(operating.iter().any(|a| a.display_name == "Operating revenue"));
        assert!(operating.iter().any(|a| a.display_name == "Operating expenses"));
        assert!(operating.iter().any(|a| a.display_name == "Owner equity"));
        assert!(
            operating.iter().all(|a| a.display_name != "Work in progress"),
            "an operating book is not a project job: {operating:?}"
        );
        assert!(
            operating.iter().all(|a| a.display_name != "Living expenses"),
            "an operating book is not a household: {operating:?}"
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

    #[test]
    fn each_kind_seeds_its_own_ingest_template_and_not_the_others() {
        // ⭐ THE CLAIM #72 IS ABOUT. A Personal book that listed
        // `custodian-positions` would force a household to pick a fund feed.
        // CreateBook writes the kind's mapping into the opening configuration,
        // so ListTemplates is kind-aware because the book only has its own.
        for kind in [
            BookKind::Personal,
            BookKind::Investment,
            BookKind::Project,
            BookKind::Operating,
        ] {
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
        assert!(personal.template("loan-payment").is_some());
        assert!(personal.template("loan-payment").unwrap().check().is_empty());
        assert!(personal.template("brokerage-statement").is_some());
        assert!(personal.template("brokerage-statement").unwrap().check().is_empty());
        assert!(personal.template("brokerage-positions").is_some());
        assert!(personal.template("brokerage-positions").unwrap().check().is_empty());
        assert!(personal.template("prime_equity_trades").is_none());
        assert!(personal.template("project-invoices").is_none());
        let project = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        assert!(project.template("custodian-positions").is_none());
        assert!(project.template("prime_equity_trades").is_none());
        assert!(project.template("bank-statement").is_none());
        let operating = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Operating)).unwrap();
        assert!(operating.template("customer-invoices").is_some());
        assert!(operating.template("vendor-bills").is_some());
        assert!(operating.template("project-invoices").is_none());
        assert!(operating.template("bank-statement").is_none());
        assert!(operating.template("custodian-positions").is_none());
    }

    #[test]
    fn seeded_rules_balance_against_the_kind_chart() {
        // ⛔ A TEMPLATE THAT POSTS AT A RULE THE CHART CANNOT EXPRESS would
        // admit a fact and then refuse the entry. The opening configuration
        // is checked the same way an approval is.
        for kind in [
            BookKind::Personal,
            BookKind::Investment,
            BookKind::Project,
            BookKind::Operating,
        ] {
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
    fn a_brokerage_statement_row_posts_a_transfer_not_a_lot() {
        // ⭐ #167. Same custodian/broker columns as prime_equity_trades;
        // the household rule is a transfer onto Investments. A buy that
        // picked equity_purchase would open a lot #187 has not elected.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Personal)).unwrap();
        let t = set.template("brokerage-statement").unwrap();
        assert!(t.fact.posts.is_some());
        assert!(set.template("brokerage-positions").unwrap().fact.posts.is_none());
        let rows = ratio_ingest::extract_csv(PRIME_TRADES_CSV).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "xfer_cash_investments");
        assert_ne!(rule, "equity_purchase");
        // 1000 × 250.00 → 25_000_000 minor units, not a float and not a guess.
        assert_eq!(minor, 25_000_000);
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-02-24"));
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "xfer_cash_investments");
        // VWRL is in the file and not in the master — same pending shape.
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
    fn a_job_cost_row_picks_phase_or_progress_bill_and_refuses_retainage() {
        // ⭐ #171. Kind names the work package or the progress-bill rule.
        // A retainage or WIP kind is not on the map: ingesting an invoice
        // must not invent a holdback, and capitalize_wip stays on /record.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        let t = set.template("project-invoices").unwrap();
        assert!(t.fact.posts.is_some());
        let csv = "\
InvoiceRef,Date,Amount,Ccy,Vendor,Memo,Kind
INV-1,2026-03-01,1200.00,USD,ACME STEEL,steel delivery,invoice_site
BILL-1,2026-03-15,5000.00,USD,OWNER,March pay app,progress_bill
EARN-1,2026-03-15,4000.00,USD,OWNER,earned to date,earn_progress
PAY-1,2026-03-20,800.00,USD,ACME STEEL,partial,pay_vendor
RET-1,2026-03-15,500.00,USD,OWNER,10 percent hold,hold_retainage
WIP-1,2026-03-16,200.00,USD,ACME STEEL,capitalize,capitalize_wip
CASH-1,2026-03-21,100.00,USD,OWNER,collection,collect_receivable
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert_eq!(p.facts.len(), 4, "mapped kinds land; refused kinds do not: {:?}", p.rejected);
        assert_eq!(p.rejected.len(), 3, "{:?}", p.rejected);
        for r in &p.rejected {
            assert!(
                r.reason.contains("hold_retainage")
                    || r.reason.contains("capitalize_wip")
                    || r.reason.contains("collect_receivable"),
                "a refused row must name the kind it will not invent: {}",
                r.reason
            );
        }
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "vendor_invoice_site");
        assert_eq!(minor, 120_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "progress_bill");
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[2]).unwrap();
        assert_eq!(rule, "earn_progress");
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[3]).unwrap();
        assert_eq!(rule, "pay_vendor");
        // ⛔ PADDING IS `{field:<12}`, NOT A SLOGAN. A hand-spaced form that
        // is not `render()` is a third syntax, and the console prints this
        // string as Template.form.
        let form = t.render();
        assert_eq!(
            form,
            "\
template project-invoices {
  reads      csv with header
  grain      one invoice per row

  entity     vendor  (counterparty)
    by     name   from \"Vendor\"
    absent   pend

  fact       invoice
    reference   from \"InvoiceRef\"
    vendor      vendor
    dated       from \"Date\" as date \"YYYY-MM-DD\"
    amount      from \"Amount\" as money in \"Ccy\"
    memo        from \"Memo\" as text optional
    kind        from \"Kind\" as { cost: cost, cost_finishes: cost_finishes, cost_site: cost_site, cost_structure: cost_structure, earn_progress: earn_progress, invoice: invoice, invoice_finishes: invoice_finishes, invoice_site: invoice_site, invoice_structure: invoice_structure, pay_vendor: pay_vendor, progress_bill: progress_bill }

  posts      by \"kind\"
    amount      amount
    dated       dated
    cost        -> project_cost
    cost_finishes-> project_cost_finishes
    cost_site   -> project_cost_site
    cost_structure-> project_cost_structure
    earn_progress-> earn_progress
    invoice     -> vendor_invoice
    invoice_finishes-> vendor_invoice_finishes
    invoice_site-> vendor_invoice_site
    invoice_structure-> vendor_invoice_structure
    pay_vendor  -> pay_vendor
    progress_bill-> progress_bill
}
"
        );
        // An unidentified vendor pends — the same shape as a missing
        // instrument on the fund path. Adding it later clears without
        // re-ingesting.
        let resolved = ratio_ingest::resolve_all(&p.facts, &[]);
        assert!(resolved.iter().all(|r| !r.is_admissible()));
    }

    #[test]
    fn a_change_order_row_posts_the_phase_rule_and_not_a_float() {
        // ⭐ KIND NAMES THE WORK PACKAGE. Phase grain is the rule, not a
        // lump CO bucket and not a rewrite of [project] budget.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        let t = set.template("change-orders").unwrap();
        assert!(t.fact.posts.is_some());
        assert!(
            t.entities.is_empty(),
            "a change order is a chart dim, not an entity master"
        );
        let csv = "\
ChangeRef,Date,Amount,Ccy,Memo,Kind
CO-1,2026-03-15,5000.00,USD,extra footings,approve_co_site
CO-2,2026-04-01,1200.00,USD,,deduct_co_site
CO-3,2026-03-20,800.00,USD,allowance,approve_co
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        assert_eq!(
            p.facts[0].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 500_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "approve_co_site");
        assert_eq!(minor, 500_000);
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "deduct_co_site");
        assert_eq!(minor, 120_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[2]).unwrap();
        assert_eq!(rule, "approve_co");
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-03-15"));
        let form = t.render();
        assert_eq!(
            form,
            "\
template change-orders {
  reads      csv with header
  grain      one change per row

  fact       change
    reference   from \"ChangeRef\"
    dated       from \"Date\" as date \"YYYY-MM-DD\"
    amount      from \"Amount\" as money in \"Ccy\"
    memo        from \"Memo\" as text optional
    kind        from \"Kind\" as { approve_co: approve_co, approve_co_finishes: approve_co_finishes, approve_co_site: approve_co_site, approve_co_structure: approve_co_structure, deduct_co: deduct_co, deduct_co_finishes: deduct_co_finishes, deduct_co_site: deduct_co_site, deduct_co_structure: deduct_co_structure }

  posts      by \"kind\"
    amount      amount
    dated       dated
    approve_co  -> approve_co
    approve_co_finishes-> approve_co_finishes
    approve_co_site-> approve_co_site
    approve_co_structure-> approve_co_structure
    deduct_co   -> deduct_co
    deduct_co_finishes-> deduct_co_finishes
    deduct_co_site-> deduct_co_site
    deduct_co_structure-> deduct_co_structure
}
"
        );
        let resolved = ratio_ingest::resolve_all(&p.facts, &[]);
        assert!(resolved.iter().all(|r| r.is_admissible()));
    }

    #[test]
    fn a_purchase_order_row_posts_the_phase_rule_and_not_a_float() {
        // ⭐ KIND NAMES THE WORK PACKAGE. Phase grain is the rule, not a
        // second WBS and not a payable. An award is the memorandum; the
        // vendor invoice stays the actual.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Project)).unwrap();
        let t = set.template("purchase-orders").unwrap();
        assert!(t.fact.posts.is_some());
        assert!(
            t.entities.is_empty(),
            "a purchase order is a chart dim, not an entity master"
        );
        let csv = "\
PurchaseRef,Date,Amount,Ccy,Memo,Kind
PO-1,2026-03-15,3000.00,USD,site subcontract,award_commitment_site
PO-2,2026-04-01,800.00,USD,,release_commitment_site
PO-3,2026-03-20,500.00,USD,allowance,award_commitment
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        assert_eq!(
            p.facts[0].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 300_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "award_commitment_site");
        assert_eq!(minor, 300_000);
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "release_commitment_site");
        assert_eq!(minor, 80_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[2]).unwrap();
        assert_eq!(rule, "award_commitment");
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-03-15"));
        let form = t.render();
        assert_eq!(
            form,
            "\
template purchase-orders {
  reads      csv with header
  grain      one purchase per row

  fact       purchase
    reference   from \"PurchaseRef\"
    dated       from \"Date\" as date \"YYYY-MM-DD\"
    amount      from \"Amount\" as money in \"Ccy\"
    memo        from \"Memo\" as text optional
    kind        from \"Kind\" as { award_commitment: award_commitment, award_commitment_finishes: award_commitment_finishes, award_commitment_site: award_commitment_site, award_commitment_structure: award_commitment_structure, release_commitment: release_commitment, release_commitment_finishes: release_commitment_finishes, release_commitment_site: release_commitment_site, release_commitment_structure: release_commitment_structure }

  posts      by \"kind\"
    amount      amount
    dated       dated
    award_commitment-> award_commitment
    award_commitment_finishes-> award_commitment_finishes
    award_commitment_site-> award_commitment_site
    award_commitment_structure-> award_commitment_structure
    release_commitment-> release_commitment
    release_commitment_finishes-> release_commitment_finishes
    release_commitment_site-> release_commitment_site
    release_commitment_structure-> release_commitment_structure
}
"
        );
        let resolved = ratio_ingest::resolve_all(&p.facts, &[]);
        assert!(resolved.iter().all(|r| r.is_admissible()));
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
        let ids: Vec<&str> = set.templates.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "bank-statement",
                "loan-payment",
                "brokerage-statement",
                "brokerage-positions"
            ]
        );
        assert!(set.template("custodian-positions").is_none());
        assert!(set.template("prime_equity_trades").is_none());
        assert!(set.template("brokerage-statement").unwrap().fact.posts.is_some());
        assert!(set.template("brokerage-positions").unwrap().fact.posts.is_none());
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
    fn funded_capital_is_not_a_commitment_and_a_commitment_is_not_funded() {
        // ⛔ ENDING CAPITAL MUST NOT INCLUDE UNDRAWN. A commitment that
        // counted as money in would make NAV and partner capital tell
        // different stories about the same cash.
        assert!(is_capital_account("Partner capital — LP"));
        assert!(is_capital_account("Capital contributions"));
        assert!(!is_capital_account("Commitments — LP"));
        assert!(!is_capital_account("Undrawn commitments — GP"));
        assert!(!is_capital_account("Unrealized gain"));
        assert!(is_commitment_account("Commitments — LP"));
        assert!(is_commitment_account("Undrawn commitments — GP"));
        assert!(!is_commitment_account("Partner capital — LP"));
        assert!(!is_commitment_account("Capital contributions"));
        assert!(is_change_order_account("Approved change orders"));
        assert!(is_change_order_account(
            "Approved change orders — Site and mobilization"
        ));
        assert!(is_change_order_account(
            "Change-order authorization — Structure"
        ));
        assert!(!is_change_order_account("Funding"));
        assert!(!is_change_order_account("Project costs"));
        assert!(!is_change_order_account("Site and mobilization"));
        assert!(is_awarded_commitment_account("Awarded commitments"));
        assert!(is_awarded_commitment_account(
            "Awarded commitments — Site and mobilization"
        ));
        assert!(is_awarded_commitment_account(
            "Commitment authorization — Structure"
        ));
        assert!(!is_awarded_commitment_account("Funding"));
        assert!(!is_awarded_commitment_account("Payables"));
        assert!(!is_awarded_commitment_account("Site and mobilization"));
        assert!(
            !is_commitment_account("Awarded commitments"),
            "project awarded commitments are not the partner-capital pair"
        );
        assert!(!is_change_order_account("Awarded commitments"));
    }

    #[test]
    fn a_capital_call_row_posts_the_partner_rule_and_not_a_float() {
        // ⭐ KIND NAMES THE CLAIM. Partner grain is the rule, not a
        // fund-level bucket and not a CRM entity.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Investment)).unwrap();
        let t = set.template("capital-calls").unwrap();
        assert!(t.fact.posts.is_some());
        assert!(t.entities.is_empty(), "a partner is a chart dim, not an entity master");
        let csv = "\
CallRef,Date,Amount,Ccy,Kind
CC-1,2026-01-15,1000000.00,USD,commit_lp
CC-2,2026-03-01,250000.00,USD,call_lp
CC-3,2026-01-15,100000.00,USD,commit_gp
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        assert_eq!(
            p.facts[0].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 100_000_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "commit_lp");
        assert_eq!(minor, 100_000_000);
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "call_lp");
        assert_eq!(minor, 25_000_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[2]).unwrap();
        assert_eq!(rule, "commit_gp");
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-01-15"));
        let form = t.render();
        assert_eq!(
            form,
            "\
template capital-calls {
  reads      csv with header
  grain      one capital per row

  fact       capital
    reference   from \"CallRef\"
    dated       from \"Date\" as date \"YYYY-MM-DD\"
    amount      from \"Amount\" as money in \"Ccy\"
    kind        from \"Kind\" as { call_gp: call_gp, call_lp: call_lp, commit_gp: commit_gp, commit_lp: commit_lp }

  posts      by \"kind\"
    amount      amount
    dated       dated
    call_gp     -> call_gp
    call_lp     -> call_lp
    commit_gp   -> commit_gp
    commit_lp   -> commit_lp
}
"
        );
        // No entity to resolve — admissible without a master.
        let resolved = ratio_ingest::resolve_all(&p.facts, &[]);
        assert!(resolved.iter().all(|r| r.is_admissible()));
    }

    #[test]
    fn a_subscription_row_posts_units_and_not_a_money_only_contribute() {
        // ⭐ KIND NAMES THE CLAIM. Quantity is a measure on the same fact
        // as the cash. A row without Quantity is not this template —
        // contribute_* stays money-only. `Ratio.Partners.wellFormedMove`.
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Investment)).unwrap();
        let t = set.template("subscriptions").unwrap();
        assert!(t.fact.posts.is_some());
        assert!(t.entities.is_empty(), "a partner is a chart dim, not an entity master");
        let csv = "\
Ref,Date,Amount,Ccy,Quantity,Kind
SUB-1,2026-01-15,1000.00,USD,10,subscribe_lp
RED-1,2026-03-01,400.00,USD,4,redeem_lp
SUB-B,2026-01-20,500.00,USD,5,subscribe
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        assert_eq!(
            p.facts[0].values.get("quantity").and_then(ratio_ingest::Value::as_minor),
            Some(1_000),
            "10 units as decimal hundredths, not a float"
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "subscribe_lp");
        assert_eq!(minor, 100_000);
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "redeem_lp");
        assert_eq!(minor, 40_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[2]).unwrap();
        assert_eq!(rule, "subscribe");
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-01-15"));
        let form = t.render();
        // ⛔ ALIGNMENT IS THE RENDER, NOT A SLOGAN. `quantity` is 8 letters;
        // padding to the `reference` column is four spaces, not five.
        assert_eq!(
            form,
            "\
template subscriptions {
  reads      csv with header
  grain      one capital per row

  fact       capital
    reference   from \"Ref\"
    dated       from \"Date\" as date \"YYYY-MM-DD\"
    amount      from \"Amount\" as money in \"Ccy\"
    quantity    from \"Quantity\" as decimal
    kind        from \"Kind\" as { redeem: redeem, redeem_gp: redeem_gp, redeem_lp: redeem_lp, subscribe: subscribe, subscribe_gp: subscribe_gp, subscribe_lp: subscribe_lp }

  posts      by \"kind\"
    amount      amount
    dated       dated
    redeem      -> redeem
    redeem_gp   -> redeem_gp
    redeem_lp   -> redeem_lp
    subscribe   -> subscribe
    subscribe_gp-> subscribe_gp
    subscribe_lp-> subscribe_lp
}
"
        );
        let resolved = ratio_ingest::resolve_all(&p.facts, &[]);
        assert!(resolved.iter().all(|r| r.is_admissible()));
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
            "commit_lp",
            "commit_gp",
            "call_lp",
            "call_gp",
            "subscribe",
            "redeem",
            "subscribe_lp",
            "subscribe_gp",
            "redeem_lp",
            "redeem_gp",
        ] {
            let r = set.rule(id).unwrap_or_else(|| panic!("missing {id} in {text}"));
            assert!(
                r.legs.iter().all(|l| !l.per_instrument),
                "{id} is capital activity and must not open lots"
            );
        }
        for id in [
            "subscribe",
            "redeem",
            "subscribe_lp",
            "subscribe_gp",
            "redeem_lp",
            "redeem_gp",
        ] {
            let r = set.rule(id).expect(id);
            assert!(
                r.legs.iter().any(|l| l.measured),
                "{id} must carry units on the capital leg"
            );
            assert!(
                r.legs.iter().all(|l| !(l.measured && l.per_instrument)),
                "{id} must not open a lot"
            );
        }
        for id in ["contribute", "contribute_lp", "distribute_lp"] {
            let r = set.rule(id).expect(id);
            assert!(
                r.legs.iter().all(|l| !l.measured),
                "{id} is funded capital without unitization"
            );
        }
        // ⭐ THE TRADE CONTRACT #90 SEEDS. Capital activity is not a lot;
        // equity_purchase is, or a buy would not pick a holding.
        let purchase = set.rule("equity_purchase").expect("missing equity_purchase");
        assert!(
            purchase.legs.iter().any(|l| l.per_instrument),
            "equity_purchase must open lots"
        );
        assert!(set.rule("disposal_proceeds").is_some());
        let templates = ratio_ingest::TemplateSet::from_toml(&text).unwrap();
        assert!(templates.template("custodian-positions").is_some());
        assert!(templates.template("prime_equity_trades").is_some());
        assert!(templates.template("capital-calls").is_some());
        assert!(templates.template("subscriptions").is_some());
        let call = set.rule("call_lp").expect("missing call_lp");
        assert_eq!(
            call.legs.len(),
            4,
            "a call is cash + partner capital + the commitment draw, not two events"
        );
        let findings = ratio_rules::check(&set, &chart);
        assert!(
            findings.iter().all(|f| f.is_question),
            "seeded capital rules must balance against the chart: {findings:?}"
        );
        assert_eq!(set.partner_cut.len(), 2, "CreateBook(Investment) writes the cut");
        assert_eq!(set.partner_cut[0].partner, "LP");
        assert_eq!(set.partner_cut[0].weight, 80);
        assert_eq!(set.partner_cut[1].partner, "GP");
        assert_eq!(set.partner_cut[1].weight, 20);
        assert_ne!(
            set.partner_cut[0].weight, set.partner_cut[1].weight,
            "two partners is not 50/50"
        );
        assert!(
            set.special_allocations.is_empty(),
            "CreateBook writes no standing special"
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
        assert!(set.rule("progress_bill").is_some());
        assert!(set.rule("hold_retainage").is_some());
        assert!(set.rule("earn_progress").is_some());
        assert!(set.rule("project_cost_site").is_some());
        assert!(set.rule("approve_co_site").is_some());
        assert!(set.rule("deduct_co_site").is_some());
        assert!(set.rule("award_commitment_site").is_some());
        assert!(set.rule("release_commitment_site").is_some());
        assert!(
            set.rules.iter().all(|r| r.legs.iter().all(|l| !l.per_instrument)),
            "a project phase is an account, not an instrument — a per_instrument \
             leg would open a lot"
        );
        assert!(
            set.project.is_none(),
            "a new project has no baseline until someone sets [project] budget \
             or [[project.phase]]"
        );
        let against_empty = check(&set, &[]);
        assert!(
            against_empty.iter().any(|f| !f.is_question),
            "project rules must not check against an empty chart: {against_empty:?}"
        );
        // Wave 2 (#75) ingest mapping is additive. Change orders (#91) are a
        // second template, not a rewrite of project-invoices. Awarded
        // commitments (#104) are a third, not a rewrite of either.
        let ingest = ratio_ingest::TemplateSet::from_toml(PROJECT_CONFIG).unwrap();
        assert_eq!(ingest.templates.len(), 3);
        assert_eq!(ingest.templates[0].id, "project-invoices");
        assert_eq!(ingest.templates[1].id, "change-orders");
        assert_eq!(ingest.templates[2].id, "purchase-orders");
    }

    #[test]
    fn initialize_seeds_wip_and_progress_billing_and_no_project_budget() {
        let dir = std::env::temp_dir().join("ratio-book-init-project-rules");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "bridge", "Bridge", BookKind::Project).unwrap();
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().unwrap();
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = RuleSet::from_toml(&text).unwrap();
        assert!(set.rule("capitalize_wip").is_some(), "{text}");
        assert!(set.rule("progress_bill").is_some(), "{text}");
        assert!(set.rule("hold_retainage").is_some(), "{text}");
        assert!(set.rule("approve_co_site").is_some(), "{text}");
        assert!(set.rule("award_commitment_site").is_some(), "{text}");
        assert!(set.project.is_none());
        let ingest = ratio_ingest::TemplateSet::from_toml(&text).unwrap();
        assert_eq!(
            ingest.templates.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["project-invoices", "change-orders", "purchase-orders"]
        );
    }

    #[test]
    fn create_book_seeds_loan_rules_and_no_loan_schedule() {
        // ⭐ THE CLAIM #87 IS ABOUT. The posting pattern is on the chart;
        // which liabilities have a schedule is `[personal.loan]`, and a new
        // household has not named one. A silent zero here would put a
        // mortgage of nothing on a book that never declared one.
        let set = ratio_rules::RuleSet::from_toml(config_for(BookKind::Personal)).unwrap();
        assert!(
            set.personal.as_ref().map(|p| p.loan.is_empty()).unwrap_or(true),
            "a new household has no loan schedule until someone sets [personal.loan]"
        );
        assert!(set.rule("mortgage_principal").is_some());
        assert!(set.rule("mortgage_interest").is_some());
        assert!(set.rule("auto_principal").is_some());
        assert!(set.rule("student_principal").is_some());
    }

    #[test]
    fn a_loan_payment_conserves_interest_and_principal_against_cash() {
        // ⭐ TWO BALANCED TEMPLATES, ONE ENTRY. A varying split cannot be
        // one amount × weights; compiling interest at 200 and principal at
        // 800 and merging the cash legs is the 3-posting identity the
        // household payment is. `Ratio.Chart.balanced_template_balances`
        // applies to each; the concatenation of balanced vectors is
        // balanced; merging same-account legs does not change the net.
        let set = ratio_rules::RuleSet::from_toml(config_for(BookKind::Personal)).unwrap();
        let interest = ratio_rules::compile(
            set.rule("mortgage_interest").unwrap(),
            &ratio_rules::Event {
                rule: "mortgage_interest".into(),
                id: "m-1".into(),
                amount: 20_000,
                days: None,
                memo: String::new(),
                instrument: None,
                quantity: None,
            },
        )
        .unwrap();
        let principal = ratio_rules::compile(
            set.rule("mortgage_principal").unwrap(),
            &ratio_rules::Event {
                rule: "mortgage_principal".into(),
                id: "m-1".into(),
                amount: 80_000,
                days: None,
                memo: String::new(),
                instrument: None,
                quantity: None,
            },
        )
        .unwrap();
        let mut legs = interest;
        legs.extend(principal);
        let merged = ratio_rules::merge_postings(legs).unwrap();
        assert_eq!(merged.len(), 3, "{merged:?}");
        let net: i64 = merged.iter().map(|p| p.amount).sum();
        assert_eq!(net, 0, "interest + principal against cash must conserve");
        let by: std::collections::BTreeMap<i64, i64> =
            merged.iter().map(|p| (p.dim, p.amount)).collect();
        assert_eq!(by.get(&12), Some(&20_000), "interest expense");
        assert_eq!(by.get(&41), Some(&80_000), "principal reduction");
        assert_eq!(by.get(&1), Some(&-100_000), "cash out");
    }

    #[test]
    fn a_loan_payment_row_posts_principal_and_interest_as_one_fact() {
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Personal)).unwrap();
        let t = set.template("loan-payment").unwrap();
        assert!(t.check().is_empty(), "{:?}", t.check());
        let csv = "\
Ref,Date,Principal,Interest,Ccy,Loan,Lender,Memo
M-1,2026-03-01,800.00,200.00,USD,mortgage,FIRST NATIONAL,March mortgage
A-1,2026-03-01,350.00,45.00,USD,auto,ALLY,
S-1,2026-03-01,100.00,,USD,student,DEPT OF ED,principal only
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 3);
        let pairs = ratio_ingest::postings_for(t, &p.facts[0]).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("mortgage_principal".into(), 80_000),
                ("mortgage_interest".into(), 20_000),
            ]
        );
        let auto = ratio_ingest::postings_for(t, &p.facts[1]).unwrap();
        assert_eq!(
            auto,
            vec![
                ("auto_principal".into(), 35_000),
                ("auto_interest".into(), 4_500),
            ]
        );
        // Optional interest left blank: principal only, not a silent $0
        // interest posting.
        let student = ratio_ingest::postings_for(t, &p.facts[2]).unwrap();
        assert_eq!(student, vec![("student_principal".into(), 10_000)]);
    }

    #[test]
    fn initialize_writes_the_operating_chart_and_files_no_fund() {
        // ⭐ THE INDEPENDENCE CONTRACT FOR #108. An operating book is a Book,
        // not a Fund filing and not a WorkOS organization.
        let dir = std::env::temp_dir().join("ratio-book-init-operating");
        let _ = std::fs::remove_dir_all(&dir);
        initialize(&dir, "studio", "Studio", BookKind::Operating).unwrap();
        let m = BookMeta::load(&dir, "studio");
        assert_eq!(m.kind, BookKind::Operating);
        assert!(m.fund.is_none());
        assert!(m.organization.is_none());
        let chart = FileBook::open(&dir).unwrap().accounts().unwrap();
        assert_eq!(chart, chart_for(BookKind::Operating));
        let b = FileBook::open(&dir).unwrap();
        let digest = b.active().unwrap().unwrap();
        let text = String::from_utf8(b.get(&digest).unwrap()).unwrap();
        let set = RuleSet::from_toml(&text).unwrap();
        for id in [
            "invoice_customer",
            "collect_receivable",
            "vendor_bill",
            "pay_vendor",
            "receive_revenue",
            "pay_expense",
            "contribute_equity",
            "draw_equity",
        ] {
            let r = set.rule(id).unwrap_or_else(|| panic!("missing {id} in {text}"));
            assert!(
                r.legs.iter().all(|l| !l.per_instrument),
                "{id} is operating activity and must not open lots"
            );
        }
        let ingest = ratio_ingest::TemplateSet::from_toml(&text).unwrap();
        assert_eq!(
            ingest.templates.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["customer-invoices", "vendor-bills"]
        );
        let mut n = 0usize;
        b.for_each_entry_since(0, &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(n, 0, "CreateBook must not invent operating history");
    }

    #[test]
    fn operating_template_rules_check_against_its_chart() {
        let set = RuleSet::from_toml(OPERATING_CONFIG).unwrap();
        let findings = check(&set, &chart_for(BookKind::Operating));
        assert!(
            findings.iter().all(|f| f.is_question),
            "operating rules must check against chart_for(Operating): {findings:?}"
        );
        assert!(
            set.rules.iter().all(|r| r.legs.iter().all(|l| !l.per_instrument)),
            "an operating invoice is not an instrument — a per_instrument \
             leg would open a lot"
        );
        let against_empty = check(&set, &[]);
        assert!(
            against_empty.iter().any(|f| !f.is_question),
            "operating rules must not check against an empty chart: {against_empty:?}"
        );
        assert!(
            set.project.is_none(),
            "an operating book must not inherit a project budget"
        );
        assert!(
            set.personal.is_none(),
            "an operating book must not inherit a household budget"
        );
    }

    #[test]
    fn a_customer_invoice_row_posts_the_receivable_and_not_a_float() {
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Operating)).unwrap();
        let t = set.template("customer-invoices").unwrap();
        assert!(t.check().is_empty(), "{:?}", t.check());
        let csv = "\
InvoiceRef,Date,Amount,Ccy,Customer,Memo,Kind
INV-1,2026-03-01,1500.00,USD,ACME STUDIO,March retainer,invoice
INV-2,2026-03-15,1500.00,USD,ACME STUDIO,,collect
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 2);
        assert_eq!(
            p.facts[0].values.get("amount"),
            Some(&ratio_ingest::Value::Money {
                minor: 150_000,
                currency: "USD".into()
            }),
        );
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "invoice_customer");
        assert_eq!(minor, 150_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "collect_receivable");
        assert!(p.facts[0].values.get("due").is_none());
        assert!(
            p.facts[0].values.get("dueDate").is_none(),
            "a column the row omitted must stay absent, not a guessed due date"
        );
        assert!(p.facts[0].values.get("application").is_none());
        assert_eq!(ratio_ingest::dated_of(t, &p.facts[0]), Some("2026-03-01"));
        let form = t.render();
        assert!(form.contains("template customer-invoices {"), "{form}");
        assert!(form.contains("one invoice per row"), "{form}");
        assert!(form.contains("invoice     -> invoice_customer"), "{form}");
        assert!(form.contains("collect     -> collect_receivable"), "{form}");
        assert!(form.contains("dueDate"), "{form}");
        assert!(form.contains("optional"), "{form}");
        assert!(form.contains("application"), "{form}");
    }

    #[test]
    fn a_customer_invoice_row_reads_due_date_and_application_when_present() {
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Operating)).unwrap();
        let t = set.template("customer-invoices").unwrap();
        let csv = "\
InvoiceRef,Date,DueDate,Amount,Ccy,Customer,Memo,Kind,AppliesTo
INV-1,2026-03-01,2026-03-31,1500.00,USD,ACME STUDIO,March retainer,invoice,
COL-1,2026-03-15,,400.00,USD,ACME STUDIO,,collect,INV-1
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(
            p.facts[0].values.get("dueDate").and_then(ratio_ingest::Value::as_date),
            Some("2026-03-31")
        );
        assert!(p.facts[1].values.get("dueDate").is_none());
        assert_eq!(
            p.facts[1].values.get("application").and_then(ratio_ingest::Value::as_text),
            Some("INV-1")
        );
    }

    #[test]
    fn a_vendor_bill_row_posts_the_payable_and_not_a_float() {
        let set = ratio_ingest::TemplateSet::from_toml(config_for(BookKind::Operating)).unwrap();
        let t = set.template("vendor-bills").unwrap();
        assert!(t.check().is_empty(), "{:?}", t.check());
        let csv = "\
BillRef,Date,Amount,Ccy,Vendor,Memo,Kind
BILL-1,2026-03-02,240.00,USD,CITY POWER,electric,bill
BILL-2,2026-03-20,240.00,USD,CITY POWER,,pay
";
        let rows = ratio_ingest::extract_csv(csv).unwrap();
        let p = ratio_ingest::project(t, &sample_delivery(), &rows, "cfg");
        assert!(p.rejected.is_empty(), "{:?}", p.rejected);
        assert_eq!(p.facts.len(), 2);
        let (rule, minor) = ratio_ingest::posting_for(t, &p.facts[0]).unwrap();
        assert_eq!(rule, "vendor_bill");
        assert_eq!(minor, 24_000);
        let (rule, _) = ratio_ingest::posting_for(t, &p.facts[1]).unwrap();
        assert_eq!(rule, "pay_vendor");
        assert!(p.facts[0].values.get("due").is_none());
        assert!(p.facts[0].values.get("dueDate").is_none());
        let form = t.render();
        assert!(form.contains("template vendor-bills {"), "{form}");
        assert!(form.contains("one bill per row"), "{form}");
        assert!(form.contains("bill        -> vendor_bill"), "{form}");
        assert!(form.contains("pay         -> pay_vendor"), "{form}");
        assert!(form.contains("dueDate"), "{form}");
        assert!(form.contains("application"), "{form}");
    }

    #[test]
    fn unspecified_is_not_an_operating_kind() {
        // ⛔ KIND_UNSPECIFIED IS ABSENCE, NOT A HIDDEN BUSINESS TEMPLATE.
        assert!(BookKind::parse("UNSPECIFIED").is_err());
        assert!(BookKind::parse("unspecified").is_err());
        assert!(BookKind::from_proto(0).is_err());
        assert_eq!(BookKind::from_proto(4).unwrap(), BookKind::Operating);
        assert_eq!(BookKind::Operating.proto(), 4);
        assert_eq!(BookKind::Operating.as_str(), "operating");
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
