//! ratio-console — the operations console's backend-for-frontend.
//!
//! Implements `ratio.v1.Console` over real books. Nothing here is fixture data:
//! a fund is a book on disk, its NAV is a fold over that book's accounts, its
//! breaks come from the break report `ratio recon --post` stored, and its
//! change log comes from what `ratio approve` recorded.
//!
//! # Why the shape differs from the domain
//!
//! `Chart` and `Ledger` speak in accounts and transactions. A fund controller
//! on a NAV day speaks in funds that are blocked or struck and breaks ordered
//! by money. Assembling the second from the first in a browser would mean a
//! fan-out of calls over a link whose latency the console does not control,
//! reassembled in a language where the reassembly is untyped. So it happens
//! here, once, next to the data.
//!
//! # A fund is a directory
//!
//! `root/<id>/` is one fund's book. A root that is itself a book — it has an
//! `accounts.json` — is treated as the single fund `demo`, so the deployed
//! demo works unchanged and grows a fund list the moment a second book appears
//! beside it.

pub mod auth;
pub mod book;
pub mod transcode;

pub use auth::Subject;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use prost::Message;
use ratio_proto::ratio::console::v1 as pb;
// The kernel's own contracts. A shadow run writes a `ratio.v1.BreakReport`,
// which the console reads and translates into its own `Break` — two packages
// because they are two APIs, and this is the seam between them.
use ratio_proto::ratio::v1 as kernel;
use ratio_rules::RuleSet;
use ratio_store::{AccountTypeRecord, ConfigStore, FileBook, Journal, Plane};

/// What stands between a fund and a NAV.
///
/// Two lists rather than one count, because they are cleared by different
/// people doing different things — an unexplained blocking break wants
/// somebody's judgment, a pending fact wants a record in the master — and a
/// refusal that said only "3 things" would send an operator looking for the
/// wrong three.
pub struct Blocking {
    /// Unexplained breaks the tolerance grades as blocking.
    pub breaks: Vec<pb::Break>,
    /// Facts read out of a delivery that do not resolve yet.
    pub pending: Vec<pb::PendingFact>,
}

impl Blocking {
    pub fn is_empty(&self) -> bool {
        self.breaks.is_empty() && self.pending.is_empty()
    }
}

/// One record in `explanations.jsonl`: why somebody decided a difference was
/// acceptable.
///
/// ⛔ APPEND-ONLY, NEWEST WINS, AND A CORRECTION IS A NEW RECORD. The same law
/// as every other plane here. An explanation somebody later thought better of
/// is part of what happened on that fund, and editing it away would leave the
/// change log saying a person accepted something that is no longer there.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BreakExplanation {
    /// The break's id WITHIN THIS BOOK — the account dimension, or `lot-{n}`.
    ///
    /// ⛔ NOT THE RESOURCE NAME, AND THE DIFFERENCE COST A DEMO. A break is
    /// named `funds/{fund}/breaks/{id}`, and the fund half is a property of how
    /// the book is being SERVED, not of the book: the same directory is fund
    /// `demo` on loopback and `pennington-select-income` under a funds root.
    /// Keying a note by the full name meant one written by the seeder never
    /// matched the break the console showed — the explanation on disk, the
    /// break on screen, and nothing connecting them.
    pub break_id: String,
    pub text: String,
    /// ⛔ THE VERIFIED SUBJECT, never anything a caller sent.
    pub actor: String,
    pub accept_time: i64,
    /// The figure this was written about. Half the currency test.
    pub difference: i64,
    /// The terms it was graded under. The other half.
    pub config_digest: String,
    /// What the accepter had in front of them. Audit, not currency.
    pub journal_position: u64,
    pub journal_digest: String,
}

/// The books this console serves.
pub struct Console {
    root: PathBuf,
    /// How many entries a book may hold before `ApplyEvent` refuses to add
    /// another. `None` is no ceiling.
    ///
    /// Read from the environment ONCE, here, rather than inside the handler.
    /// A handler that reads a process-global on every call cannot be tested
    /// without mutating one, and `std::env::set_var` in a test is a race
    /// against every other test in the binary — which is exactly what it was
    /// when this lived in `apply_event`.
    max_entries: Option<usize>,

    /// One projection per fund, kept across requests and brought up to date
    /// rather than rebuilt.
    ///
    /// ⛔ THE CACHE IS THE POINT, NOT AN OPTIMIZATION ON TOP OF ONE.
    /// `Projection::of_book` folds the whole journal — 546 ms on a
    /// 140,000-entry book and growing with every trade ever made. `follow`
    /// seeks to where it stopped, so an unchanged book costs a stat and a seek.
    /// Without something holding the projection between calls, the incremental
    /// read has nowhere to be incremental.
    ///
    /// ⚠ A `Mutex` and not a `RefCell`: `Console` is handed to a server that
    /// may serve two requests at once, and the failure of getting that wrong is
    /// a panic in production rather than a compile error here.
    ///
    /// ⚠ AND STALENESS IS SAFE, which is why this needs no invalidation
    /// protocol. Every read returns `AsOf` carrying the prefix it folded, so a
    /// figure built from a lagging projection cites the lagging prefix and
    /// replays from it: `//tla:projection_check`. A cache that had to be
    /// correct about freshness would be a cache that could be wrong about it.
    projections: std::sync::Mutex<BTreeMap<String, ratio_project::Projection>>,

    /// The funds the caller may open, resolved ONCE from `MEMBERSHIP.tsv` at
    /// construction so `book_path` is a set lookup rather than a file read per
    /// handler. `None` means unrestricted — the `Local` identity, which is not a
    /// tenant. `Some(set)` restricts, and an empty set is a real, valid answer
    /// (the operator is a member of nothing) rather than a refusal.
    allowed: Option<BTreeSet<String>>,

    /// Who a write on this console is attributed to. A `Member`'s stable id, or
    /// `RATIO_ACTOR` for the `Local` CLI/loopback console — resolved from the
    /// subject once at construction, NEVER from a request body. `None` is a
    /// genuine absence (a local run with no `RATIO_ACTOR`), recorded honestly as
    /// an empty actor rather than a made-up one.
    actor: Option<String>,
}

impl Console {
    /// The console as the CLI and loopback surfaces use it: `Subject::Local`,
    /// which sees every book. `ratio watch --book`, `ratio-mcp` and the tests
    /// all reach the data this way, unchanged by the tenancy work.
    pub fn new(root: impl AsRef<Path>) -> Self {
        // The CLI/loopback console attributes writes to RATIO_ACTOR, the same
        // string `ratio approve` and `ratio strike` record. Not authentication —
        // there is none on a loopback surface — but the honest local identity.
        Self::build(root.as_ref().to_path_buf(), None, std::env::var("RATIO_ACTOR").ok())
    }

    /// The same console, attributing writes to a name the caller has already
    /// resolved.
    ///
    /// ⛔ FOR THE CLI ONLY, AND IT IS NOT A WAY TO SET AN ACTOR ON THE NETWORK.
    /// `ratio accept` resolves `RATIO_ACTOR` ‖ `USER` ‖ `operator` before it
    /// gets here, which is one fallback more than [`Console::new`] does, and
    /// recording an empty actor for somebody who has a name is a worse audit
    /// trail than the one this exists to write. The authenticated constructors
    /// take their subject from the gateway and nothing may override it — see
    /// `record_change`, where the actor is `self.actor` and never a request
    /// body.
    pub fn as_actor(mut self, name: &str) -> Self {
        if !name.is_empty() {
            self.actor = Some(name.to_string());
        }
        self
    }

    /// The console scoped to an authenticated subject: it sees only the funds
    /// `MEMBERSHIP.tsv` grants them, enforced at `book_path`. This is the only
    /// constructor the network server uses; the membership set is resolved here,
    /// once, from the funds root.
    pub fn scoped(root: impl AsRef<Path>, subject: Subject) -> Self {
        let root = root.as_ref().to_path_buf();
        let allowed = match &subject {
            Subject::Local => None,
            member => Some(auth::funds_for(&root, member)),
        };
        // A Member's writes are signed with their verified id; a Local scoped
        // console (unusual) falls back to RATIO_ACTOR like `new`.
        let actor = match subject.actor() {
            Some(a) => Some(a.to_string()),
            None => std::env::var("RATIO_ACTOR").ok(),
        };
        Self::build(root, allowed, actor)
    }

    /// The console for an OPEN, shared demo: any authenticated subject sees
    /// every fund, while a write is still signed with their verified identity.
    ///
    /// ⛔ NOT THE TENANT PATH, AND DELIBERATELY SEPARATE FROM `scoped`. A demo
    /// whose audience is not known ahead of time cannot be an allow-list of
    /// emails; instead every signed-in caller is granted every fund
    /// (`allowed = None`, exactly as `Local` is) while the subject's id is kept
    /// as the actor — so "anyone who signs in sees the demo" costs nothing in
    /// attribution and, crucially, nothing in the tenancy code: `funds_for`,
    /// `book_path`'s membership check and their tests are untouched. The server
    /// selects this only when `RATIO_DEMO_OPEN` is set; every real deployment
    /// scopes. Sign-in is still required — this changes what an authenticated
    /// caller may see, not whether one is needed.
    pub fn open(root: impl AsRef<Path>, subject: Subject) -> Self {
        let root = root.as_ref().to_path_buf();
        let actor = match subject.actor() {
            Some(a) => Some(a.to_string()),
            None => std::env::var("RATIO_ACTOR").ok(),
        };
        Self::build(root, None, actor)
    }

    fn build(root: PathBuf, allowed: Option<BTreeSet<String>>, actor: Option<String>) -> Self {
        Console {
            root,
            max_entries: std::env::var("RATIO_MAX_API_ENTRIES")
                .ok()
                .and_then(|v| v.parse().ok()),
            projections: Default::default(),
            allowed,
            actor,
        }
    }

    /// Append one line to the fund's audit log — who did what, when, to what,
    /// under which configuration — as tab-separated
    /// `epoch \t actor \t action \t subject \t config_digest`.
    ///
    /// ⛔ THE ACTOR IS `self.actor`, THE VERIFIED SUBJECT, NEVER THE REQUEST
    /// BODY. Attribution to a name the caller chose is attribution to nobody,
    /// and the product is that a figure is attributable to a named person. An
    /// absent actor is recorded as empty rather than invented — an audit trail
    /// that makes up a name is worse than one that admits a gap.
    fn record_change(&self, book: &Path, action: &str, subject: &str, digest: &str) -> Result<()> {
        use std::io::Write;
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // A tab or newline in any field would split the record. The actor is a
        // Cognito id or email and the rest are ids/digests, none of which carry
        // either, but a stray control character is dropped rather than trusted.
        let clean = |s: &str| -> String {
            s.chars().filter(|c| *c != '\t' && *c != '\n' && *c != '\r').collect()
        };
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(book.join("CHANGELOG"))
            .context("opening CHANGELOG")?;
        writeln!(
            f,
            "{when}\t{}\t{}\t{}\t{}",
            clean(self.actor.as_deref().unwrap_or("")),
            clean(action),
            clean(subject),
            clean(digest)
        )
        .context("appending to CHANGELOG")?;
        Ok(())
    }

    /// This fund's projection, brought up to date with whatever has been
    /// appended since it was last read.
    ///
    /// Returns a clone so the lock is not held across a caller's work. The
    /// clone is of the folded TOTALS, not of the journal — a chart, not a
    /// history.
    pub fn projection(&self, fund: &str) -> Result<ratio_project::Projection> {
        let path = self.book_path(fund)?;
        let mut cache = self
            .projections
            .lock()
            .map_err(|_| anyhow::anyhow!("the projection cache was poisoned by a panic"))?;
        let p = cache.entry(fund.to_string()).or_default();
        // ⛔ If the journal was REPLACED rather than appended to, `follow`
        // refuses — an append-only log does not shrink, so a shorter file at
        // this path is a different book. Start again rather than splice two
        // histories together.
        if p.follow(&path).is_err() {
            *p = ratio_project::Projection::of_book(&path)?;
        }
        Ok(p.clone())
    }

    /// The same console with an explicit ceiling, for a caller that has one —
    /// and for a test that must not reach for a process-global to set it.
    pub fn with_max_entries(mut self, max: Option<usize>) -> Self {
        self.max_entries = max;
        self
    }

    /// Every book directory the subject may see, in a stable order.
    ///
    /// Sorted by id rather than by anything derived, so the list does not
    /// reorder under an operator between two glances at the same screen.
    fn listed_ids(&self) -> Result<Vec<String>> {
        let mut ids: Vec<String> = if self.root.join("accounts.json").is_file() {
            vec!["demo".to_string()]
        } else {
            let mut v: Vec<String> = std::fs::read_dir(&self.root)
                .with_context(|| format!("reading {}", self.root.display()))?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().join("accounts.json").is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            v.sort();
            v
        };
        // ⛔ WHAT THE CALLER MAY SEE, not what is on disk. An operator restricted
        // to some books gets exactly those; an operator restricted to NONE gets
        // an empty list — a valid answer, not a refusal, and the two must not
        // look alike. `book_path` re-guards each id a list then reads, so this
        // filter is the visible half of a boundary the storage layer enforces
        // regardless of it.
        if let Some(allowed) = &self.allowed {
            ids.retain(|id| allowed.contains(id));
        }
        Ok(ids)
    }

    fn book_ids(&self) -> Result<Vec<String>> {
        self.listed_ids()
    }

    /// Books that carry a fund layer — a missing sidecar (legacy) or an explicit
    /// `fund` in `book.toml`. An independent book CreateBook wrote is absent.
    fn fund_ids(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for id in self.listed_ids()? {
            let path = if self.root.join("accounts.json").is_file() {
                self.root.clone()
            } else {
                self.root.join(&id)
            };
            if book::BookMeta::load(&path, &id).fund.is_some() {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    fn book_path(&self, id: &str) -> Result<PathBuf> {
        // The id reaches the filesystem, and this service is behind a public
        // endpoint. Anything that is not a fund id is refused before it is
        // joined to a path.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!("{id:?} is not a fund id");
        }
        // ⛔ TENANCY IS ENFORCED HERE, at the one place a fund id becomes a
        // path — not in the handlers, which are one forgotten call away from
        // bypassing it. A fund the caller may not see is refused with the SAME
        // error as a fund that does not exist, and BEFORE the filesystem is
        // touched, so a caller scoped to one fund can neither read another's
        // book nor learn that it exists by watching which denial they get.
        // `Local` (the CLI) is unrestricted; a person at a terminal is not a
        // tenant. `allowed` is `None` for it and `Some(set)` for a `Member`.
        if let Some(allowed) = &self.allowed {
            if !allowed.contains(id) {
                bail!("no fund {id:?}");
            }
        }
        if self.root.join("accounts.json").is_file() {
            if id != "demo" {
                bail!("no fund {id:?}");
            }
            return Ok(self.root.clone());
        }
        let p = self.root.join(id);
        if !p.join("accounts.json").is_file() {
            bail!("no fund {id:?}");
        }
        Ok(p)
    }

    // ── Console methods ───────────────────────────────────────────────────

    /// The trial balance: every account in the chart with what it holds.
    ///
    /// The chart is the source of the rows, not the journal — an account with
    /// no postings is a real row with a zero on it, and dropping it would make
    /// an empty chart and a complete one look the same. `filter=posted` is
    /// there for when you want only what moved.
    ///
    /// A month (`YYYY-MM`) or a year (`YYYY`) is a suffix on the chip, not a
    /// second List field (AIP-132). `pnl-2026-03` is March; bare `pnl` is
    /// refused — cumulative income is the ABOR-shaped view this filter exists
    /// to refuse as a default.
    pub fn list_accounts(
        &self,
        parent: &str,
        filter: &str,
    ) -> Result<pb::ListAccountsResponse> {
        let (fund, view) = view_scoped_parent(parent)?;
        let (kind, period) = list_accounts_window(filter);
        if kind == "pnl" && period.is_empty() {
            bail!("a period P&L needs a month (YYYY-MM) or a year (YYYY)");
        }
        let fold = match (kind, period) {
            ("pnl", p) => AccountFold::Activity(parse_period(p)?),
            (_, p) if !p.is_empty() => AccountFold::AsOf(parse_period(p)?),
            _ => AccountFold::Current,
        };
        let accounts = self.accounts_folded(&fund, &view, fold)?;
        let keep: Vec<pb::Account> = accounts
            .into_iter()
            .filter(|a| match kind {
                "posted" => a.posting_count != "0",
                "abnormal" => a.abnormal,
                "pnl" => {
                    a.r#type == pb::account::Type::Revenue as i32
                        || a.r#type == pb::account::Type::Expense as i32
                }
                "sheet" => {
                    a.r#type == pb::account::Type::Asset as i32
                        || a.r#type == pb::account::Type::Liability as i32
                        || a.r#type == pb::account::Type::Equity as i32
                        || a.r#type == pb::account::Type::Revenue as i32
                        || a.r#type == pb::account::Type::Expense as i32
                }
                _ => true,
            })
            .collect();
        Ok(pb::ListAccountsResponse { accounts: keep, next_page_token: String::new() })
    }

    pub fn get_account(&self, name: &str) -> Result<pb::Account> {
        let (fund, view, dim) = view_scoped_id(name, "accounts")?;
        self.accounts_of(&fund, &view)?
            .into_iter()
            .find(|a| a.dimension == dim)
            .with_context(|| format!("no account {dim:?} in {fund}"))
    }

    /// Every account in a fund's chart, with its debit and credit totals.
    fn accounts_of(&self, fund: &str, view: &str) -> Result<Vec<pb::Account>> {
        self.accounts_folded(fund, view, AccountFold::Current)
    }

    fn accounts_folded(
        &self,
        fund: &str,
        view: &str,
        fold: AccountFold,
    ) -> Result<Vec<pb::Account>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        // ⛔ TRANSLATED INTO THE FUND'S CURRENCY, because a `pb::Account` is
        // ONE ROW PER DIMENSION and the fold underneath is one row per
        // (dimension, currency). Summing the pairs raw is precisely the flat
        // total that reported a NAV of 133,915,377.28 where the answer was
        // 134,439,187.51 — so the denominations are converted, not merged.
        //
        // ⚠ AND THE UNTRANSLATED SPLIT IS CARRIED ALONGSIDE, in
        // `Account.currency_totals`. The translated figure is a judgment about
        // a rate; the denominations are a fact, and a reader checking the first
        // needs the second. Every other surface shows the split, and the one
        // screen a customer actually looks at was the one that could not.
        let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
        let raw = match &fold {
            AccountFold::Current => {
                // ⛔ OFF THE MAINTAINED FOLD, PER VIEW — NOT `b.balances_by_dim()`.
                // The file's answer sums the whole journal, which is exactly one
                // view's answer wearing no label: a settlement view's trial
                // balance excludes what it has not recognised, and until this
                // read moved, every view's accounts screen showed the recorded
                // figures under its own name.
                let proj = self.projection(fund)?;
                let balances = proj.balances(view)?;
                balances
                    .value
                    .iter()
                    .map(|((dim, ccy), row)| {
                        (*dim, ccy.as_deref().map(str::to_string), row.debit, row.credit, row.postings)
                    })
                    .collect::<Vec<_>>()
            }
            AccountFold::AsOf(w) | AccountFold::Activity(w) => {
                // ⚠ A SECOND WALK, NOT A SECOND FOLD. The maintained projection
                // has no term for a calendar window, so a period figure has to
                // re-read the journal. It skips exactly what `recognised`
                // skips, so a settlement view's January P&L is still that
                // view's January — not the recorded view's wearing a date.
                //
                // ⛔ AN ENTRY WITH NO DATE HAS NO PERIOD. Same refusal as a
                // lot with no acquisition date: defaulting to the epoch or to
                // today would put it in a window nobody elected, and both
                // defaults are wrong in a direction that looks ordinary.
                let activity = matches!(fold, AccountFold::Activity(_));
                let proj = self.projection(fund)?;
                let mut rows: BTreeMap<(i64, Option<String>), (i128, i128, i64)> = BTreeMap::new();
                b.for_each_entry_since(0, &mut |entry| {
                    if !proj.recognised(view, entry)? {
                        return Ok(());
                    }
                    let Some(day) = entry.trade_date.as_deref() else {
                        return Ok(());
                    };
                    if activity {
                        if day < w.start.as_str() || day > w.end.as_str() {
                            return Ok(());
                        }
                    } else if day > w.end.as_str() {
                        return Ok(());
                    }
                    for p in &entry.postings {
                        let amount = p.amount as i128;
                        let slot = rows.entry((p.dim, p.currency.clone())).or_default();
                        slot.2 += 1;
                        if amount >= 0 {
                            slot.0 += amount;
                        } else {
                            slot.1 += -amount;
                        }
                    }
                    Ok(())
                })?;
                rows.into_iter()
                    .map(|((dim, ccy), (d, c, n))| (dim, ccy, d, c, n))
                    .collect()
            }
        };

        let mut totals: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
        let mut split: BTreeMap<i64, Vec<pb::CurrencyTotal>> = BTreeMap::new();
        let mut counts: BTreeMap<i64, i64> = BTreeMap::new();
        for (dim, ccy, debit, credit, postings) in raw {
            let ccy_ref = ccy.as_deref();
            let factor = rates.factor_of_optional(ccy_ref).with_context(|| {
                format!(
                    "this fund holds {} and no rate for it was supplied — an account total \
                     mixing denominations is not a total",
                    ccy_ref.unwrap_or("an untyped balance")
                )
            })? as i128;
            let s = totals.entry(dim).or_insert((0, 0));
            let scale = ratio_project::RATE_SCALE as i128;
            s.0 += (debit * factor / scale) as i64;
            s.1 += (credit * factor / scale) as i64;
            *counts.entry(dim).or_default() += postings;
            split.entry(dim).or_default().push(pb::CurrencyTotal {
                currency_code: ccy_ref.unwrap_or("").to_string(),
                debit: debit.to_string(),
                credit: credit.to_string(),
                balance: (debit - credit).to_string(),
                // ⛔ EMPTY FOR THE BASE AND FOR AN UNTYPED LEG, not "100".
                // Both translate at par, and both do so WITHOUT a rate fact —
                // printing a rate nobody recorded would invent the evidence the
                // column exists to supply.
                rate: match ccy_ref {
                    Some(c) if c != FUND_CURRENCY => factor.to_string(),
                    _ => String::new(),
                },
            });
        }

        Ok(b.accounts()?
            .into_iter()
            .map(|a| {
                let (debit, credit) = totals.get(&a.dim).copied().unwrap_or((0, 0));
                let balance = debit - credit;
                pb::Account {
                    name: format!("funds/{fund}/views/{view}/accounts/{}", a.dim),
                    display_name: a.display_name,
                    dimension: a.dim.to_string(),
                    r#type: account_type(a.account_type) as i32,
                    debit: debit.to_string(),
                    credit: credit.to_string(),
                    balance: balance.to_string(),
                    // The proved classification, not a second copy of it:
                    // `is_normal_side` is emitted from the Lean that proves
                    // one theorem per account type.
                    abnormal: !ratio_chart::is_normal_side(a.account_type.into(), balance),
                    posting_count: counts.get(&a.dim).copied().unwrap_or(0).to_string(),
                    // ⚠ EMPTY WHEN THERE IS ONE DENOMINATION, because a split
                    // into one row adds nothing and a screen that renders it
                    // anyway says "USD 100 (of which USD 100)".
                    currency_totals: match split.remove(&a.dim) {
                        Some(v) if v.len() > 1 => v,
                        _ => Vec::new(),
                    },
                }
            })
            .collect())
    }

    /// Every posting on one account, in journal order, each with the balance
    /// after it.
    pub fn list_postings(&self, parent: &str) -> Result<pb::ListPostingsResponse> {
        let (fund, view, dim_str) = view_scoped_id(parent, "accounts")?;
        // ⛔ TENANCY BEFORE THE DIMENSION PARSE. A caller who may not see this
        // fund is refused here — not after we have judged whether their account
        // id was well-formed. The denial must not depend on the caller's input,
        // or "no fund" and "not a dimension" tell an outsider which is which.
        let path = self.book_path(&fund)?;
        let dim: i64 = dim_str.parse().with_context(|| format!("{dim_str:?} is not a dimension"))?;
        let b = FileBook::open(&path)?;
        let proj = self.projection(&fund)?;

        let mut running = 0i64;
        let mut out = Vec::new();
        b.for_each_entry_since(0, &mut |entry| {
            // ⛔ THE VIEW'S ENTRIES, DECIDED BY THE FOLD'S OWN RULE. The fold
            // keeps totals, not history, so this walk reads the journal — and
            // it must skip exactly what the view has not recognised, or the
            // rows and the account total they claim to sum to disagree.
            // `Projection::recognised` is the fold's decision, not a second
            // spelling of it.
            if !proj.recognised(&view, entry)? {
                return Ok(());
            }
            // `leg` counts within the entry, so an entry that touches the same
            // account twice yields two citable postings rather than one name
            // for two lines.
            for (leg, p) in entry.postings.iter().enumerate() {
                if p.dim != dim {
                    continue;
                }
                running += p.amount;
                out.push(pb::Posting {
                    name: format!("funds/{fund}/views/{view}/accounts/{dim}/postings/{}.{leg}", entry.id),
                    entry_id: entry.id.clone(),
                    memo: entry.memo.clone(),
                    amount: p.amount.to_string(),
                    running_balance: running.to_string(),
                    config_digest: entry.config.as_str().to_string(),
                });
            }
            Ok(())
        })?;
        Ok(pb::ListPostingsResponse { postings: out, next_page_token: String::new() })
    }

    /// Every journal entry, in the order the journal holds them.
    ///
    /// ⛔ UNBOUNDED. `next_page_token` is declared and left empty, like every
    /// other list on this service. A book with millions of entries is the
    /// twenty-million-lot problem; this is the citation surface, not a scan
    /// of that book.
    pub fn list_entries(&self, parent: &str) -> Result<pb::ListEntriesResponse> {
        let fund = resource_id(parent, "funds")?;
        // ⛔ TENANCY BEFORE THE WALK. Same reason as GetEntry.
        let path = self.book_path(&fund)?;
        let b = FileBook::open(&path)?;
        let chart = b.accounts()?;
        let default_view = self.default_view_of(&fund)?;
        let mut out = Vec::new();
        b.for_each_entry_since(0, &mut |e| {
            out.push(Self::entry_of(&fund, &default_view, &chart, e));
            Ok(())
        })?;
        Ok(pb::ListEntriesResponse {
            entries: out,
            next_page_token: String::new(),
        })
    }

    /// One journal entry, and the postings the rule produced.
    ///
    /// ⭐ THE CITATION HOP. A posting names this id; this is what makes that
    /// name a URL rather than plain text. Same `pb::Entry` ApplyEvent returns,
    /// built by the same helper, so a commit and a later Get cannot disagree
    /// about the resource they both claim to be.
    pub fn get_entry(&self, name: &str) -> Result<pb::Entry> {
        let (fund, id) = nested_id(name, "funds", "entries")?;
        // ⛔ TENANCY BEFORE THE LOOKUP. A caller who may not see this book is
        // refused here — not after we have said whether the entry exists. The
        // denial must not depend on the caller's id, or "no fund" and "no
        // entry" tell an outsider which is which.
        let path = self.book_path(&fund)?;
        let b = FileBook::open(&path)?;
        let chart = b.accounts()?;
        let default_view = self.default_view_of(&fund)?;
        let mut found = None;
        b.for_each_entry_since(0, &mut |e| {
            if e.id == id {
                found = Some(e.clone());
            }
            Ok(())
        })?;
        let entry = found.with_context(|| format!("no entry {id:?} in this journal"))?;
        Ok(Self::entry_of(&fund, &default_view, &chart, &entry))
    }

    /// The wire `Entry` for one journal row — ApplyEvent and GetEntry share it.
    ///
    /// ⚠ ACCOUNT NAMES ARE VIEW-SCOPED. GetEntry is a fund-level RPC, and a
    /// name that omitted the view would not resolve. Both callers name the
    /// default view, the same one ApplyEvent's NAV quote is about.
    fn entry_of(
        fund: &str,
        view: &str,
        chart: &[ratio_store::Account],
        entry: &ratio_store::JournalEntry,
    ) -> pb::Entry {
        pb::Entry {
            name: format!("funds/{fund}/entries/{}", entry.id),
            entry_id: entry.id.clone(),
            memo: entry.memo.clone(),
            config_digest: entry.config.as_str().to_string(),
            postings: entry
                .postings
                .iter()
                .map(|p| pb::EntryPosting {
                    account: format!("funds/{fund}/views/{view}/accounts/{}", p.dim),
                    display_name: chart
                        .iter()
                        .find(|a| a.dim == p.dim)
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| format!("dimension {}", p.dim)),
                    amount: p.amount.to_string(),
                })
                .collect(),
        }
    }

    pub fn get_posting(&self, name: &str) -> Result<pb::Posting> {
        // `funds/f/views/v/accounts/1/postings/t1.0` — one segment deeper than
        // `view_scoped_id` handles, and the id itself contains a dot rather than
        // a slash, so the split stays at eight.
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() != 8
            || parts[0] != "funds"
            || parts[2] != "views"
            || parts[4] != "accounts"
            || parts[6] != "postings"
        {
            bail!("{name:?} is not a funds/*/views/*/accounts/*/postings/* name");
        }
        let parent = format!("funds/{}/views/{}/accounts/{}", parts[1], parts[3], parts[5]);
        self.list_postings(&parent)?
            .postings
            .into_iter()
            .find(|p| p.name == name)
            .with_context(|| format!("no posting {:?}", parts[7]))
    }


    /// The rules in force, structured enough to choose one.
    pub fn list_rules(&self, parent: &str) -> Result<pb::ListRulesResponse> {
        let fund = resource_id(parent, "funds")?;
        Ok(pb::ListRulesResponse {
            rules: self.rules_of(&fund)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_rule(&self, name: &str) -> Result<pb::Rule> {
        let (fund, id) = nested_id(name, "funds", "rules")?;
        self.rules_of(&fund)?
            .into_iter()
            .find(|r| r.rule_id == id)
            .with_context(|| format!("no rule {id:?} in the active configuration"))
    }

    fn rules_of(&self, fund: &str) -> Result<Vec<pb::Rule>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let chart = b.accounts()?;
        let Some(digest) = b.active()? else { return Ok(Vec::new()) };
        let set = RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest)?))?;

        Ok(set
            .rules
            .iter()
            .map(|r| pb::Rule {
                name: format!("funds/{fund}/rules/{}", r.id),
                rule_id: r.id.clone(),
                kind: match r.kind {
                    ratio_rules::RuleKind::Trade => pb::rule::Kind::Trade,
                    ratio_rules::RuleKind::Dividend => pb::rule::Kind::Dividend,
                    ratio_rules::RuleKind::Accrual => pb::rule::Kind::Accrual,
                    ratio_rules::RuleKind::Mark => pb::rule::Kind::Mark,
                } as i32,
                description: r.description.clone(),
                form: ratio_rules::render(r, &chart),
                accounts: r
                    .legs
                    .iter()
                    .map(|l| {
                        chart
                            .iter()
                            .find(|a| a.dim == l.account)
                            .map(|a| a.display_name.clone())
                            .unwrap_or_else(|| format!("dimension {}", l.account))
                    })
                    .collect(),
            })
            .collect())
    }

    /// Record an event, and return the entry the active configuration made of
    /// it. The ONLY write on this service.
    pub fn apply_event(&self, req: &pb::ApplyEventRequest) -> Result<pb::ApplyEventResponse> {
        let fund = resource_id(&req.parent, "funds")?;
        let path = self.book_path(&fund)?;

        // The event id reaches a filename-shaped identifier and, on the public
        // demo, a screen other visitors read. Nothing but an id gets through.
        let id = req.event_id.trim();
        if id.is_empty() || id.len() > 64
            || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            bail!("{:?} is not an event id — letters, digits, - _ . and at most 64 of them", req.event_id);
        }

        let amount = parse_amount(&req.amount)?;
        let days = if req.days.trim().is_empty() {
            None
        } else {
            Some(req.days.trim().parse::<i64>().context("days must be a whole number")?)
        };

        // ── what makes this a trade rather than a movement of value ─────────
        //
        // ⛔ THESE THREE ARE WHY A LOT OPENS. Until they were carried, this
        // method built its event with `instrument: None, quantity: None` and
        // its entry with `trade_date: None` — and `Projection::walk` skips any
        // posting lacking BOTH an instrument and a quantity, so every trade
        // recorded here opened no lot and relieved none. The entry balanced,
        // the trial balance tied, the NAV moved by the right amount, and the
        // position's unit count was somebody else's. HANDOFF.md's table calls
        // that shape "the books tie and the number is wrong"; the realized gain
        // is the figure with no counterparty, and nobody catches it.
        let instrument = req.instrument.trim();
        let instrument = if instrument.is_empty() {
            None
        } else {
            // The same charset as the event id, and for the same reason: this
            // reaches a position's resource name, a URL, and a screen other
            // people read.
            if instrument.len() > 64
                || !instrument
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                bail!(
                    "{:?} is not an instrument identifier — letters, digits, - _ . \
                     and at most 64 of them",
                    req.instrument
                );
            }
            Some(instrument.to_string())
        };

        // ⛔ WHOLE UNITS, REFUSED RATHER THAN ROUNDED. `parse_minor` gives
        // hundredths, so a fractional quantity is expressible here and NOT on
        // `PostingRecord::quantity`, which is whole units. The data plane
        // carries such a fact as no quantity at all; doing that here would
        // silently produce exactly the lot-less entry these fields exist to
        // prevent, on the one screen where a person typed the number.
        let quantity = if req.quantity.trim().is_empty() {
            None
        } else {
            let hundredths = ratio_common::parse_minor(&req.quantity)
                .context("the quantity is a number of units")?;
            if hundredths % 100 != 0 {
                bail!(
                    "{:?} is not a whole number of units, and a lot is kept in whole \
                     units — a fractional quantity has to arrive on the data plane",
                    req.quantity
                );
            }
            if hundredths < 0 {
                bail!(
                    "a quantity is not negative — the RULE decides the direction, and a \
                     leg's own sign moves the units the right way"
                );
            }
            Some(hundredths / 100)
        };

        // ⛔ ONE WITHOUT THE OTHER IS THE DEFECT WEARING A DISGUISE. A posting
        // that names an instrument and no quantity is skipped by the walk just
        // as surely as one that names neither, so accepting half of this would
        // report a trade as attributed while it opened nothing.
        if instrument.is_some() != quantity.is_some() {
            bail!(
                "an instrument and a quantity go together: with one of them the posting \
                 carries no lot, which is the same as sending neither"
            );
        }

        // ⚠ VALIDATED HERE, NOT WHERE IT IS READ. A date that will not parse is
        // recorded as a BREAK by the projection rather than an absence — it has
        // to be, because a journal is append-only and the lots it opened are
        // already wrong. Refusing it at the door is the only place it is free.
        let trade_date = match &req.trade_date {
            None => None,
            Some(d) => {
                let iso = format!("{:04}-{:02}-{:02}", d.year, d.month, d.day);
                // ⭐ THE CALENDAR CHECK IS THE PARSER'S, NOT THIS DOOR'S. This
                // used to convert back and compare here, because
                // `days_from_iso_date` range-checked the day at 1..=31 and then
                // let the civil arithmetic carry `2026-02-30` into the 2nd of
                // March. That check now lives inside the parser, where every
                // caller gets it — a market calendar's holidays are read
                // through the same function, with `filter_map(…ok())`, and a
                // date that silently succeeds there moves every T+n settlement
                // computed from it.
                ratio_common::days_from_iso_date(&iso)
                    .with_context(|| format!("{iso:?} is not a trade date"))?;
                Some(iso)
            }
        };

        let mut b = FileBook::open(&path)?;
        let digest = b.active()?.context("no configuration is in force on this fund")?;
        let chart = b.accounts()?;
        let set = RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest)?))?;
        let rule = set
            .rule(&req.rule_id)
            .with_context(|| format!("no rule {:?} in the configuration in force", req.rule_id))?;

        // ⛔ STREAMED, and it does not stop early — but it holds one bool
        // rather than the journal.
        // ⛔ STREAMED. One bool and one counter rather than the journal — and
        // this is the WRITE path, so it runs on every event the console posts.
        let mut seen = false;
        let mut existing_len = 0usize;
        b.for_each_entry_since(0, &mut |e| {
            if e.id == id {
                seen = true;
            }
            existing_len += 1;
            Ok(())
        })?;
        if seen {
            bail!("{id:?} is already in this journal — an event is recorded once");
        }

        // ⚠ The deployment sets a ceiling; a local run has none. The journal
        // itself is no longer healed by a cold start — when the object store
        // is wired, a write survives, so the ceiling is the bound that remains.
        if let Some(max) = self.max_entries {
            if !req.validate_only && existing_len >= max {
                bail!(
                    "this book has {} entries, which is as many as the demo accepts",
                    existing_len
                );
            }
        }

        let event = ratio_rules::Event {
            rule: rule.id.clone(),
            id: id.to_string(),
            amount,
            days,
            memo: String::new(),
            instrument,
            quantity,
        };
        let postings = ratio_rules::compile(rule, &event)?;

        // The memo is COMPOSED from what the rule and the event say, never
        // taken from the caller. On a public endpoint, free text on the
        // journal is free text on somebody else's screen.
        let memo = format!("{id} via {}", rule.id);

        let previous = self.default_view_nav(&fund)?;
        // ⚠ THE ACCOUNT NAMES BELOW ARE VIEW-SCOPED RESOURCES NOW, and this is
        // a fund-level RPC — so they name the default view, the one `previous`
        // is also about. The chart itself does not depend on a view; only its
        // totals do. A name that omitted the segment would not resolve.
        let default_view = self.default_view_of(&fund)?;

        let entry = ratio_store::JournalEntry {
            id: id.to_string(),
            memo,
            config: digest.clone(),
            postings: postings.clone(),
            trade_date,
            announcement: None,
        };

        // The kernel refuses an unbalanced entry at the door, so `append` is
        // the check — there is no state in which this wrote something that
        // does not conserve.
        if !req.validate_only {
            b.append(&entry)?;
            self.record_change(&path, "posted", id, digest.as_str())?;
        }

        let net_asset_value = if req.validate_only {
            // Nothing was written, so the fund still reports the old figure.
            // Fold the proposed postings over it instead of reading it back.
            //
            // ⛔ TRANSLATED, LIKE THE FIGURE IT IS ADDED TO. This summed
            // `p.amount` raw, so a dry run of a EUR event moved the previewed
            // NAV by its FACE value while `get_fund` — the figure it is added
            // to, and the figure the user sees a moment later — reports the
            // TRANSLATED one. The preview and the commit disagreed on the same
            // event, and a preview that does not predict the commit is worse
            // than no preview.
            let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
            let mut delta = 0i128;
            for p in postings.iter().filter(|p| {
                matches!(
                    chart.iter().find(|a| a.dim == p.dim).map(|a| a.account_type),
                    Some(AccountTypeRecord::Asset) | Some(AccountTypeRecord::Liability)
                )
            }) {
                let factor = rates.factor_of_optional(p.currency.as_deref()).with_context(|| {
                    format!(
                        "this event is in {} and the fund has no rate for it — a preview that \
                         guesses a rate is a preview of a different event",
                        p.currency.as_deref().unwrap_or("no currency")
                    )
                })?;
                delta += p.amount as i128 * factor as i128 / ratio_project::RATE_SCALE as i128;
            }
            (previous.parse::<i128>().unwrap_or(0) + delta).to_string()
        } else {
            self.default_view_nav(&fund)?
        };

        Ok(pb::ApplyEventResponse {
            entry: Some(Self::entry_of(&fund, &default_view, &chart, &entry)),
            validate_only: req.validate_only,
            net_asset_value,
            previous_net_asset_value: previous,
        })
    }


    /// Files received on the data plane, newest first.
    pub fn list_deliveries(&self, parent: &str) -> Result<pb::ListDeliveriesResponse> {
        let fund = resource_id(parent, "funds")?;
        let path = self.book_path(&fund)?;
        let b = FileBook::open(&path)?;
        let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
        let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
        let resolved = ratio_ingest::resolve_all(&facts, &master);

        let mut out: Vec<pb::Delivery> = b
            .records::<ratio_ingest::Delivery>(Plane::Deliveries)?
            .into_iter()
            .map(|d| {
                let mine: Vec<&ratio_ingest::Resolved> = resolved
                    .iter()
                    .filter(|r| r.fact.provenance.delivery == d.digest)
                    .collect();
                pb::Delivery {
                    name: format!("funds/{fund}/deliveries/{}", &d.digest[..16]),
                    digest: d.digest.clone(),
                    origin: d.origin,
                    receive_time: Some(stamp(d.received)),
                    byte_count: d.bytes.to_string(),
                    fact_count: mine.len().to_string(),
                    pending_fact_count: mine
                        .iter()
                        .filter(|r| !r.is_admissible())
                        .count()
                        .to_string(),
                }
            })
            .collect();
        // Newest first. The same file delivered twice is one row per delivery,
        // because when it arrived is part of the record even when the bytes
        // are not new.
        out.reverse();
        Ok(pb::ListDeliveriesResponse { deliveries: out, next_page_token: String::new() })
    }

    pub fn get_delivery(&self, name: &str) -> Result<pb::Delivery> {
        let (fund, id) = nested_id(name, "funds", "deliveries")?;
        self.list_deliveries(&format!("funds/{fund}"))?
            .deliveries
            .into_iter()
            .find(|d| d.digest.starts_with(&id))
            .with_context(|| format!("no delivery {id:?}"))
    }

    /// Facts that cannot post yet, and what blocks each.
    ///
    /// Recomputed on every call, never cached. That is the whole design: a fact
    /// that was pending this morning and is admissible now needs no
    /// re-ingestion, because only the master changed.
    pub fn list_pending_facts(&self, parent: &str) -> Result<pb::ListPendingFactsResponse> {
        let fund = resource_id(parent, "funds")?;
        Ok(pb::ListPendingFactsResponse {
            pending_facts: self.pending_of(&fund)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_pending_fact(&self, name: &str) -> Result<pb::PendingFact> {
        let (fund, id) = nested_id(name, "funds", "pendingFacts")?;
        self.pending_of(&fund)?
            .into_iter()
            .find(|p| p.name.ends_with(&format!("/{id}")))
            .with_context(|| format!("no pending fact {id:?}"))
    }

    fn pending_of(&self, fund: &str) -> Result<Vec<pb::PendingFact>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
        if facts.is_empty() {
            return Ok(Vec::new());
        }
        let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;

        // ⛔ A FACT ALREADY IN THE JOURNAL IS NOT PENDING.
        //
        // Its entry was written under the resolution in force at the time and
        // is immutable; re-resolving it afterwards asks a question that has
        // already been answered. Without this, a duplicate identifier arriving
        // weeks later would make a posted fact ambiguous again and re-block a
        // NAV that was already struck.
        //
        // Found by `//tla:control_plane_check`, which produced the sequence in
        // four steps. `resolved_never_becomes_absent` in Lean is still true and
        // still the right theorem — it allows resolved -> AMBIGUOUS by design.
        // This is the system-level consequence of that allowance, and no
        // theorem about resolution could have shown it.
        let posted: std::collections::BTreeSet<String> =
            {
                // ⚠ STREAMED, AND STILL O(entries) BY NATURE — this is a set of
                // every id the journal holds. What it no longer holds is the
                // ENTRIES behind them. The real fix is an index rather than a
                // scan; noted rather than pretended away.
                let mut ids: std::collections::BTreeSet<String> = Default::default();
                b.for_each_entry_since(0, &mut |e| {
                    ids.insert(e.id.clone());
                    Ok(())
                })?;
                ids
            };

        Ok(ratio_ingest::resolve_all(&facts, &master)
            .into_iter()
            .filter(|r| !r.is_admissible() && !posted.contains(&r.fact.reference))
            .map(|r| pending_fact(fund, &r))
            .collect())
    }

    /// What the fund holds, by account and instrument.
    ///
    /// ⛔ THE UNATTRIBUTED REMAINDER IS IN THE LIST, as a row with no
    /// instrument — not a field beside it. A caller can render a list without
    /// noticing a field, and a positions view that quietly omits what it does
    /// not attribute disagrees with the trial balance by exactly the amount it
    /// hid. In the list, the rows sum to the accounts by construction, which is
    /// `Ratio.Ingest.positions_roll_up` made structural rather than remembered.
    pub fn list_positions(&self, parent: &str) -> Result<pb::ListPositionsResponse> {
        let (fund, view) = view_scoped_parent(parent)?;
        Ok(pb::ListPositionsResponse {
            positions: self.positions_of(&fund, &view)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_position(&self, name: &str) -> Result<pb::Position> {
        let (fund, view, id) = view_scoped_id(name, "positions")?;
        self.positions_of(&fund, &view)?
            .into_iter()
            .find(|p| p.name.ends_with(&format!("/{id}")))
            .with_context(|| format!("no position {id:?}"))
    }

    /// The open lots behind one position, oldest first.
    ///
    /// ⚠ THE ONE READ ON THIS SERVICE WHOSE COST GROWS WITH TRADING HISTORY
    /// rather than with the chart, which is why it is reachable only through a
    /// position. A fund's positions are five hundred lines whatever its age;
    /// its lots are every purchase it still holds.
    pub fn list_lots(&self, parent: &str) -> Result<pb::ListLotsResponse> {
        let (fund, view, id) = view_scoped_id(parent, "positions")?;
        // ⛔ TENANCY BEFORE THE POSITION-KEY PARSE. `projection` opens the book
        // through `book_path`, so a caller who may not see this fund is refused
        // before their position id is parsed — the denial does not depend on
        // whether the id was well-formed.
        let proj = self.projection(&fund)?;
        let (dim, instrument) = position_key(&id)?;
        Ok(pb::ListLotsResponse {
            lots: proj
                .lots_of(&view, dim, &instrument)?
                .value
                .into_iter()
                .map(|l| pb::Lot {
                    name: format!("funds/{fund}/views/{view}/positions/{id}/lots/{}", l.seq),
                    sequence: l.seq as i64,
                    units: l.units.to_string(),
                    cost: l.cost.to_string(),
                    // The lot stores a day; the wire wants a calendar date.
                    // Rendered on the way out rather than retained as text on
                    // every one of a million lots.
                    acquired: l
                        .acquired
                        .map(|d| ratio_common::iso_date_from_days(d as i64))
                        .as_deref()
                        .and_then(iso_date),
                })
                .collect(),
            next_page_token: String::new(),
        })
    }

    pub fn get_lot(&self, name: &str) -> Result<pb::Lot> {
        // ⛔ EIGHT SEGMENTS, NOT SIX. A lot hangs off a position, which hangs off
        // a VIEW — which entries are recognised decides which lots are open, so
        // dropping the view here would answer about a different book of record
        // than the caller named. `view_scoped_id` handles the six-segment case;
        // this one is a level deeper and stays spelled out.
        let parts: Vec<&str> = name.split('/').collect();
        let ["funds", fund, "views", view, "positions", pos, "lots", lot] = parts[..] else {
            bail!("{name:?} is not a funds/*/views/*/positions/*/lots/* name");
        };
        self.list_lots(&format!("funds/{fund}/views/{view}/positions/{pos}"))?
            .lots
            .into_iter()
            .find(|l| l.name.ends_with(&format!("/{lot}")))
            .with_context(|| format!("no lot {lot:?} open in position {pos:?}"))
    }

    fn positions_of(&self, fund: &str, view: &str) -> Result<Vec<pb::Position>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        // ⛔ THE VIEW'S POSITIONS, OFF THE MAINTAINED FOLD — not
        // `b.positions()`, whose whole-journal answer is one view's wearing no
        // label. A trade in flight under a settlement view is cash there and a
        // holding here, and this list has to say which book of record it read.
        let proj = self.projection(fund)?;
        let as_of = proj.positions(view)?;
        let held: Vec<((i64, String), (i64, i64))> = as_of
            .value
            .held
            .iter()
            .map(|((d, i), v)| ((*d, i.to_string()), *v))
            .collect();
        let rest: Vec<(i64, i64)> =
            as_of.value.rest.iter().map(|(d, v)| (*d, *v)).collect();
        let chart: BTreeMap<i64, String> = b
            .accounts()?
            .into_iter()
            .map(|a| (a.dim, a.display_name))
            .collect();
        // The master's label for an instrument, so a screen shows "Vanguard
        // S&P 500 ETF" rather than the internal id it resolved to.
        let names: BTreeMap<String, String> =
            ratio_ingest::current(&b.records::<ratio_ingest::Entity>(Plane::Entities)?)
                .into_iter()
                .map(|e| (e.id, e.display_name))
                .collect();
        let label = |d: i64| chart.get(&d).cloned().unwrap_or_else(|| format!("dimension {d}"));

        // When each instrument was last marked, from the journal's own entry
        // ids. Derived rather than stored: a "last marked" field would be one
        // more thing that can disagree with the entries, and the entries are
        // the record.
        let mut marked: BTreeMap<String, String> = BTreeMap::new();
        b.for_each_entry_since(0, &mut |e| {
            let Some(rest) = e.id.strip_prefix("mark-") else { return Ok(()) };
            // `mark-<instrument>-<YYYY-MM-DD>`; the date is the last ten.
            if rest.len() < 11 {
                return Ok(());
            }
            let (inst, day) = rest.split_at(rest.len() - 11);
            let day = &day[1..];
            let slot = marked.entry(inst.to_string()).or_default();
            if day > slot.as_str() {
                *slot = day.to_string();
            }
            Ok(())
        })?;

        // The lot book, so a position can say how many lots stand behind it.
        // ⛔ `Ratio.Closure.factored_nav_never_reads_the_lots` is the claim that
        // this number does not appear in the NAV; showing it beside one that
        // does is what lets a reader see the claim rather than be told it.
        let mut out: Vec<pb::Position> = held
            .into_iter()
            .map(|((dim, instrument), (value, quantity))| pb::Position {
                open_lot_count: proj
                    .lots_of(view, dim, &instrument)
                    .map(|l| l.value.len() as i64)
                    .unwrap_or(0),
                name: format!("funds/{fund}/views/{view}/positions/{dim}-{instrument}"),
                account: format!("funds/{fund}/views/{view}/accounts/{dim}"),
                account_label: label(dim),
                instrument_label: names
                    .get(&instrument)
                    .cloned()
                    .unwrap_or_else(|| instrument.clone()),
                mark_date: marked.get(&instrument).and_then(|d| iso_date(d)),
                instrument,
                quantity: quantity.to_string(),
                value: value.to_string(),
            })
            .collect();

        for (dim, value) in rest {
            if value == 0 {
                continue;
            }
            out.push(pb::Position {
                name: format!("funds/{fund}/views/{view}/positions/{dim}-unattributed"),
                account: format!("funds/{fund}/views/{view}/accounts/{dim}"),
                account_label: label(dim),
                instrument: String::new(),
                instrument_label: "Not attributed".into(),
                quantity: "0".into(),
                value: value.to_string(),
                // ⚠ Value attributed to no instrument has no lots by
                // construction — a lot is a purchase of something.
                open_lot_count: 0,
                mark_date: None,
            });
        }
        // By account, then by what it holds most of — an operator scanning a
        // position list is looking for the big ones.
        out.sort_by(|a, b| {
            a.account_label.cmp(&b.account_label).then_with(|| {
                b.value
                    .parse::<i64>()
                    .unwrap_or(0)
                    .abs()
                    .cmp(&a.value.parse::<i64>().unwrap_or(0).abs())
            })
        });
        Ok(out)
    }

    /// The mapping templates in force.
    pub fn list_templates(&self, parent: &str) -> Result<pb::ListTemplatesResponse> {
        let fund = resource_id(parent, "funds")?;
        Ok(pb::ListTemplatesResponse {
            templates: self.templates_of(&fund)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_template(&self, name: &str) -> Result<pb::Template> {
        let (fund, id) = nested_id(name, "funds", "templates")?;
        self.templates_of(&fund)?
            .into_iter()
            .find(|t| t.template_id == id)
            .with_context(|| format!("no template {id:?} in the configuration in force"))
    }

    fn templates_of(&self, fund: &str) -> Result<Vec<pb::Template>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let Some(digest) = b.active()? else { return Ok(Vec::new()) };
        let text = String::from_utf8_lossy(&b.get(&digest)?).into_owned();
        Ok(ratio_ingest::TemplateSet::from_toml(&text)?
            .templates
            .iter()
            .map(|t| pb::Template {
                name: format!("funds/{fund}/templates/{}", t.id),
                template_id: t.id.clone(),
                fact_kind: t.fact.kind.clone(),
                form: t.render(),
                posts: t.fact.posts.is_some(),
            })
            .collect())
    }

    /// Read a delivered file into facts.
    ///
    /// `validate_only` runs the same code path and records nothing (AIP-163).
    /// What you preview is what lands, because it is not a second path.
    pub fn ingest_delivery(
        &self,
        req: &pb::IngestDeliveryRequest,
    ) -> Result<pb::IngestDeliveryResponse> {
        let fund = resource_id(&req.parent, "funds")?;
        let path = self.book_path(&fund)?;
        let mut b = FileBook::open(&path)?;

        let digest = b.active()?.context("no configuration is in force on this fund")?;
        let text = String::from_utf8_lossy(&b.get(&digest)?).into_owned();
        let set = ratio_ingest::TemplateSet::from_toml(&text)?;
        let template = set.template(&req.template_id).with_context(|| {
            format!(
                "no template {:?} in the configuration in force ({} there: {})",
                req.template_id,
                set.templates.len(),
                set.templates.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", "),
            )
        })?;
        let problems = template.check();
        if !problems.is_empty() {
            bail!("the template does not check: {}", problems.join("; "));
        }

        if req.content.trim().is_empty() {
            bail!("the file is empty");
        }
        let bytes = req.content.as_bytes();
        let delivery = ratio_ingest::Delivery {
            digest: ratio_store::Digest::of(bytes).as_str().to_string(),
            // Composed, never taken raw: on a public endpoint this reaches a
            // screen other people read.
            origin: sanitize_origin(&req.origin),
            received: now(),
            bytes: bytes.len() as i64,
        };

        let rows = match template.reads {
            ratio_ingest::Reader::Csv => ratio_ingest::extract_csv(&req.content)?,
        };
        let projection =
            ratio_ingest::project(template, &delivery, &rows, digest.as_str());

        let known: BTreeMap<String, ()> = b
            .records::<ratio_ingest::Fact>(Plane::Facts)?
            .into_iter()
            .map(|f| (f.id, ()))
            .collect();
        let fresh: Vec<&ratio_ingest::Fact> =
            projection.facts.iter().filter(|f| !known.contains_key(&f.id)).collect();

        if !req.validate_only {
            if let Some(max) = self.max_entries {
                let held = known.len() + fresh.len();
                if held > max {
                    bail!(
                        "this book would hold {held} facts, which is more than the demo \
                         accepts"
                    );
                }
            }
            b.append_record(Plane::Deliveries, &delivery)?;
            for f in &fresh {
                b.append_record(Plane::Facts, f)?;
            }
            self.record_change(&path, "ingested", &delivery.digest, digest.as_str())?;
        }

        // Resolve against the master as it stands, so the preview shows what
        // would pend rather than only what would parse.
        let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
        let resolved = ratio_ingest::resolve_all(&projection.facts, &master);
        let ready = resolved.iter().filter(|r| r.is_admissible()).count();

        Ok(pb::IngestDeliveryResponse {
            delivery_digest: delivery.digest,
            row_count: rows.len().to_string(),
            fact_count: projection.facts.len().to_string(),
            new_fact_count: fresh.len().to_string(),
            ready_count: ready.to_string(),
            rejected: projection
                .rejected
                .iter()
                .map(|r| pb::RejectedRow { row: r.row.to_string(), reason: r.reason.clone() })
                .collect(),
            pending: resolved
                .iter()
                .filter(|r| !r.is_admissible())
                .map(|r| pending_fact(&fund, r))
                .collect(),
            validate_only: req.validate_only,
        })
    }

    /// Strikes that were taken without an action that should have been in them.
    ///
    /// ⛔ THE OBLIGATION THAT REFUSING RESTATEMENT CREATES.
    /// `Ratio.Period.one_answer_per_view_per_day` says a valuation point is struck
    /// once IN EACH BOOK OF RECORD
    /// and never replaced — the first answer is what somebody was paid on. So
    /// an action arriving late CANNOT correct the NAVs it should have been in,
    /// and the only honest thing left is to be able to NAME them.
    /// `//tla:actions_check` calls this `StalenessIsAttributable`, and a fund
    /// that cannot compute it is publishing figures it cannot qualify.
    ///
    /// ⭐ DERIVED, NOT STORED. A strike pins a journal POSITION; an applied
    /// action IS a journal entry. So "was this action in that strike" is
    /// answerable by comparing the two — no staleness flag to maintain, and
    /// nothing that can disagree with the journal, because it is the journal.
    pub fn stale_strikes(&self, fund: &str) -> Result<Vec<(String, String, String)>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;

        // Where each action landed in the journal, if it landed at all.
        // ⛔ STREAMED. Bounded by the number of ACTIONS, not by the journal.
        let mut applied_at: BTreeMap<String, usize> = BTreeMap::new();
        {
            let mut i = 0usize;
            b.for_each_entry_since(0, &mut |e| {
                if let Some(a) = e.id.strip_prefix("action-") {
                    applied_at.insert(a.to_string(), i);
                }
                i += 1;
                Ok(())
            })?;
        }

        let mut out = Vec::new();
        for (a, _) in self.announcements(fund)? {
            for s in ratio_nav::list(&path)? {
                // The strike's own day, as the ex-date is written.
                let day = ratio_nav::rfc3339(s.valuation_time);
                let day = day.get(..10).unwrap_or("").to_string();
                if day < a.ex_date {
                    continue; // effective after this strike; correctly absent
                }
                let included = applied_at
                    .get(&a.id)
                    .is_some_and(|i| *i < s.journal_position);
                if !included {
                    out.push((
                        s.id.clone(),
                        a.id.clone(),
                        format!(
                            "{} {}-for-{} effective {} — {}",
                            a.instrument,
                            a.numerator,
                            a.denominator,
                            a.ex_date,
                            if applied_at.contains_key(&a.id) {
                                "applied after this NAV was struck"
                            } else {
                                "not applied"
                            },
                        ),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Apply a corporate action to every position in an instrument.
    ///
    /// ⛔ THE JOURNAL IS THE IDEMPOTENCE RECORD. The entry id is
    /// `action-{id}`, and this refuses when the journal already has it —
    /// because `Ratio.Actions.applying_twice_is_not_applying_once` and there is
    /// no arithmetic underneath that would notice. `//tla:reapply_action_check`
    /// is the same guard one layer up: without it a retry doubles the position
    /// and the trial balance still ties.
    ///
    /// A split posts an entry whose AMOUNTS ARE ZERO and whose QUANTITY moves.
    /// That is exactly the proved shape — cost conserved, units changed — and
    /// it balances by construction, so the kernel accepts it without being told
    /// anything special.
    pub fn apply_action(
        &self,
        fund: &str,
        action_id: &str,
        instrument: &str,
        s: ratio_ingest::actions::Split,
    ) -> Result<(i64, i64)> {
        let path = self.book_path(fund)?;
        let mut b = FileBook::open(&path)?;
        let digest = b.active()?.context("no configuration is in force")?;

        let entry_id = format!("action-{action_id}");
        let mut applied = false;
        b.for_each_entry_since(0, &mut |e| {
            if e.id == entry_id {
                applied = true;
            }
            Ok(())
        })?;
        if applied {
            bail!(
                "{action_id:?} has already been applied to this book. An action is not \
                 idempotent — applying it again would change the position while the trial \
                 balance went on tying."
            );
        }

        // Units held now, and what they cost, per account holding this
        // instrument. A split applies to the POSITION, not to one lot: the lot
        // detail is what `Ratio.Lots` relieves, and this crate's journal holds
        // positions.
        let (held, _) = b.positions()?;
        let mut moved = 0i64;
        let mut dim_touched = 0i64;
        for ((dim, i), (cost, units)) in held {
            if i != instrument || units == 0 {
                continue;
            }
            let after = ratio_ingest::actions::split(
                s,
                &ratio_ingest::actions::Holding { units, cost },
            )?;
            moved += after.units - units;
            dim_touched = dim;
        }
        if moved == 0 {
            bail!("no position in {instrument:?} for this action to apply to");
        }

        b.append(&ratio_store::JournalEntry {
            id: entry_id,
            memo: format!(
                "{instrument} {}-for-{} · units only, cost unchanged",
                s.num, s.den
            ),
            config: digest,
            // ⚠ AMOUNT ZERO. A split moves units and not value, so the entry
            // conserves trivially — and a reader who checked only the amounts
            // would see nothing happen.
            postings: vec![ratio_store::PostingRecord::of(dim_touched, 0, instrument, Some(moved))],
        
            trade_date: None,
            announcement: None,
        })?;
        Ok((moved, dim_touched))
    }

    /// Mark the book to market at a valuation date.
    ///
    /// ⛔ A POSTING, NOT AN ASSIGNMENT. Each position moves by the difference
    /// between what the book holds it at and what it is worth, with the contra
    /// in unrealized gain. Nothing is overwritten, so "why is this worth more
    /// than we paid" has an entry to point at.
    ///
    /// The delta is taken from the CARRYING value, never from cost.
    /// `//tla:mark_from_cost_check` shows the other way: the book drifts by the
    /// whole gain on every mark, and every entry is still balanced — the trial
    /// balance ties while the figure is wrong.
    pub fn mark_positions(
        &self,
        req: &pb::MarkPositionsRequest,
    ) -> Result<pb::MarkPositionsResponse> {
        use ratio_ingest::value::{observations, value_position, Valuation};

        let fund = resource_id(&req.parent, "funds")?;
        let as_of = req
            .valuation_date
            .as_ref()
            .map(|d| format!("{:04}-{:02}-{:02}", d.year, d.month, d.day))
            .filter(|s| s.len() == 10 && !s.starts_with("0000"))
            .context("a valuation date is required")?;
        let previous = self.default_view_nav(&fund)?;

        let path = self.book_path(&fund)?;
        let mut b = FileBook::open(&path)?;
        let digest = b.active()?.context("no configuration is in force")?;
        let text = String::from_utf8_lossy(&b.get(&digest)?).into_owned();
        let rules = RuleSet::from_toml(&text)?;

        // The configuration decides where a mark lands, the same as every other
        // posting. A book with no mark rule cannot be marked, and says so
        // rather than inventing accounts.
        let rule = rules
            .rules
            .iter()
            .find(|r| r.kind == ratio_rules::RuleKind::Mark)
            .context(
                "no rule of kind `mark` is in force, so there is nowhere for a valuation \
                 to post",
            )?;
        let held_in = rule
            .legs
            .iter()
            .find(|l| l.per_instrument)
            .context(
                "the mark rule has no `per_instrument` leg, so it does not say which \
                 account holds positions",
            )?
            .account;

        let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
        let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
        let observed = observations(&ratio_ingest::resolve_all(&facts, &master))?;
        let (positions, _) = b.positions()?;
        let names: BTreeMap<String, String> =
            ratio_ingest::current(&master).into_iter().map(|e| (e.id, e.display_name)).collect();

        let (mut marks, mut unpriced, mut inexact) = (Vec::new(), Vec::new(), Vec::new());
        let mut posted = 0usize;

        for ((dim, instrument), (carrying, quantity)) in positions {
            if dim != held_in {
                continue;
            }
            let label = names.get(&instrument).cloned().unwrap_or_else(|| instrument.clone());
            let row = |market: i64, movement: i64, price: i64, day: &str| pb::Mark {
                instrument: instrument.clone(),
                instrument_label: label.clone(),
                quantity: quantity.to_string(),
                carrying: carrying.to_string(),
                market: market.to_string(),
                movement: movement.to_string(),
                price: price.to_string(),
                price_date: iso_date(day),
            };

            // Units in hundredths, because the proved `marketValue` takes them
            // that way — and a whole quantity always values exactly
            // (`whole_units_value_exactly`).
            let units = quantity.saturating_mul(100);
            match value_position(&as_of, units, carrying, observed.get(&instrument).map_or(&[][..], |v| v)) {
                Valuation::Unpriced => {
                    unpriced.push(row(0, 0, 0, ""));
                }
                Valuation::Inexact { price, reason } => {
                    inexact.push(format!("{label}: {reason} (price {price})"));
                }
                Valuation::Marked { market, delta, price, on_day, .. } => {
                    marks.push(row(market, delta, price, &on_day));
                    // A position already at market moves by nothing and posts
                    // nothing — `Ratio.Valuation.mark_again_posts_nothing`.
                    if delta == 0 {
                        continue;
                    }
                    if !req.validate_only {
                        b.append(&ratio_store::JournalEntry {
                            id: format!("mark-{instrument}-{as_of}"),
                            memo: format!(
                                "{label} to {} on {on_day} · {} · {as_of}",
                                money_words(price),
                                rule.id,
                            ),
                            config: digest.clone(),
                            postings: ratio_rules::compile(
                                rule,
                                &ratio_rules::Event {
                                    rule: rule.id.clone(),
                                    id: format!("mark-{instrument}-{as_of}"),
                                    amount: delta,
                                    days: None,
                                    memo: String::new(),
                                    instrument: Some(instrument.clone()),
                                    // A mark moves value, not units.
                                    quantity: None,
                                },
                            )?,
                        
                            trade_date: None,
                            announcement: None,
                        })?;
                    }
                    posted += 1;
                }
            }
        }

        if !req.validate_only && posted > 0 {
            self.record_change(&path, "marked", &as_of, digest.as_str())?;
        }

        let net_asset_value = if req.validate_only {
            previous.clone()
        } else {
            self.default_view_nav(&fund)?
        };
        Ok(pb::MarkPositionsResponse {
            marks,
            unpriced,
            inexact,
            posted_count: posted.to_string(),
            net_asset_value,
            previous_net_asset_value: previous,
            validate_only: req.validate_only,
        })
    }

    /// The positions that cannot be valued at a date.
    ///
    /// ⛔ WHAT BLOCKS A STRIKE. A net asset value computed over a position
    /// nobody has priced is not a net asset value — it is a number with a hole
    /// in it, and the hole is invisible in the figure.
    ///
    /// `Ratio.Valuation.strike_refuses_exactly_when_something_is_unpriced`
    /// proves the refusal is exactly this list, so the message and the decision
    /// are one derivation and cannot disagree. And
    /// `a_later_price_does_not_help` proves a price observed after the date
    /// does not clear it — otherwise the strike could not be replayed from what
    /// existed when it was taken.
    pub fn unpriced_at(&self, fund: &str, as_of: &str) -> Result<Vec<(String, i64)>> {
        use ratio_ingest::value::{mark_price, observations};

        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
        let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
        let observed = observations(&ratio_ingest::resolve_all(&facts, &master))?;
        let names: BTreeMap<String, String> =
            ratio_ingest::current(&master).into_iter().map(|e| (e.id, e.display_name)).collect();

        let (held, _) = b.positions()?;
        let mut out = Vec::new();
        for ((_, instrument), (_, quantity)) in held {
            // A position closed out holds nothing, so nothing about it is
            // unknown. Blocking on a zero holding would stop a strike over a
            // security the fund no longer owns.
            if quantity == 0 {
                continue;
            }
            let none = observed
                .get(&instrument)
                .map_or(true, |os| mark_price(as_of, os).is_none());
            if none {
                out.push((
                    names.get(&instrument).cloned().unwrap_or_else(|| instrument.clone()),
                    quantity,
                ));
            }
        }
        Ok(out)
    }

    /// Post every fact that fully resolves.
    ///
    /// ⛔ THE ONE IMPLEMENTATION. `ratio admit` calls this too — a second copy
    /// for the CLI would be a second set of decisions about what posts.
    pub fn admit_facts(&self, req: &pb::AdmitFactsRequest) -> Result<pb::AdmitFactsResponse> {
        let fund = resource_id(&req.parent, "funds")?;
        let path = self.book_path(&fund)?;
        let previous = self.default_view_nav(&fund)?;

        let mut b = FileBook::open(&path)?;
        let digest = b.active()?.context("no configuration is in force")?;
        let text = String::from_utf8_lossy(&b.get(&digest)?).into_owned();
        let templates = ratio_ingest::TemplateSet::from_toml(&text)?;
        let rules = RuleSet::from_toml(&text)?;

        let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
        let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
        let resolved = ratio_ingest::resolve_all(&facts, &master);
        let posted: BTreeMap<String, ()> =
            {
                // ⚠ As above: streamed, still O(distinct ids), wants an index.
                let mut ids: BTreeMap<String, ()> = Default::default();
                b.for_each_entry_since(0, &mut |e| {
                    ids.insert(e.id.clone(), ());
                    Ok(())
                })?;
                ids
            };

        let (mut n, mut recorded) = (0usize, 0usize);
        let mut refused = Vec::new();
        for r in resolved.iter().filter(|r| r.is_admissible()) {
            if posted.contains_key(&r.fact.reference) {
                continue;
            }
            let Some(t) = templates.template(&r.fact.provenance.template_id) else {
                refused.push(format!(
                    "{}: template {:?} is not in the configuration in force",
                    r.fact.reference, r.fact.provenance.template_id
                ));
                continue;
            };
            // Reference data posts nothing BY DESIGN. Counted apart from a
            // refusal, because reporting the design as a fault is how a demo
            // comes to show red for the thing working.
            if t.fact.posts.is_none() {
                recorded += 1;
                continue;
            }
            let (rule_id, amount) = match ratio_ingest::posting_for(t, &r.fact) {
                Ok(v) => v,
                Err(e) => {
                    refused.push(format!("{}: {e:#}", r.fact.reference));
                    continue;
                }
            };
            let Some(rule) = rules.rule(&rule_id) else {
                refused.push(format!(
                    "{}: the template posts it as `{rule_id}`, which is not a rule in force",
                    r.fact.reference
                ));
                continue;
            };
            let who: Vec<String> = r
                .entities
                .keys()
                .filter_map(|k| r.entity(k).map(str::to_string))
                .collect();
            // The instrument the fact RESOLVED to, not the identifier the file
            // happened to carry. Two counterparties calling the same security
            // by different identifiers must land on one position, which is the
            // whole reason resolution exists.
            let instrument = r
                .entities
                .keys()
                .find(|k| {
                    r.fact
                        .entities
                        .get(*k)
                        .is_some_and(|e| e.kind == ratio_ingest::EntityKind::Instrument)
                })
                .and_then(|k| r.entity(k))
                .map(str::to_string);
            let postings = ratio_rules::compile(
                rule,
                &ratio_rules::Event {
                    rule: rule_id.clone(),
                    id: r.fact.reference.clone(),
                    amount,
                    days: None,
                    memo: String::new(),
                    instrument,
                    // Whole units. The fact carries it in minor units because
                    // fractional shares exist; a quantity that is not whole is
                    // carried as none rather than rounded.
                    quantity: r
                        .fact
                        .values
                        .get("quantity")
                        .and_then(ratio_ingest::Value::as_minor)
                        .filter(|q| q % 100 == 0)
                        .map(|q| q / 100),
                },
            )?;
            if !req.validate_only {
                b.append(&ratio_store::JournalEntry {
                    id: r.fact.reference.clone(),
                    memo: format!(
                        "{} · {} · row {} of {}",
                        who.join(" "),
                        rule_id,
                        r.fact.provenance.row,
                        &r.fact.provenance.delivery[..12],
                    ),
                    config: digest.clone(),
                    postings,
                    // ⛔ THE DAY THE TEMPLATE SAYS THIS HAPPENED. This was
                    // `None` while the trade template beside it declared a
                    // `traded` date and read it into every fact — so a delivery
                    // posted lots with no acquisition date, every
                    // holding-period method refused them, and the whole of that
                    // fund's realized gain fell into the unclassified residue.
                    // The books tied throughout.
                    //
                    // ⚠ STILL `None` FOR A TEMPLATE THAT DECLARES NO DATE, which
                    // is the honest answer for a file that carries none.
                    trade_date: ratio_ingest::dated_of(t, &r.fact).map(str::to_string),
                    announcement: None,
                })?;
            }
            n += 1;
        }

        if !req.validate_only && n > 0 {
            self.record_change(&path, "admitted", &format!("{n} facts"), digest.as_str())?;
        }

        let net_asset_value = if req.validate_only {
            previous.clone()
        } else {
            self.default_view_nav(&fund)?
        };
        Ok(pb::AdmitFactsResponse {
            posted_count: n.to_string(),
            recorded_count: recorded.to_string(),
            pending_count: resolved.iter().filter(|r| !r.is_admissible()).count().to_string(),
            refused,
            net_asset_value,
            previous_net_asset_value: previous,
            validate_only: req.validate_only,
        })
    }

    pub fn list_funds(&self) -> Result<pb::ListFundsResponse> {
        let mut funds = Vec::new();
        for id in self.fund_ids()? {
            funds.push(self.get_fund(&format!("funds/{id}"))?);
        }
        Ok(pb::ListFundsResponse { funds, next_page_token: String::new() })
    }

    pub fn list_books(&self) -> Result<pb::ListBooksResponse> {
        let mut books = Vec::new();
        for id in self.book_ids()? {
            books.push(self.get_book(&format!("books/{id}"))?);
        }
        Ok(pb::ListBooksResponse { books, next_page_token: String::new() })
    }

    pub fn get_book(&self, name: &str) -> Result<pb::Book> {
        let id = resource_id(name, "books").context("bad book name")?;
        let path = self.book_path(&id)?;
        let meta = book::BookMeta::load(&path, &id);
        // Figures are the same ones GetFund already folds. GetFund still
        // answers for any book directory so existing screens and rewrites keep
        // working; the sidecar is what distinguishes a fund listing from a book.
        let fund = self.get_fund(&format!("funds/{id}"))?;
        Ok(pb::Book {
            name: format!("books/{id}"),
            display_name: meta.display_name,
            kind: meta.kind.proto(),
            currency_code: fund.currency_code,
            fund: meta.fund.map(|f| format!("funds/{f}")).unwrap_or_default(),
            organization: meta.organization.unwrap_or_default(),
            default_view: fund.default_view,
            entry_count: fund.entry_count,
            config_digest: fund.config_digest,
            trial_balance_difference: fund.trial_balance_difference,
        })
    }

    pub fn create_book(&self, req: pb::CreateBookRequest) -> Result<pb::Book> {
        let id = req.book_id.trim();
        if id.is_empty()
            || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("{id:?} is not a book id");
        }
        let Some(spec) = req.book else {
            bail!("book is required");
        };
        let path = self.root.join(id);
        if path.join("accounts.json").is_file() || path.join("book.toml").is_file() {
            bail!("books/{id} already exists");
        }
        let kind = book::BookKind::from_proto(spec.kind)?;
        let display = if spec.display_name.trim().is_empty() {
            display_name(id)
        } else {
            spec.display_name.trim().to_string()
        };
        book::initialize(&path, id, &display, kind)?;
        if let Some(actor) = &self.actor {
            book::grant(&self.root, actor, id)?;
        }
        // ⚠ Do not call `book_path` here: `allowed` is computed once at
        // construction. The grant is on disk; the next request sees it.
        let meta = book::BookMeta::load(&path, id);
        Ok(pb::Book {
            name: format!("books/{id}"),
            display_name: meta.display_name,
            kind: meta.kind.proto(),
            currency_code: String::new(),
            fund: String::new(),
            organization: String::new(),
            default_view: String::new(),
            entry_count: 0,
            config_digest: String::new(),
            trial_balance_difference: "0".into(),
        })
    }

    /// Check a view exists on this fund, and that the projection can answer for
    /// it.
    ///
    /// ⛔ A REFUSAL RATHER THAN THE SAME NUMBERS UNDER A SECOND LABEL. The
    /// projection folds the WHOLE journal — it has no cut — so it can answer for
    /// a `recorded` view, which recognises entries in the journal's own order
    /// and consults no date. It cannot yet answer for a trade-date or
    /// settlement view, because those differ from `recorded` only AT A CUT, and
    /// serving one without a cut would return figures identical to the other's
    /// under a different name. That is exactly the defect this whole feature
    /// exists to prevent: a figure that does not say which question it answers,
    /// or worse, says the wrong one.
    ///
    /// ⚠ `ratio strike` DOES cut — it derives the day from the valuation point —
    /// so the RECORDED NAV is already per view. It is the maintained projection
    /// behind these screens that is not, and the gap is named here rather than
    /// hidden behind an equal number.
 
    /// The default view's id: which book of record a fund-level answer is about.
    ///
    /// ⛔ A FUND-LEVEL FIGURE STILL BELONGS TO A VIEW. `ApplyEvent`,
    /// `MarkPositions` and `AdmitFacts` each quote a NAV before and after, and
    /// there is no such thing as a NAV without a recognition convention — so
    /// they quote the DEFAULT view's, and `Fund.default_view` is what tells a
    /// reader which that is. Quoting one with no view named is the row already
    /// in HANDOFF.md's failure table.
    fn default_view_of(&self, fund: &str) -> Result<String> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        Ok(match b.active()? {
            Some(d) => ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&d)?))
                .unwrap_or_default()
                .default_view(),
            None => ratio_rules::RuleSet::default().default_view(),
        })
    }

    /// The default view's NAV in minor units, as a fund-level answer quotes it.
    ///
    /// ⚠ REFUSES FOR THE SAME REASON `GetView` DOES. If a fund's default view
    /// recognises by date, the maintained projection cannot answer for it — and
    /// a preview quoting the recorded figure under that view's name is exactly
    /// the substitution this feature exists to prevent.
    fn default_view_nav(&self, fund: &str) -> Result<String> {
        let view = self.default_view_of(fund)?;
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
        let proj = self.projection(fund)?;
        Ok(nav_from(&b, &proj, &view, &rates)?.0.to_string())
    }

    pub fn get_fund(&self, name: &str) -> Result<pb::Fund> {
        let id = resource_id(name, "funds").context("bad fund name")?;
        let path = self.book_path(&id)?;
        let b = FileBook::open(&path)?;
        let tb = b.trial_balance()?;

        // ⛔ THE TERMS THE ACTIVE CONFIGURATION DECLARES, read rather than
        // assumed. The lot method decides the realized gain
        // (`the_method_decides_the_taxable_gain`) and the threshold decides
        // which rate it is taxed at — so both are reported beside the figure
        // rather than left as something a reader has to go and look up.
        let set = match b.active()? {
            Some(d) => ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&d)?)).ok(),
            None => None,
        };
        // ⭐ WHICH BOOKS OF RECORD THIS FUND KEEPS. From ACTIVE, because that is
        // a question about NOW — how an ENTRY is recognised comes from the
        // digest that entry pinned, which the fold resolves per entry.
        let effective = set.clone().unwrap_or_default();
        let default_view = effective.default_view();
        let view_count = effective.effective_views().len();

        // ⚠ AND THE BREAKS BELOW ARE THE DEFAULT VIEW'S, which is what
        // `Fund.state` and `Fund.open_break_count` say they are. A client that
        // renders either without `default_view` beside it is asserting a figure
        // it cannot qualify.
        let breaks = self.breaks_for(&path, &id, &default_view)?;
        let open: Vec<&pb::Break> = breaks.iter().filter(|k| !k.explained).collect();

        // A fact that cannot resolve is an exception in the same sense a
        // reconciliation break is: a figure missing an input is not a figure.
        // It blocks for the same reason and is counted with them.
        let pending = self.pending_of(&id)?;

        // ⛔ COUNTED. `entry_count` below is the only thing this was for.
        let mut entries_len = 0usize;
        b.for_each_entry_since(0, &mut |_| {
            entries_len += 1;
            Ok(())
        })?;
        // ⛔ THE SAME PREDICATE `ratio strike` REFUSES ON. Derived here from
        // `blocking_at` rather than restated, so the badge and the refusal are
        // one derivation — two folds of "what blocks" would be plausible,
        // independently maintained, and one field apart within a month.
        let blocking = self.blocking_at(&id)?;
        let state = if entries_len == 0 && pending.is_empty() {
            pb::fund::State::AwaitingPrices
        } else if !blocking.is_empty() {
            pb::fund::State::Blocked
        } else if !breaks.is_empty() {
            pb::fund::State::InReview
        } else {
            pb::fund::State::Struck
        };

        Ok(pb::Fund {
            name: format!("funds/{id}"),
            display_name: display_name(&id),
            currency_code: FUND_CURRENCY.into(),
            // ⛔ THE DEFAULT VIEW'S, AND `default_view` SITS BESIDE THEM SO A
            // CLIENT CANNOT RENDER EITHER WITHOUT THE LABEL. Both depend on
            // which entries are recognised; answering for one view without
            // saying which is the row already in HANDOFF.md's failure table.
            state: state as i32,
            open_break_count: open.len() as i64,
            default_view: default_view.clone(),
            view_count: view_count as i64,
            // ⛔ THE METHOD THE ENGINE USED, AND SEPARATELY WHETHER ANYONE CHOSE
            // IT. Reporting only the first made the console assert an election
            // nobody had made: the seeded demo books declare no method, are
            // relieved oldest-first by custom, and the screen called that "a
            // term of the administration agreement".
            //
            // ⚠ STILL FUND-LEVEL, because a view overrides TIMING only. Every
            // view relieves under the same election and they still reach
            // different realized gains, because each has recognised a different
            // set of open lots by the time a sale arrives.
            lot_method: set
                .as_ref()
                .map(|s| {
                    ratio_project::relief::Method::from(s.effective_lot_method())
                        .describe()
                        .to_string()
                })
                .unwrap_or_default(),
            lot_method_declared: set.as_ref().is_some_and(|s| s.lot_method.is_some()),
            long_term_days: set.as_ref().map(|s| s.long_term_days).unwrap_or(0),
            pending_fact_count: pending.len().to_string(),
            // ⭐ THE ONE TRIAL-BALANCE FIGURE THAT IS NOT VIEW-DEPENDENT, AND ITS
            // STAYING HERE IS THE CHECK THAT THE LINE IS DRAWN RIGHT. A view
            // keeps or drops WHOLE entries and each entry conserves, so the
            // difference cannot move — `Ratio.Views.every_view_conserves`. The
            // two COLUMN totals can, and they live on `View`.
            trial_balance_difference: (tb.debits - tb.credits).to_string(),
            entry_count: entries_len as i64,
            config_digest: b.active()?.map(|d| d.as_str().to_string()).unwrap_or_default(),
        })
    }

    /// The books of record this fund keeps.
    pub fn list_views(&self, parent: &str) -> Result<pb::ListViewsResponse> {
        let id = resource_id(parent, "funds").context("bad parent")?;
        let path = self.book_path(&id)?;
        let b = FileBook::open(&path)?;
        let set = match b.active()? {
            Some(d) => ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&d)?))
                .unwrap_or_default(),
            None => ratio_rules::RuleSet::default(),
        };
        let mut views = Vec::new();
        for v in set.effective_views() {
            // ⚠ THE LIST IS CHEAP AND THE FIGURES ARE NOT. A rail rendering six
            // funds would fold six journals if this filled every view in full;
            // the switcher needs the id, the basis and whether anybody declared
            // it, and `GetView` is where a figure comes from.
            views.push(view_pb(&id, &set, &v));
        }
        // ⚠ NO `default_view` ON THE COLLECTION. AIP-132 admits only the list
        // and its page token, and `Fund.default_view` already answers it — a
        // second copy is a second thing to keep in step, which is the argument
        // views exist to avoid.
        Ok(pb::ListViewsResponse { views, next_page_token: String::new() })
    }

    /// One book of record, with the figures it recognises.
    pub fn get_view(&self, name: &str) -> Result<pb::View> {
        let (id, view) = view_scoped_parent(name).context("bad view name")?;
        let path = self.book_path(&id)?;
        let b = FileBook::open(&path)?;
        let set = match b.active()? {
            Some(d) => ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&d)?))
                .unwrap_or_default(),
            None => ratio_rules::RuleSet::default(),
        };
        let def = set
            .effective_views()
            .into_iter()
            .find(|v| v.id == view)
            .with_context(|| format!("no view {view:?} on this fund"))?;
        let mut out = view_pb(&id, &set, &def);

        // ⛔ THE FIGURES CAME OFF `Fund`, AND THEY LAND HERE RATHER THAN
        // NOWHERE. Every one depends on which entries are recognised, so this is
        // where they belong — but the maintained projection still folds the
        // whole journal with no cut, so it can only answer for a view that
        // recognises in journal order. Anything else refuses, loudly, rather
        // than returning the recorded view's numbers under another name.

        let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
        let proj = self.projection(&id)?;
        // ⭐ THE SAME `nav_from` EVERY FUND-LEVEL PREVIEW CALLS, so a NAV quoted
        // by `ApplyEvent` and a NAV shown on this screen cannot drift apart.
        //
        // ⚠ IT TAKES THE PROJECTION RATHER THAN FETCHING ONE. `projection`
        // hands back a CLONE, and a fund holding a quarter of a million open
        // lots does not want two of them alive to answer one request.
        let (nav, nav_strike) = nav_from(&b, &proj, &view, &rates)?;
        let realized = proj
            .realized(&view, set.chart_roles, &rates)
            .ok()
            .and_then(|r| r.value);

        let breaks = self.breaks_for(&path, &id, &view)?;
        let open: Vec<&pb::Break> = breaks.iter().filter(|k| !k.explained).collect();
        // ⛔ THE VIEW'S OWN COLUMNS, summed off its fold — `b.trial_balance()`
        // is the whole journal, which is one view's answer wearing no label.
        let balances = proj.balances(&view)?;
        let (td, tc) = balances
            .value
            .values()
            .fold((0i128, 0i128), |(d, c), r| (d + r.debit, c + r.credit));

        out.net_asset_value = nav.to_string();
        out.total_debit = td.to_string();
        out.total_credit = tc.to_string();
        out.open_difference = open
            .iter()
            .filter_map(|k| k.difference.parse::<i64>().ok().map(i64::abs))
            .sum::<i64>()
            .to_string();
        out.open_break_count = open.len() as i64;
        out.state = self.get_fund(&format!("funds/{id}"))?.state;
        out.realized_gain = realized.map(|r| r.gain.to_string()).unwrap_or_default();
        out.basis_relieved = realized.map(|r| r.basis.to_string()).unwrap_or_default();
        out.short_term_gain = realized.map(|r| r.short_term.to_string()).unwrap_or_default();
        out.long_term_gain = realized.map(|r| r.long_term.to_string()).unwrap_or_default();
        out.unclassified_gain =
            realized.map(|r| r.unclassified().to_string()).unwrap_or_default();
        out.open_lot_count = proj.open_lots(&view)?;
        out.position_count = proj.positions(&view)?.value.held.len() as i64;
        out.journal_position = proj.prefix() as i64;
        // The third coordinate of a settlement figure, and what the view could
        // not place — both off the fold that produced every number above.
        out.recognised_through = balances
            .through
            .map(|d| ratio_common::iso_date_from_days(i64::from(d)))
            .as_deref()
            .and_then(iso_date);
        out.unplaceable_entry_count = proj.unplaceable(&view)?.len() as i64;
        out.nav_strike = Some(ratio_proto::duration_proto::google::protobuf::Duration {
            seconds: nav_strike.as_secs() as i64,
            nanos: nav_strike.subsec_nanos() as i32,
        });
        Ok(out)
    }

    /// What two books of record over one journal disagree about.
    ///
    /// ⭐ A DERIVATION, NOT A SUBTRACTION. `Projection::reconcile` walks the two
    /// bands — bounded by the settlement lag, never the journal — and returns
    /// the LIST of entries one view recognises and the other does not, with the
    /// difference as what the list sums to. Both figures and the list come off
    /// ONE projection at ONE prefix; fetching two views separately would
    /// compare figures at two journal positions, which is
    /// `//tla:views_at_two_prefixes_check`'s failure.
    pub fn reconcile_views(&self, name: &str, against: &str) -> Result<pb::ReconcileViewsResponse> {
        let (id, view) = view_scoped_parent(name).context("bad view name")?;
        let path = self.book_path(&id)?;
        if against.is_empty() {
            bail!("reconciling is a question about two views — name the other with ?against=");
        }
        let b = FileBook::open(&path)?;
        let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
        let proj = self.projection(&id)?;
        let by_dim: BTreeMap<i64, AccountTypeRecord> =
            b.accounts()?.into_iter().map(|a| (a.dim, a.account_type)).collect();
        let is_al = |d: i64| {
            matches!(
                by_dim.get(&d),
                Some(AccountTypeRecord::Asset) | Some(AccountTypeRecord::Liability)
            )
        };
        let rec = proj.reconcile(&view, against, &is_al, &rates)?;
        let nav_here = proj.nav(&view, &is_al, &rates)?.value.0;
        let nav_there = proj.nav(against, &is_al, &rates)?.value.0;

        let date = |d: Option<ratio_project::views::Day>| {
            d.map(|n| ratio_common::iso_date_from_days(i64::from(n))).as_deref().and_then(iso_date)
        };
        let row = |e: &ratio_project::InFlightEntry| pb::RecognitionDifference {
            entry_id: e.id.clone(),
            memo: e.memo.clone(),
            trade_date: date(Some(e.trade_day)),
            recognised_here: date(e.recognised_here),
            recognised_there: date(e.recognised_there),
            net_asset_value_effect: e.effect.to_string(),
        };
        Ok(pb::ReconcileViewsResponse {
            name: name.to_string(),
            against: format!("funds/{id}/views/{against}"),
            net_asset_value: nav_here.to_string(),
            against_net_asset_value: nav_there.to_string(),
            difference: rec.value.difference.to_string(),
            recognised_here: rec.value.entries.iter().filter(|e| e.in_here).map(row).collect(),
            recognised_there: rec.value.entries.iter().filter(|e| !e.in_here).map(row).collect(),
            // ⛔ SHOWN, NOT OMITTED. These contribute to neither figure, and a
            // difference that looks fully explained while entries sit outside
            // both books of record is the shape of every defect in HANDOFF.md's
            // table. Entries only ONE view cannot place refuse the whole read,
            // inside `Projection::reconcile`.
            unplaceable: rec
                .value
                .unplaceable
                .iter()
                .map(|u| pb::RecognitionDifference {
                    entry_id: u.id.clone(),
                    memo: u.memo.clone(),
                    trade_date: date(u.trade_day),
                    recognised_here: None,
                    recognised_there: None,
                    net_asset_value_effect: "0".to_string(),
                })
                .collect(),
            journal_position: rec.prefix as i64,
        })
    }

    pub fn list_breaks(&self, parent: &str, filter: &str) -> Result<pb::ListBreaksResponse> {
        let (id, view) = view_scoped_parent(parent).context("bad parent")?;
        let path = self.book_path(&id)?;
        let mut breaks = self.breaks_for(&path, &id, &view)?;
        breaks.retain(|k| match filter {
            "blocking" => k.severity == pb::Severity::High as i32,
            "unexplained" => !k.explained,
            _ => true,
        });
        Ok(pb::ListBreaksResponse { breaks, next_page_token: String::new() })
    }

    pub fn get_break(&self, name: &str) -> Result<pb::Break> {
        let (fund, view, brk) = view_scoped_id(name, "breaks").context("bad break name")?;
        let path = self.book_path(&fund)?;
        self.breaks_for(&path, &fund, &view)?
            .into_iter()
            .find(|k| k.name.ends_with(&format!("/breaks/{brk}")))
            .with_context(|| format!("no break {brk:?} on {fund:?}"))
    }

    pub fn list_config_versions(&self, parent: &str) -> Result<pb::ListConfigVersionsResponse> {
        let id = resource_id(parent, "funds").context("bad parent")?;
        let mut versions = self.config_versions(&id)?;
        versions.reverse(); // newest first
        Ok(pb::ListConfigVersionsResponse {
            config_versions: versions,
            next_page_token: String::new(),
        })
    }

    pub fn get_config_version(&self, name: &str) -> Result<pb::ConfigVersion> {
        let (fund, digest) = nested_id(name, "funds", "configVersions").context("bad name")?;
        self.config_versions(&fund)?
            .into_iter()
            .find(|v| v.digest == digest)
            .with_context(|| format!("no configuration version {digest:?}"))
    }

    /// What changed between two versions.
    ///
    /// `base` defaults to the version immediately before `name` in this fund's
    /// history — the comparison somebody almost always wants, and the one that
    /// is tedious to look up by hand.
    pub fn diff_config_versions(
        &self,
        name: &str,
        base: &str,
    ) -> Result<pb::DiffConfigVersionsResponse> {
        let (fund, digest) = nested_id(name, "funds", "configVersions").context("bad name")?;
        let path = self.book_path(&fund)?;
        let versions = self.config_versions(&fund)?;

        let at = versions
            .iter()
            .position(|v| v.digest == digest)
            .with_context(|| format!("no configuration version {digest:?}"))?;
        let base_digest = if base.is_empty() {
            // The first version has nothing before it, so the base is the
            // empty rule set and every rule reads as ADDED. That is what
            // happened: a book opens with no rules and the opening
            // configuration puts them there.
            versions.get(at.wrapping_sub(1)).map(|v| v.digest.clone()).unwrap_or_default()
        } else {
            base.to_string()
        };

        let b = FileBook::open(&path)?;
        let chart = b.accounts()?;
        let load = |d: &str| -> Result<RuleSet> {
            if d.is_empty() {
                return Ok(RuleSet::default());
            }
            let dig = ratio_store::Digest::parse(d)?;
            RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&dig)?))
        };
        let (from, to) = (load(&base_digest)?, load(&digest)?);

        let render = |r: &ratio_rules::Rule| ratio_rules::render(r, &chart);
        let mut changes = Vec::new();
        for r in &to.rules {
            match from.rule(&r.id) {
                None => changes.push(pb::RuleChange {
                    rule_id: r.id.clone(),
                    kind: pb::rule_change::Kind::Added as i32,
                    base_form: String::new(),
                    form: render(r),
                }),
                Some(prev) if prev != r => changes.push(pb::RuleChange {
                    rule_id: r.id.clone(),
                    kind: pb::rule_change::Kind::Changed as i32,
                    base_form: render(prev),
                    form: render(r),
                }),
                Some(_) => {}
            }
        }
        for r in &from.rules {
            if to.rule(&r.id).is_none() {
                changes.push(pb::RuleChange {
                    rule_id: r.id.clone(),
                    kind: pb::rule_change::Kind::Removed as i32,
                    base_form: render(r),
                    form: String::new(),
                });
            }
        }
        changes.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));

        Ok(pb::DiffConfigVersionsResponse { base_digest, digest, changes })
    }

    /// Every configuration this fund has run, oldest first.
    ///
    /// `config/HISTORY` is the sequence; `CHANGELOG` supplies who promoted each
    /// one. A version with no changelog line gets an empty actor rather than a
    /// guessed one — an audit trail that invents an actor is worse than one
    /// that admits a gap.
    fn config_versions(&self, fund: &str) -> Result<Vec<pb::ConfigVersion>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let active = b.active()?.map(|d| d.as_str().to_string()).unwrap_or_default();
        let chart = b.accounts()?;

        let mut promoted: BTreeMap<String, (i64, String, String)> = BTreeMap::new();
        for l in std::fs::read_to_string(path.join("CHANGELOG")).unwrap_or_default().lines() {
            let f: Vec<&str> = l.split('\t').collect();
            // ⛔ ONLY AN "approved" LINE IS A PROMOTION. The CHANGELOG is the
            // fund's whole audit trail — it also carries who posted, ingested,
            // marked and admitted, each with a config digest in the last field.
            // Without this filter a posted-event line, keyed by that digest,
            // would overwrite the real promotion and report the last person who
            // posted under a configuration as the one who approved it.
            if f.len() >= 5 && f[2] == "approved" {
                promoted.insert(
                    f[4].to_string(),
                    (f[0].parse().unwrap_or(0), f[1].to_string(), f[3].to_string()),
                );
            }
        }

        // ⚠ `history()` returns NEWEST FIRST — the trait says so, while the
        // struct's own layout comment says `config/HISTORY` is oldest first.
        // Both are true and they sit fifty lines apart, which is how this was
        // written backwards: sequence numbers counted from the newest end and
        // the diff's "previous version" walked forwards in time.
        let mut history = b.history()?;
        history.reverse(); // oldest first, so `sequence` means what it says

        let mut out = Vec::new();
        for (i, digest) in history.iter().enumerate() {
            let d = digest.as_str().to_string();
            let rules = match RuleSet::from_toml(&String::from_utf8_lossy(&b.get(digest)?)) {
                Ok(set) => set.rules.iter().map(|r| ratio_rules::render(r, &chart)).collect(),
                // A version that no longer parses is still part of the history
                // and is shown as such. Dropping it would make the sequence lie
                // about what this fund has run.
                Err(e) => vec![format!("(does not parse: {e})")],
            };
            let (time, actor, subject) = promoted.get(&d).cloned().unwrap_or_default();
            out.push(pb::ConfigVersion {
                name: format!("funds/{fund}/configVersions/{d}"),
                digest: d.clone(),
                sequence: i as i64 + 1,
                active: d == active,
                actor,
                approve_time: (time > 0).then(|| {
                    ratio_proto::timestamp_proto::google::protobuf::Timestamp {
                        seconds: time,
                        nanos: 0,
                    }
                }),
                subject,
                rules,
            });
        }
        Ok(out)
    }

    pub fn list_nav_strikes(&self, parent: &str) -> Result<pb::ListNavStrikesResponse> {
        let (id, view) = view_scoped_parent(parent).context("bad parent")?;
        let path = self.book_path(&id)?;
        Ok(pb::ListNavStrikesResponse {
            nav_strikes: {
                // The qualification travels WITH the figure. Same derivation as
                // `stale_strikes`: a strike pins a journal position and an
                // applied action is a journal entry, so nothing is stored.
                let stale = self.stale_strikes(&id).unwrap_or_default();
                ratio_nav::list_in(&path, &view)?
                    .into_iter()
                    .map(|s| {
                        let why: Vec<String> = stale
                            .iter()
                            .filter(|(strike, _, _)| *strike == s.id)
                            .map(|(_, _, why)| why.clone())
                            .collect();
                        to_pb(&id, &s, &why)
                    })
                    .collect()
            },
            next_page_token: String::new(),
        })
    }

    pub fn get_nav_strike(&self, name: &str) -> Result<pb::NavStrike> {
        let (fund, view, id) =
            view_scoped_id(name, "navStrikes").context("bad name")?;
        let path = self.book_path(&fund)?;
        let s = ratio_nav::get(&path, &view, &id)?;
        // Getting one strike qualifies it the same way listing them does — a
        // figure fetched on its own must not look sounder than the same figure
        // in a list.
        let why: Vec<String> = self
            .stale_strikes(&fund)
            .unwrap_or_default()
            .into_iter()
            .filter(|(strike, _, _)| *strike == s.id)
            .map(|(_, _, why)| why)
            .collect();
        Ok(to_pb(&fund, &s, &why))
    }

    /// The corporate actions announced on this fund, applied or not.
    ///
    /// ⛔ EVERY FIELD IS DERIVED FROM THE JOURNAL, nothing about application is
    /// stored beside the announcement. The applied action IS a journal entry,
    /// so a book cannot come to disagree with itself about whether one
    /// happened — which matters more here than anywhere else on this surface,
    /// because `Ratio.Actions.applying_twice_is_not_applying_once` means a
    /// second application would double the position while the trial balance
    /// went on tying. A stored `applied` flag that drifted would be the thing
    /// that let that happen.
    pub fn list_corporate_actions(
        &self,
        parent: &str,
    ) -> Result<pb::ListCorporateActionsResponse> {
        let id = resource_id(parent, "funds").context("bad parent")?;
        Ok(pb::ListCorporateActionsResponse {
            corporate_actions: self.corporate_actions(&id)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_corporate_action(&self, name: &str) -> Result<pb::CorporateAction> {
        let (fund, id) = nested_id(name, "funds", "corporateActions").context("bad name")?;
        self.corporate_actions(&fund)?
            .into_iter()
            .find(|a| a.name == format!("funds/{fund}/corporateActions/{id}"))
            .with_context(|| format!("{name:?} is not an announced corporate action"))
    }

    /// Every announcement on a fund, with the journal position that pins it.
    ///
    /// ⛔ TWO SOURCES, AND THE DIFFERENCE IS NOT COSMETIC. An announcement in
    /// the JOURNAL sits inside the prefix every later strike pins, so a replay
    /// re-derives the same figure forever
    /// (`Ratio.Actions.Factor.replay_is_determined_by_the_prefix`). One in
    /// `Plane::Actions` — where every book written before this change has them
    /// — is pinned by nothing, so under the factor representation a replay
    /// would read whatever arrived since and answer differently.
    ///
    /// ⚠ THE OLD ONES ARE STILL READ, and reported with position 0 rather than
    /// hidden or silently migrated. A book cannot be made retroactively
    /// pinnable: the announcements were not in the order, and no rewrite can
    /// put them there without changing every digest after the point. Saying so
    /// is the only honest option.
    fn announcements(&self, fund: &str) -> Result<Vec<(ratio_store::AnnouncementRecord, usize)>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        // ⛔ STREAMED. Bounded by the number of ANNOUNCEMENTS.
        let mut out: Vec<(ratio_store::AnnouncementRecord, usize)> = Vec::new();
        {
            let mut i = 0usize;
            b.for_each_entry_since(0, &mut |e| {
                i += 1;
                if let Some(a) = &e.announcement {
                    out.push((a.clone(), i));
                }
                Ok(())
            })?;
        }

        // Books written before announcements were journal entries.
        let known: BTreeSet<String> = out.iter().map(|(a, _)| a.id.clone()).collect();
        for a in b.records::<ratio_ingest::actions::Announced>(Plane::Actions)? {
            if known.contains(&a.id) {
                continue;
            }
            out.push((
                ratio_store::AnnouncementRecord {
                    id: a.id,
                    instrument: a.instrument,
                    numerator: a.split.num,
                    denominator: a.split.den,
                    ex_date: a.ex_date,
                    announced: a.announced,
                },
                0, // ⛔ pinned by nothing
            ));
        }
        Ok(out)
    }

    /// Every announced action on a fund, with what the journal says about it.
    fn corporate_actions(&self, fund: &str) -> Result<Vec<pb::CorporateAction>> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;

        // Where each action landed, if it landed. Same derivation as
        // `stale_strikes` — one reading of the journal, not two conventions.
        // ⛔ STREAMED. Bounded by the number of ACTIONS, not by the journal.
        let mut applied_at: BTreeMap<String, usize> = BTreeMap::new();
        {
            let mut i = 0usize;
            b.for_each_entry_since(0, &mut |e| {
                if let Some(a) = e.id.strip_prefix("action-") {
                    applied_at.insert(a.to_string(), i);
                }
                i += 1;
                Ok(())
            })?;
        }

        let strikes = ratio_nav::list(&path)?;

        self.announcements(fund)?
            .into_iter()
            .map(|(a, announce_at)| {
                let at = applied_at.get(&a.id).copied();

                // The strikes this action was not in. The reverse of
                // `NavStrike.qualification`: the strike knows what qualifies
                // it, and only the action knows everything it disturbed.
                let qualified: Vec<String> = strikes
                    .iter()
                    .filter(|s| {
                        let day = ratio_nav::rfc3339(s.valuation_time);
                        let day = day.get(..10).unwrap_or("");
                        day >= a.ex_date.as_str()
                            && !at.is_some_and(|i| i < s.journal_position)
                    })
                    .map(|s| format!("funds/{fund}/views/{}/navStrikes/{}", s.view, s.id))
                    .collect();

                Ok(pb::CorporateAction {
                    name: format!("funds/{fund}/corporateActions/{}", a.id),
                    instrument: a.instrument.clone(),
                    numerator: a.numerator.to_string(),
                    denominator: a.denominator.to_string(),
                    // Rendered once, here, so two screens cannot disagree about
                    // how to say a ratio out loud.
                    form: format!("{}-for-{}", a.numerator, a.denominator),
                    ex_date: iso_date(&a.ex_date),
                    // Zero yields none rather than the epoch: an announcement
                    // time we do not have should read as absent, not as 1970.
                    announce_time: (a.announced > 0).then(|| {
                        ratio_proto::timestamp_proto::google::protobuf::Timestamp {
                            seconds: a.announced,
                            nanos: 0,
                        }
                    }),
                    applied: at.is_some(),
                    journal_position: at.map(|i| i as i64 + 1).unwrap_or(0),
                    // ⛔ ZERO MEANS PINNED BY NOTHING — the announcement is in
                    // `Plane::Actions`, outside every strike's prefix, so a
                    // replay could answer differently as more arrives.
                    announce_position: announce_at as i64,
                    qualified_nav_strikes: qualified,
                })
            })
            .collect()
    }

    /// Re-derive a strike. Read-only: it folds a journal prefix and compares.
    pub fn replay_nav_strike(&self, name: &str) -> Result<pb::ReplayNavStrikeResponse> {
        let (fund, view, id) =
            view_scoped_id(name, "navStrikes").context("bad name")?;
        let path = self.book_path(&fund)?;
        let s = ratio_nav::get(&path, &view, &id)?;
        // ⛔ AND THE REPLAY RESOLVES THE VIEW'S TERMS FROM THE DIGEST EACH ENTRY
        // PINNED, which `NavFold::def_for` does — never from ACTIVE. A calendar
        // amended since would otherwise re-derive a different settlement date,
        // and this endpoint would report a sound strike as unreproducible.
        let r = ratio_nav::replay(&path, &s)?;
        Ok(pb::ReplayNavStrikeResponse {
            name: name.to_string(),
            history_intact: r.history_intact,
            reproduced: r.reproduced,
            net_asset_value: r.net_asset_value.to_string(),
            journal_digest: r.journal_digest,
        })
    }

    /// What this strike did, step by step, and what the alternatives cost.
    ///
    /// ⛔ THE ESTIMATE IS GATED BY THE SAME PREDICATE `GetView` USES, AND ITS
    /// REFUSAL IS CARRIED RATHER THAN SWALLOWED. The maintained projection folds
    /// the whole journal with no cut, so it cannot supply a shape for a trade-
    /// or settlement-basis view; returning the recorded view's securities,
    /// currencies and lots under another view's name is the exact defect
    /// multi-view books exist to prevent, one layer out from `ReconcileViews`.
    ///
    /// ⚠ AND `analyze` STILL WORKS FOR SUCH A VIEW, because the fold DOES cut.
    /// So the screen can be empty of estimates and full of measurements, which
    /// is an honest state and not a broken one.
    ///
    /// ⛔ NO BLOCKED-FUND GATE, AND THAT MATCHES `get_nav_strike` AND
    /// `replay_nav_strike` RATHER THAN DIVERGING FROM THEM. `refuse_if_blocked`
    /// guards STRIKING — taking a new figure while something is unpriced or a
    /// break is unexplained. This reads a strike somebody already took and
    /// signed. A blocked fund has no strike to explain, so `ratio_nav::get`
    /// refuses first and for the right reason; adding a second gate here would
    /// be a third answer to "may this fund's figures be read", which is the
    /// disagreement `blocking_at` exists to make impossible.
    pub fn explain_nav_strike(
        &self,
        name: &str,
        analyze: bool,
    ) -> Result<pb::ExplainNavStrikeResponse> {
        let (fund, view, id) = view_scoped_id(name, "navStrikes").context("bad name")?;
        let path = self.book_path(&fund)?;
        let s = ratio_nav::get(&path, &view, &id)?;
        let cal = ratio_nav::closure::rate_for(&path);

        let b = FileBook::open(&path)?;
        let accounts = b.accounts()?.len() as i64;
        let proj = self.projection(&fund)?;
        let (shape, refusal) = match ratio_nav::shape_of(&proj, &view, accounts, cal) {
            Ok(s) => (Some(s), String::new()),
            // ⛔ `{e:#}` — the WHOLE chain. The refusal's prose is the answer
            // this endpoint gives, and truncating it to the outermost line
            // would leave a screen saying "cannot" without saying why.
            Err(e) => (None, format!("{e:#}")),
        };

        // ⛔ MEASURED NOW, RE-DERIVING THE PINNED PREFIX — NOT WHAT THE ORIGINAL
        // STRIKE COST. Nothing was recorded at strike time. `ExplainNavStrikeResponse
        // .analyzed` is what lets the screen say so, and it says so beside every
        // actual rather than once at the bottom.
        let measured = if analyze { Some(ratio_nav::analyze(&path, &s)?) } else { None };

        let plan = ratio_nav::explain::plan_of(name, &s, shape.as_ref(), &refusal, measured.as_ref(), cal);
        Ok(plan_pb(&plan))
    }

    pub fn list_change_log_entries(&self, parent: &str) -> Result<pb::ListChangeLogEntriesResponse> {
        let id = resource_id(parent, "funds").context("bad parent")?;
        let path = self.book_path(&id)?;
        Ok(pb::ListChangeLogEntriesResponse {
            change_log_entries: self.change_log_for(&path, &id)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_change_log_entry(&self, name: &str) -> Result<pb::ChangeLogEntry> {
        let (fund, entry) = nested_id(name, "funds", "changeLogEntries").context("bad name")?;
        let path = self.book_path(&fund)?;
        self.change_log_for(&path, &fund)?
            .into_iter()
            .find(|e| e.name.ends_with(&format!("/changeLogEntries/{entry}")))
            .with_context(|| format!("no change-log entry {entry:?}"))
    }

    // ── derivation ────────────────────────────────────────────────────────

    /// The breaks from the newest stored report, enriched with the postings
    /// behind Ratio's figure.
    ///
    /// Returns none rather than failing when a fund has no report: a fund that
    /// has not been reconciled yet is a normal state on a NAV morning, not an
    /// error.
    /// Sales the lot engine could not relieve, as breaks.
    ///
    /// ⛔ IN THE EXCEPTIONS LIST, NOT A SCREEN OF THEIR OWN. A lot break is an
    /// unresolved difference on a fund that an operator has to work — which is
    /// exactly what a break is — and inventing a second place to look is how a
    /// thing gets looked at by nobody. `Ratio.Lots.Edges` and
    /// `partial_relief_is_exactly_pro_rata` are what produce them.
    ///
    /// ⚠ SEVERITY HIGH, and not because of the amount. Every other break is
    /// graded by how much money is at stake; these are graded by what they mean.
    /// A sale that could not be relieved leaves the lot book and the position
    /// disagreeing, and the figure it corrupts is the REALIZED GAIN — which no
    /// reconciliation reaches, because it has no counterparty. A small one is
    /// not a small problem.
    fn lot_breaks_for(&self, fund: &str, view: &str) -> Result<Vec<pb::Break>> {
        let proj = self.projection(fund)?;
        Ok(proj
            .lot_breaks(view)?
            .iter()
            .enumerate()
            .map(|(i, why)| pb::Break {
                name: format!("funds/{fund}/views/{view}/breaks/lot-{}", i + 1),
                account: "Tax lots".into(),
                account_dimension: 0,
                severity: pb::Severity::High as i32,
                explained: false,
                cause: why.clone(),
                ratio_amount: "0".into(),
                reported_amount: "0".into(),
                difference: "0".into(),
                postings: Vec::new(),
                config_digest: String::new(),
                // ⚠ NO TOLERANCE, BECAUSE NO TOLERANCE DECIDED THIS. A lot
                // break is HIGH by what it means, not by how much, so reporting
                // bounds beside it would suggest a different number would have
                // graded it differently. None ever would.
                tolerance: None,
                // ⚠ And a lot break never gains one: `accept_explanation`
                // refuses them, because their names are positions in a list.
                explanation: None,
            })
            .collect())
    }

    /// Everything standing between this fund and a NAV.
    ///
    /// ⛔ ONE DERIVATION, READ BY BOTH THE SCREEN AND THE REFUSAL. `get_fund`
    /// computes `STATE_BLOCKED` from this and `ratio strike` refuses on it, so
    /// the badge an operator is looking at and the reason their command was
    /// declined cannot come to disagree. That is the property
    /// `unpriced_at`'s doc comment names — the message and the decision being
    /// one derivation — and the failure it prevents is the ordinary one: two
    /// folds of "what blocks", both plausible, drifting a field apart.
    ///
    /// ⚠ THE PENDING FOLD HAS TO BE THIS ONE. `pending_of` drops facts already
    /// posted to the journal, a filter that exists because a duplicate
    /// identifier arriving later would otherwise re-block a NAV that was
    /// already struck — found by `//tla:control_plane_check`. The CLI's own
    /// `pending` screen does not filter, so reaching for it here would refuse
    /// strikes for a reason the console never shows.
    ///
    /// ⚠ THE DEFAULT VIEW'S BREAKS, because that is what `Fund.state` reports
    /// and what `ratio strike` refuses over. Taking a view parameter would let
    /// a caller ask whether some OTHER book of record is ready and get an answer
    /// the badge contradicts — the disagreement this whole method exists to make
    /// impossible. A per-view gate is a real question and a later one; it needs
    /// the screen to say which view it refused for.
    pub fn blocking_at(&self, fund: &str) -> Result<Blocking> {
        let path = self.book_path(fund)?;
        let view = self.default_view_of(fund)?;
        let breaks = self.breaks_for(&path, fund, &view)?;
        Ok(Blocking {
            breaks: breaks
                .into_iter()
                .filter(|k| !k.explained && k.severity == pb::Severity::High as i32)
                .collect(),
            pending: self.pending_of(fund)?,
        })
    }

    /// The newest explanation recorded for each break on this fund.
    ///
    /// A fold over the plane, newest wins. Nothing is indexed and nothing is
    /// retracted: a correction is a later record for the same break name.
    fn explanations_of(&self, book: &Path) -> Result<BTreeMap<String, BreakExplanation>> {
        let b = FileBook::open(book)?;
        let mut out: BTreeMap<String, BreakExplanation> = BTreeMap::new();
        for e in b.records::<BreakExplanation>(Plane::Explanations)? {
            out.insert(e.break_id.clone(), e);
        }
        Ok(out)
    }

    /// Record why a difference is acceptable.
    ///
    /// ⛔ THE ONE IMPLEMENTATION. `ratio accept` calls this; a second copy for
    /// the CLI would be a second set of decisions about what counts as
    /// explaining something.
    ///
    /// ⛔ AND THERE IS NO RPC FOR IT, DELIBERATELY. Acceptance is a verb that
    /// changes what a fund is allowed to do, and this console offers no way to
    /// perform one — the same fence that keeps `approve_rule` off the model's
    /// tool list and the approve button off the rules screen. The mechanism
    /// enforcing it is not discipline: `console/scripts/route_manifest_test.py`
    /// requires every contract route to be called by the client and every
    /// client call to be read by a screen, so an `AcceptBreakExplanation` RPC
    /// would DEMAND the write screen that must not exist.
    pub fn accept_explanation(&self, break_name: &str, text: &str) -> Result<pb::BreakExplanation> {
        // ⛔ THE VIEW-SCOPED NAME, BECAUSE THAT IS THE ONE A PERSON HAS. Both
        // the refusal `ratio strike` prints and the console's URL are
        // `funds/*/views/*/breaks/*`; accepting the shorter form would mean the
        // name somebody was shown is not the name this verb takes.
        let (fund, view, want) =
            view_scoped_id(break_name, "breaks").context("bad break name")?;
        let path = self.book_path(&fund)?;

        let text = text.trim();
        if text.is_empty() {
            bail!("an explanation with no words in it explains nothing");
        }

        // ⛔ THE BREAK HAS TO BE THERE. An explanation naming nothing is a
        // citation that does not resolve, and ORCHESTRATION.md's proposal shape
        // requires those to fail before a person reads them — here, before one
        // is recorded at all.
        let breaks = self.breaks_for(&path, &fund, &view)?;
        let Some(brk) = breaks.iter().find(|k| break_id_of(&k.name) == want.as_str()) else {
            bail!(
                "no break {break_name} on this fund — the breaks it does have are listed by \
                 `ratio watch` and on the exceptions screen"
            );
        };

        // ⚠ A LOT BREAK CANNOT BE EXPLAINED, AND THE REFUSAL NAMES WHAT DOES
        // CLEAR IT. Lot break names are `lot-{n}` — a POSITION IN A LIST — so an
        // explanation keyed on one would follow the position rather than the
        // sale the moment an earlier lot break clears: every citation still
        // resolves, the books still tie, and the words are attached to a
        // different disposal. Making those names durable is a `ratio-project`
        // change and its own commit.
        if brk.tolerance.is_none() && brk.name.contains("/breaks/lot-") {
            bail!(
                "a lot break is not explained, it is corrected. The lot book and the position \
                 disagree, which corrupts the realized gain — the figure with no counterparty — \
                 and what closes it is an entry that makes them agree, not a note saying the \
                 difference is acceptable."
            );
        }

        let b = FileBook::open(&path)?;
        let mut position = 0u64;
        b.for_each_entry_since(0, &mut |_| {
            position += 1;
            Ok(())
        })?;

        let record = BreakExplanation {
            break_id: want.to_string(),
            text: text.to_string(),
            actor: self.actor.clone().unwrap_or_default(),
            accept_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            difference: brk.difference.parse().unwrap_or(0),
            config_digest: brk.config_digest.clone(),
            journal_position: position,
            journal_digest: String::new(),
        };

        let mut w = FileBook::open(&path)?;
        w.append_record(Plane::Explanations, &record)?;
        self.record_change(&path, "accepted", break_name, &record.config_digest)?;
        Ok(to_pb_explanation(&record))
    }

    /// The tolerance a report's breaks are graded against, or `None` when the
    /// question cannot be answered.
    ///
    /// ⛔ AND `None` MEANS BLOCKING, NOT DEFAULT. Three things reach it: a
    /// digest that is not a digest, one naming bytes the book does not hold,
    /// and bytes that are not a rule set. In every one of them the honest
    /// statement is "this difference was not graded" — and the whole product
    /// argument is that a figure nobody could check is not a figure. Falling
    /// back to the custom bands would certify a break as small using a
    /// tolerance nobody could read, on a book that ties, which is the failure
    /// this repository exists not to have. Erring towards blocking costs an
    /// operator a look; erring the other way costs a NAV.
    ///
    /// ⚠ IT DOES NOT ERROR. A fund whose oldest report names a configuration
    /// that has since been pruned should still show its breaks — as blocking —
    /// rather than turning the whole screen into a stack trace.
    fn tolerance_of(&self, b: &FileBook, digest: &str) -> Option<(ratio_rules::Tolerance, bool)> {
        let d = ratio_store::Digest::parse(digest).ok()?;
        let bytes = b.get(&d).ok()?;
        let set = RuleSet::from_toml(&String::from_utf8_lossy(&bytes)).ok()?;
        Some((set.effective_tolerance(), set.tolerance.is_some()))
    }

    /// Attach whatever somebody recorded about each break, and decide whether
    /// it still stands.
    ///
    /// ⭐ AN EXPLANATION IS CURRENT WHEN THE BREAK IT NAMES STILL SHOWS THE
    /// SAME DIFFERENCE UNDER THE SAME CONFIGURATION. That sentence is the whole
    /// staleness design, and both halves of it were chosen against a specific
    /// failure:
    ///
    /// ⛔ POSTING AN ENTRY MUST NOT UN-EXPLAIN A BREAK. The obvious rule —
    /// an explanation names the journal prefix it read, so a longer journal
    /// makes it stale — retires every explanation on the next posting. On a NAV
    /// morning the journal grows constantly, so the gate becomes one nobody can
    /// ever clear, in a way that looks like the software being careful.
    /// `//tla:explanation_pinned_to_the_prefix_check` is that design, model
    /// checked, deadlocking.
    ///
    /// ⛔ AND A NEW FIGURE MUST RETIRE THE OLD EXPLANATION. "The 2,000.00 is
    /// the custodian's unsettled dividend" is a claim about 2,000.00. When the
    /// next reconciliation reports 2,750.00, the words are about something that
    /// is no longer there — and an explanation that outlived its figure is how
    /// a fund gets struck on a difference nobody has actually looked at.
    /// `//tla:stale_explanation_unblocks_check` is that one.
    ///
    /// Nothing about staleness is stored. It is two values compared, both of
    /// which are already on the record — the same arrangement `Plane::Actions`
    /// uses to identify a strike taken before an action without storing
    /// anything either.
    fn explain(&self, breaks: &mut [pb::Break], recorded: &BTreeMap<String, BreakExplanation>) {
        for k in breaks.iter_mut() {
            let Some(e) = recorded.get(break_id_of(&k.name)) else { continue };
            let same_figure = e.difference.to_string() == k.difference;
            let same_terms = e.config_digest == k.config_digest;
            k.explained = same_figure && same_terms;
            k.explanation = Some(to_pb_explanation(e));
            if !k.explained {
                let why = if !same_figure {
                    format!(
                        "explained against a difference of {}; this report shows {}",
                        e.difference, k.difference
                    )
                } else {
                    "explained under a different configuration".to_string()
                };
                if let Some(x) = k.explanation.as_mut() {
                    x.qualification = vec![why];
                }
            }
        }
    }

    fn breaks_for(&self, book: &Path, fund: &str, view: &str) -> Result<Vec<pb::Break>> {
        let Some(report) = newest_report(book)? else {
            // ⚠ STILL RETURNS THE LOT BREAKS. A fund with no reconciliation
            // report has no recon breaks, and used to have no breaks at all —
            // so a lot break on such a fund would have been invisible for the
            // want of an unrelated file.
            return self.lot_breaks_for(fund, view);
        };
        let b = FileBook::open(book)?;
        let dims: BTreeMap<String, i64> =
            b.accounts()?.into_iter().map(|a| (a.display_name, a.dim)).collect();
        // ⛔ ONE PASS, FOR THE DIMENSIONS THAT ACTUALLY HAVE BREAKS. This held
        // the whole journal and re-filtered it once PER BREAK — O(breaks x
        // entries) time on top of O(entries) memory. Streaming naively would be
        // worse still: a fresh read of the journal per break.
        let wanted: std::collections::BTreeSet<i64> = report
            .breaks
            .iter()
            .map(|l| dims.get(&l.display_name).copied().unwrap_or(l.account))
            .collect();
        let mut by_dim: BTreeMap<i64, Vec<pb::BreakPosting>> = BTreeMap::new();
        b.for_each_entry_since(0, &mut |e| {
            for p in &e.postings {
                if wanted.contains(&p.dim) {
                    by_dim.entry(p.dim).or_default().push(pb::BreakPosting {
                        entry_id: e.id.clone(),
                        memo: e.memo.clone(),
                        amount: p.amount.to_string(),
                        config_digest: e.config.short().to_string(),
                    });
                }
            }
            Ok(())
        })?;

        // ⛔ GRADED BY THE CONFIGURATION THE REPORT NAMES, NOT THE ONE IN FORCE
        // NOW, and resolved once rather than per line. A break is a comparison
        // between two figures produced under one configuration; the tolerance
        // agreed then is the term that applies to it. Reading `active()` here
        // would regrade a report whose bytes have not changed the moment
        // somebody promotes a new rule set — the shape
        // `an_unpinned_announcement_changes_the_answer` is about, applied to a
        // severity instead of a factor.
        let graded_under = self.tolerance_of(&b, &report.config_digest);
        let mut out = Vec::new();
        for line in &report.breaks {
            let diff: i64 = line.difference;
            let severity = match graded_under {
                Some((t, _)) => match t.severity(diff) {
                    ratio_rules::Severity::High => pb::Severity::High,
                    ratio_rules::Severity::Medium => pb::Severity::Medium,
                    ratio_rules::Severity::Low => pb::Severity::Low,
                },
                // ⛔ ANYTHING THE GRADER COULD NOT ANSWER GRADES HIGH. See
                // `tolerance_of`.
                None => pb::Severity::High,
            };
            let dim = dims.get(&line.display_name).copied().unwrap_or(line.account);

            let postings: Vec<pb::BreakPosting> =
                by_dim.get(&dim).cloned().unwrap_or_default();

            out.push(pb::Break {
                // Derived from the dimension, so a break keeps the same URL
                // across two runs of the same period. A name that moved every
                // time the report was regenerated would make every link in an
                // email dead by morning.
                name: format!("funds/{fund}/views/{view}/breaks/{dim}"),
                account: line.display_name.clone(),
                account_dimension: dim,
                severity: severity as i32,
                // ⚠ FALSE HERE, AND SET BY `explain` BELOW IF SOMEBODY WROTE
                // ONE. The default is the honest one: a break the software
                // decided was fine, with no person's words behind it, is
                // exactly the thing this product exists not to do. Only a
                // recorded explanation moves it, and only while it still names
                // this figure under these terms.
                explained: false,
                cause: cause_text(line.cause),
                ratio_amount: line.ratio_amount.to_string(),
                reported_amount: line.reported_amount.to_string(),
                difference: diff.to_string(),
                postings,
                config_digest: report.config_digest.clone(),
                tolerance: graded_under.map(|(t, declared)| pb::Tolerance {
                    below_notice: t.below_notice.to_string(),
                    blocks_nav: t.blocks_nav.to_string(),
                    declared,
                }),
                explanation: None,
            });
        }
        // Largest first: the queue is ordered by money, because that is the
        // order an operator with a deadline works in.
        out.sort_by_key(|k| -k.difference.parse::<i64>().unwrap_or(0).abs());
        // ⛔ And the lot engine's, in the same list.
        out.extend(self.lot_breaks_for(fund, view)?);
        // What anybody has recorded about them, and whether it still stands.
        let recorded = self.explanations_of(book)?;
        self.explain(&mut out, &recorded);
        Ok(out)
    }

    /// What `ratio approve` recorded, newest first, with `config/HISTORY` as
    /// the fallback for configurations that predate the record.
    fn change_log_for(&self, book: &Path, fund: &str) -> Result<Vec<pb::ChangeLogEntry>> {
        let mut out = Vec::new();
        let text = std::fs::read_to_string(book.join("CHANGELOG")).unwrap_or_default();
        for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 5 {
                continue; // a truncated line is skipped, not guessed at
            }
            out.push(pb::ChangeLogEntry {
                name: format!("funds/{fund}/changeLogEntries/{i}"),
                actor: f[1].to_string(),
                actor_kind: pb::ActorKind::Person as i32,
                action: f[2].to_string(),
                subject: f[3].to_string(),
                detail: String::new(),
                config_digest: f[4].to_string(),
            });
        }

        // Proposals nobody has approved: the other half of the story, and the
        // only place a model appears in this log.
        if let Ok(rd) = std::fs::read_dir(book.join("proposals")) {
            let mut ids: Vec<String> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "toml"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                .collect();
            ids.sort();
            for id in ids {
                if out.iter().any(|e| e.subject == id && e.action == "approved") {
                    continue; // already approved; it appears above as a person's act
                }
                out.push(pb::ChangeLogEntry {
                    name: format!("funds/{fund}/changeLogEntries/proposal-{id}"),
                    actor: "Assistant".into(),
                    actor_kind: pb::ActorKind::Model as i32,
                    action: "drafted".into(),
                    subject: id,
                    detail: "awaiting a person".into(),
                    config_digest: "proposal".into(),
                });
            }
        }
        out.reverse(); // newest first
        Ok(out)
    }
}

fn to_pb(fund: &str, s: &ratio_nav::Strike, why: &[String]) -> pb::NavStrike {
    pb::NavStrike {
        // ⛔ THE VIEW IS IN THE NAME, NOT ONLY IN THE FIELD. `id_for` derives the
        // id from the valuation time alone, so two views striking one moment
        // share it — and a resource name that did not carry the view would name
        // two different figures.
        name: format!("funds/{fund}/views/{}/navStrikes/{}", s.view, s.id),
        view: s.view.clone(),
        // The generated crate's own Timestamp, re-exported by ratio_proto — NOT
        // `prost_types::Timestamp`. rules_rust_prost compiles the well-known
        // types itself, so the two are distinct types with the same name and
        // the same shape, and the compiler says so in as many words.
        valuation_time: Some(ratio_proto::timestamp_proto::google::protobuf::Timestamp {
            seconds: s.valuation_time,
            nanos: 0,
        }),
        actor: s.actor.clone(),
        journal_position: s.journal_position as i64,
        journal_digest: s.journal_digest.clone(),
        net_asset_value: s.net_asset_value.to_string(),
        trial_balance_difference: s.trial_balance_difference.to_string(),
        config_digest: s.config_digest.clone(),
        qualification: why.to_vec(),
    }
}

fn newest_report(book: &Path) -> Result<Option<kernel::BreakReport>> {
    let dir = book.join("reports");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "pb"))
                .collect()
        })
        .unwrap_or_default();
    // ⚠ MTIME FIRST, THEN PATH. Two reports written in one filesystem
    // timestamp (the test helper's `r0.pb` / `r1.pb`) used to order
    // arbitrarily, so "the later figure retires the explanation" was a
    // coin flip on a fast disk.
    found.sort_by_key(|p| {
        let mtime = std::fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        (mtime, p.clone())
    });
    match found.last() {
        None => Ok(None),
        Some(p) => Ok(Some(
            kernel::BreakReport::decode(&std::fs::read(p)?[..])
                .with_context(|| format!("reading {}", p.display()))?,
        )),
    }
}

/// A break's id within its book: the last segment of its resource name.
///
/// ⛔ The fund half of a break name says how the book is being served, not
/// what it is. See `BreakExplanation::break_id`.
fn break_id_of(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// A stored explanation as the contract shows it.
///
/// The qualification is left empty and filled in by the caller, which is the
/// only place that knows what moved underneath it.
fn to_pb_explanation(e: &BreakExplanation) -> pb::BreakExplanation {
    pb::BreakExplanation {
        text: e.text.clone(),
        actor: e.actor.clone(),
        accept_time: Some(ratio_proto::timestamp_proto::google::protobuf::Timestamp {
            seconds: e.accept_time,
            nanos: 0,
        }),
        difference: e.difference.to_string(),
        config_digest: e.config_digest.clone(),
        journal_position: e.journal_position as i64,
        journal_digest: e.journal_digest.clone(),
        qualification: Vec::new(),
    }
}

fn cause_text(cause: i32) -> String {
    match kernel::Cause::try_from(cause) {
        Ok(kernel::Cause::AmountDiffers) => "Figures differ",
        Ok(kernel::Cause::AbsentFromReport) => "Not in the report",
        Ok(kernel::Cause::AbsentFromRatio) => "Ratio produced nothing",
        _ => "Unspecified",
    }
    .to_string()
}

/// A book id turned into something a person would read.
pub(crate) fn display_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The one money parser, in `ratio-common`.
///
/// This was briefly a second implementation, written here for the entry form
/// without noticing `ratio-recon` already had one for counterparty files. They
/// agreed by luck. Two parsers for the same product's money is two places for
/// the same rounding bug, and only one of them had the `1.005` test.
pub use ratio_common::parse_minor as parse_amount;


/// One unresolved fact, as the console reports it.
///
/// Shared by the pending list and the ingest preview so the two cannot describe
/// the same fact differently.
fn pending_fact(fund: &str, r: &ratio_ingest::Resolved) -> pb::PendingFact {
    // Absent and ambiguous take different remedies, so they are reported apart
    // rather than as one "unresolved".
    let blocker = if r
        .entities
        .values()
        .any(|e| matches!(e, ratio_ingest::Resolution::Ambiguous { .. }))
    {
        pb::pending_fact::Blocker::Ambiguous
    } else {
        pb::pending_fact::Blocker::Absent
    };
    pb::PendingFact {
        name: format!("funds/{fund}/pendingFacts/{}", r.fact.id.replace(':', "-")),
        reference: r.fact.reference.clone(),
        kind: r.fact.kind.clone(),
        blocker: blocker as i32,
        detail: r.blocker().unwrap_or_default(),
        delivery_digest: r.fact.provenance.delivery.clone(),
        row: r.fact.provenance.row.to_string(),
        template_id: r.fact.provenance.template_id.clone(),
    }
}

/// What to call a delivery. Composed from what the caller sent rather than
/// taken raw: on a public endpoint this reaches a screen other people read.
fn sanitize_origin(origin: &str) -> String {
    let cleaned: String = origin
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "._-/".contains(*c))
        .take(120)
        .collect();
    if cleaned.trim().is_empty() {
        "upload".into()
    } else {
        cleaned
    }
}

/// Seconds since the epoch, for a received-at stamp.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-02-26` as a `google.type.Date`. An empty day yields none rather than
/// a date of zeroes, which would read as the first of January in year nought.
fn iso_date(day: &str) -> Option<ratio_proto::date_proto::google::r#type::Date> {
    let p: Vec<&str> = day.split('-').collect();
    if p.len() != 3 {
        return None;
    }
    Some(ratio_proto::date_proto::google::r#type::Date {
        year: p[0].parse().ok()?,
        month: p[1].parse().ok()?,
        day: p[2].parse().ok()?,
    })
}

/// Minor units as a decimal, for a memo a person reads.
fn money_words(minor: i64) -> String {
    let (sign, m) = if minor < 0 { ("-", -minor) } else { ("", minor) };
    format!("{sign}{}.{:02}", m / 100, m % 100)
}

/// Seconds since the epoch as a proto timestamp.
fn stamp(seconds: i64) -> ratio_proto::timestamp_proto::google::protobuf::Timestamp {
    ratio_proto::timestamp_proto::google::protobuf::Timestamp { seconds, nanos: 0 }
}

/// The store's classification as the console's enum. A `match` rather than a
/// cast: the two enums are declared in different files and nothing but this
/// function stops them drifting apart silently.
fn account_type(t: AccountTypeRecord) -> pb::account::Type {
    match t {
        AccountTypeRecord::Asset => pb::account::Type::Asset,
        AccountTypeRecord::Liability => pb::account::Type::Liability,
        AccountTypeRecord::Equity => pb::account::Type::Equity,
        AccountTypeRecord::Income => pb::account::Type::Revenue,
        AccountTypeRecord::Expense => pb::account::Type::Expense,
    }
}

/// `funds/abc` → `abc`, checking the collection is the one expected.
pub fn resource_id(name: &str, collection: &str) -> Result<String> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 2 || parts[0] != collection {
        bail!("{name:?} is not a {collection}/* name");
    }
    Ok(parts[1].to_string())
}

/// `funds/a/breaks/b` → `("a", "b")`.
/// A position id — `{dim}-{instrument}` — back into its parts.
///
/// ⛔ SPLITS ON THE FIRST HYPHEN ONLY. Tickers contain them (`BRK-B`), and
/// splitting on the last or on all of them would resolve a real instrument to a
/// different one — or to none, which at least fails loudly.
pub fn position_key(id: &str) -> Result<(i64, String)> {
    let (dim, instrument) = id
        .split_once('-')
        .with_context(|| format!("{id:?} is not a position id — expected {{dim}}-{{instrument}}"))?;
    Ok((
        dim.parse()
            .with_context(|| format!("{id:?} does not begin with a dimension"))?,
        instrument.to_string(),
    ))
}

/// The currency a fund reports in.
///
/// ⛔ ONE CONSTANT, BECAUSE IT IS TWO ANSWERS TO ONE QUESTION OTHERWISE. It is
/// what `Fund.currency_code` tells a reader the figures are in, AND the base
/// `Rates` translates every other currency into. Those must be the same
/// currency: a NAV translated into euros and labelled USD is wrong by the rate,
/// and nothing about the number looks unusual.
///
/// ⚠ HARDCODED, AND THAT IS A REAL LIMITATION rather than a placeholder to
/// forget. A fund's reporting currency is a property of the fund, and when a
/// second one arrives this becomes a field on the book.
pub use ratio_store::BASE_CURRENCY as FUND_CURRENCY;

/// How `ListAccounts` folds the journal.
///
/// `Current` is the maintained projection — inception-to-date, including
/// undated entries. A period fold re-reads the journal and skips any entry
/// that names no day, because an undated entry has no period.
enum AccountFold {
    Current,
    AsOf(PeriodWindow),
    Activity(PeriodWindow),
}

#[derive(Clone, Debug)]
struct PeriodWindow {
    start: String,
    end: String,
}

/// AIP-132 List requests carry `filter` (AIP-160), not a custom period field.
/// `pnl-2026-03` / `sheet-2026` — hyphen because `param_of` does not decode.
fn list_accounts_window(filter: &str) -> (&str, &str) {
    if let Some(rest) = filter.strip_prefix("pnl-") {
        ("pnl", rest)
    } else if let Some(rest) = filter.strip_prefix("sheet-") {
        ("sheet", rest)
    } else {
        (filter, "")
    }
}

/// A month (`YYYY-MM`) or a year (`YYYY`) as an inclusive calendar window.
///
/// ⛔ THE LAST DAY IS THE CALENDAR'S, NOT DAY 31. `2026-02` ending on the
/// 31st would either refuse a real February or, worse, carry into March the
/// way an unvalidated ISO date once did.
fn parse_period(spec: &str) -> Result<PeriodWindow> {
    let spec = spec.trim();
    if spec.len() == 4 && spec.bytes().all(|b| b.is_ascii_digit()) {
        let y = spec;
        let start = format!("{y}-01-01");
        let end = format!("{y}-12-31");
        ratio_common::days_from_iso_date(&start)
            .with_context(|| format!("{spec:?} is not a year"))?;
        ratio_common::days_from_iso_date(&end)?;
        return Ok(PeriodWindow { start, end });
    }
    if spec.len() == 7 && spec.as_bytes().get(4) == Some(&b'-') {
        let y: i32 = spec[..4]
            .parse()
            .with_context(|| format!("{spec:?} is not a month (YYYY-MM)"))?;
        let m: i32 = spec[5..]
            .parse()
            .with_context(|| format!("{spec:?} is not a month (YYYY-MM)"))?;
        if !(1..=12).contains(&m) {
            bail!("{spec:?} names month {m}");
        }
        if !(0..=9999).contains(&y) {
            bail!("{spec:?} names year {y}");
        }
        let start = format!("{y:04}-{m:02}-01");
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        let next = format!("{ny:04}-{nm:02}-01");
        let start_d = ratio_common::days_from_iso_date(&start)?;
        let next_d = ratio_common::days_from_iso_date(&next)?;
        let end = ratio_common::iso_date_from_days(next_d - 1);
        ratio_common::days_from_iso_date(&end)?;
        let _ = start_d;
        return Ok(PeriodWindow { start, end });
    }
    bail!("{spec:?} is not a month (YYYY-MM) or a year (YYYY)")
}


/// One view's NAV in minor units, and how long the fold that struck it took.
///
/// ⭐ ONE IMPLEMENTATION, BECAUSE IT IS ONE NUMBER. `GetView` shows it and
/// `ApplyEvent`, `MarkPositions` and `AdmitFacts` each quote it before and
/// after; two derivations of it become two numbers the day one is edited, and
/// the console and the CLI disagreeing about a NAV is already a row in
/// HANDOFF.md's failure table.
///
/// ⛔ TIMED AROUND THE FOLD, NOT AROUND THE REQUEST. The same `Projection::nav`
/// call `ratio bench` times, so it reports the maintained strike — O(chart) —
/// and not the cold build.
///
/// ⚠ TAKES THE PROJECTION AND THE RATES rather than fetching them, so a caller
/// that already holds both does not pay for a second copy of either.
fn nav_from(
    b: &FileBook,
    proj: &ratio_project::Projection,
    view: &str,
    rates: &ratio_project::Rates,
) -> Result<(i64, std::time::Duration)> {
    let by_dim: BTreeMap<i64, AccountTypeRecord> =
        b.accounts()?.into_iter().map(|a| (a.dim, a.account_type)).collect();
    let is_asset_or_liability = |d: i64| {
        matches!(
            by_dim.get(&d),
            Some(AccountTypeRecord::Asset) | Some(AccountTypeRecord::Liability)
        )
    };
    let struck_at = std::time::Instant::now();
    let nav = proj.nav(view, &is_asset_or_liability, rates)?.value.0;
    Ok((nav, struck_at.elapsed()))
}

/// A figure, or the absence of one.
///
/// ⛔ THE EMPTY STRING IS NOT `"0"`, AND THIS FUNCTION EXISTS SO THE TWO CANNOT
/// BE TYPED INTERCHANGEABLY. `View.realized_gain` already carries the same
/// convention — empty when the chart names no realized-gain role — and on a plan
/// the distinction is the whole point: a step nothing measured rendering as `0`
/// reads as "instant", and a step nothing costs rendering as `0` reads as
/// "free". Neither is a claim anybody made.
fn figure(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

/// A duration, or the absence of one.
///
/// ⛔ `None` BECOMES JSON `null`, WHICH IS THE POINT OF THE TYPE. A step nothing
/// measured has no duration, and the wire says so structurally rather than by a
/// convention a reader has to know. The COUNTS beside it still use the empty
/// string, because they are int64s and every int64 on this contract is a string.
///
/// ⚠ Non-negative by construction — these are elapsed times and read counts
/// multiplied by a rate — so the truncating division is the right one.
fn duration(ns: Option<i64>) -> Option<ratio_proto::duration_proto::google::protobuf::Duration> {
    ns.map(|n| ratio_proto::duration_proto::google::protobuf::Duration {
        seconds: n / 1_000_000_000,
        nanos: (n % 1_000_000_000) as i32,
    })
}

/// A plan, as the contract carries it.
///
/// ⚠ ONE BUILDER, AND IT IS A PLAIN MAPPING. Every judgement about what a step
/// costs was made in `ratio_nav::explain`, where it is tested; anything decided
/// here would be a second opinion in the layer with no tests over it.
fn plan_pb(p: &ratio_nav::explain::Plan) -> pb::ExplainNavStrikeResponse {
    use ratio_nav::explain::{EdgeKind, Group, Role};
    pb::ExplainNavStrikeResponse {
        name: p.name.clone(),
        view: p.view.clone(),
        nodes: p
            .nodes
            .iter()
            .map(|n| pb::PlanNode {
                id: n.id.clone(),
                operator: n.operator.clone(),
                detail: n.detail.clone(),
                group: match n.group {
                    Group::Recorded => pb::plan_node::Group::Recorded,
                    Group::Maintained => pb::plan_node::Group::Maintained,
                } as i32,
                role: match n.role {
                    Role::Chosen => pb::plan_node::Role::Chosen,
                    Role::Rejected => pb::plan_node::Role::Rejected,
                    Role::Refusal => pb::plan_node::Role::Refusal,
                    Role::Unread => pb::plan_node::Role::Unread,
                } as i32,
                cites: n.cites.clone(),
                note: n.note.clone(),
                estimated_reads: figure(n.estimated_reads),
                // ⚠ `_nanos` ON THE MODEL, `_duration` ON THE WIRE, AND THAT
                // MISMATCH IS DELIBERATE — unlike `source`/`target`, which had
                // to agree because both sides carry the same string. These are
                // different TYPES: an `Option<i64>` of nanoseconds becomes a
                // `google.protobuf.Duration`, and `duration` is the conversion.
                // Naming the integer `_duration` would hide that a conversion
                // happens; naming the message `_nanos` fails the AIP linter.
                estimated_duration: duration(n.estimated_nanos),
                actual_rows: figure(n.actual_rows),
                actual_duration: duration(n.actual_nanos),
            })
            .collect(),
        edges: p
            .edges
            .iter()
            .map(|e| pb::PlanEdge {
                source: e.source.clone(),
                target: e.target.clone(),
                kind: match e.kind {
                    EdgeKind::Flow => pb::plan_edge::Kind::Flow,
                    EdgeKind::Refusal => pb::plan_edge::Kind::Refusal,
                    EdgeKind::Unread => pb::plan_edge::Kind::Unread,
                } as i32,
                rows: figure(e.rows),
            })
            .collect(),
        dials: p.shape.as_ref().map(|s| pb::PlanDials {
            securities: s.estimate.dials.securities.to_string(),
            currencies: s.estimate.dials.currencies.to_string(),
            lots_per: s.estimate.dials.lots_per.to_string(),
            open_actions: s.estimate.dials.open_actions.to_string(),
            accounts: s.accounts.to_string(),
            total_rows: s.total_rows.to_string(),
            open_lots: s.open_lots_held.to_string(),
        }),
        estimate_refusal: p.estimate_refusal.clone(),
        analyzed: p.analyzed,
        nanos_per_read: p.nanos_per_read.to_string(),
        provenance: p.provenance.clone(),
        chosen_reads: figure(p.chosen_reads),
        rewrite_reads: figure(p.rewrite_reads),
        scan_reads: figure(p.scan_reads),
    }
}

/// A view's declared terms, as the contract carries them.
///
/// ⛔ ONE BUILDER FOR BOTH `ListViews` AND `GetView`. Two would be two answers
/// to what a view IS, differing in whichever field somebody added to one.
fn view_pb(fund: &str, set: &ratio_rules::RuleSet, v: &ratio_rules::View) -> pb::View {
    let cal = v.calendar.as_deref().and_then(|c| set.calendar(c));
    pb::View {
        name: format!("funds/{fund}/views/{}", v.id),
        display_name: v.label().to_string(),
        basis: match v.basis {
            ratio_rules::Basis::Recorded => pb::view::Basis::Recorded,
            ratio_rules::Basis::Trade => pb::view::Basis::Trade,
            ratio_rules::Basis::Settlement => pb::view::Basis::Settlement,
        } as i32,
        settlement_open_days: v.settles_in.unwrap_or(0),
        calendar: v.calendar.clone().unwrap_or_default(),
        holiday_count: cal.map(|c| c.holidays.len() as i64).unwrap_or(0),
        // ⛔ WHETHER ANYBODY CHOSE IT. A book declaring nothing has one view and
        // it is not an election; the same trap `lot_method_declared` exists for.
        declared: set.views_declared(),
        ..Default::default()
    }
}

/// `funds/a/views/v` → `("a", "v")`.
///
/// ⛔ A VIEW IS NOT A TENANCY BOUNDARY, and this deliberately does not check
/// one. `book_path` remains the single place a fund id becomes a path; adding a
/// second check here would be a second place to forget it, and the id it
/// returns still has to go through that door.
pub fn view_scoped_parent(parent: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = parent.split('/').collect();
    if parts.len() != 4 || parts[0] != "funds" || parts[2] != "views" {
        bail!("{parent:?} is not a funds/*/views/* name");
    }
    Ok((parts[1].to_string(), parts[3].to_string()))
}

/// `funds/a/views/v/navStrikes/s` → `("a", "v", "s")`.
pub fn view_scoped_id(name: &str, collection: &str) -> Result<(String, String, String)> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 6 || parts[0] != "funds" || parts[2] != "views" || parts[4] != collection {
        bail!("{name:?} is not a funds/*/views/*/{collection}/* name");
    }
    Ok((parts[1].to_string(), parts[3].to_string(), parts[5].to_string()))
}

pub fn nested_id(name: &str, outer: &str, inner: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 4 || parts[0] != outer || parts[2] != inner {
        bail!("{name:?} is not a {outer}/*/{inner}/* name");
    }
    Ok((parts[1].to_string(), parts[3].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one view every seeded test book has, for direct projection reads.
    use ratio_rules::UNDECLARED_VIEW as B;

    /// The demo book's one view, as a resource name — which is also the parent
    /// every view-scoped read below takes.
    ///
    /// ⛔ `book`, NOT `abor`, AND THAT IS THE ASSERTION. Every seeded test book
    /// declares no views, so it has exactly one and nobody elected it. Naming
    /// it after a real basis here would let a bug that treated silence as an
    /// election pass every test in this file.
    fn demo_view() -> String {
        format!("funds/demo/views/{}", ratio_rules::UNDECLARED_VIEW)
    }

    /// A break's full resource name on the seeded book — the name `ratio
    /// accept` takes and the console's URL carries.
    fn demo_break(id: &str) -> String {
        format!("{}/breaks/{id}", demo_view())
    }

    fn fresh(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ratio-console-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A book with a chart, an active configuration, and a fund that has taken
    /// capital, bought something, and accrued a fee.
    ///
    /// The entries are real double-entry rather than convenient pairs: the fee
    /// CREDITS the payable, which is what makes the NAV assertion below test
    /// the sign rather than agree with it. My first version debited it, the
    /// two families cancelled to zero, and the test agreed with an answer that
    /// happened to be right for the wrong reason.
    fn book(at: &Path) {
        use ratio_store::{Account, AccountTypeRecord as A, JournalEntry, PostingRecord};
        let mut b = FileBook::open(at).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments at fair value".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash and equivalents".into(), account_type: A::Asset },
            Account { dim: 10, display_name: "Management fee expense".into(), account_type: A::Expense },
            Account { dim: 20, display_name: "Capital contributions".into(), account_type: A::Equity },
            Account { dim: 40, display_name: "Management fee payable".into(), account_type: A::Liability },
        ])
        .unwrap();
        let d = b.put(b"rules = []\n").unwrap();
        b.set_active(&d).unwrap();
        let mut post = |id: &str, memo: &str, legs: Vec<(i64, i64)>| {
            b.append(&JournalEntry {
                id: id.into(),
                memo: memo.into(),
                config: d.clone(),
                postings: legs.into_iter().map(|(dim, amount)| PostingRecord::new(dim, amount)).collect(),
            
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        };
        post("c1", "capital in", vec![(2, 30_000_000), (20, -30_000_000)]);
        post("t1", "buy", vec![(1, 25_000_000), (2, -25_000_000)]);
        post("f1", "fee accrued", vec![(10, 100_000), (40, -100_000)]);
    }

    /// Store a configuration declaring a tolerance and return its digest, so a
    /// report can name it.
    ///
    /// ⛔ THE DIGEST IS THE POINT. A report grades against the configuration IT
    /// NAMES, so a test that wants to assert a grade has to put the bands
    /// somewhere the book can actually read them back from. The earlier version
    /// of these tests wrote `config_digest: "abc123"` and asserted severities
    /// that came from two constants — true by coincidence, and it stayed true
    /// when the constants were the only thing deciding.
    fn config_with_tolerance(at: &Path, below_notice: i64, blocks_nav: i64) -> String {
        let mut b = FileBook::open(at).unwrap();
        let toml = format!(
            "rules = []\n[tolerance]\nbelow_notice = {below_notice}\nblocks_nav = {blocks_nav}\n"
        );
        let d = b.put(toml.as_bytes()).unwrap();
        d.as_str().to_string()
    }

    /// Expand a route template to a concrete path for one fund.
    /// `/v1/{parent=funds/*}/breaks` → `/v1/funds/<fund>/breaks`;
    /// `/v1/{name=funds/*/accounts/*}` → `/v1/funds/<fund>/accounts/x`;
    /// `/v1/{parent=funds/*}:applyEvent` → `/v1/funds/<fund>:applyEvent`.
    /// The first `*` is the fund; every later one is a placeholder child id.
    fn expand_template(template: &str, fund: &str) -> String {
        let open = template.find('{').expect("a fund route carries a capture");
        let close = template.find('}').expect("a capture closes");
        let inside = &template[open + 1..close];
        let pattern = inside.split_once('=').map(|(_, p)| p).unwrap_or(inside);
        let expanded = format!("{}{}{}", &template[..open], pattern, &template[close + 1..]);
        let mut out = String::new();
        let mut n = 0;
        for ch in expanded.chars() {
            if ch == '*' {
                out.push_str(if n == 0 { fund } else { "x" });
                n += 1;
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// A body that parses for each write route, so the request reaches
    /// `book_path` and the tenancy denial is what the route fails on — not a
    /// malformed body, which would let the test pass without testing the guard.
    fn post_body(template: &str) -> &'static str {
        if template.ends_with(":mark") {
            r#"{"valuationDate":{"year":2026,"month":1,"day":1}}"#
        } else {
            "{}"
        }
    }

    #[test]
    fn a_subject_scoped_to_one_fund_cannot_reach_another_through_any_route() {
        // Two real funds under one root; the subject is granted only `a`. `b` is
        // a book ON DISK — that is what makes this a real negative test. With the
        // `book_path` guard removed, every route below would reach `b` and
        // return its data, so these assertions go red for the right reason
        // rather than because `b` happened not to exist (which the pre-existing
        // "no fund" existence check would have reported anyway).
        let root = fresh("tenancy");
        book(&root.join("a"));
        book(&root.join("b"));
        std::fs::write(root.join("MEMBERSHIP.tsv"), "S\ta\n").unwrap();

        let console = Console::scoped(
            &root,
            Subject::Member {
                sub: "S".into(),
                email: "s@example.test".into(),
                organization: String::new(),
                groups: vec![],
            },
        );

        // Every route that names a fund, instantiated against `b`, is refused —
        // and refused as "no fund", the same answer a nonexistent fund gets, so
        // `b`'s existence does not leak. The list is the LIVE `ROUTES` slice, so
        // a route added later is covered here without anyone remembering to.
        let mut checked = 0;
        let mut view_scoped = 0;
        for route in transcode::ROUTES {
            if !route.template.contains("funds/*") && !route.template.contains("books/*") {
                continue; // enumerations (/v1/funds, /v1/books) are tested below.
            }
            if route.template.contains("views/*") {
                view_scoped += 1;
            }
            let path = expand_template(route.template, "b");
            let body = post_body(route.template);
            let msg = match transcode::serve(&console, route.method, &path, "", body) {
                Err(e) => format!("{e:#}"),
                Ok(leaked) => panic!(
                    "{} {} reached fund b — the tenant boundary is not enforced on \
                     this route. Served: {leaked}",
                    route.method, path
                ),
            };
            assert!(
                msg.contains("no fund"),
                "{} {} failed, but not as a tenancy denial: {msg}",
                route.method,
                path
            );
            checked += 1;
        }
        assert!(checked >= 36, "expected every fund route covered, only saw {checked}");

        // ⛔ AND THE VIEW-SCOPED ONES ARE AMONG THEM. Fifteen routes now carry a
        // `views/{view}` segment, and `expand_template` fills the second `*`
        // with a placeholder id — so a boundary enforced only on `funds/*` and
        // forgotten one level in is what this second floor catches. Drop the
        // segment from the templates and `checked` still clears its floor while
        // this drops to zero, which is the failure that would otherwise be
        // invisible.
        assert!(
            view_scoped >= 15,
            "expected the view-scoped routes covered too, only saw {view_scoped}"
        );

        // The guard admits what it should: the subject's OWN fund is readable.
        // Without this the loop above could be passing because `a` was blocked
        // too, which would be a different bug wearing the same green.
        assert!(
            transcode::serve(&console, "GET", "/v1/funds/a/views/book/accounts", "", "").is_ok(),
            "the subject's own fund a must be readable"
        );

        // ListFunds returns what the caller may see: `a`, not `b`. An empty list
        // and a refusal are different answers; here it is neither empty nor
        // leaking.
        let funds = transcode::serve(&console, "GET", "/v1/funds", "", "").unwrap();
        assert!(funds.contains("funds/a"), "the subject's fund is missing: {funds}");
        assert!(!funds.contains("funds/b"), "another tenant's fund leaked into the list: {funds}");

        // The same boundary on the book collection: `b` is on disk and absent
        // from the list, and GetBook refuses it as "no fund".
        let books = transcode::serve(&console, "GET", "/v1/books", "", "").unwrap();
        assert!(books.contains("books/a"), "the subject's book is missing: {books}");
        assert!(!books.contains("books/b"), "another tenant's book leaked into the list: {books}");
    }

    #[test]
    fn the_local_console_is_unrestricted_and_sees_every_fund() {
        // The CLI and `ratio watch --book` reach the data as `Local`, which is
        // not a tenant. The tenancy work must not change what they see.
        let root = fresh("tenancy-local");
        book(&root.join("a"));
        book(&root.join("b"));
        std::fs::write(root.join("MEMBERSHIP.tsv"), "S\ta\n").unwrap();

        let console = Console::new(&root); // Subject::Local
        assert!(transcode::serve(&console, "GET", "/v1/funds/a/views/book/accounts", "", "").is_ok());
        assert!(transcode::serve(&console, "GET", "/v1/funds/b/views/book/accounts", "", "").is_ok());
        let funds = transcode::serve(&console, "GET", "/v1/funds", "", "").unwrap();
        assert!(funds.contains("funds/a") && funds.contains("funds/b"));
    }

    #[test]
    fn an_open_console_grants_a_member_every_fund_but_keeps_their_identity() {
        // The open, shared demo: a demo whose audience is not known ahead of time
        // cannot be an allow-list, so any authenticated caller sees every fund —
        // yet the write is still attributed to their verified id. MEMBERSHIP here
        // grants only `a`, and open mode ignores it on purpose; `scoped` on the
        // SAME inputs refuses `b`, which is the boundary this does not touch.
        let root = fresh("open-demo");
        book(&root.join("a"));
        book(&root.join("b"));
        std::fs::write(root.join("MEMBERSHIP.tsv"), "S\ta\n").unwrap();

        let subject =
            Subject::Member {
                sub: "S".into(),
                email: "s@x.test".into(),
                organization: String::new(),
                groups: vec![],
            };
        let console = Console::open(&root, subject);

        // `b` is granted by nobody and is seen anyway — the whole point. If open
        // mode leaked into `scoped`, the tenancy test above would already be red.
        assert!(transcode::serve(&console, "GET", "/v1/funds/a/views/book/accounts", "", "").is_ok());
        assert!(
            transcode::serve(&console, "GET", "/v1/funds/b/views/book/accounts", "", "").is_ok(),
            "open mode must grant a fund MEMBERSHIP omits"
        );
        let funds = transcode::serve(&console, "GET", "/v1/funds", "", "").unwrap();
        assert!(
            funds.contains("funds/a") && funds.contains("funds/b"),
            "open mode lists every fund: {funds}"
        );

        // But the identity is not lost: a write is signed by the subject, not by
        // Local's RATIO_ACTOR. Open changes what is SEEN, never who ACTED.
        let book_a = root.join("a");
        let digest = FileBook::open(&book_a).unwrap().active().unwrap().unwrap();
        console.record_change(&book_a, "posted", "evt-1", digest.as_str()).unwrap();
        let log = std::fs::read_to_string(book_a.join("CHANGELOG")).unwrap();
        assert!(log.contains("\tS\tposted\t"), "the write must be signed by the subject: {log}");
    }

    #[test]
    fn a_write_is_attributed_to_the_verified_subject_and_does_not_pollute_config_versions() {
        let root = fresh("attribution");
        book(&root.join("a"));
        std::fs::write(root.join("MEMBERSHIP.tsv"), "signer-sub\ta\n").unwrap();
        let console = Console::scoped(
            &root,
            Subject::Member {
                sub: "signer-sub".into(),
                email: "s@x.test".into(),
                organization: String::new(),
                groups: vec![],
            },
        );
        let book_a = root.join("a");
        let digest = FileBook::open(&book_a).unwrap().active().unwrap().unwrap();

        // What every write handler records after a successful append.
        console.record_change(&book_a, "posted", "evt-1", digest.as_str()).unwrap();

        // The change log shows it, attributed to the VERIFIED subject — the id
        // resolved from the gateway's claims, never a string the caller chose.
        let log = console.change_log_for(&book_a, "a").unwrap();
        let posted = log.iter().find(|e| e.action == "posted").expect("the write is in the log");
        assert_eq!(posted.actor.as_str(), "signer-sub");
        assert_eq!(posted.subject.as_str(), "evt-1");
        assert_eq!(posted.actor_kind, pb::ActorKind::Person as i32);

        // ⛔ AND IT IS NOT MISREAD AS A CONFIG PROMOTION. `config_versions` keys
        // on the digest in the last field; this posted-event line carries the
        // same digest. `book` promoted that config with no "approved" line, so
        // the version must carry an EMPTY actor. Drop the `action == "approved"`
        // filter and this reads "signer-sub" — the last poster mistaken for the
        // approver. That is the negative test.
        let versions = console.config_versions("a").unwrap();
        let v = versions
            .iter()
            .find(|v| v.digest.as_str() == digest.as_str())
            .expect("the active config is a version");
        assert_eq!(v.actor.as_str(), "", "a posted-event line must not be read as the approver");
    }

    #[test]
    fn the_projection_is_kept_between_calls_and_only_catches_up() {
        // ⭐ WITHOUT THIS THE INCREMENTAL READ HAS NOWHERE TO BE INCREMENTAL.
        // `Projection::of_book` folds the whole journal — 546 ms on a
        // 140,000-entry book, growing with every trade ever made. The cache is
        // what turns `follow` into maintenance rather than a stat before a
        // rebuild.
        let d = fresh("cached");
        book(&d);
        let c = Console::new(&d);

        let first = c.projection("demo").unwrap();
        assert_eq!(first.prefix(), 3);

        // A second call must not re-fold. Asserted by the count `follow`
        // returns rather than by timing: a rebuild fast enough to look
        // incremental passes a stopwatch and fails this.
        {
            let mut cache = c.projections.lock().unwrap();
            assert_eq!(
                cache.get_mut("demo").unwrap().follow(&d).unwrap(),
                0,
                "nothing was appended, so nothing is folded"
            );
        }

        // One arrives, and only it is folded.
        {
            use ratio_store::{JournalEntry, PostingRecord};
            let mut b = FileBook::open(&d).unwrap();
            let cfg = b.active().unwrap().unwrap();
            b.append(&JournalEntry {
                id: "t2".into(),
                memo: "buy".into(),
                config: cfg,
                postings: vec![PostingRecord::new(1, 5_000_000), PostingRecord::new(2, -5_000_000)],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        let after = c.projection("demo").unwrap();
        assert_eq!(after.prefix(), 4, "caught up");

        // And it agrees with a cold build, which is the only thing that makes
        // the cache safe to have at all.
        let cold = ratio_project::Projection::of_book(&d).unwrap();
        let assets = |dim: i64| dim == 1 || dim == 2 || dim == 40;
        assert_eq!(after.nav(B, &assets, &ratio_project::Rates::none()).unwrap().value, cold.nav(B, &assets, &ratio_project::Rates::none()).unwrap().value);
        assert_eq!(after.positions(B).unwrap().value, &cold.positions(B).unwrap().value.clone());
    }

    #[test]
    fn a_replaced_book_is_rebuilt_rather_than_spliced() {
        // ⛔ An append-only log does not shrink, so a shorter file at the same
        // path is a DIFFERENT BOOK. `follow` refuses it; the cache must start
        // again rather than fold the new history onto the old totals.
        let d = fresh("replaced");
        book(&d);
        let c = Console::new(&d);
        assert_eq!(c.projection("demo").unwrap().prefix(), 3);

        // Replace it with a shorter one.
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        {
            use ratio_store::{Account, AccountTypeRecord as A, JournalEntry, PostingRecord};
            let mut b = FileBook::open(&d).unwrap();
            b.put_accounts(&[Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
                             Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset }])
                .unwrap();
            let cfg = b.put(b"rules = []\n").unwrap();
            b.set_active(&cfg).unwrap();
            b.append(&JournalEntry {
                id: "only".into(),
                memo: "one".into(),
                config: cfg,
                postings: vec![PostingRecord::new(1, 11), PostingRecord::new(2, -11)],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }

        let p = c.projection("demo").unwrap();
        assert_eq!(p.prefix(), 1, "rebuilt from the new book, not spliced onto the old");
        assert_eq!(p.nav(B, &|dim| dim == 1, &ratio_project::Rates::none()).unwrap().value.0, 11, "and the totals are the new book's");
    }

    #[test]
    fn a_sale_the_lot_engine_could_not_relieve_is_an_exception() {
        // ⛔ IN THE EXCEPTIONS LIST, WHERE AN OPERATOR ALREADY LOOKS. A lot
        // break was computed into a void: the projection reported it and no
        // surface showed it. Inventing a second screen is how a thing gets
        // looked at by nobody.
        let d = fresh("lotbrk");
        book(&d);
        {
            use ratio_store::{JournalEntry, PostingRecord};
            let mut b = FileBook::open(&d).unwrap();
            let cfg = b.active().unwrap().unwrap();
            // Seven units in, three out — a pro-rata split that will not divide.
            b.append(&JournalEntry {
                id: "buy".into(),
                memo: "buy".into(),
                config: cfg.clone(),
                postings: vec![
                    PostingRecord { dim: 1, amount: 100, currency: None, instrument: Some("VTI".into()), quantity: Some(7) },
                    PostingRecord::new(2, -100),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
            b.append(&JournalEntry {
                id: "sell".into(),
                memo: "sell".into(),
                config: cfg,
                postings: vec![
                    PostingRecord { dim: 1, amount: -45, currency: None, instrument: Some("VTI".into()), quantity: Some(-3) },
                    PostingRecord::new(2, 45),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }

        let c = Console::new(&d);
        let breaks = c.list_breaks(&demo_view(), "").unwrap().breaks;
        let lot: Vec<_> = breaks.iter().filter(|b| b.name.contains("lot-")).collect();
        assert_eq!(lot.len(), 1, "{:?}", breaks.iter().map(|b| &b.cause).collect::<Vec<_>>());
        assert!(lot[0].cause.contains("administration agreement"), "{}", lot[0].cause);

        // ⚠ HIGH, and not because of the amount. Every other break is graded by
        // money at stake; this one is graded by what it means — the figure it
        // corrupts is the realized gain, which no reconciliation reaches.
        assert_eq!(lot[0].severity, pb::Severity::High as i32);

        // And it survives the blocking filter, which is what an operator uses
        // to find the things that stop a NAV.
        let blocking = c.list_breaks(&demo_view(), "blocking").unwrap().breaks;
        assert!(blocking.iter().any(|b| b.name.contains("lot-")));
    }

    #[test]
    fn a_book_can_be_created_with_no_fund_and_no_org() {
        // ⭐ THE INDEPENDENCE CONTRACT. Create writes a sidecar that names no
        // fund and no organization. ListBooks includes it; ListFunds does not.
        // The same kernel, a different chart — not a second ledger.
        let root = fresh("create-independent-book");
        let console = Console::new(&root);
        let created = console
            .create_book(pb::CreateBookRequest {
                book: Some(pb::Book {
                    display_name: "Household".into(),
                    kind: book::BookKind::Personal.proto(),
                    ..Default::default()
                }),
                book_id: "household".into(),
            })
            .unwrap();
        assert_eq!(created.name, "books/household");
        assert_eq!(created.display_name, "Household");
        assert_eq!(created.kind, book::BookKind::Personal.proto());
        assert!(created.fund.is_empty(), "CreateBook must not file a fund: {:?}", created.fund);
        assert!(
            created.organization.is_empty(),
            "CreateBook must not file an org: {:?}",
            created.organization
        );

        let books = console.list_books().unwrap().books;
        assert!(books.iter().any(|b| b.name == "books/household"));
        let funds = console.list_funds().unwrap().funds;
        assert!(
            funds.iter().all(|f| f.name != "funds/household"),
            "an independent book must not appear as a fund: {funds:?}"
        );

        // GetFund still answers — existing screens and /books/:id rewrites
        // keep working against the same directory.
        assert_eq!(console.get_fund("funds/household").unwrap().name, "funds/household");
    }

    fn household_req(id: &str, rule: &str, amount: &str, y: i32, m: i32, d: i32) -> pb::ApplyEventRequest {
        pb::ApplyEventRequest {
            parent: "funds/household".into(),
            rule_id: rule.into(),
            event_id: id.into(),
            amount: amount.into(),
            days: String::new(),
            instrument: String::new(),
            quantity: String::new(),
            trade_date: Some(ratio_proto::date_proto::google::r#type::Date {
                year: y,
                month: m,
                day: d,
            }),
            validate_only: false,
        }
    }

    #[test]
    fn a_personal_transfer_posts_without_opening_a_lot() {
        // ⭐ CASH → INVESTMENTS IS A TRANSFER, NOT A SALE. The household
        // template's transfer rules carry no instrument and no per-instrument
        // leg, so the walk that opens lots skips them. The entry still
        // conserves; the trial balance still ties; the lot book stays empty.
        let root = fresh("personal-xfer");
        let c = Console::new(&root);
        c.create_book(pb::CreateBookRequest {
            book: Some(pb::Book {
                display_name: "Household".into(),
                kind: book::BookKind::Personal.proto(),
                ..Default::default()
            }),
            book_id: "household".into(),
        })
        .unwrap();
        c.apply_event(&household_req("xfer-1", "xfer_cash_investments", "250.00", 2026, 3, 15))
            .unwrap();

        let view = format!("funds/household/views/{}", ratio_rules::UNDECLARED_VIEW);
        let sheet = c.list_accounts(&view, "sheet").unwrap().accounts;
        assert!(
            sheet.iter().any(|a| a.display_name == "Cash and bank"),
            "the sheet names chart_for(Personal), not a fund: {sheet:?}"
        );
        assert!(sheet.iter().any(|a| a.display_name == "Investments"));
        assert!(sheet.iter().any(|a| a.display_name == "Credit cards and loans"));
        assert_eq!(c.get_fund("funds/household").unwrap().trial_balance_difference, "0");

        let proj = c.projection("household").unwrap();
        assert_eq!(
            proj.open_lots(ratio_rules::UNDECLARED_VIEW).unwrap(),
            0,
            "a household transfer must not claim lot relief"
        );
        assert!(
            proj.positions(ratio_rules::UNDECLARED_VIEW)
                .unwrap()
                .value
                .held
                .is_empty(),
            "a transfer that opened a position invented a fund holding"
        );
    }

    #[test]
    fn a_period_pnl_keeps_one_month_and_drops_an_undated_entry() {
        // ⛔ CUMULATIVE-ONLY IS THE ABOR-SHAPED VIEW. March's living expenses
        // are March's; an April spend and an undated spend must not join them.
        let root = fresh("personal-pnl");
        let c = Console::new(&root);
        c.create_book(pb::CreateBookRequest {
            book: Some(pb::Book {
                display_name: "Household".into(),
                kind: book::BookKind::Personal.proto(),
                ..Default::default()
            }),
            book_id: "household".into(),
        })
        .unwrap();
        c.apply_event(&household_req("mar", "spend_cash", "40.00", 2026, 3, 10))
            .unwrap();
        c.apply_event(&household_req("apr", "spend_cash", "60.00", 2026, 4, 2))
            .unwrap();
        {
            let mut req = household_req("undated", "spend_cash", "99.00", 2026, 3, 1);
            req.trade_date = None;
            c.apply_event(&req).unwrap();
        }

        let view = format!("funds/household/views/{}", ratio_rules::UNDECLARED_VIEW);
        let march = c.list_accounts(&view, "pnl-2026-03").unwrap().accounts;
        let living = march
            .iter()
            .find(|a| a.display_name == "Living expenses")
            .expect("pnl names the personal expense account");
        assert_eq!(living.debit, "4000", "March is 40.00, not 40+60+99: {living:?}");
        assert_eq!(living.posting_count, "1");
        assert!(
            march.iter().any(|a| a.display_name == "Income"),
            "the chart is the source of the rows even when income did not move"
        );
        assert!(
            march.iter().all(|a| a.display_name != "Cash and bank"),
            "a P&L that lists cash is a balance sheet wearing the wrong label"
        );

        let year = c.list_accounts(&view, "pnl-2026").unwrap().accounts;
        let living_y = year.iter().find(|a| a.display_name == "Living expenses").unwrap();
        assert_eq!(living_y.debit, "10000", "the year is March+April, still not the undated 99");

        let refused = c.list_accounts(&view, "pnl");
        assert!(refused.is_err(), "a P&L without a period is the cumulative default this refuses");
    }

    #[test]
    fn parse_period_ends_february_on_the_calendar() {
        let w = parse_period("2026-02").unwrap();
        assert_eq!(w.start, "2026-02-01");
        assert_eq!(w.end, "2026-02-28");
        let leap = parse_period("2024-02").unwrap();
        assert_eq!(leap.end, "2024-02-29");
        let y = parse_period("2026").unwrap();
        assert_eq!(y.start, "2026-01-01");
        assert_eq!(y.end, "2026-12-31");
        assert!(parse_period("2026-13").is_err());
        assert!(parse_period("soon").is_err());
    }

    #[test]
    fn create_book_grants_the_creator_and_not_their_org() {
        let root = fresh("create-grants-sub");
        book(&root.join("legacy"));
        std::fs::write(root.join("MEMBERSHIP.tsv"), "user_1\tlegacy\n").unwrap();
        let subject = Subject::Member {
            sub: "user_1".into(),
            email: "a@x.test".into(),
            organization: "org_01a".into(),
            groups: vec![],
        };
        let created = Console::scoped(&root, subject.clone())
            .create_book(pb::CreateBookRequest {
                book: Some(pb::Book {
                    display_name: "Mine".into(),
                    kind: book::BookKind::Project.proto(),
                    ..Default::default()
                }),
                book_id: "mine".into(),
            })
            .unwrap();
        assert_eq!(created.name, "books/mine");

        // ⚠ `allowed` is computed once. The grant is on disk; a new Console
        // is what a subsequent HTTP request constructs.
        let again = Console::scoped(&root, subject);
        assert_eq!(again.get_book("books/mine").unwrap().name, "books/mine");
        let membership = std::fs::read_to_string(root.join("MEMBERSHIP.tsv")).unwrap();
        assert!(membership.contains("user_1\tmine"), "{membership}");
        assert!(
            !membership.contains("org:org_01a\tmine"),
            "a personal create must not grant the org: {membership}"
        );
    }

    #[test]
    fn a_subject_scoped_to_one_book_cannot_read_another() {
        let root = fresh("book-tenancy");
        book(&root.join("ours"));
        book(&root.join("theirs"));
        std::fs::write(root.join("MEMBERSHIP.tsv"), "S\tours\n").unwrap();
        let console = Console::scoped(
            &root,
            Subject::Member {
                sub: "S".into(),
                email: "s@example.test".into(),
                organization: String::new(),
                groups: vec![],
            },
        );
        let err = console.get_book("books/theirs").unwrap_err().to_string();
        assert!(err.contains("no fund"), "{err}");
        assert!(console.get_book("books/ours").is_ok());
    }

    #[test]
    fn a_root_that_is_itself_a_book_is_one_fund() {
        // The deployed demo is a single book. It must appear as a fund list of
        // one rather than as an empty console.
        let d = fresh("single");
        book(&d);
        let c = Console::new(&d);
        let funds = c.list_funds().unwrap().funds;
        assert_eq!(funds.len(), 1);
        assert_eq!(funds[0].name, "funds/demo");
    }

    #[test]
    fn each_subdirectory_is_a_fund() {
        let d = fresh("many");
        for id in ["harbourline", "calderwood"] {
            std::fs::create_dir_all(d.join(id)).unwrap();
            book(&d.join(id));
        }
        let names: Vec<String> =
            Console::new(&d).list_funds().unwrap().funds.iter().map(|f| f.name.clone()).collect();
        assert_eq!(names, vec!["funds/calderwood", "funds/harbourline"], "and sorted");
    }

    #[test]
    fn nav_is_assets_minus_liabilities() {
        //   investments   +25,000,000   (asset, debit)
        //   cash          + 5,000,000   (30,000,000 in, 25,000,000 spent)
        //   payable       −   100,000   (liability, CREDIT — this is the sign
        //                                the fold has to get right)
        //   ────────────────────────────
        //   NAV            29,900,000
        //
        // Equity and expense are outside the fold, which is why the answer is
        // not simply the trial balance. A sign error here is invisible in a
        // screenshot and wrong by twice the liability.
        let d = fresh("nav");
        book(&d);
        let c = Console::new(&d);
        let f = c.get_fund("funds/demo").unwrap();
        // ⛔ THE NAV IS ON THE VIEW NOW, AND THE FUND SAYS WHICH VIEW. A book
        // declaring none has exactly one, so this is the same figure it always
        // was — and reading it through `default_view` is the assertion that the
        // two agree about which question is being answered.
        assert_eq!(f.default_view, ratio_rules::UNDECLARED_VIEW);
        assert_eq!(f.view_count, 1);
        let v = c.get_view(&format!("funds/demo/views/{}", f.default_view)).unwrap();
        assert_eq!(v.net_asset_value, "29900000");
        assert_eq!(f.trial_balance_difference, "0", "any accepted book ties");
        assert_eq!(f.entry_count, 3);
    }

    #[test]
    fn a_fund_with_no_entries_is_awaiting_prices_not_struck() {
        // An empty book must not read as "done". That is the difference
        // between a NAV nobody has started and a NAV that is finished.
        let d = fresh("empty");
        FileBook::open(&d).unwrap().put_accounts(&[]).unwrap();
        let f = Console::new(&d).get_fund("funds/demo").unwrap();
        assert_eq!(f.state, pb::fund::State::AwaitingPrices as i32);
    }

    #[test]
    fn a_fund_id_cannot_walk_out_of_the_root() {
        // Fund ids arrive from a URL on a public endpoint and become paths.
        let d = fresh("traversal");
        book(&d);
        let c = Console::new(&d);
        for bad in ["../etc", "..", "a/b", ""] {
            assert!(c.get_fund(&format!("funds/{bad}")).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn every_int64_crosses_the_wire_as_a_string() {
        // proto3's canonical JSON mapping, which is what makes a generated
        // TypeScript client correct against this wire form without a
        // translation layer. A number here would also be a silent double in
        // the browser — the failure the integer kernel exists to prevent,
        // arriving in the last hop.
        use crate::transcode::JsonView;
        let d = fresh("wireints");
        book(&d);
        let c = Console::new(&d);
        let f = c.get_fund("funds/demo").unwrap().to_json();
        for field in ["entryCount", "openBreakCount", "trialBalanceDifference",
                      "pendingFactCount", "viewCount", "longTermDays"] {
            assert!(
                f.contains(&format!("\"{field}\":\"")),
                "{field} is not a string in {f}"
            );
        }

        // ⛔ AND THE VIEW, WHICH IS WHERE THE MONEY WENT. Eleven figures moved
        // off `Fund` onto `View`, and checking only the fund would leave the
        // message carrying almost every int64 on this service unchecked — the
        // test would have gone on passing while saying less each time a field
        // moved.
        let v = c.get_view(&demo_view()).unwrap().to_json();
        for field in ["netAssetValue", "totalDebit", "totalCredit", "openDifference",
                      "openBreakCount", "openLotCount", "positionCount",
                      "journalPosition", "settlementOpenDays", "holidayCount",
                      "unplaceableEntryCount"] {
            assert!(
                v.contains(&format!("\"{field}\":\"")),
                "{field} is not a string in {v}"
            );
        }

        // And enums cross as their names, not their numbers — on both, since
        // both carry the same `Fund.State`.
        for json in [&f, &v] {
            assert!(json.contains("\"state\":\"BLOCKED\"") || json.contains("\"state\":\"STRUCK\"")
                    || json.contains("\"state\":\"IN_REVIEW\"")
                    || json.contains("\"state\":\"AWAITING_PRICES\""),
                    "state is not a canonical enum name: {json}");
        }
        assert!(v.contains("\"basis\":\"RECORDED\""),
                "a book declaring no views recognises in journal order: {v}");
    }

    #[test]
    fn resource_names_are_parsed_strictly() {
        assert_eq!(resource_id("funds/abc", "funds").unwrap(), "abc");
        assert!(resource_id("funds/abc/breaks/1", "funds").is_err());
        assert!(resource_id("books/abc", "funds").is_err());
        assert_eq!(
            nested_id("funds/a/breaks/2", "funds", "breaks").unwrap(),
            ("a".into(), "2".into())
        );
        assert!(nested_id("funds/a/exceptions/2", "funds", "breaks").is_err());
    }

    #[test]
    fn a_fund_with_no_report_has_no_breaks_rather_than_an_error() {
        // A NAV morning before the first reconciliation is a normal state.
        let d = fresh("noreport");
        book(&d);
        let r = Console::new(&d).list_breaks(&demo_view(), "").unwrap();
        assert!(r.breaks.is_empty());
    }

    #[test]
    fn breaks_are_ordered_by_money_and_severity_follows_the_tolerance() {
        let d = fresh("breaks");
        book(&d);
        // The bands this report is graded against, stored so the book can read
        // them back. 5.00 and 1,000.00 — the custom numbers, but DECLARED here,
        // which is what makes the assertions below about a term of an agreement
        // rather than about two constants.
        let digest = config_with_tolerance(&d, 500, 100_000);
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(),
            config_digest: digest,
            scope: None,
            transactions_replayed: 2,
            entries_posted: 2,
            breaks: vec![
                kernel::BreakLine { account: 40, display_name: "Management fee payable".into(),
                    ratio_amount: 100, reported_amount: 0, difference: 100,
                    cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
                kernel::BreakLine { account: 1, display_name: "Investments at fair value".into(),
                    ratio_amount: 25_000_000, reported_amount: 24_000_000, difference: 1_000_000,
                    cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
            ],
            exceptions: vec![],
            book_ties: true,
        };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        std::fs::write(d.join("reports/r.pb"), report.encode_to_vec()).unwrap();

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(ks.len(), 2);
        assert_eq!(ks[0].difference, "1000000", "largest first");
        assert_eq!(ks[0].severity, pb::Severity::High as i32, "1,000,000 blocks the NAV");
        assert_eq!(ks[1].severity, pb::Severity::Low as i32, "100 is below notice");
        // The postings behind Ratio's figure travel with the break.
        assert_eq!(ks[0].postings.len(), 1);
        assert_eq!(ks[0].postings[0].entry_id, "t1");
        assert!(!ks[0].config_digest.is_empty(), "a break must cite its configuration");
        // And the terms it was graded against travel with it, so a reader does
        // not have to go and look them up to know what the grade meant.
        let t = ks[0].tolerance.as_ref().expect("a graded break names its bounds");
        assert_eq!(t.blocks_nav, "100000");
        assert_eq!(t.below_notice, "500");
        assert!(t.declared, "this configuration says so rather than getting it by custom");
        // Nothing has explained anything, and nothing pretends otherwise.
        assert!(ks.iter().all(|k| !k.explained));

        // A blocking break makes the fund blocked.
        let c = Console::new(&d);
        let f = c.get_fund("funds/demo").unwrap();
        assert_eq!(f.state, pb::fund::State::Blocked as i32);
        assert_eq!(f.open_break_count, 2);
        // ⛔ THE UNRESOLVED TOTAL IS THE VIEW'S. Which breaks are open depends
        // on which entries are recognised, so it moved to `View` — while
        // `state` and `open_break_count` stay on `Fund` and answer for the
        // DEFAULT view, which is what their comments in the contract promise.
        assert_eq!(c.get_view(&demo_view()).unwrap().open_difference, "1000100");
    }

    #[test]
    fn a_break_is_graded_by_the_configuration_its_report_names_not_the_one_in_force_now() {
        // ⭐ THE LOAD-BEARING ONE. A break is a comparison between two figures
        // produced under one configuration, and the tolerance agreed then is
        // the term that applies to it. Reading the ACTIVE configuration instead
        // would regrade a report whose bytes have not changed the moment
        // somebody promotes a new rule set — February's break silently graded
        // under June's agreement, on a book that still ties and still replays.
        let d = fresh("gradedbyreport");
        book(&d);
        // Reconciled under bands that make 1,000.00 merely reportable.
        let loose = config_with_tolerance(&d, 500, 500_000);
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(),
            config_digest: loose,
            scope: None,
            transactions_replayed: 1,
            entries_posted: 1,
            breaks: vec![kernel::BreakLine {
                account: 1, display_name: "Investments at fair value".into(),
                ratio_amount: 25_000_000, reported_amount: 24_900_000, difference: 100_000,
                cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into(),
            }],
            exceptions: vec![],
            book_ties: true,
        };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        std::fs::write(d.join("reports/r.pb"), report.encode_to_vec()).unwrap();

        let before = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(before[0].severity, pb::Severity::Medium as i32, "reportable under those bands");

        // Now the fund tightens its tolerance and promotes it. The report is
        // untouched — same bytes, same figures, same digest.
        let mut b = FileBook::open(&d).unwrap();
        let tight = b
            .put(b"rules = []\n[tolerance]\nbelow_notice = 100\nblocks_nav = 1000\n")
            .unwrap();
        b.set_active(&tight).unwrap();

        let after = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(
            after[0].severity,
            pb::Severity::Medium as i32,
            "still graded under the terms the reconciliation ran on"
        );
        assert_eq!(after[0].tolerance.as_ref().unwrap().blocks_nav, "500000");
    }

    #[test]
    fn a_report_naming_a_configuration_that_is_not_stored_grades_every_break_high() {
        // ⛔ AND THAT IS THE DESIGN, NOT A BUG TO BE FIXED BY DEFAULTING. The
        // honest statement about a difference nobody can grade is that it was
        // not graded — and a product whose claim is that a figure can be
        // checked cannot answer "small" using a tolerance it could not read.
        // Erring towards blocking costs an operator a look; erring the other
        // way costs a NAV.
        let d = fresh("ungradable");
        book(&d);
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(),
            // Well-formed, and naming bytes this book does not hold.
            config_digest: "0".repeat(64),
            scope: None,
            transactions_replayed: 1,
            entries_posted: 1,
            breaks: vec![kernel::BreakLine {
                account: 40, display_name: "Management fee payable".into(),
                ratio_amount: 1, reported_amount: 0, difference: 1,
                cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into(),
            }],
            exceptions: vec![],
            book_ties: true,
        };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        std::fs::write(d.join("reports/r.pb"), report.encode_to_vec()).unwrap();

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(ks[0].severity, pb::Severity::High as i32, "one minor unit, and it blocks");
        assert!(ks[0].tolerance.is_none(), "and it does not claim bounds it could not read");
        // The screen still renders rather than erroring: a pruned configuration
        // must not turn the exceptions queue into a stack trace.
        assert_eq!(ks.len(), 1);
    }

    /// A book with one 1,000.00 break, graded under declared bands.
    fn book_with_a_break(name: &str, difference: i64) -> std::path::PathBuf {
        let d = fresh(name);
        book(&d);
        let digest = config_with_tolerance(&d, 500, 100_000);
        write_report(&d, &digest, difference);
        d
    }

    fn write_report(d: &Path, digest: &str, difference: i64) {
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(),
            config_digest: digest.to_string(),
            scope: None,
            transactions_replayed: 1,
            entries_posted: 1,
            breaks: vec![kernel::BreakLine {
                account: 1,
                display_name: "Investments at fair value".into(),
                ratio_amount: 25_000_000,
                reported_amount: 25_000_000 - difference,
                difference,
                cause: kernel::Cause::AmountDiffers as i32,
                ratio_basis: "1".into(),
            }],
            exceptions: vec![],
            book_ties: true,
        };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        // ⚠ A DISTINCT NAME PER REPORT. `newest_report` picks by mtime, and two
        // written inside one filesystem timestamp order arbitrarily.
        let n = std::fs::read_dir(d.join("reports")).map(|r| r.count()).unwrap_or(0);
        std::fs::write(d.join(format!("reports/r{n}.pb")), report.encode_to_vec()).unwrap();
    }

    #[test]
    fn a_break_with_an_accepted_explanation_reports_explained() {
        let d = book_with_a_break("explained", 200_000);
        let c = Console::new(&d).as_actor("e.marsh");
        c.accept_explanation(&demo_break("1"), "the custodian's unsettled dividend")
            .unwrap();

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert!(ks[0].explained, "a person recorded why, and the break says so");
        let e = ks[0].explanation.as_ref().unwrap();
        assert_eq!(e.text, "the custodian's unsettled dividend");
        assert_eq!(e.actor, "e.marsh");
        assert!(e.qualification.is_empty(), "nothing has moved under it");
        // ⛔ EXPLAINED, NOT GONE. It keeps its place in the queue.
        assert_eq!(ks.len(), 1, "an explained break is still a break");
    }

    #[test]
    fn posting_an_entry_does_not_unexplain_a_break() {
        // ⭐ THE ANTI-DEADLOCK TEST, and the reason the currency test is the
        // FIGURE rather than the journal prefix. The obvious design — an
        // explanation names the prefix it read, so a longer journal makes it
        // stale — retires every explanation on the next posting. A NAV morning
        // posts constantly, so the gate becomes one nobody can ever clear while
        // looking like the software being careful.
        let d = book_with_a_break("stillexplained", 200_000);
        let c = Console::new(&d).as_actor("e.marsh");
        c.accept_explanation(&demo_break("1"), "clears T+2").unwrap();

        let mut b = FileBook::open(&d).unwrap();
        let cfg = b.active().unwrap().unwrap();
        b.append(&ratio_store::JournalEntry {
            id: "later".into(),
            memo: "an unrelated accrual, on the same morning".into(),
            config: cfg,
            postings: vec![
                ratio_store::PostingRecord::new(10, 1_000),
                ratio_store::PostingRecord::new(40, -1_000),
            ],
            trade_date: None,
            announcement: None,
        })
        .unwrap();
        drop(b);

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert!(ks[0].explained, "the journal grew; the figure did not");
        assert!(ks[0].explanation.as_ref().unwrap().qualification.is_empty());
    }

    #[test]
    fn a_reconciliation_that_moves_the_figure_retires_the_explanation_and_says_what_moved() {
        // ⭐ THE OTHER SIDE. "The 2,000.00 is the custodian's unsettled
        // dividend" is a claim about 2,000.00. When the next run reports
        // 2,750.00 the words are about something that is no longer there, and
        // an explanation that outlived its figure is how a fund gets struck on
        // a difference nobody has actually looked at.
        let d = book_with_a_break("retired", 200_000);
        let digest = {
            let b = FileBook::open(&d).unwrap();
            b.records::<ratio_ingest::Fact>(Plane::Facts).ok();
            newest_report(&d).unwrap().unwrap().config_digest
        };
        Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "clears T+2")
            .unwrap();

        // A later run, same terms, different figure.
        write_report(&d, &digest, 275_000);

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert!(!ks[0].explained, "the note was about a figure that has moved");
        let e = ks[0].explanation.as_ref().unwrap();
        assert_eq!(e.text, "clears T+2", "and the note is still visible as evidence");
        assert_eq!(e.difference, "200000", "carrying the figure it was written about");
        assert_eq!(e.qualification.len(), 1, "and saying what moved");
        assert!(e.qualification[0].contains("275000"), "{:?}", e.qualification);
    }

    #[test]
    fn an_explanation_written_about_a_different_figure_does_not_explain_this_one() {
        // The same property stated over two breaks rather than two runs: an
        // explanation is keyed to a break AND a figure, so it cannot drift onto
        // a difference nobody wrote it about.
        let d = book_with_a_break("wrongfigure", 200_000);
        Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "about the old number")
            .unwrap();

        // Same break name, same configuration, a different difference.
        let digest = newest_report(&d).unwrap().unwrap().config_digest;
        write_report(&d, &digest, 1);

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert!(!ks[0].explained);
    }

    #[test]
    fn the_newest_explanation_wins_and_the_earlier_one_is_still_on_disk() {
        // Append-only, like every plane here. A correction is a new record; the
        // one somebody thought better of is part of what happened.
        let d = book_with_a_break("corrected", 200_000);
        let c = Console::new(&d).as_actor("e.marsh");
        c.accept_explanation(&demo_break("1"), "first thought").unwrap();
        c.accept_explanation(&demo_break("1"), "second thought").unwrap();

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(ks[0].explanation.as_ref().unwrap().text, "second thought");

        let raw = std::fs::read_to_string(d.join("explanations.jsonl")).unwrap();
        assert_eq!(raw.lines().count(), 2, "the first is still on disk");
        assert!(raw.contains("first thought"));
    }

    #[test]
    fn an_explanation_is_recorded_against_the_verified_actor_not_the_text() {
        // ⛔ `record_change`'s law, applied to the new verb: the actor is the
        // console's own, never anything a caller supplied. An audit trail that
        // takes the author's word for who the author is records nothing.
        let d = book_with_a_break("actor", 200_000);
        Console::new(&d)
            .as_actor("k.oyelaran")
            .accept_explanation(&demo_break("1"), "signed by somebody else, allegedly")
            .unwrap();

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(ks[0].explanation.as_ref().unwrap().actor, "k.oyelaran");

        let log = std::fs::read_to_string(d.join("CHANGELOG")).unwrap();
        let line = log.lines().find(|l| l.contains("accepted")).expect("an accepted line");
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 5, "five tab-separated fields");
        assert_eq!(f[1], "k.oyelaran");
        assert_eq!(f[2], "accepted");
        assert_eq!(f[3], demo_break("1"));
    }

    #[test]
    fn a_break_that_is_not_there_cannot_be_explained() {
        // ORCHESTRATION.md's proposal shape requires a citation that does not
        // resolve to fail before a person reads it. Here it fails before one is
        // recorded at all.
        let d = book_with_a_break("nosuch", 200_000);
        let e = Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("999"), "about nothing")
            .unwrap_err()
            .to_string();
        assert!(e.contains("no break"), "{e}");
        assert!(!d.join("explanations.jsonl").exists(), "and nothing was written");
    }

    #[test]
    fn an_explanation_with_no_words_in_it_is_refused() {
        let d = book_with_a_break("empty", 200_000);
        let e = Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "   ")
            .unwrap_err()
            .to_string();
        assert!(e.contains("explains nothing"), "{e}");
    }

    #[test]
    fn the_unexplained_filter_hides_a_break_a_person_explained() {
        // ⚠ This test could not have failed before: `explained` was a constant,
        // so `unexplained` returned everything and agreed with every assertion
        // anybody made about it.
        let d = book_with_a_break("filterexplained", 200_000);
        let c = Console::new(&d);
        assert_eq!(c.list_breaks(&demo_view(), "unexplained").unwrap().breaks.len(), 1);

        Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "known and accepted")
            .unwrap();

        assert_eq!(
            c.list_breaks(&demo_view(), "unexplained").unwrap().breaks.len(),
            0,
            "explained is not unexplained"
        );
        assert_eq!(c.list_breaks(&demo_view(), "").unwrap().breaks.len(), 1, "and it is still there");
    }

    #[test]
    fn an_explanation_survives_the_book_being_served_under_another_fund_name() {
        // ⛔ THE BUG THIS EXISTS FOR, AND ONLY THE SEEDED DEMO FOUND IT. A note
        // was keyed by the break's RESOURCE NAME, whose fund half is a property
        // of how the book is served rather than of the book: the seeder writes
        // one against `funds/demo/views/book/breaks/1` on a loopback book, and
        // the same directory under a funds root serves that break as
        // `funds/pennington-select-income/views/book/breaks/1`. It sat on
        // disk, the break sat on screen, and nothing connected them — so a fund
        // seeded as explained came up BLOCKED with the note invisible.
        //
        // ⚠ NO UNIT TEST COULD HAVE SEEN IT: every one of them uses a
        // root-that-is-a-book, where the fund is always `demo`.
        let root = fresh("servedelsewhere");
        let inner = root.join("pennington-select-income");
        std::fs::create_dir_all(&inner).unwrap();
        book(&inner);
        let digest = config_with_tolerance(&inner, 500, 100_000);
        write_report(&inner, &digest, 200_000);

        // Accepted the way the seeder does it: against the book directly,
        // where the fund is `demo`.
        Console::new(&inner)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "known and accepted")
            .unwrap();

        // Read the way the console serves it: as one fund among several.
        let served = Console::new(&root)
            .list_breaks(
                &format!(
                    "funds/pennington-select-income/views/{}",
                    ratio_rules::UNDECLARED_VIEW
                ),
                "",
            )
            .unwrap()
            .breaks;
        assert!(
            served[0].explained,
            "the note followed the book, not the name it is served under",
        );
        assert_eq!(served[0].explanation.as_ref().unwrap().actor, "e.marsh");
    }

    #[test]
    fn the_gate_and_the_fund_state_are_one_derivation() {
        // ⭐ THE PROPERTY THAT STOPS THE SCREEN AND THE REFUSAL DRIFTING. The
        // console reported BLOCKED for months while `ratio strike` never asked,
        // and the fix is not "make the command check too" — it is that there is
        // one fold of what blocks and both read it. Two plausible folds,
        // independently maintained, are one field apart within a month.
        let d = book_with_a_break("onederivation", 200_000);
        let c = Console::new(&d);

        let blocked_now = |c: &Console| {
            c.get_fund("funds/demo").unwrap().state == pb::fund::State::Blocked as i32
        };

        // Blocking, on both readings.
        assert!(!c.blocking_at("demo").unwrap().is_empty());
        assert!(blocked_now(&c));

        // Explained: neither reading blocks.
        Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "known and accepted")
            .unwrap();
        assert!(c.blocking_at("demo").unwrap().is_empty());
        assert!(!blocked_now(&c), "the badge agrees with the gate");

        // And a fund below its own tolerance blocks on neither.
        let e = book_with_a_break("onederivationlow", 100);
        let c2 = Console::new(&e);
        assert!(c2.blocking_at("demo").unwrap().is_empty());
        assert!(!blocked_now(&c2));
    }

    #[test]
    fn an_accepted_line_is_not_read_as_a_configuration_promotion() {
        // ⛔ `config_versions` FILTERS CHANGELOG ON `approved`, and it does that
        // because a line keyed by the same digest under a different verb would
        // report the last person who did something under a configuration as the
        // one who approved it. `accepted` is a new verb writing the break's
        // config digest into that same column.
        let d = book_with_a_break("changelogverbs", 200_000);
        let before = Console::new(&d).list_config_versions("funds/demo").unwrap().config_versions;

        Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&demo_break("1"), "known and accepted")
            .unwrap();

        let after = Console::new(&d).list_config_versions("funds/demo").unwrap().config_versions;
        assert_eq!(after.len(), before.len(), "an acceptance is not a promotion");
        for v in &after {
            assert_ne!(v.actor, "e.marsh", "and it did not sign one: {v:?}");
        }
    }

    #[test]
    fn a_lot_break_is_not_explained_it_is_corrected() {
        // ⚠ THE NAME IS A POSITION IN A LIST. `lot-1` is the first lot break
        // this projection reports, so an explanation keyed on it would follow
        // the position rather than the sale the moment an earlier one clears —
        // every citation still resolving, the books still tying, the words
        // attached to a different disposal. And what closes a lot break is an
        // entry that makes the lot book and the position agree, not a note.
        let d = fresh("lotaccept");
        book(&d);
        {
            use ratio_store::{JournalEntry, PostingRecord};
            let mut b = FileBook::open(&d).unwrap();
            let cfg = b.active().unwrap().unwrap();
            // Seven units in, three out — a pro-rata split that will not divide.
            b.append(&JournalEntry {
                id: "buy".into(),
                memo: "buy".into(),
                config: cfg.clone(),
                postings: vec![
                    PostingRecord { dim: 1, amount: 100, currency: None, instrument: Some("VTI".into()), quantity: Some(7) },
                    PostingRecord::new(2, -100),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
            b.append(&JournalEntry {
                id: "sell".into(),
                memo: "sell".into(),
                config: cfg,
                postings: vec![
                    PostingRecord { dim: 1, amount: -45, currency: None, instrument: Some("VTI".into()), quantity: Some(-3) },
                    PostingRecord::new(2, 45),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        let lot = ks
            .iter()
            .find(|k| k.name.contains("/breaks/lot-"))
            .expect("this shape produces a lot break");
        let e = Console::new(&d)
            .as_actor("e.marsh")
            .accept_explanation(&lot.name, "looks fine to me")
            .unwrap_err()
            .to_string();
        assert!(e.contains("not explained, it is corrected"), "{e}");
        assert!(e.contains("realized gain"), "the refusal says what is at stake: {e}");
    }

    #[test]
    fn a_lot_break_carries_no_tolerance_because_it_was_not_graded_by_one() {
        // A lot break is HIGH by what it MEANS — the lot book and the position
        // disagreeing corrupts the realized gain, which no reconciliation
        // reaches. Reporting bounds beside it would suggest some other number
        // would have graded it differently. None would.
        let d = fresh("lottolerance");
        book(&d);
        let mut b = FileBook::open(&d).unwrap();
        let cfg = b.active().unwrap().unwrap();
        b.append(&ratio_store::JournalEntry {
            id: "s1".into(),
            memo: "sell with no basis anybody can relieve".into(),
            config: cfg,
            postings: vec![ratio_store::PostingRecord::new(1, -1_000_000)],
            trade_date: None,
            announcement: None,
        })
        .unwrap_err();

        let ks = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks;
        for k in ks.iter().filter(|k| k.name.contains("/breaks/lot-")) {
            assert_eq!(k.severity, pb::Severity::High as i32);
            assert!(k.tolerance.is_none(), "graded by meaning, not by amount");
        }
    }

    #[test]
    fn a_break_name_round_trips_through_the_api() {
        // The name a list returns must be fetchable. It once came from the
        // report's own `books/<dir>/...` field, so a book reconciled in a
        // directory called `loop` produced breaks named `funds/loop/...` that
        // 404'd when followed — a list whose links are all dead.
        let d = fresh("roundtrip");
        book(&d);
        let report = kernel::BreakReport {
            name: "books/SOMETHING-ELSE/breakReports/r".into(),
            config_digest: "c".into(), scope: None,
            transactions_replayed: 1, entries_posted: 1,
            breaks: vec![kernel::BreakLine { account: 1,
                display_name: "Investments at fair value".into(),
                ratio_amount: 5, reported_amount: 4, difference: 1,
                cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into() }],
            exceptions: vec![], book_ties: true };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        std::fs::write(d.join("reports/r.pb"), report.encode_to_vec()).unwrap();

        let c = Console::new(&d);
        let listed = c.list_breaks(&demo_view(), "").unwrap().breaks;
        assert_eq!(listed[0].name, "funds/demo/views/book/breaks/1",
            "the name must name the fund that was asked for, not the report's book");
        assert!(c.get_break(&listed[0].name).is_ok(), "the listed name did not fetch");
    }

    #[test]
    fn a_break_keeps_its_name_across_two_runs_of_the_same_period() {
        // Names are derived from the account, not from the report. A URL that
        // died every time the report was regenerated would make every link in
        // an email dead by morning.
        let d = fresh("stablename");
        book(&d);
        let line = kernel::BreakLine { account: 1, display_name: "Investments at fair value".into(),
            ratio_amount: 5, reported_amount: 4, difference: 1,
            cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into() };
        let mk = |n: &str| kernel::BreakReport {
            name: format!("books/demo/breakReports/{n}"), config_digest: "c".into(), scope: None,
            transactions_replayed: 1, entries_posted: 1, breaks: vec![line.clone()],
            exceptions: vec![], book_ties: true };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        std::fs::write(d.join("reports/a.pb"), mk("a").encode_to_vec()).unwrap();
        let first = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks[0].name.clone();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(d.join("reports/b.pb"), mk("b").encode_to_vec()).unwrap();
        let second = Console::new(&d).list_breaks(&demo_view(), "").unwrap().breaks[0].name.clone();
        assert_eq!(first, second);
        assert_eq!(first, "funds/demo/views/book/breaks/1");
        // And it is fetchable by that name.
        assert!(Console::new(&d).get_break(&first).is_ok());
    }

    /// Promote a rule set and return its digest, the way `ratio approve` does.
    fn promote(at: &Path, toml: &str, actor: &str, subject: &str) -> String {
        let mut b = FileBook::open(at).unwrap();
        let d = b.put(toml.as_bytes()).unwrap();
        b.set_active(&d).unwrap();
        let log = at.join("CHANGELOG");
        let mut prior = std::fs::read_to_string(&log).unwrap_or_default();
        prior.push_str(&format!("1780000000\t{actor}\tapproved\t{subject}\t{}\n", d.as_str()));
        std::fs::write(&log, prior).unwrap();
        d.as_str().to_string()
    }

    const R1: &str = "[[rule]]\nid = \"fee\"\nkind = \"accrual\"\nrate_bp = 75\n\
                      day_count = \"act/365\"\n\
                      [[rule.posting]]\naccount = 10\nweight = 1\n\
                      [[rule.posting]]\naccount = 40\nweight = -1\n";

    #[test]
    fn config_versions_are_sequenced_and_attributed() {
        let d = fresh("cfgversions");
        book(&d);
        promote(&d, R1, "e.marsh", "fee_q2");

        let vs = Console::new(&d).list_config_versions("funds/demo").unwrap().config_versions;
        assert!(vs.len() >= 2, "expected the initial version and the promoted one");
        // Newest first, and the newest is the active one.
        assert!(vs[0].active, "the newest version should be active");
        assert_eq!(vs[0].actor, "e.marsh");
        assert_eq!(vs[0].subject, "fee_q2");
        assert!(vs[0].approve_time.is_some());
        // Sequence counts from the start of history, so it does not change when
        // a new version is added — unlike an index into a reversed list.
        assert_eq!(vs.last().unwrap().sequence, 1);
        // The version that predates the CHANGELOG has no actor, and says so
        // rather than inventing one.
        assert_eq!(vs.last().unwrap().actor, "");
        assert!(vs.last().unwrap().approve_time.is_none());
    }

    #[test]
    fn a_preview_writes_nothing_and_a_commit_writes_once() {
        let d = fresh("applyevent");
        book(&d);
        promote(&d, R1, "e.marsh", "fee_q2");
        let c = Console::new(&d);

        // ⚠ THE DEFAULT VIEW'S NAV, which is what `ApplyEvent` quotes.
        // `previous_net_asset_value` is a figure about a recognition
        // convention like every other, so the comparison has to be against
        // the same one rather than against "the fund".
        let before = c.get_view(&format!("funds/demo/views/{}", ratio_rules::UNDECLARED_VIEW)).unwrap();
        let entries_before = FileBook::open(&d).unwrap().entries().unwrap().len();

        let req = |validate_only: bool| pb::ApplyEventRequest {
            parent: "funds/demo".into(),
            rule_id: "fee".into(),
            event_id: "acc-1".into(),
            amount: "1000000.00".into(),
            days: "30".into(),
            instrument: String::new(),
            quantity: String::new(),
            trade_date: None,
            validate_only,
        };

        // A preview returns the entry and the NAV it WOULD produce…
        let preview = c.apply_event(&req(true)).unwrap();
        assert!(preview.validate_only);
        let entry = preview.entry.as_ref().unwrap();
        assert!(!entry.postings.is_empty(), "a preview still shows the postings");
        assert_eq!(entry.config_digest.len(), 64, "and which configuration decided them");
        assert_eq!(preview.previous_net_asset_value, before.net_asset_value);

        // …and writes nothing.
        assert_eq!(
            FileBook::open(&d).unwrap().entries().unwrap().len(),
            entries_before,
            "a preview must not touch the journal",
        );
        assert_eq!(c.get_view(&format!("funds/demo/views/{}", ratio_rules::UNDECLARED_VIEW)).unwrap().net_asset_value, before.net_asset_value);

        // The commit writes exactly one entry, and the NAV the preview
        // predicted is the NAV that results.
        let done = c.apply_event(&req(false)).unwrap();
        assert!(!done.validate_only);
        assert_eq!(
            FileBook::open(&d).unwrap().entries().unwrap().len(),
            entries_before + 1,
        );
        assert_eq!(
            done.net_asset_value, preview.net_asset_value,
            "the preview predicted a different NAV from the one the commit produced",
        );
        // ⚠ …and the NAV actually MOVED. Without this the assertion above is
        // satisfied by two identical figures, which is what it would report if
        // the fold ignored the entry entirely. An accrual credits a liability,
        // so the NAV must fall.
        assert_ne!(
            done.net_asset_value, done.previous_net_asset_value,
            "the entry left the NAV unchanged, so the check above proved nothing",
        );
        assert!(
            done.net_asset_value.parse::<i64>().unwrap()
                < done.previous_net_asset_value.parse::<i64>().unwrap(),
            "accruing a fee raises a liability, so the NAV falls",
        );

        // An event is recorded once. A retried POST is a conflict, not a
        // second entry.
        let again = c.apply_event(&req(false));
        assert!(again.is_err(), "the same event id must not post twice");
        assert_eq!(
            FileBook::open(&d).unwrap().entries().unwrap().len(),
            entries_before + 1,
        );
    }

    #[test]
    fn the_ceiling_stops_writes_and_not_previews() {
        let d = fresh("ceiling");
        book(&d);
        promote(&d, R1, "e.marsh", "fee_q2");
        let c = Console::new(&d);
        let req = |id: &str, validate_only: bool| pb::ApplyEventRequest {
            parent: "funds/demo".into(),
            rule_id: "fee".into(),
            event_id: id.into(),
            amount: "100.00".into(),
            days: "1".into(),
            instrument: String::new(),
            quantity: String::new(),
            trade_date: None,
            validate_only,
        };

        // `book()` posts three entries, so a ceiling of three is already met.
        let c = c.with_max_entries(Some(3));
        let refused = c.apply_event(&req("over-1", false));
        assert!(refused.is_err(), "a write past the ceiling must be refused");
        assert!(
            format!("{:#}", refused.unwrap_err()).contains("as many as the demo accepts"),
            "and must say what happens next",
        );

        // A preview writes nothing, so the ceiling has no business refusing it.
        assert!(c.apply_event(&req("over-1", true)).is_ok(), "a preview is not a write");

        // With no ceiling there is no limit — a local run is not the demo.
        let c = c.with_max_entries(None);
        assert!(c.apply_event(&req("over-1", false)).is_ok());
    }

    /// A purchase: investments up and attributed, cash down and not.
    const TRADE: &str = "[[rule]]\nid = \"buy\"\nkind = \"trade\"\n\
                         [[rule.posting]]\naccount = 1\nweight = 1\nper_instrument = true\n\
                         [[rule.posting]]\naccount = 2\nweight = -1\n";

    /// The request a trade ticket sends, with everything filled in.
    fn trade_req(id: &str) -> pb::ApplyEventRequest {
        pb::ApplyEventRequest {
            parent: "funds/demo".into(),
            rule_id: "buy".into(),
            event_id: id.into(),
            amount: "341750.00".into(),
            days: String::new(),
            instrument: "ACME".into(),
            quantity: "1000".into(),
            trade_date: Some(ratio_proto::date_proto::google::r#type::Date {
                year: 2026,
                month: 2,
                day: 26,
            }),
            validate_only: false,
        }
    }

    #[test]
    fn a_recorded_trade_carries_the_instrument_the_units_and_the_day() {
        // ⭐ THE WHOLE POINT OF THE THREE FIELDS. Before them this method built
        // its event with `instrument: None, quantity: None` and its entry with
        // `trade_date: None`, and `Projection::walk` skips any posting lacking
        // BOTH an instrument and a quantity — so every trade the console
        // recorded opened no lot and relieved none, while the entry balanced and
        // the trial balance tied. Nothing downstream said a word.
        let d = fresh("tradefields");
        book(&d);
        promote(&d, TRADE, "e.marsh", "buy");
        Console::new(&d).apply_event(&trade_req("trd-1")).unwrap();

        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        let e = entries.iter().find(|e| e.id == "trd-1").expect("the trade was written");

        // ⛔ THE DAY IS ON THE ENTRY, which is where `Ratio.Lots.Relief` reads a
        // holding period from. A lot opened by an entry without one is refused
        // by every method rather than defaulted.
        assert_eq!(e.trade_date.as_deref(), Some("2026-02-26"));

        // ⛔ AND ONLY THE `per_instrument` LEG IS ATTRIBUTED. The cash leg names
        // no instrument, because cash is not held in one.
        let inv = e.postings.iter().find(|p| p.dim == 1).expect("the investments leg");
        let cash = e.postings.iter().find(|p| p.dim == 2).expect("the cash leg");
        assert_eq!(inv.instrument.as_deref(), Some("ACME"));
        assert_eq!(inv.quantity, Some(1_000), "a purchase adds units");
        assert_eq!(cash.instrument, None);
        assert_eq!(cash.quantity, None);
    }

    #[test]
    fn a_sale_gives_up_units_without_the_caller_negating_anything() {
        // ⛔ A SIDE IS A RULE, NOT A SIGN. The quantity is positive on both
        // sides and `compile` takes the direction from the LEG'S weight, so a
        // disposal removes units. A caller that negated the quantity itself
        // would ADD units on a sale, and the entry would still balance.
        let sell = "[[rule]]\nid = \"sell\"\nkind = \"trade\"\n\
                    [[rule.posting]]\naccount = 2\nweight = 1\n\
                    [[rule.posting]]\naccount = 1\nweight = -1\nper_instrument = true\n";
        let d = fresh("tradesell");
        book(&d);
        promote(&d, sell, "e.marsh", "sell");
        let mut req = trade_req("trd-sell");
        req.rule_id = "sell".into();
        Console::new(&d).apply_event(&req).unwrap();

        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        let e = entries.iter().find(|e| e.id == "trd-sell").unwrap();
        let inv = e.postings.iter().find(|p| p.dim == 1).unwrap();
        assert_eq!(inv.quantity, Some(-1_000), "a disposal gives units up");
    }

    #[test]
    fn a_fractional_quantity_is_refused_rather_than_carried_as_none() {
        // ⛔ THE DATA PLANE DROPS IT; THIS MUST NOT. `admit_facts` carries a
        // non-whole quantity as `None`, which is defensible for a file nobody
        // read — and indefensible here, where a person typed the number and
        // would be handed back the lot-less entry these fields exist to prevent.
        let d = fresh("tradefrac");
        book(&d);
        promote(&d, TRADE, "e.marsh", "buy");
        let c = Console::new(&d);

        let mut req = trade_req("trd-frac");
        req.quantity = "10.5".into();
        let r = c.apply_event(&req);
        assert!(r.is_err(), "a fractional quantity must be refused");
        assert!(
            format!("{:#}", r.unwrap_err()).contains("whole number of units"),
            "and must say why",
        );

        // ⛔ AND NEGATIVE IS THE OTHER WAY TO BOOK A TRADE BACKWARDS.
        let mut req = trade_req("trd-neg");
        req.quantity = "-1000".into();
        assert!(c.apply_event(&req).is_err(), "a negative quantity must be refused");
    }

    #[test]
    fn an_instrument_without_a_quantity_is_refused() {
        // ⛔ HALF OF IT IS THE DEFECT WEARING A DISGUISE. `walk` needs BOTH, so
        // a posting naming an instrument and no units opens no lot either — and
        // reports itself as attributed while doing it.
        let d = fresh("tradehalf");
        book(&d);
        promote(&d, TRADE, "e.marsh", "buy");
        let c = Console::new(&d);

        for (instrument, quantity) in [("ACME", ""), ("", "1000")] {
            let mut req = trade_req("trd-half");
            req.instrument = instrument.into();
            req.quantity = quantity.into();
            let r = c.apply_event(&req);
            assert!(r.is_err(), "{instrument:?}/{quantity:?} should be refused");
            assert!(format!("{:#}", r.unwrap_err()).contains("go together"));
        }

        // Neither is still fine: that is a movement of value, and some events
        // genuinely are one.
        let mut req = trade_req("trd-none");
        req.instrument = String::new();
        req.quantity = String::new();
        assert!(c.apply_event(&req).is_ok(), "an event with no instrument is still an event");
    }

    #[test]
    fn a_trade_date_that_is_not_a_date_is_refused_at_the_door() {
        // ⚠ THE ONLY PLACE IT IS FREE. A journal is append-only, so a date the
        // projection cannot parse is recorded as a BREAK and the lots it opened
        // are already wrong. `ratio_project` has the test for that shape; this
        // is the door it should never have got through.
        let d = fresh("tradedate");
        book(&d);
        promote(&d, TRADE, "e.marsh", "buy");
        let c = Console::new(&d);

        for bad in [(2026, 13, 1), (2026, 2, 30), (0, 0, 0)] {
            let mut req = trade_req("trd-baddate");
            req.trade_date = Some(ratio_proto::date_proto::google::r#type::Date {
                year: bad.0,
                month: bad.1,
                day: bad.2,
            });
            assert!(c.apply_event(&req).is_err(), "{bad:?} should not be a trade date");
        }

        // ⚠ And ABSENT is not the same as invalid. An event that names no day is
        // accepted; the lot it opens carries none, and the holding-period
        // methods refuse THAT rather than guessing.
        let mut req = trade_req("trd-nodate");
        req.trade_date = None;
        assert!(c.apply_event(&req).is_ok());
    }

    #[test]
    fn an_event_id_that_is_not_an_id_is_refused() {
        let d = fresh("eventid");
        book(&d);
        promote(&d, R1, "e.marsh", "fee_q2");
        let c = Console::new(&d);

        // This reaches a journal other people read, on a public endpoint.
        for bad in ["", "../../etc/passwd", "a b", "<script>", "x".repeat(65).as_str()] {
            let r = c.apply_event(&pb::ApplyEventRequest {
                parent: "funds/demo".into(),
                rule_id: "fee".into(),
                event_id: bad.into(),
                amount: "1.00".into(),
                days: "1".into(),
                instrument: String::new(),
                quantity: String::new(),
                trade_date: None,
                validate_only: false,
            });
            assert!(r.is_err(), "{bad:?} should not be accepted as an event id");
        }
        // And the memo is composed, never supplied — there is no field for it.
        let ok = c.apply_event(&pb::ApplyEventRequest {
            parent: "funds/demo".into(),
            rule_id: "fee".into(),
            event_id: "acc-2".into(),
            amount: "1000.00".into(),
            days: "1".into(),
            instrument: String::new(),
            quantity: String::new(),
            trade_date: None,
            validate_only: true,
        })
        .unwrap();
        assert_eq!(ok.entry.unwrap().memo, "acc-2 via fee");
    }

    #[test]
    fn a_late_action_names_the_navs_it_was_not_in() {
        use ratio_ingest::actions::{Announced, Split};
        use ratio_store::Plane;

        let d = fresh("stale");
        book(&d);
        // A NAV, struck before anybody heard about the action.
        let s =
            ratio_nav::strike_and_record(&d, ratio_rules::UNDECLARED_VIEW, 1_780_000_000, "e.marsh")
                .unwrap();
        let day = ratio_nav::rfc3339(s.valuation_time)[..10].to_string();

        let mut b = FileBook::open(&d).unwrap();
        let announce = |b: &mut FileBook, id: &str, ex: &str| {
            b.append_record(
                Plane::Actions,
                &Announced {
                    id: id.into(),
                    instrument: "inst-x".into(),
                    split: Split { num: 2, den: 1 },
                    ex_date: ex.into(),
                    announced: 1_780_000_100,
                },
            )
            .unwrap();
        };
        // One effective BEFORE the strike, one after.
        announce(&mut b, "before", "2000-01-01");
        announce(&mut b, "after", "2999-01-01");
        drop(b);

        let rows = Console::new(&d).stale_strikes("demo").unwrap();
        assert_eq!(rows.len(), 1, "only the one that should have been in it: {rows:?}");
        assert_eq!(rows[0].0, s.id);
        assert_eq!(rows[0].1, "before");
        assert!(rows[0].2.contains("not applied"), "{}", rows[0].2);

        // ⛔ AND THE STRIKE ITSELF IS UNTOUCHED.
        // `Ratio.Period.one_answer_per_view_per_day`
        // refuses restatement, so naming a stale figure must not quietly change
        // it — the first answer is what somebody was paid on.
        let after = ratio_nav::list(&d).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].net_asset_value, s.net_asset_value);
        assert_eq!(after[0].journal_digest, s.journal_digest);

        // Sanity: `{day}` is the strike's own day, so the comparison above was
        // against a real date rather than an empty string.
        assert_eq!(day.len(), 10, "the strike has a valuation day: {day:?}");
    }

    #[test]
    fn the_trial_balance_agrees_with_itself() {
        use ratio_store::{Account, AccountTypeRecord as A};
        let d = fresh("trialbalance");
        book(&d);
        // `book()` posts to all five of its accounts, so a chart-sourced list
        // and a journal-sourced one would be indistinguishable on it. This
        // sixth account is the one that tells them apart.
        {
            let mut b = FileBook::open(&d).unwrap();
            let mut chart = b.accounts().unwrap();
            chart.push(Account {
                dim: 3,
                display_name: "Dividends receivable".into(),
                account_type: A::Asset,
            });
            b.put_accounts(&chart).unwrap();
        }

        let c = Console::new(&d);
        let rows = c.list_accounts(&demo_view(), "").unwrap().accounts;

        assert_eq!(rows.len(), 6, "every account in the chart, posted to or not");
        let idle = rows.iter().find(|a| a.dimension == "3").expect("the untouched account is a row");
        assert_eq!(idle.posting_count, "0");
        assert_eq!(idle.balance, "0");
        // …and `posted` is what drops it.
        assert_eq!(c.list_accounts(&demo_view(), "posted").unwrap().accounts.len(), 5);

        let debit: i64 = rows.iter().map(|a| a.debit.parse::<i64>().unwrap()).sum();
        let credit: i64 = rows.iter().map(|a| a.credit.parse::<i64>().unwrap()).sum();
        assert_eq!(debit, credit, "the two columns must agree");

        // And the view reports the same two figures — they must come from one
        // read of the journal, not two.
        //
        // ⚠ THE COLUMNS ARE THE VIEW'S AND THE DIFFERENCE IS THE FUND'S, which
        // is the line this feature draws. A view that has not recognised a
        // trade has folded neither of its legs, so both columns shrink — while
        // their difference is the same zero in every view, because conservation
        // is per entry. `Ratio.Views.every_view_conserves`.
        let v = c.get_view(&demo_view()).unwrap();
        assert_eq!(v.total_debit, debit.to_string());
        assert_eq!(v.total_credit, credit.to_string());
        assert_eq!(c.get_fund("funds/demo").unwrap().trial_balance_difference, "0");
    }

    #[test]
    fn a_balance_on_the_wrong_side_is_flagged() {
        // ⚠ The demo book has no abnormal account, so `filter=abnormal`
        // returns nothing there — and "nothing" reads the same whether the
        // flag works or the filter is broken. This is the case that tells them
        // apart: an asset with a credit balance.
        use ratio_store::{Account, AccountTypeRecord as A, JournalEntry, PostingRecord};
        let d = fresh("abnormal");
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 2, display_name: "Cash and equivalents".into(), account_type: A::Asset },
            Account { dim: 20, display_name: "Capital contributions".into(), account_type: A::Equity },
        ])
        .unwrap();
        let cfg = b.put(b"rules = []\n").unwrap();
        b.set_active(&cfg).unwrap();
        // Cash goes negative: an overdrawn account. Legal, and worth a look.
        b.append(&JournalEntry {
            id: "w1".into(),
            memo: "drawn down past zero".into(),
            config: cfg.clone(),
            postings: vec![
                PostingRecord::new(2, -500),
                PostingRecord::new(20, 500),
            ],
        
            trade_date: None,
            announcement: None,
        })
        .unwrap();
        drop(b);

        let c = Console::new(&d);
        let cash = c.get_account(&format!("{}/accounts/2", demo_view())).unwrap();
        assert_eq!(cash.balance, "-500");
        assert!(cash.abnormal, "an asset with a credit balance sits on the abnormal side");

        // Equity is credit-normal, so a credit balance is ordinary there —
        // otherwise the flag would just be reporting the sign.
        let equity = c.get_account(&format!("{}/accounts/20", demo_view())).unwrap();
        assert_eq!(equity.balance, "500");
        assert!(equity.abnormal, "equity holding a DEBIT balance is the abnormal one");

        let flagged = c.list_accounts(&demo_view(), "abnormal").unwrap().accounts;
        assert_eq!(flagged.len(), 2, "the filter returns exactly what is flagged");
    }

    #[test]
    fn the_running_balance_lands_on_the_account_balance() {
        let d = fresh("running");
        book(&d);
        let c = Console::new(&d);

        for a in c.list_accounts(&demo_view(), "posted").unwrap().accounts {
            let lines = c.list_postings(&a.name).unwrap().postings;
            assert!(!lines.is_empty(), "{} was filtered as posted-to", a.name);
            // The whole claim of the screen: the lines add up to the figure.
            assert_eq!(
                lines.last().unwrap().running_balance,
                a.balance,
                "the last running balance on {} is not the account balance",
                a.display_name,
            );
            // And each line is citable on its own.
            let one = c.get_posting(&lines[0].name).unwrap();
            assert_eq!(one.entry_id, lines[0].entry_id);
            // ⭐ THE HOP THE POSTING USED TO PRINT AS TEXT. The id GetPosting
            // carries is a GetEntry name, and the two agree about the memo
            // and the configuration that produced the line.
            let entry = c
                .get_entry(&format!("funds/demo/entries/{}", one.entry_id))
                .unwrap();
            assert_eq!(entry.entry_id, one.entry_id);
            assert_eq!(entry.memo, one.memo);
            assert_eq!(entry.config_digest, one.config_digest);
            assert_eq!(entry.name, format!("funds/demo/entries/{}", one.entry_id));
            assert!(
                entry.postings.iter().any(|p| p.amount == one.amount),
                "the entry must carry the posting that cited it"
            );
        }
    }

    #[test]
    fn a_journal_entry_is_citable_by_id() {
        let d = fresh("getentry");
        book(&d);
        let c = Console::new(&d);
        let e = c.get_entry("funds/demo/entries/t1").unwrap();
        assert_eq!(e.entry_id, "t1");
        assert_eq!(e.memo, "buy");
        assert_eq!(e.name, "funds/demo/entries/t1");
        assert!(!e.config_digest.is_empty(), "an entry cites the configuration that posted it");
        assert_eq!(e.postings.len(), 2, "a buy moves two accounts");
        // ⚠ VIEW-SCOPED, so a pasted name resolves after the jobs moved under
        // `/books`. A fund-only account name would 404 in the confident voice.
        assert!(
            e.postings.iter().all(|p| p.account.contains("/views/book/accounts/")),
            "account names must name the default view: {:?}",
            e.postings.iter().map(|p| &p.account).collect::<Vec<_>>(),
        );
        let miss = c.get_entry("funds/demo/entries/nope");
        assert!(miss.is_err(), "an id the journal does not have is an absence");
        assert!(
            format!("{:#}", miss.unwrap_err()).contains("no entry"),
            "the 404 mapping in watch.rs keys on this phrase",
        );

        // AIP-121: the list is the journal, and every listed name GetEntry
        // answers. A list whose links 404 is how this started one layer down.
        let listed = c.list_entries("funds/demo").unwrap().entries;
        assert_eq!(listed.len(), 3, "the seeded book posts capital, a buy, a fee");
        assert_eq!(listed[1].entry_id, "t1");
        for e in &listed {
            let one = c.get_entry(&e.name).unwrap();
            assert_eq!(one.entry_id, e.entry_id);
            assert_eq!(one.postings, e.postings);
        }
    }

    #[test]
    fn apply_event_and_get_entry_agree_about_what_was_posted() {
        // ⭐ TWO PATHS, ONE RESOURCE. ApplyEvent returns an Entry; GetEntry
        // must return the same one, or a citation from the ticket to the
        // journal page is a different figure wearing the same id.
        let d = fresh("entryroundtrip");
        book(&d);
        promote(&d, R1, "e.marsh", "fee_q2");
        let c = Console::new(&d);
        let done = c
            .apply_event(&pb::ApplyEventRequest {
                parent: "funds/demo".into(),
                rule_id: "fee".into(),
                event_id: "acc-cite".into(),
                amount: "1000000.00".into(),
                days: "30".into(),
                instrument: String::new(),
                quantity: String::new(),
                trade_date: None,
                validate_only: false,
            })
            .unwrap();
        let posted = done.entry.expect("a commit returns the entry");
        let fetched = c.get_entry(&posted.name).unwrap();
        assert_eq!(fetched.name, posted.name);
        assert_eq!(fetched.entry_id, posted.entry_id);
        assert_eq!(fetched.memo, posted.memo);
        assert_eq!(fetched.config_digest, posted.config_digest);
        assert_eq!(fetched.postings, posted.postings);
    }

    #[test]
    fn the_first_version_reads_as_all_added() {
        use ratio_store::{Account, AccountTypeRecord as A};
        // Not `book()` — that one opens with `rules = []`, so its first version
        // has nothing in it to read as added. This book's opening
        // configuration is the rule.
        let d = fresh("firstversion");
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 10, display_name: "Management fee expense".into(), account_type: A::Expense },
            Account { dim: 40, display_name: "Management fee payable".into(), account_type: A::Liability },
        ])
        .unwrap();
        drop(b);
        let first = promote(&d, R1, "e.marsh", "opening");

        let out = Console::new(&d)
            .diff_config_versions(&format!("funds/demo/configVersions/{first}"), "")
            .unwrap();

        // Nothing precedes it, so there is no base digest to name.
        assert_eq!(out.base_digest, "", "the first version has no predecessor");
        assert!(!out.changes.is_empty(), "the opening configuration is not empty");
        assert!(
            out.changes.iter().all(|c| c.kind == pb::rule_change::Kind::Added as i32),
            "every rule in the first version was added by it",
        );
        assert!(
            out.changes.iter().all(|c| c.base_form.is_empty()),
            "there is no earlier form of a rule that did not exist",
        );
    }

    #[test]
    fn a_diff_names_what_was_added_changed_and_removed() {
        let d = fresh("cfgdiff");
        book(&d);
        let v1 = promote(&d, R1, "e.marsh", "fee_q2");
        // Change the rate, add a rule, and remove nothing.
        let r2 = R1.replace("rate_bp = 75", "rate_bp = 80")
            + "\n[[rule]]\nid = \"perf\"\nkind = \"trade\"\n\
               [[rule.posting]]\naccount = 1\nweight = 1\n\
               [[rule.posting]]\naccount = 2\nweight = -1\n";
        let v2 = promote(&d, &r2, "j.okafor", "perf_fee");

        let c = Console::new(&d);
        let diff = c
            .diff_config_versions(&format!("funds/demo/configVersions/{v2}"), "")
            .unwrap();
        assert_eq!(diff.base_digest, v1, "base should default to the previous version");
        let by: BTreeMap<&str, i32> =
            diff.changes.iter().map(|c| (c.rule_id.as_str(), c.kind)).collect();
        assert_eq!(by["fee"], pb::rule_change::Kind::Changed as i32);
        assert_eq!(by["perf"], pb::rule_change::Kind::Added as i32);
        // A changed rule carries both forms, which is what makes the diff
        // readable rather than merely true.
        let fee = diff.changes.iter().find(|c| c.rule_id == "fee").unwrap();
        assert!(fee.base_form.contains("75bp"), "{}", fee.base_form);
        assert!(fee.form.contains("80bp"), "{}", fee.form);

        // And removal, diffing the other direction.
        let back = c
            .diff_config_versions(&format!("funds/demo/configVersions/{v1}"), &v2)
            .unwrap();
        let kinds: BTreeMap<&str, i32> =
            back.changes.iter().map(|c| (c.rule_id.as_str(), c.kind)).collect();
        assert_eq!(kinds["perf"], pb::rule_change::Kind::Removed as i32);
    }

    #[test]
    fn diffing_a_version_against_itself_finds_nothing() {
        let d = fresh("cfgsame");
        book(&d);
        let v = promote(&d, R1, "e.marsh", "fee");
        let diff = Console::new(&d)
            .diff_config_versions(&format!("funds/demo/configVersions/{v}"), &v)
            .unwrap();
        assert!(diff.changes.is_empty(), "{:?}", diff.changes);
    }

    #[test]
    fn the_first_version_diffs_against_nothing_rather_than_panicking() {
        // `at - 1` on the first element underflows. A history of one is the
        // normal state of a fund on its first day.
        let d = fresh("cfgfirst");
        book(&d);
        let first = Console::new(&d).list_config_versions("funds/demo").unwrap()
            .config_versions.last().unwrap().digest.clone();
        let diff = Console::new(&d)
            .diff_config_versions(&format!("funds/demo/configVersions/{first}"), "")
            .unwrap();
        assert_eq!(diff.base_digest, "", "nothing precedes the first version");
    }

    #[test]
    fn an_unknown_version_is_an_error_not_an_empty_diff() {
        let d = fresh("cfgunknown");
        book(&d);
        let c = Console::new(&d);
        assert!(c.get_config_version("funds/demo/configVersions/deadbeef").is_err());
        assert!(c
            .diff_config_versions("funds/demo/configVersions/deadbeef", "")
            .is_err());
    }

    #[test]
    fn the_change_log_reads_what_approve_recorded_and_a_model_never_approves() {
        let d = fresh("log");
        book(&d);
        std::fs::write(d.join("CHANGELOG"),
            "1780000000\te.marsh\tapproved\tfee_q2\tabc123\n").unwrap();
        std::fs::create_dir_all(d.join("proposals")).unwrap();
        std::fs::write(d.join("proposals/perf_fee.toml"), "").unwrap();

        let es = Console::new(&d).list_change_log_entries("funds/demo").unwrap().change_log_entries;
        assert_eq!(es.len(), 2);
        // Newest first: the pending draft is the most recent thing to happen.
        assert_eq!(es[0].action, "drafted");
        assert_eq!(es[0].actor_kind, pb::ActorKind::Model as i32);
        assert_eq!(es[1].action, "approved");
        assert_eq!(es[1].actor, "e.marsh");
        assert_eq!(es[1].actor_kind, pb::ActorKind::Person as i32);
        // The load-bearing one: no model ever appears as the author of an
        // approval, in the log a regulator would read.
        assert!(
            es.iter().all(|e| !(e.actor_kind == pb::ActorKind::Model as i32 && e.action == "approved")),
            "a model appears as an approver in the change log"
        );
        assert!(Console::new(&d).get_change_log_entry(&es[1].name).is_ok());
    }

    #[test]
    fn an_approved_proposal_stops_appearing_as_a_pending_draft() {
        let d = fresh("logdedupe");
        book(&d);
        std::fs::write(d.join("CHANGELOG"),
            "1780000000\te.marsh\tapproved\tfee_q2\tabc123\n").unwrap();
        std::fs::create_dir_all(d.join("proposals")).unwrap();
        std::fs::write(d.join("proposals/fee_q2.toml"), "").unwrap();
        let es = Console::new(&d).list_change_log_entries("funds/demo").unwrap().change_log_entries;
        assert_eq!(es.len(), 1, "the draft and its approval collapsed to the approval");
        assert_eq!(es[0].action, "approved");
    }

    #[test]
    fn filters_are_the_three_the_console_offers() {
        let d = fresh("filter");
        book(&d);
        // ⚠ A REAL DIGEST, BECAUSE `blocking` NOW DEPENDS ON ONE. With an
        // unreadable configuration every break grades HIGH and this test would
        // have gone on passing while asserting nothing about the filter —
        // `blocking` returning "all of them" looks identical to `blocking`
        // working when everything happens to block.
        let digest = config_with_tolerance(&d, 500, 100_000);
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(), config_digest: digest, scope: None,
            transactions_replayed: 1, entries_posted: 1,
            breaks: vec![
                kernel::BreakLine { account: 1, display_name: "Investments at fair value".into(),
                    ratio_amount: 200_000, reported_amount: 0, difference: 200_000,
                    cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
                kernel::BreakLine { account: 40, display_name: "Management fee payable".into(),
                    ratio_amount: 10, reported_amount: 0, difference: 10,
                    cause: kernel::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
            ],
            exceptions: vec![], book_ties: true };
        std::fs::create_dir_all(d.join("reports")).unwrap();
        std::fs::write(d.join("reports/r.pb"), report.encode_to_vec()).unwrap();
        let c = Console::new(&d);
        assert_eq!(c.list_breaks(&demo_view(), "").unwrap().breaks.len(), 2);
        assert_eq!(c.list_breaks(&demo_view(), "blocking").unwrap().breaks.len(), 1);
        assert_eq!(c.list_breaks(&demo_view(), "unexplained").unwrap().breaks.len(), 2);
    }
}
