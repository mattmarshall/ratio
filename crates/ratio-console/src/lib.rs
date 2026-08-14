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

/// Above this, a difference blocks the NAV. Below the lower one, it is noise.
///
/// Both are in minor units and are the demo's stand-in for a real tolerance
/// policy, which belongs in the configuration rather than in a constant — a
/// fund's tolerance is a term of its administration agreement, not a property
/// of the software. Named here so that is visible rather than buried.
const BLOCKS_NAV: i64 = 100_000; // 1,000.00
const BELOW_NOTICE: i64 = 500; //     5.00

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

    /// The funds, in a stable order.
    ///
    /// Sorted by id rather than by anything derived, so the list does not
    /// reorder under an operator between two glances at the same screen.
    fn fund_ids(&self) -> Result<Vec<String>> {
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
        // to some funds gets exactly those; an operator restricted to NONE gets
        // an empty list — a valid answer, not a refusal, and the two must not
        // look alike. `book_path` re-guards each id `list_funds` then reads, so
        // this filter is the visible half of a boundary the storage layer
        // enforces regardless of it.
        if let Some(allowed) = &self.allowed {
            ids.retain(|id| allowed.contains(id));
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
    pub fn list_accounts(&self, parent: &str, filter: &str) -> Result<pb::ListAccountsResponse> {
        let (fund, view) = view_scoped_parent(parent)?;
        self.view_the_projection_can_answer(&fund, &view)?;
        let accounts = self.accounts_of(&fund, &view)?;
        let keep: Vec<pb::Account> = accounts
            .into_iter()
            .filter(|a| match filter {
                "posted" => a.posting_count != "0",
                "abnormal" => a.abnormal,
                _ => true,
            })
            .collect();
        Ok(pb::ListAccountsResponse { accounts: keep, next_page_token: String::new() })
    }

    pub fn get_account(&self, name: &str) -> Result<pb::Account> {
        let (fund, view, dim) = view_scoped_id(name, "accounts")?;
        self.view_the_projection_can_answer(&fund, &view)?;
        self.accounts_of(&fund, &view)?
            .into_iter()
            .find(|a| a.dimension == dim)
            .with_context(|| format!("no account {dim:?} in {fund}"))
    }

    /// Every account in a fund's chart, with its debit and credit totals.
    fn accounts_of(&self, fund: &str, view: &str) -> Result<Vec<pb::Account>> {
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
        let mut totals: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
        let mut split: BTreeMap<i64, Vec<pb::CurrencyTotal>> = BTreeMap::new();
        for ((dim, ccy), (debit, credit)) in b.balances_by_dim()? {
            let factor = rates.factor_of_optional(ccy.as_deref()).with_context(|| {
                format!(
                    "this fund holds {} and no rate for it was supplied — an account total \
                     mixing denominations is not a total",
                    ccy.as_deref().unwrap_or("an untyped balance")
                )
            })? as i128;
            let s = totals.entry(dim).or_insert((0, 0));
            let scale = ratio_project::RATE_SCALE as i128;
            s.0 += (debit as i128 * factor / scale) as i64;
            s.1 += (credit as i128 * factor / scale) as i64;
            split.entry(dim).or_default().push(pb::CurrencyTotal {
                currency_code: ccy.clone().unwrap_or_default(),
                debit: debit.to_string(),
                credit: credit.to_string(),
                balance: (debit - credit).to_string(),
                // ⛔ EMPTY FOR THE BASE AND FOR AN UNTYPED LEG, not "100".
                // Both translate at par, and both do so WITHOUT a rate fact —
                // printing a rate nobody recorded would invent the evidence the
                // column exists to supply.
                rate: match ccy.as_deref() {
                    Some(c) if c != FUND_CURRENCY => factor.to_string(),
                    _ => String::new(),
                },
            });
        }

        let mut counts: BTreeMap<i64, i64> = BTreeMap::new();
        b.for_each_entry_since(0, &mut |e| {
            for p in &e.postings {
                *counts.entry(p.dim).or_default() += 1;
            }
            Ok(())
        })?;

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
        self.view_the_projection_can_answer(&fund, &view)?;
        // ⛔ TENANCY BEFORE THE DIMENSION PARSE. A caller who may not see this
        // fund is refused here — not after we have judged whether their account
        // id was well-formed. The denial must not depend on the caller's input,
        // or "no fund" and "not a dimension" tell an outsider which is which.
        let path = self.book_path(&fund)?;
        let dim: i64 = dim_str.parse().with_context(|| format!("{dim_str:?} is not a dimension"))?;
        let b = FileBook::open(&path)?;

        let mut running = 0i64;
        let mut out = Vec::new();
        b.for_each_entry_since(0, &mut |entry| {
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

        // ⚠ On the public demo this endpoint is unauthenticated, because the
        // demo is the point. The exposure is deliberately small — the fields
        // are structured, the memo is composed rather than supplied, the event
        // id is validated, and a cold start restores the books — but "small"
        // is not "none", and an unbounded journal in a 512 MB /tmp is the one
        // part that does not heal itself. The deployment sets a ceiling; a
        // local run has none.
        if let Some(max) = self.max_entries {
            if !req.validate_only && existing_len >= max {
                bail!(
                    "this book has {} entries, which is as many as the demo accepts; \
                     it resets on the next cold start",
                    existing_len
                );
            }
        }

        let event = ratio_rules::Event {
            rule: rule.id.clone(),
            id: id.to_string(),
            amount,
            days,
            memo: String::new(), instrument: None, quantity: None };
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
            memo: memo.clone(),
            config: digest.clone(),
            postings: postings.clone(),
        
            trade_date: None,
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
            entry: Some(pb::Entry {
                name: format!("funds/{fund}/entries/{id}"),
                entry_id: id.to_string(),
                memo,
                config_digest: digest.as_str().to_string(),
                postings: postings
                    .iter()
                    .map(|p| pb::EntryPosting {
                        account: format!(
                            "funds/{fund}/views/{default_view}/accounts/{}",
                            p.dim
                        ),
                        display_name: chart
                            .iter()
                            .find(|a| a.dim == p.dim)
                            .map(|a| a.display_name.clone())
                            .unwrap_or_else(|| format!("dimension {}", p.dim)),
                        amount: p.amount.to_string(),
                    })
                    .collect(),
            }),
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
        self.view_the_projection_can_answer(&fund, &view)?;
        Ok(pb::ListPositionsResponse {
            positions: self.positions_of(&fund, &view)?,
            next_page_token: String::new(),
        })
    }

    pub fn get_position(&self, name: &str) -> Result<pb::Position> {
        let (fund, view, id) = view_scoped_id(name, "positions")?;
        self.view_the_projection_can_answer(&fund, &view)?;
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
        self.view_the_projection_can_answer(&fund, &view)?;
        // ⛔ TENANCY BEFORE THE POSITION-KEY PARSE. `projection` opens the book
        // through `book_path`, so a caller who may not see this fund is refused
        // before their position id is parsed — the denial does not depend on
        // whether the id was well-formed.
        let proj = self.projection(&fund)?;
        let (dim, instrument) = position_key(&id)?;
        Ok(pb::ListLotsResponse {
            lots: proj
                .lots_of(dim, &instrument)
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
        let (held, rest) = b.positions()?;
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
        let proj = self.projection(fund).ok();
        let mut out: Vec<pb::Position> = held
            .into_iter()
            .map(|((dim, instrument), (value, quantity))| pb::Position {
                open_lot_count: proj
                    .as_ref()
                    .map(|p| p.lots_of(dim, &instrument).value.len() as i64)
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
                         accepts; it resets on the next cold start"
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
                
                    trade_date: None,
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
    fn view_the_projection_can_answer(&self, fund: &str, view: &str) -> Result<()> {
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let set = match b.active()? {
            Some(d) => ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&d)?))
                .unwrap_or_default(),
            None => ratio_rules::RuleSet::default(),
        };
        let declared = set.effective_views();
        let Some(v) = declared.iter().find(|v| v.id == view) else {
            bail!(
                "no view {view:?} on this fund. It keeps: {}",
                declared.iter().map(|v| v.id.as_str()).collect::<Vec<_>>().join(", ")
            );
        };
        if v.basis != ratio_rules::Basis::Recorded {
            bail!(
                "view {view:?} recognises entries by date, and the maintained projection \
                 behind this screen folds the whole journal with no cut — so it would \
                 return the same figures as every other view rather than that view's. \
                 `ratio strike --view {view}` and `ratio navs --view {view}` do cut and \
                 are correct; these screens are not, and refusing is the honest answer \
                 until the projection folds per view"
            );
        }
        Ok(())
    }

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
        self.view_the_projection_can_answer(fund, &view)?;
        let path = self.book_path(fund)?;
        let b = FileBook::open(&path)?;
        let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
        let proj = self.projection(fund)?;
        Ok(nav_from(&b, &proj, &rates)?.0.to_string())
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
        let state = if entries_len == 0 && pending.is_empty() {
            pb::fund::State::AwaitingPrices
        } else if !pending.is_empty()
            || open.iter().any(|k| k.severity == pb::Severity::High as i32)
        {
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
        self.view_the_projection_can_answer(&id, &view)?;

        let rates = ratio_project::Rates::of_facts(FUND_CURRENCY, &b.records(Plane::Facts)?);
        let proj = self.projection(&id)?;
        // ⭐ THE SAME `nav_from` EVERY FUND-LEVEL PREVIEW CALLS, so a NAV quoted
        // by `ApplyEvent` and a NAV shown on this screen cannot drift apart.
        //
        // ⚠ IT TAKES THE PROJECTION RATHER THAN FETCHING ONE. `projection`
        // hands back a CLONE, and a fund holding a quarter of a million open
        // lots does not want two of them alive to answer one request.
        let (nav, nav_strike) = nav_from(&b, &proj, &rates)?;
        let realized = proj
            .realized(set.chart_roles, &rates)
            .ok()
            .and_then(|r| r.value);

        let breaks = self.breaks_for(&path, &id, &view)?;
        let open: Vec<&pb::Break> = breaks.iter().filter(|k| !k.explained).collect();
        let tb = b.trial_balance()?;

        out.net_asset_value = nav.to_string();
        out.total_debit = tb.debits.to_string();
        out.total_credit = tb.credits.to_string();
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
        out.open_lot_count = proj.open_lots();
        out.position_count = proj.positions().value.held.len() as i64;
        out.journal_position = proj.prefix() as i64;
        out.nav_strike = Some(ratio_proto::duration_proto::google::protobuf::Duration {
            seconds: nav_strike.as_secs() as i64,
            nanos: nav_strike.subsec_nanos() as i32,
        });
        Ok(out)
    }

    /// What two books of record over one journal disagree about.
    ///
    /// ⛔ REFUSES RATHER THAN SUBTRACTING TWO FIGURES. The difference between
    /// two views is a LIST OF ENTRIES — `Ratio.Views.two_views_differ_by_
    /// exactly_what_is_in_flight` — and producing that list needs a fold that
    /// knows which entries each view recognised. The maintained projection does
    /// not have one yet. Differencing two NAVs and calling the result a
    /// reconciliation would show a number with nothing behind it, which is the
    /// one thing this screen exists not to do.
    pub fn reconcile_views(&self, name: &str, against: &str) -> Result<pb::ReconcileViewsResponse> {
        let (id, view) = view_scoped_parent(name).context("bad view name")?;
        let _ = self.book_path(&id)?;
        if against.is_empty() {
            bail!("reconciling is a question about two views — name the other with ?against=");
        }
        bail!(
            "cannot reconcile {view:?} against {against:?} yet: the difference between two \
             books of record is the entries one recognises and the other does not, and the \
             maintained projection folds the whole journal with no cut, so it cannot say \
             which those are. `ratio strike --view` already cuts and is correct; this \
             screen waits on the projection folding per view"
        );
    }

    pub fn list_breaks(&self, parent: &str, filter: &str) -> Result<pb::ListBreaksResponse> {
        let (id, view) = view_scoped_parent(parent).context("bad parent")?;
        self.view_the_projection_can_answer(&id, &view)?;
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
        self.view_the_projection_can_answer(&fund, &view)?;
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
            .lot_breaks()
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
            })
            .collect())
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

        let mut out = Vec::new();
        for line in &report.breaks {
            let diff: i64 = line.difference;
            let severity = if diff.abs() >= BLOCKS_NAV {
                pb::Severity::High
            } else if diff.abs() >= BELOW_NOTICE {
                pb::Severity::Medium
            } else {
                pb::Severity::Low
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
                // Nothing records an explanation yet, so nothing claims one.
                // A break the software decided was fine is exactly the kind of
                // thing this product exists not to do.
                explained: false,
                cause: cause_text(line.cause),
                ratio_amount: line.ratio_amount.to_string(),
                reported_amount: line.reported_amount.to_string(),
                difference: diff.to_string(),
                postings,
                config_digest: report.config_digest.clone(),
            });
        }
        // Largest first: the queue is ordered by money, because that is the
        // order an operator with a deadline works in.
        out.sort_by_key(|k| -k.difference.parse::<i64>().unwrap_or(0).abs());
        // ⛔ And the lot engine's, in the same list.
        out.extend(self.lot_breaks_for(fund, view)?);
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
    found.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    match found.last() {
        None => Ok(None),
        Some(p) => Ok(Some(
            kernel::BreakReport::decode(&std::fs::read(p)?[..])
                .with_context(|| format!("reading {}", p.display()))?,
        )),
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
fn display_name(id: &str) -> String {
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
    let nav = proj.nav(&is_asset_or_liability, rates)?.value.0;
    Ok((nav, struck_at.elapsed()))
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
            Subject::Member { sub: "S".into(), email: "s@example.test".into(), groups: vec![] },
        );

        // Every route that names a fund, instantiated against `b`, is refused —
        // and refused as "no fund", the same answer a nonexistent fund gets, so
        // `b`'s existence does not leak. The list is the LIVE `ROUTES` slice, so
        // a route added later is covered here without anyone remembering to.
        let mut checked = 0;
        let mut view_scoped = 0;
        for route in transcode::ROUTES {
            if !route.template.contains("funds/*") {
                continue; // /v1/funds is the enumeration, tested separately below.
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
            transcode::serve(&console, "GET", "/v1/funds/a/accounts", "", "").is_ok(),
            "the subject's own fund a must be readable"
        );

        // ListFunds returns what the caller may see: `a`, not `b`. An empty list
        // and a refusal are different answers; here it is neither empty nor
        // leaking.
        let funds = transcode::serve(&console, "GET", "/v1/funds", "", "").unwrap();
        assert!(funds.contains("funds/a"), "the subject's fund is missing: {funds}");
        assert!(!funds.contains("funds/b"), "another tenant's fund leaked into the list: {funds}");
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
        assert!(transcode::serve(&console, "GET", "/v1/funds/a/accounts", "", "").is_ok());
        assert!(transcode::serve(&console, "GET", "/v1/funds/b/accounts", "", "").is_ok());
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
            Subject::Member { sub: "S".into(), email: "s@x.test".into(), groups: vec![] };
        let console = Console::open(&root, subject);

        // `b` is granted by nobody and is seen anyway — the whole point. If open
        // mode leaked into `scoped`, the tenancy test above would already be red.
        assert!(transcode::serve(&console, "GET", "/v1/funds/a/accounts", "", "").is_ok());
        assert!(
            transcode::serve(&console, "GET", "/v1/funds/b/accounts", "", "").is_ok(),
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
            Subject::Member { sub: "signer-sub".into(), email: "s@x.test".into(), groups: vec![] },
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
        assert_eq!(after.nav(&assets, &ratio_project::Rates::none()).unwrap().value, cold.nav(&assets, &ratio_project::Rates::none()).unwrap().value);
        assert_eq!(after.positions().value, &cold.positions().value.clone());
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
        assert_eq!(p.nav(&|dim| dim == 1, &ratio_project::Rates::none()).unwrap().value.0, 11, "and the totals are the new book's");
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
        let breaks = c.list_breaks("funds/demo", "").unwrap().breaks;
        let lot: Vec<_> = breaks.iter().filter(|b| b.name.contains("lot-")).collect();
        assert_eq!(lot.len(), 1, "{:?}", breaks.iter().map(|b| &b.cause).collect::<Vec<_>>());
        assert!(lot[0].cause.contains("administration agreement"), "{}", lot[0].cause);

        // ⚠ HIGH, and not because of the amount. Every other break is graded by
        // money at stake; this one is graded by what it means — the figure it
        // corrupts is the realized gain, which no reconciliation reaches.
        assert_eq!(lot[0].severity, pb::Severity::High as i32);

        // And it survives the blocking filter, which is what an operator uses
        // to find the things that stop a NAV.
        let blocking = c.list_breaks("funds/demo", "blocking").unwrap().breaks;
        assert!(blocking.iter().any(|b| b.name.contains("lot-")));
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
        let f = Console::new(&d).get_fund("funds/demo").unwrap().to_json();
        for field in ["entryCount", "openBreakCount", "netAssetValue",
                      "trialBalanceDifference", "openDifference"] {
            assert!(
                f.contains(&format!("\"{field}\":\"")),
                "{field} is not a string in {f}"
            );
        }
        // And enums cross as their names, not their numbers.
        assert!(f.contains("\"state\":\"BLOCKED\"") || f.contains("\"state\":\"STRUCK\"")
                || f.contains("\"state\":\"IN_REVIEW\"") || f.contains("\"state\":\"AWAITING_PRICES\""),
                "state is not a canonical enum name: {f}");
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
        let r = Console::new(&d).list_breaks("funds/demo", "").unwrap();
        assert!(r.breaks.is_empty());
    }

    #[test]
    fn breaks_are_ordered_by_money_and_severity_follows_the_tolerance() {
        let d = fresh("breaks");
        book(&d);
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(),
            config_digest: "abc123".into(),
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

        let ks = Console::new(&d).list_breaks("funds/demo", "").unwrap().breaks;
        assert_eq!(ks.len(), 2);
        assert_eq!(ks[0].difference, "1000000", "largest first");
        assert_eq!(ks[0].severity, pb::Severity::High as i32, "1,000,000 blocks the NAV");
        assert_eq!(ks[1].severity, pb::Severity::Low as i32, "100 is below notice");
        // The postings behind Ratio's figure travel with the break.
        assert_eq!(ks[0].postings.len(), 1);
        assert_eq!(ks[0].postings[0].entry_id, "t1");
        assert!(!ks[0].config_digest.is_empty(), "a break must cite its configuration");
        // Nothing has explained anything, and nothing pretends otherwise.
        assert!(ks.iter().all(|k| !k.explained));

        // A blocking break makes the fund blocked.
        let f = Console::new(&d).get_fund("funds/demo").unwrap();
        assert_eq!(f.state, pb::fund::State::Blocked as i32);
        assert_eq!(f.open_break_count, 2);
        assert_eq!(f.open_difference, "1000100");
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
        let listed = c.list_breaks("funds/demo", "").unwrap().breaks;
        assert_eq!(listed[0].name, "funds/demo/breaks/1",
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
        let first = Console::new(&d).list_breaks("funds/demo", "").unwrap().breaks[0].name.clone();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(d.join("reports/b.pb"), mk("b").encode_to_vec()).unwrap();
        let second = Console::new(&d).list_breaks("funds/demo", "").unwrap().breaks[0].name.clone();
        assert_eq!(first, second);
        assert_eq!(first, "funds/demo/breaks/1");
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
            validate_only,
        };

        // `book()` posts three entries, so a ceiling of three is already met.
        let c = c.with_max_entries(Some(3));
        let refused = c.apply_event(&req("over-1", false));
        assert!(refused.is_err(), "a write past the ceiling must be refused");
        assert!(
            format!("{:#}", refused.unwrap_err()).contains("cold start"),
            "and must say what happens next",
        );

        // A preview writes nothing, so the ceiling has no business refusing it.
        assert!(c.apply_event(&req("over-1", true)).is_ok(), "a preview is not a write");

        // With no ceiling there is no limit — a local run is not the demo.
        let c = c.with_max_entries(None);
        assert!(c.apply_event(&req("over-1", false)).is_ok());
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
        let s = ratio_nav::strike_and_record(&d, 1_780_000_000, "e.marsh").unwrap();
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
        let rows = c.list_accounts("funds/demo", "").unwrap().accounts;

        assert_eq!(rows.len(), 6, "every account in the chart, posted to or not");
        let idle = rows.iter().find(|a| a.dimension == "3").expect("the untouched account is a row");
        assert_eq!(idle.posting_count, "0");
        assert_eq!(idle.balance, "0");
        // …and `posted` is what drops it.
        assert_eq!(c.list_accounts("funds/demo", "posted").unwrap().accounts.len(), 5);

        let debit: i64 = rows.iter().map(|a| a.debit.parse::<i64>().unwrap()).sum();
        let credit: i64 = rows.iter().map(|a| a.credit.parse::<i64>().unwrap()).sum();
        assert_eq!(debit, credit, "the two columns must agree");

        // And the fund reports the same two figures — they must come from one
        // read of the journal, not two.
        let f = c.get_fund("funds/demo").unwrap();
        assert_eq!(f.total_debit, debit.to_string());
        assert_eq!(f.total_credit, credit.to_string());
        assert_eq!(f.trial_balance_difference, "0");
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
        let cash = c.get_account("funds/demo/accounts/2").unwrap();
        assert_eq!(cash.balance, "-500");
        assert!(cash.abnormal, "an asset with a credit balance sits on the abnormal side");

        // Equity is credit-normal, so a credit balance is ordinary there —
        // otherwise the flag would just be reporting the sign.
        let equity = c.get_account("funds/demo/accounts/20").unwrap();
        assert_eq!(equity.balance, "500");
        assert!(equity.abnormal, "equity holding a DEBIT balance is the abnormal one");

        let flagged = c.list_accounts("funds/demo", "abnormal").unwrap().accounts;
        assert_eq!(flagged.len(), 2, "the filter returns exactly what is flagged");
    }

    #[test]
    fn the_running_balance_lands_on_the_account_balance() {
        let d = fresh("running");
        book(&d);
        let c = Console::new(&d);

        for a in c.list_accounts("funds/demo", "posted").unwrap().accounts {
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
        }
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
        let report = kernel::BreakReport {
            name: "books/demo/breakReports/r".into(), config_digest: "c".into(), scope: None,
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
        assert_eq!(c.list_breaks("funds/demo", "").unwrap().breaks.len(), 2);
        assert_eq!(c.list_breaks("funds/demo", "blocking").unwrap().breaks.len(), 1);
        assert_eq!(c.list_breaks("funds/demo", "unexplained").unwrap().breaks.len(), 2);
    }
}
