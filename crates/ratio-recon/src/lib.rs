//! ratio-recon — the shadow run. PLAN.md Stage 3.
//!
//! A prospect hands over a period's transactions and the positions their
//! incumbent reported. Ratio replays the transactions through a configuration
//! a human approved, compares, and produces a break report: every difference,
//! its cause, and the configuration hash that produced Ratio's figure.
//!
//! # The gate is the design
//!
//! PLAN.md: *"a fund with FX, corporate actions or tax lots will generate
//! false breaks and the engagement dies on the first call."* So coverage is a
//! **gate, not a best effort**. A file containing one row this crate does not
//! model produces no breaks at all — only exceptions, naming every such row
//! and what to do about it.
//!
//! That asymmetry is deliberate and it is the whole commercial argument. A
//! missed break costs a conversation. A false break costs the engagement: the
//! customer checks two, finds them spurious, and stops reading. Everything
//! here is arranged so Ratio cannot report a difference it created itself.
//!
//! # Why disposals need a basis
//!
//! A sale relieves the investment at **cost**, not at proceeds. Without tax
//! lots there is no way to know the cost, and relieving at proceeds instead
//! produces an entry that *balances* while leaving the investment figure
//! wrong — a break that reads as Ratio's fault and is. So a disposal without a
//! `basis` column is refused.
//!
//! Given a basis, no tax-lot engine is needed: the source system already chose
//! which lots to relieve, and taking that choice as input is correct for a
//! shadow run, which is trying to reproduce their books rather than replace
//! their decisions.
//!
//! # How a sale becomes two balanced events
//!
//! A sale posts three legs — cash `+proceeds`, investments `−basis`, realized
//! gain `−(proceeds − basis)` — and a `Rule` is a weight vector applied to one
//! amount, which cannot express two independent amounts. Rather than extend
//! the rule model, a sale is decomposed into two rule applications:
//!
//! ```text
//!   disposal_proceeds  at proceeds:  cash        +1,  realized_gain −1
//!   disposal_basis     at basis:     investments −1,  realized_gain +1
//!   ─────────────────────────────────────────────────────────────────
//!   sum:                             cash +P, investments −B, gain −(P−B)
//! ```
//!
//! Both halves conserve, so their sum does. This works because the ledger is a
//! monoidal fold over conserved vectors — the same property `Ratio.Core`
//! proves — and it is why the rule model did not need changing.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use ratio_rules::{compile, Event, RuleSet};
use ratio_proto::ratio::v1 as pb;
use ratio_store::{Account, Digest, JournalEntry};

/// What a run covers. Declared up front so the gate has something to check a
/// file against, and so a clean report is never read as a broader claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub label: String,
    pub base_currency: String,
    /// Transaction type → the rule that handles it. A type absent here is
    /// refused; a type present whose rule is missing from the configuration is
    /// also refused, because the scope would be claiming coverage the
    /// configuration cannot deliver.
    pub types: BTreeMap<String, TypeHandling>,
    pub exclusions: Vec<String>,
}

/// How one transaction type turns into rule applications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeHandling {
    /// One rule, applied at the row's amount.
    Simple { rule: String },
    /// A disposal: two rules, at proceeds and at basis. Refused without a
    /// basis — see the module docs.
    Disposal { proceeds_rule: String, basis_rule: String },
}

impl TypeHandling {
    fn rules(&self) -> Vec<&str> {
        match self {
            TypeHandling::Simple { rule } => vec![rule.as_str()],
            TypeHandling::Disposal { proceeds_rule, basis_rule } => {
                vec![proceeds_rule.as_str(), basis_rule.as_str()]
            }
        }
    }
}

/// The one scope this crate ships with: PLAN.md's "single currency, long-only
/// equities, plain trades, cash dividends, one management fee".
///
/// Deliberately concrete rather than configurable. PLAN.md: *"Pick the one the
/// first prospect actually has; do not build a framework for formats you have
/// not seen."* A second scope should be added when a second prospect exists,
/// by copying this and changing it — not by generalizing it in advance.
pub fn equity_long_only_single_ccy(base_currency: &str) -> Scope {
    let mut types = BTreeMap::new();
    types.insert(
        "buy".into(),
        TypeHandling::Simple { rule: "equity_purchase".into() },
    );
    types.insert(
        "sell".into(),
        TypeHandling::Disposal {
            proceeds_rule: "disposal_proceeds".into(),
            basis_rule: "disposal_basis".into(),
        },
    );
    types.insert(
        "dividend".into(),
        TypeHandling::Simple { rule: "cash_dividend".into() },
    );
    types.insert(
        "fee".into(),
        TypeHandling::Simple { rule: "management_fee_accrual".into() },
    );
    Scope {
        label: "equity-long-only-single-ccy".into(),
        base_currency: base_currency.to_string(),
        types,
        exclusions: vec![
            "foreign currency and any FX translation".into(),
            "corporate actions — splits, mergers, spin-offs, returns of capital".into(),
            "tax-lot selection — a disposal must carry the basis the source system relieved".into(),
            "derivatives, short positions, and securities lending".into(),
        ],
    }
}

/// Why a row was refused. Mirrors `ratio.v1.Refusal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    UnknownTransactionType,
    ForeignCurrency,
    DisposalWithoutBasis,
    NoRuleForType,
}

impl From<Refusal> for pb::Refusal {
    fn from(r: Refusal) -> Self {
        match r {
            Refusal::UnknownTransactionType => pb::Refusal::UnknownTransactionType,
            Refusal::ForeignCurrency => pb::Refusal::ForeignCurrency,
            Refusal::DisposalWithoutBasis => pb::Refusal::DisposalWithoutBasis,
            Refusal::NoRuleForType => pb::Refusal::NoRuleForType,
        }
    }
}

/// One row the run does not model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exception {
    pub transaction_id: String,
    pub refusal: Refusal,
    pub detail: String,
}

/// A transaction as it appears in the customer's file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Txn {
    pub id: String,
    pub date: String,
    pub kind: String,
    pub security: String,
    /// Minor units. Sign is the file's; the rule decides the posting's sign.
    pub amount: i64,
    /// Minor units. Present only for a disposal.
    pub basis: Option<i64>,
    pub currency: String,
    /// Accruals only: days in the period.
    pub days: Option<i64>,
}

/// What kind of difference a break is. Mirrors `ratio.v1.Cause`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    AmountDiffers,
    AbsentFromReport,
    AbsentFromRatio,
}

impl From<Cause> for pb::Cause {
    fn from(c: Cause) -> Self {
        match c {
            Cause::AmountDiffers => pb::Cause::AmountDiffers,
            Cause::AbsentFromReport => pb::Cause::AbsentFromReport,
            Cause::AbsentFromRatio => pb::Cause::AbsentFromRatio,
        }
    }
}

/// One difference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Break {
    pub account: i64,
    pub display_name: String,
    pub ratio_amount: i64,
    pub reported_amount: i64,
    pub difference: i64,
    pub cause: Cause,
    pub ratio_basis: String,
}

/// The result of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakReport {
    pub config_digest: String,
    pub scope: Scope,
    pub transactions_replayed: i64,
    pub entries_posted: i64,
    pub breaks: Vec<Break>,
    pub exceptions: Vec<Exception>,
    pub book_ties: bool,
}

impl BreakReport {
    /// True when the run produced a report rather than a refusal.
    pub fn was_reconciled(&self) -> bool {
        self.exceptions.is_empty()
    }
    /// True when it reconciled and found nothing — Stage 3's first "done when".
    pub fn is_clean(&self) -> bool {
        self.was_reconciled() && self.breaks.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Split one CSV line, honoring double-quoted fields.
///
/// Hand-rolled rather than pulled in, because the grammar that actually has to
/// be handled is small and adding a dependency to a proof-carrying repo is a
/// decision with a cost. What it does handle: quoted fields, commas inside
/// them, and `""` as an escaped quote. What it does not: embedded newlines
/// inside a quoted field. `parse_transactions` **rejects** a file whose quotes
/// do not close on their line rather than mis-splitting it, so the unhandled
/// case is an error and never a silent misread.
fn split_csv_line(line: &str) -> Result<Vec<String>> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cur.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => fields.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    if in_quotes {
        bail!("unterminated quote — a field containing a newline is not supported");
    }
    fields.push(cur);
    Ok(fields)
}

/// Read a decimal-major-unit amount as exact minor units.
///
/// `"-25,000.00"` → `-2_500_000`. Never touches a float: `"0.1"` has no exact
/// binary representation, and a cent lost in parsing is a break that no amount
/// of downstream exactness recovers.
pub use ratio_common::parse_minor;

/// Parse a transactions CSV.
///
/// Required columns: `id,date,type,amount,currency`. Optional: `security`,
/// `basis`, `days`. Columns are located by header name, not position — a
/// customer's export reorders columns between runs and positional parsing
/// would silently read the wrong ones.
pub fn parse_transactions(csv: &str) -> Result<Vec<Txn>> {
    let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().context("the transactions file is empty")?;
    let cols: Vec<String> = split_csv_line(header)?
        .into_iter()
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let idx = |name: &str| cols.iter().position(|c| c == name);
    let need = |name: &str| idx(name).with_context(|| format!("no `{name}` column"));

    let (i_id, i_date, i_type, i_amount, i_ccy) = (
        need("id")?,
        need("date")?,
        need("type")?,
        need("amount")?,
        need("currency")?,
    );
    let (i_sec, i_basis, i_days) = (idx("security"), idx("basis"), idx("days"));

    let mut out = Vec::new();
    for (n, line) in lines.enumerate() {
        let row = n + 2; // 1-based, after the header
        let f = split_csv_line(line).with_context(|| format!("row {row}"))?;
        let get = |i: usize| -> Result<&str> {
            f.get(i)
                .map(|s| s.trim())
                .with_context(|| format!("row {row} has {} fields, needs {}", f.len(), i + 1))
        };
        // An optional column that is present-but-blank is absent, not zero —
        // a blank basis must reach the gate as a missing basis and be refused.
        let opt = |i: Option<usize>| -> Option<&str> {
            i.and_then(|i| f.get(i)).map(|s| s.trim()).filter(|s| !s.is_empty())
        };
        out.push(Txn {
            id: get(i_id)?.to_string(),
            date: get(i_date)?.to_string(),
            kind: get(i_type)?.to_ascii_lowercase(),
            security: opt(i_sec).unwrap_or_default().to_string(),
            amount: parse_minor(get(i_amount)?).with_context(|| format!("row {row} amount"))?,
            basis: match opt(i_basis) {
                Some(b) => {
                    Some(parse_minor(b).with_context(|| format!("row {row} basis"))?)
                }
                None => None,
            },
            currency: get(i_ccy)?.to_ascii_uppercase(),
            days: match opt(i_days) {
                Some(d) => Some(d.parse().with_context(|| format!("row {row} days"))?),
                None => None,
            },
        });
    }
    Ok(out)
}

/// Parse a reported-positions CSV: `account,amount`, where `account` is a
/// dimension number or an account's display name.
pub fn parse_reported(csv: &str, chart: &[Account]) -> Result<BTreeMap<i64, i64>> {
    let by_name: BTreeMap<String, i64> = chart
        .iter()
        .map(|a| (a.display_name.trim().to_ascii_lowercase(), a.dim))
        .collect();

    let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().context("the positions file is empty")?;
    let cols: Vec<String> = split_csv_line(header)?
        .into_iter()
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let i_acct = cols
        .iter()
        .position(|c| c == "account")
        .context("no `account` column")?;
    let i_amt = cols
        .iter()
        .position(|c| c == "amount" || c == "balance")
        .context("no `amount` or `balance` column")?;

    let mut out = BTreeMap::new();
    for (n, line) in lines.enumerate() {
        let row = n + 2;
        let f = split_csv_line(line).with_context(|| format!("row {row}"))?;
        let key = f.get(i_acct).map(|s| s.trim()).unwrap_or("");
        let amt = f
            .get(i_amt)
            .map(|s| s.trim())
            .with_context(|| format!("row {row} has no amount"))?;
        let dim = match key.parse::<i64>() {
            Ok(d) => d,
            Err(_) => *by_name
                .get(&key.to_ascii_lowercase())
                .with_context(|| format!("row {row}: no account named `{key}` in the chart"))?,
        };
        let amount = parse_minor(amt).with_context(|| format!("row {row} amount"))?;
        // A duplicate account in the report is ambiguous, and picking one
        // would produce a break the customer cannot reproduce.
        if out.insert(dim, amount).is_some() {
            bail!("row {row}: account {dim} appears twice in the positions file");
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// Check every row against the scope and the active configuration.
///
/// Returns the events to replay, or every reason the file cannot be. **Both
/// are never returned**: one exception means no events, because a partial
/// replay compared against a whole period's positions produces breaks for
/// everything the run skipped.
pub fn cover(txns: &[Txn], scope: &Scope, set: &RuleSet) -> Result<Vec<Event>, Vec<Exception>> {
    let mut events = Vec::new();
    let mut exceptions = Vec::new();

    for t in txns {
        if t.currency != scope.base_currency {
            exceptions.push(Exception {
                transaction_id: t.id.clone(),
                refusal: Refusal::ForeignCurrency,
                detail: format!(
                    "{} is in {}, and this run covers {} only. Modeling it needs an FX rate \
                     and a translation policy; guessing either produces breaks that are \
                     Ratio's fault, not yours.",
                    t.id, t.currency, scope.base_currency
                ),
            });
            continue;
        }

        let Some(handling) = scope.types.get(&t.kind) else {
            exceptions.push(Exception {
                transaction_id: t.id.clone(),
                refusal: Refusal::UnknownTransactionType,
                detail: format!(
                    "`{}` is not a transaction type this run covers. It handles: {}.",
                    t.kind,
                    scope.types.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            });
            continue;
        };

        // The scope may name a rule the approved configuration does not carry.
        // That is a configuration gap, not a data problem, and it is refused
        // rather than skipped so it cannot look like a clean run.
        let missing: Vec<&str> = handling
            .rules()
            .into_iter()
            .filter(|r| set.rule(r).is_none())
            .collect();
        if !missing.is_empty() {
            exceptions.push(Exception {
                transaction_id: t.id.clone(),
                refusal: Refusal::NoRuleForType,
                detail: format!(
                    "`{}` needs rule(s) {} which the active configuration does not have. \
                     Approve them before running.",
                    t.kind,
                    missing.join(", ")
                ),
            });
            continue;
        }

        match handling {
            TypeHandling::Simple { rule } => events.push(Event {
                rule: rule.clone(),
                id: t.id.clone(),
                amount: t.amount.abs(),
                days: t.days,
                memo: memo(t), instrument: None, quantity: None }),
            TypeHandling::Disposal { proceeds_rule, basis_rule } => {
                let Some(basis) = t.basis else {
                    exceptions.push(Exception {
                        transaction_id: t.id.clone(),
                        refusal: Refusal::DisposalWithoutBasis,
                        detail: format!(
                            "{} is a disposal with no basis. Relieving the investment at \
                             proceeds instead would balance the entry and still leave the \
                             investment figure wrong. Add the cost your system relieved.",
                            t.id
                        ),
                    });
                    continue;
                };
                // Two balanced halves; see the module docs.
                events.push(Event {
                    rule: proceeds_rule.clone(),
                    id: format!("{}-proceeds", t.id),
                    amount: t.amount.abs(),
                    days: None,
                    memo: memo(t), instrument: None, quantity: None });
                events.push(Event {
                    rule: basis_rule.clone(),
                    id: format!("{}-basis", t.id),
                    amount: basis.abs(),
                    days: None,
                    memo: memo(t), instrument: None, quantity: None });
            }
        }
    }

    if exceptions.is_empty() {
        Ok(events)
    } else {
        Err(exceptions)
    }
}

fn memo(t: &Txn) -> String {
    if t.security.is_empty() {
        format!("{} {}", t.date, t.kind)
    } else {
        format!("{} {} {}", t.date, t.kind, t.security)
    }
}

// ---------------------------------------------------------------------------
// Replay and compare
// ---------------------------------------------------------------------------

/// Turn covered events into journal entries under one configuration.
pub fn replay(events: &[Event], set: &RuleSet, config: &Digest) -> Result<Vec<JournalEntry>> {
    let mut entries = Vec::new();
    for e in events {
        let rule = set
            .rule(&e.rule)
            .with_context(|| format!("no rule `{}` — the gate should have caught this", e.rule))?;
        entries.push(JournalEntry {
            id: e.id.clone(),
            memo: e.memo.clone(),
            config: config.clone(),
            postings: compile(rule, e).with_context(|| format!("compiling {}", e.id))?,
        
            announcement: None,
        });
    }
    Ok(entries)
}

/// Net balance per dimension across entries.
pub fn balances(entries: &[JournalEntry]) -> BTreeMap<i64, i64> {
    let mut out: BTreeMap<i64, i64> = BTreeMap::new();
    for e in entries {
        for p in &e.postings {
            *out.entry(p.dim).or_default() += p.amount;
        }
    }
    out
}

/// Compare Ratio's balances to the reported ones.
///
/// A zero balance Ratio produced is **not** a break when the report omits the
/// account: "no position" and "a position of zero" are the same statement, and
/// reporting them as a difference is exactly the kind of false break that ends
/// a shadow run.
pub fn compare(
    ratio: &BTreeMap<i64, i64>,
    reported: &BTreeMap<i64, i64>,
    chart: &[Account],
    entries: &[JournalEntry],
) -> Vec<Break> {
    let names: BTreeMap<i64, &str> = chart
        .iter()
        .map(|a| (a.dim, a.display_name.as_str()))
        .collect();
    let name = |d: i64| names.get(&d).map(|s| s.to_string()).unwrap_or_else(|| format!("dim {d}"));

    // How many postings produced each figure, so a break says where to look.
    let mut posting_count: BTreeMap<i64, usize> = BTreeMap::new();
    for e in entries {
        for p in &e.postings {
            *posting_count.entry(p.dim).or_default() += 1;
        }
    }
    let basis = |d: i64| {
        format!(
            "{} posting(s) under this configuration",
            posting_count.get(&d).copied().unwrap_or(0)
        )
    };

    let dims: BTreeSet<i64> = ratio.keys().chain(reported.keys()).copied().collect();
    let mut breaks = Vec::new();
    for d in dims {
        let r = ratio.get(&d).copied();
        let p = reported.get(&d).copied();
        let (ratio_amount, reported_amount, cause) = match (r, p) {
            (Some(r), Some(p)) => {
                if r == p {
                    continue;
                }
                (r, p, Cause::AmountDiffers)
            }
            (Some(r), None) => {
                if r == 0 {
                    continue; // no position and a zero position agree
                }
                (r, 0, Cause::AbsentFromReport)
            }
            (None, Some(p)) => {
                if p == 0 {
                    continue;
                }
                (0, p, Cause::AbsentFromRatio)
            }
            (None, None) => continue,
        };
        breaks.push(Break {
            account: d,
            display_name: name(d),
            ratio_amount,
            reported_amount,
            difference: ratio_amount - reported_amount,
            cause,
            ratio_basis: basis(d),
        });
    }
    // Largest absolute difference first: the one worth arguing about is rarely
    // the one with the lowest account number.
    breaks.sort_by_key(|b| (-(b.difference.abs()), b.account));
    breaks
}

/// Run the whole thing.
#[allow(clippy::too_many_arguments)]
pub fn reconcile(
    txns: &[Txn],
    reported: &BTreeMap<i64, i64>,
    scope: &Scope,
    set: &RuleSet,
    chart: &[Account],
    config: &Digest,
) -> Result<BreakReport> {
    let events = match cover(txns, scope, set) {
        Ok(e) => e,
        Err(exceptions) => {
            return Ok(BreakReport {
                config_digest: config.as_str().to_string(),
                scope: scope.clone(),
                transactions_replayed: 0,
                entries_posted: 0,
                breaks: Vec::new(),
                exceptions,
                book_ties: true,
            })
        }
    };
    let entries = replay(&events, set, config)?;
    let ratio = balances(&entries);
    // Every entry conserves, so the sum over all dimensions is zero. This
    // cannot be false for entries the kernel compiled; it is asserted because
    // a false value would mean a defect in Ratio rather than a break.
    let book_ties = ratio.values().sum::<i64>() == 0;

    Ok(BreakReport {
        config_digest: config.as_str().to_string(),
        scope: scope.clone(),
        transactions_replayed: txns.len() as i64,
        entries_posted: entries.len() as i64,
        breaks: compare(&ratio, reported, chart, &entries),
        exceptions: Vec::new(),
        book_ties,
    })
}

/// The same run, also handing back the entries it replayed.
///
/// `reconcile` drops them because a comparison does not need them; a caller
/// that wants to keep the shadow book uses this instead of replaying twice.
/// Returns no entries when the file was refused — there is nothing to post.
pub fn reconcile_with_entries(
    txns: &[Txn],
    reported: &BTreeMap<i64, i64>,
    scope: &Scope,
    set: &RuleSet,
    chart: &[Account],
    config: &Digest,
) -> Result<(BreakReport, Vec<JournalEntry>)> {
    let events = match cover(txns, scope, set) {
        Ok(e) => e,
        Err(exceptions) => {
            return Ok((
                BreakReport {
                    config_digest: config.as_str().to_string(),
                    scope: scope.clone(),
                    transactions_replayed: 0,
                    entries_posted: 0,
                    breaks: Vec::new(),
                    exceptions,
                    book_ties: true,
                },
                Vec::new(),
            ))
        }
    };
    let entries = replay(&events, set, config)?;
    let ratio = balances(&entries);
    let report = BreakReport {
        config_digest: config.as_str().to_string(),
        scope: scope.clone(),
        transactions_replayed: txns.len() as i64,
        entries_posted: entries.len() as i64,
        breaks: compare(&ratio, reported, chart, &entries),
        exceptions: Vec::new(),
        book_ties: ratio.values().sum::<i64>() == 0,
    };
    Ok((report, entries))
}

// ---------------------------------------------------------------------------
// The wire form
// ---------------------------------------------------------------------------

/// Convert to the `ratio.v1.BreakReport` contract.
///
/// The report is the artifact the customer reads, keeps, and argues with, so
/// it leaves this process as a proto rather than as an ad-hoc struct. Going
/// through the generated types is also what keeps the two enums honest: a
/// value added to `Refusal` here fails to compile until `recon.proto` has it.
impl BreakReport {
    pub fn to_proto(&self, book: &str, id: &str) -> pb::BreakReport {
        pb::BreakReport {
            name: format!("books/{book}/breakReports/{id}"),
            config_digest: self.config_digest.clone(),
            scope: Some(pb::Scope {
                label: self.scope.label.clone(),
                base_currency: self.scope.base_currency.clone(),
                transaction_types: self.scope.types.keys().cloned().collect(),
                exclusions: self.scope.exclusions.clone(),
            }),
            transactions_replayed: self.transactions_replayed,
            entries_posted: self.entries_posted,
            breaks: self
                .breaks
                .iter()
                .map(|b| pb::BreakLine {
                    account: b.account,
                    display_name: b.display_name.clone(),
                    ratio_amount: b.ratio_amount,
                    reported_amount: b.reported_amount,
                    difference: b.difference,
                    cause: pb::Cause::from(b.cause) as i32,
                    ratio_basis: b.ratio_basis.clone(),
                })
                .collect(),
            exceptions: self
                .exceptions
                .iter()
                .map(|e| pb::Exception {
                    transaction_id: e.transaction_id.clone(),
                    refusal: pb::Refusal::from(e.refusal) as i32,
                    detail: e.detail.clone(),
                })
                .collect(),
            book_ties: self.book_ties,
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn minor(v: i64) -> String {
    let neg = v < 0;
    let a = v.unsigned_abs();
    let s = format!("{}.{:02}", a / 100, a % 100);
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

/// Render a report for a person to read.
pub fn render(r: &BreakReport) -> String {
    let mut out = String::new();

    if !r.was_reconciled() {
        out.push_str("NOT RECONCILED\n\n");
        out.push_str(&format!(
            "This file contains {} row(s) outside what this run covers, so no\n\
             comparison was made. A partial replay compared against a whole\n\
             period's positions reports a break for everything it skipped, and\n\
             those breaks would be Ratio's fault rather than yours.\n\n",
            r.exceptions.len()
        ));
        for e in &r.exceptions {
            out.push_str(&format!("  {}\n    {}\n\n", e.transaction_id, e.detail));
        }
        out.push_str(&format!("scope  {}\n", r.scope.label));
        for x in &r.scope.exclusions {
            out.push_str(&format!("  excludes  {x}\n"));
        }
        return out;
    }

    out.push_str(if r.is_clean() {
        "RECONCILED — no differences\n\n"
    } else {
        "BREAKS\n\n"
    });

    if !r.breaks.is_empty() {
        out.push_str(&format!(
            "{:<32}{:>14}{:>14}{:>14}  {}\n",
            "ACCOUNT", "RATIO", "REPORTED", "DIFFERENCE", "CAUSE"
        ));
        for b in &r.breaks {
            out.push_str(&format!(
                "{:<32}{:>14}{:>14}{:>14}  {}\n",
                b.display_name,
                minor(b.ratio_amount),
                minor(b.reported_amount),
                minor(b.difference),
                match b.cause {
                    Cause::AmountDiffers => "figures differ",
                    Cause::AbsentFromReport => "not in the report",
                    Cause::AbsentFromRatio => "Ratio produced nothing",
                }
            ));
            out.push_str(&format!("{:<32}  from {}\n", "", b.ratio_basis));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "{} transaction(s) replayed into {} entrie(s)\n",
        r.transactions_replayed, r.entries_posted
    ));
    out.push_str(&format!("book ties  {}\n", if r.book_ties { "yes" } else { "NO" }));
    out.push_str(&format!("config     {}\n", r.config_digest));
    out.push_str(&format!("scope      {}\n", r.scope.label));
    for x in &r.scope.exclusions {
        out.push_str(&format!("  excludes {x}\n"));
    }
    out.push_str(
        "\nEvery Ratio figure above was produced by that configuration, and\n\
         re-running it reproduces them exactly. Add `--post` to write the\n\
         entries into the book, then `ratio explain <account>` reads back the\n\
         postings behind any one figure.\n",
    );
    out
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
            acct(20, "Dividend income", A::Income),
            acct(30, "Realized gain on investments", A::Income),
            acct(40, "Management fee payable", A::Liability),
        ]
    }
    fn acct(dim: i64, name: &str, account_type: A) -> Account {
        Account { dim, display_name: name.into(), account_type }
    }

    /// The configuration a human would have approved for this scope.
    const RULES: &str = r#"
[[rule]]
id = "equity_purchase"
kind = "trade"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "disposal_proceeds"
kind = "trade"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 30
weight = -1

[[rule]]
id = "disposal_basis"
kind = "trade"
[[rule.posting]]
account = 30
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "cash_dividend"
kind = "dividend"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 20
weight = -1

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

    fn rules() -> RuleSet {
        RuleSet::from_toml(RULES).unwrap()
    }
    fn digest() -> Digest {
        Digest::of(RULES.as_bytes())
    }
    fn scope() -> Scope {
        equity_long_only_single_ccy("USD")
    }

    // -- parsing -----------------------------------------------------------

    #[test]
    fn amounts_are_exact_and_never_touch_a_float() {
        assert_eq!(parse_minor("0.10").unwrap(), 10);
        assert_eq!(parse_minor("0.1").unwrap(), 10);
        assert_eq!(parse_minor("25000.00").unwrap(), 2_500_000);
        assert_eq!(parse_minor("-25,000.00").unwrap(), -2_500_000);
        assert_eq!(parse_minor("$1,204,880.11").unwrap(), 120_488_011);
        assert_eq!(parse_minor("7").unwrap(), 700);
        assert_eq!(parse_minor(".5").unwrap(), 50);
        // Three decimals is a different unit, not a rounding opportunity.
        assert!(parse_minor("1.005").is_err());
        assert!(parse_minor("abc").is_err());
        assert!(parse_minor("").is_err());
    }

    #[test]
    fn a_quoted_field_containing_a_comma_is_one_field() {
        let f = split_csv_line(r#"a,"b,c",d"#).unwrap();
        assert_eq!(f, vec!["a", "b,c", "d"]);
        let f = split_csv_line(r#""say ""hi""",x"#).unwrap();
        assert_eq!(f, vec![r#"say "hi""#, "x"]);
    }

    #[test]
    fn an_unterminated_quote_is_an_error_not_a_misread() {
        // The alternative — splitting anyway — silently shifts every later
        // column by one and produces breaks nobody can explain.
        assert!(split_csv_line(r#"a,"b,c"#).is_err());
    }

    #[test]
    fn columns_are_found_by_name_not_position() {
        let a = "id,date,type,amount,currency\nt1,2026-01-02,buy,100.00,USD\n";
        let b = "currency,amount,type,date,id\nUSD,100.00,buy,2026-01-02,t1\n";
        assert_eq!(parse_transactions(a).unwrap(), parse_transactions(b).unwrap());
    }

    #[test]
    fn a_blank_optional_column_is_absent_not_zero() {
        // A blank basis must reach the gate as missing so it is refused; read
        // as zero it would relieve the investment at nothing and tie anyway.
        let t = parse_transactions(
            "id,date,type,amount,currency,basis\nt1,2026-01-02,sell,100.00,USD,\n",
        )
        .unwrap();
        assert_eq!(t[0].basis, None);
    }

    #[test]
    fn reported_positions_accept_a_name_or_a_dimension() {
        let by_dim = parse_reported("account,amount\n2,100.00\n", &chart()).unwrap();
        let by_name =
            parse_reported("account,amount\nCash and equivalents,100.00\n", &chart()).unwrap();
        assert_eq!(by_dim, by_name);
        assert_eq!(by_dim[&2], 10_000);
    }

    #[test]
    fn a_duplicated_account_in_the_report_is_an_error() {
        let r = parse_reported("account,amount\n2,100.00\n2,50.00\n", &chart());
        assert!(r.is_err(), "picking one would produce an unreproducible break");
    }

    // -- the gate ----------------------------------------------------------

    #[test]
    fn a_foreign_currency_row_refuses_the_whole_file() {
        let txns = parse_transactions(
            "id,date,type,amount,currency\n\
             t1,2026-01-02,buy,100.00,USD\n\
             t2,2026-01-03,buy,100.00,GBP\n",
        )
        .unwrap();
        let e = cover(&txns, &scope(), &rules()).unwrap_err();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].refusal, Refusal::ForeignCurrency);
        assert_eq!(e[0].transaction_id, "t2");
    }

    #[test]
    fn one_bad_row_yields_no_events_at_all() {
        // The load-bearing property. Replaying the good rows and comparing
        // against a whole period's positions manufactures a break for every
        // row that was skipped.
        let txns = parse_transactions(
            "id,date,type,amount,currency\n\
             t1,2026-01-02,buy,100.00,USD\n\
             t2,2026-01-03,merger,100.00,USD\n",
        )
        .unwrap();
        assert!(cover(&txns, &scope(), &rules()).is_err());

        let r = reconcile(&txns, &BTreeMap::new(), &scope(), &rules(), &chart(), &digest()).unwrap();
        assert!(!r.was_reconciled());
        assert_eq!(r.transactions_replayed, 0, "nothing may be replayed");
        assert!(r.breaks.is_empty(), "a refused file must report no breaks");
    }

    #[test]
    fn a_corporate_action_is_named_rather_than_guessed_at() {
        let txns =
            parse_transactions("id,date,type,amount,currency\nt1,2026-01-02,split,0.00,USD\n")
                .unwrap();
        let e = cover(&txns, &scope(), &rules()).unwrap_err();
        assert_eq!(e[0].refusal, Refusal::UnknownTransactionType);
        assert!(e[0].detail.contains("split"), "{}", e[0].detail);
        assert!(e[0].detail.contains("buy"), "the refusal should say what it does cover");
    }

    #[test]
    fn a_disposal_without_a_basis_is_refused() {
        let txns = parse_transactions(
            "id,date,type,amount,currency\nt1,2026-01-02,sell,100.00,USD\n",
        )
        .unwrap();
        let e = cover(&txns, &scope(), &rules()).unwrap_err();
        assert_eq!(e[0].refusal, Refusal::DisposalWithoutBasis);
        assert!(e[0].detail.contains("basis"), "{}", e[0].detail);
    }

    #[test]
    fn a_scope_naming_a_rule_the_configuration_lacks_is_refused() {
        // The scope claims coverage the approved configuration cannot deliver.
        // Skipping the row would look like a clean run over a smaller file.
        let thin = RuleSet::from_toml(
            "[[rule]]\nid = \"cash_dividend\"\nkind = \"dividend\"\n\
             [[rule.posting]]\naccount = 2\nweight = 1\n\
             [[rule.posting]]\naccount = 20\nweight = -1\n",
        )
        .unwrap();
        let txns =
            parse_transactions("id,date,type,amount,currency\nt1,2026-01-02,buy,100.00,USD\n")
                .unwrap();
        let e = cover(&txns, &scope(), &thin).unwrap_err();
        assert_eq!(e[0].refusal, Refusal::NoRuleForType);
        assert!(e[0].detail.contains("equity_purchase"), "{}", e[0].detail);
    }

    // -- replay ------------------------------------------------------------

    #[test]
    fn a_disposal_becomes_two_balanced_events_that_sum_correctly() {
        // Sold for 150.00 what cost 100.00: cash +150, investments -100,
        // realized gain -50 (a credit; gains are revenue).
        let txns = parse_transactions(
            "id,date,type,amount,currency,basis\nt1,2026-02-01,sell,150.00,USD,100.00\n",
        )
        .unwrap();
        let events = cover(&txns, &scope(), &rules()).unwrap();
        assert_eq!(events.len(), 2, "a disposal is two rule applications");

        let entries = replay(&events, &rules(), &digest()).unwrap();
        // Each half conserves on its own, which is what makes the sum conserve.
        for e in &entries {
            assert_eq!(e.postings.iter().map(|p| p.amount).sum::<i64>(), 0, "{:?}", e.id);
        }
        let b = balances(&entries);
        assert_eq!(b[&2], 15_000, "cash");
        assert_eq!(b[&1], -10_000, "investments relieved at cost, not proceeds");
        assert_eq!(b[&30], -5_000, "realized gain");
        assert_eq!(b.values().sum::<i64>(), 0);
    }

    #[test]
    fn a_period_that_agrees_reconciles_to_nothing() {
        let txns = parse_transactions(
            "id,date,type,security,amount,currency,basis\n\
             t1,2026-01-02,buy,VTI,25000.00,USD,\n\
             t2,2026-01-15,dividend,VTI,120.00,USD,\n\
             t3,2026-02-01,sell,VTI,15000.00,USD,10000.00\n",
        )
        .unwrap();
        let reported = parse_reported(
            "account,amount\n\
             Investments at fair value,15000.00\n\
             Cash and equivalents,-9880.00\n\
             Dividend income,-120.00\n\
             Realized gain on investments,-5000.00\n",
            &chart(),
        )
        .unwrap();

        let r = reconcile(&txns, &reported, &scope(), &rules(), &chart(), &digest()).unwrap();
        assert!(r.was_reconciled());
        assert!(r.is_clean(), "{}", render(&r));
        assert!(r.book_ties);
        assert_eq!(r.transactions_replayed, 3);
        assert_eq!(r.entries_posted, 4, "the disposal contributes two");
    }

    #[test]
    fn a_disagreement_is_reported_with_its_cause_and_the_config() {
        let txns = parse_transactions(
            "id,date,type,amount,currency\nt1,2026-01-02,buy,25000.00,USD\n",
        )
        .unwrap();
        // The report says the fund holds 24,000 where Ratio computes 25,000.
        let reported = parse_reported(
            "account,amount\n\
             Investments at fair value,24000.00\n\
             Cash and equivalents,-25000.00\n",
            &chart(),
        )
        .unwrap();

        let r = reconcile(&txns, &reported, &scope(), &rules(), &chart(), &digest()).unwrap();
        assert_eq!(r.breaks.len(), 1, "{}", render(&r));
        let b = &r.breaks[0];
        assert_eq!(b.account, 1);
        assert_eq!(b.ratio_amount, 2_500_000);
        assert_eq!(b.reported_amount, 2_400_000);
        assert_eq!(b.difference, 100_000);
        assert_eq!(b.cause, Cause::AmountDiffers);
        assert_eq!(r.config_digest, digest().as_str());

        let text = render(&r);
        assert!(text.contains("1000.00"), "{text}");
        assert!(text.contains(digest().as_str()), "the report must cite the config");
    }

    #[test]
    fn a_zero_balance_absent_from_the_report_is_not_a_break() {
        // "No position" and "a position of zero" are the same statement.
        // Reporting them as a difference is the false break that ends a run.
        let ratio = BTreeMap::from([(1, 0i64), (2, 500i64)]);
        let reported = BTreeMap::from([(2, 500i64)]);
        assert!(compare(&ratio, &reported, &chart(), &[]).is_empty());
    }

    #[test]
    fn an_account_only_the_report_carries_is_a_break() {
        let ratio = BTreeMap::new();
        let reported = BTreeMap::from([(30, 12_345i64)]);
        let b = compare(&ratio, &reported, &chart(), &[]);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].cause, Cause::AbsentFromRatio);
        assert_eq!(b[0].difference, -12_345);
    }

    #[test]
    fn breaks_are_ordered_by_how_much_they_matter() {
        let ratio = BTreeMap::from([(1, 100i64), (2, 10_000i64), (30, -50i64)]);
        let reported = BTreeMap::from([(1, 0i64), (2, 0i64), (30, 0i64)]);
        let b = compare(&ratio, &reported, &chart(), &[]);
        let diffs: Vec<i64> = b.iter().map(|x| x.difference.abs()).collect();
        assert_eq!(diffs, vec![10_000, 100, 50]);
    }

    // -- rendering ---------------------------------------------------------

    #[test]
    fn a_refusal_reads_as_a_refusal_and_never_as_a_clean_run() {
        let txns =
            parse_transactions("id,date,type,amount,currency\nt1,2026-01-02,merger,1.00,USD\n")
                .unwrap();
        let r = reconcile(&txns, &BTreeMap::new(), &scope(), &rules(), &chart(), &digest()).unwrap();
        let text = render(&r);
        assert!(text.starts_with("NOT RECONCILED"), "{text}");
        assert!(!text.contains("RECONCILED — no differences"), "{text}");
        assert!(text.contains("merger") || text.contains("not a transaction type"), "{text}");
    }

    // -- the wire form -----------------------------------------------------

    #[test]
    fn every_refusal_and_cause_maps_to_a_named_proto_value() {
        // Not a tautology: `pb::Refusal` is generated from recon.proto, so a
        // variant added here without a matching proto value fails to compile,
        // and a mismatched pairing shows up as a wrong number below.
        for (r, want) in [
            (Refusal::UnknownTransactionType, 1),
            (Refusal::ForeignCurrency, 2),
            (Refusal::DisposalWithoutBasis, 3),
            (Refusal::NoRuleForType, 4),
        ] {
            assert_eq!(pb::Refusal::from(r) as i32, want, "{r:?}");
        }
        for (c, want) in [
            (Cause::AmountDiffers, 1),
            (Cause::AbsentFromReport, 2),
            (Cause::AbsentFromRatio, 3),
        ] {
            assert_eq!(pb::Cause::from(c) as i32, want, "{c:?}");
        }
        // And nothing maps onto the UNSPECIFIED zero, which would make a real
        // refusal indistinguishable from an unset field on the wire.
        assert_ne!(pb::Refusal::from(Refusal::UnknownTransactionType) as i32, 0);
        assert_ne!(pb::Cause::from(Cause::AmountDiffers) as i32, 0);
    }

    #[test]
    fn a_report_survives_the_wire_intact() {
        use prost::Message;

        let txns = parse_transactions(
            "id,date,type,amount,currency\nt1,2026-01-02,buy,25000.00,USD\n",
        )
        .unwrap();
        let reported = parse_reported(
            "account,amount\nInvestments at fair value,24000.00\n\
             Cash and equivalents,-25000.00\n",
            &chart(),
        )
        .unwrap();
        let r = reconcile(&txns, &reported, &scope(), &rules(), &chart(), &digest()).unwrap();

        let msg = r.to_proto("demo", "run-1");
        let bytes = msg.encode_to_vec();
        let back = pb::BreakReport::decode(&bytes[..]).unwrap();

        assert_eq!(back.name, "books/demo/breakReports/run-1");
        assert_eq!(back.config_digest, digest().as_str());
        assert_eq!(back.breaks.len(), 1);
        assert_eq!(back.breaks[0].difference, 100_000);
        assert_eq!(back.breaks[0].cause, pb::Cause::AmountDiffers as i32);
        assert_eq!(back.scope.as_ref().unwrap().base_currency, "USD");
        // The scope's exclusions must survive: a report read off the wire
        // without them is a narrow result that looks like a broad one.
        assert!(!back.scope.unwrap().exclusions.is_empty());
        assert!(back.book_ties);
    }

    #[test]
    fn a_refusal_crosses_the_wire_as_a_refusal() {
        use prost::Message;

        let txns = parse_transactions(
            "id,date,type,amount,currency\nt1,2026-01-02,merger,1.00,USD\n",
        )
        .unwrap();
        let r = reconcile(&txns, &BTreeMap::new(), &scope(), &rules(), &chart(), &digest()).unwrap();
        let back =
            pb::BreakReport::decode(&r.to_proto("demo", "run-2").encode_to_vec()[..]).unwrap();

        assert!(back.breaks.is_empty(), "a refused run must carry no breaks");
        assert_eq!(back.exceptions.len(), 1);
        assert_eq!(
            back.exceptions[0].refusal,
            pb::Refusal::UnknownTransactionType as i32
        );
        assert_eq!(back.transactions_replayed, 0);
    }

    #[test]
    fn a_clean_report_still_states_what_it_did_not_cover() {
        // A clean result over a narrow scope must not read as a broad claim.
        let txns =
            parse_transactions("id,date,type,amount,currency\nt1,2026-01-02,buy,100.00,USD\n")
                .unwrap();
        let reported = BTreeMap::from([(1, 10_000i64), (2, -10_000i64)]);
        let r = reconcile(&txns, &reported, &scope(), &rules(), &chart(), &digest()).unwrap();
        assert!(r.is_clean(), "{}", render(&r));
        let text = render(&r);
        assert!(text.contains("excludes"), "{text}");
        assert!(text.contains("corporate actions"), "{text}");
    }
}
