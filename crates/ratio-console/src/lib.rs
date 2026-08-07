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

pub mod transcode;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use prost::Message;
use ratio_proto::ratio::v1 as pb;
use ratio_rules::RuleSet;
use ratio_store::{AccountTypeRecord, ConfigStore, FileBook, Journal};

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
}

impl Console {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Console { root: root.as_ref().to_path_buf() }
    }

    /// The funds, in a stable order.
    ///
    /// Sorted by id rather than by anything derived, so the list does not
    /// reorder under an operator between two glances at the same screen.
    fn fund_ids(&self) -> Result<Vec<String>> {
        if self.root.join("accounts.json").is_file() {
            return Ok(vec!["demo".to_string()]);
        }
        let mut ids: Vec<String> = std::fs::read_dir(&self.root)
            .with_context(|| format!("reading {}", self.root.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("accounts.json").is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn book_path(&self, id: &str) -> Result<PathBuf> {
        // The id reaches the filesystem, and this service is behind a public
        // endpoint. Anything that is not a fund id is refused before it is
        // joined to a path.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!("{id:?} is not a fund id");
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

    pub fn list_funds(&self) -> Result<pb::ListFundsResponse> {
        let mut funds = Vec::new();
        for id in self.fund_ids()? {
            funds.push(self.get_fund(&format!("funds/{id}"))?);
        }
        Ok(pb::ListFundsResponse { funds, next_page_token: String::new() })
    }

    pub fn get_fund(&self, name: &str) -> Result<pb::Fund> {
        let id = resource_id(name, "funds").context("bad fund name")?;
        let path = self.book_path(&id)?;
        let b = FileBook::open(&path)?;
        let accounts = b.accounts()?;
        let by_dim: BTreeMap<i64, AccountTypeRecord> =
            accounts.iter().map(|a| (a.dim, a.account_type)).collect();
        let balances = b.balances_by_dim()?;
        let tb = b.trial_balance()?;

        // NAV is assets minus liabilities, and both are the same fold: a
        // liability's net is negative because it is credit-normal, so summing
        // the two families subtracts one from the other without a special case.
        let nav: i64 = balances
            .iter()
            .filter(|(d, _)| {
                matches!(
                    by_dim.get(d),
                    Some(AccountTypeRecord::Asset) | Some(AccountTypeRecord::Liability)
                )
            })
            .map(|(_, (dr, cr))| dr - cr)
            .sum();

        let breaks = self.breaks_for(&path, &id)?;
        let open: Vec<&pb::Break> = breaks.iter().filter(|k| !k.explained).collect();
        let open_difference: i64 = open.iter().filter_map(|k| k.difference.parse::<i64>().ok().map(i64::abs)).sum();

        let entries = b.entries()? as Vec<_>;
        let state = if entries.is_empty() {
            pb::fund::State::AwaitingPrices
        } else if open.iter().any(|k| k.severity == pb::Severity::High as i32) {
            pb::fund::State::Blocked
        } else if !breaks.is_empty() {
            pb::fund::State::InReview
        } else {
            pb::fund::State::Struck
        };

        Ok(pb::Fund {
            name: format!("funds/{id}"),
            display_name: display_name(&id),
            currency_code: "USD".into(),
            state: state as i32,
            net_asset_value: nav.to_string(),
            trial_balance_difference: (tb.debits - tb.credits).to_string(),
            open_difference: open_difference.to_string(),
            entry_count: entries.len() as i64,
            open_break_count: open.len() as i64,
            config_digest: b.active()?.map(|d| d.as_str().to_string()).unwrap_or_default(),
        })
    }

    pub fn list_breaks(&self, parent: &str, filter: &str) -> Result<pb::ListBreaksResponse> {
        let id = resource_id(parent, "funds").context("bad parent")?;
        let path = self.book_path(&id)?;
        let mut breaks = self.breaks_for(&path, &id)?;
        breaks.retain(|k| match filter {
            "blocking" => k.severity == pb::Severity::High as i32,
            "unexplained" => !k.explained,
            _ => true,
        });
        Ok(pb::ListBreaksResponse { breaks, next_page_token: String::new() })
    }

    pub fn get_break(&self, name: &str) -> Result<pb::Break> {
        let (fund, brk) = nested_id(name, "funds", "breaks").context("bad break name")?;
        let path = self.book_path(&fund)?;
        self.breaks_for(&path, &fund)?
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
            if f.len() >= 5 {
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
        let id = resource_id(parent, "funds").context("bad parent")?;
        let path = self.book_path(&id)?;
        Ok(pb::ListNavStrikesResponse {
            nav_strikes: ratio_nav::list(&path)?
                .into_iter()
                .map(|s| to_pb(&id, &s))
                .collect(),
            next_page_token: String::new(),
        })
    }

    pub fn get_nav_strike(&self, name: &str) -> Result<pb::NavStrike> {
        let (fund, id) = nested_id(name, "funds", "navStrikes").context("bad name")?;
        let path = self.book_path(&fund)?;
        Ok(to_pb(&fund, &ratio_nav::get(&path, &id)?))
    }

    /// Re-derive a strike. Read-only: it folds a journal prefix and compares.
    pub fn replay_nav_strike(&self, name: &str) -> Result<pb::ReplayNavStrikeResponse> {
        let (fund, id) = nested_id(name, "funds", "navStrikes").context("bad name")?;
        let path = self.book_path(&fund)?;
        let s = ratio_nav::get(&path, &id)?;
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
    fn breaks_for(&self, book: &Path, fund: &str) -> Result<Vec<pb::Break>> {
        let Some(report) = newest_report(book)? else {
            return Ok(Vec::new());
        };
        let b = FileBook::open(book)?;
        let dims: BTreeMap<String, i64> =
            b.accounts()?.into_iter().map(|a| (a.display_name, a.dim)).collect();
        let entries = b.entries()?;

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

            let postings: Vec<pb::BreakPosting> = entries
                .iter()
                .flat_map(|e| e.postings.iter().map(move |p| (e, p)))
                .filter(|(_, p)| p.dim == dim)
                .map(|(e, p)| pb::BreakPosting {
                    entry_id: e.id.clone(),
                    memo: e.memo.clone(),
                    amount: p.amount.to_string(),
                    config_digest: e.config.short().to_string(),
                })
                .collect();

            out.push(pb::Break {
                // Derived from the dimension, so a break keeps the same URL
                // across two runs of the same period. A name that moved every
                // time the report was regenerated would make every link in an
                // email dead by morning.
                name: format!("funds/{fund}/breaks/{dim}"),
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

fn to_pb(fund: &str, s: &ratio_nav::Strike) -> pb::NavStrike {
    pb::NavStrike {
        name: format!("funds/{fund}/navStrikes/{}", s.id),
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
    }
}

fn newest_report(book: &Path) -> Result<Option<pb::BreakReport>> {
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
            pb::BreakReport::decode(&std::fs::read(p)?[..])
                .with_context(|| format!("reading {}", p.display()))?,
        )),
    }
}

fn cause_text(cause: i32) -> String {
    match pb::Cause::try_from(cause) {
        Ok(pb::Cause::AmountDiffers) => "Figures differ",
        Ok(pb::Cause::AbsentFromReport) => "Not in the report",
        Ok(pb::Cause::AbsentFromRatio) => "Ratio produced nothing",
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

/// `funds/abc` → `abc`, checking the collection is the one expected.
pub fn resource_id(name: &str, collection: &str) -> Result<String> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 2 || parts[0] != collection {
        bail!("{name:?} is not a {collection}/* name");
    }
    Ok(parts[1].to_string())
}

/// `funds/a/breaks/b` → `("a", "b")`.
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
                postings: legs.into_iter().map(|(dim, amount)| PostingRecord { dim, amount }).collect(),
            })
            .unwrap();
        };
        post("c1", "capital in", vec![(2, 30_000_000), (20, -30_000_000)]);
        post("t1", "buy", vec![(1, 25_000_000), (2, -25_000_000)]);
        post("f1", "fee accrued", vec![(10, 100_000), (40, -100_000)]);
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
        let f = Console::new(&d).get_fund("funds/demo").unwrap();
        assert_eq!(f.net_asset_value, "29900000");
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
        let report = pb::BreakReport {
            name: "books/demo/breakReports/r".into(),
            config_digest: "abc123".into(),
            scope: None,
            transactions_replayed: 2,
            entries_posted: 2,
            breaks: vec![
                pb::BreakLine { account: 40, display_name: "Management fee payable".into(),
                    ratio_amount: 100, reported_amount: 0, difference: 100,
                    cause: pb::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
                pb::BreakLine { account: 1, display_name: "Investments at fair value".into(),
                    ratio_amount: 25_000_000, reported_amount: 24_000_000, difference: 1_000_000,
                    cause: pb::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
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
        let report = pb::BreakReport {
            name: "books/SOMETHING-ELSE/breakReports/r".into(),
            config_digest: "c".into(), scope: None,
            transactions_replayed: 1, entries_posted: 1,
            breaks: vec![pb::BreakLine { account: 1,
                display_name: "Investments at fair value".into(),
                ratio_amount: 5, reported_amount: 4, difference: 1,
                cause: pb::Cause::AmountDiffers as i32, ratio_basis: "1".into() }],
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
        let line = pb::BreakLine { account: 1, display_name: "Investments at fair value".into(),
            ratio_amount: 5, reported_amount: 4, difference: 1,
            cause: pb::Cause::AmountDiffers as i32, ratio_basis: "1".into() };
        let mk = |n: &str| pb::BreakReport {
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
        let report = pb::BreakReport {
            name: "books/demo/breakReports/r".into(), config_digest: "c".into(), scope: None,
            transactions_replayed: 1, entries_posted: 1,
            breaks: vec![
                pb::BreakLine { account: 1, display_name: "Investments at fair value".into(),
                    ratio_amount: 200_000, reported_amount: 0, difference: 200_000,
                    cause: pb::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
                pb::BreakLine { account: 40, display_name: "Management fee payable".into(),
                    ratio_amount: 10, reported_amount: 0, difference: 10,
                    cause: pb::Cause::AmountDiffers as i32, ratio_basis: "1".into() },
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
