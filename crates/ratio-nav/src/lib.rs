//! ratio-nav — striking a NAV, and proving it again later.
//!
//! # What a strike is
//!
//! A NAV is a number somebody signs. This records the number **and everything
//! needed to derive it again**: the journal position it folds, a digest of
//! exactly those entries, and the configuration in force at the last one.
//!
//! # Why replay checks two things
//!
//! Re-deriving a strike can fail in two unrelated ways, and collapsing them
//! into one boolean would lose the distinction that matters:
//!
//! * **History was rewritten.** The prefix no longer hashes to what the strike
//!   recorded. An append-only journal should make this impossible; a system
//!   whose central claim is reproducibility should still check rather than
//!   assume, because the interesting failure is the one you ruled out.
//! * **The engine is not deterministic.** The prefix is identical and the fold
//!   lands somewhere else. That is a defect in Ratio and nowhere else.
//!
//! The first is an accusation about the data. The second is an accusation about
//! the software. `Replay` reports them separately.
//!
//! # What it does not do
//!
//! Nothing here strikes automatically. A NAV is signed by a person, and a
//! system that struck its own NAV on a timer would be asserting exactly the
//! thing this product exists to stop asserting.

/// The cost model, emitted from `Ratio.Closure` — what a period end reads,
/// term by term. Kept out of `lib.rs` because it is authored in Lean.
mod generated;

/// What a period end costs before anybody runs it: the emitted model, plus a
/// rate measured against a real store so the answer can be given in time as
/// well as in reads.
pub mod closure;

use anyhow::{bail, Context, Result};
use ratio_store::{AccountTypeRecord, ConfigStore, Digest, FileBook, Journal, JournalEntry};

/// A NAV, pinned to the journal that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strike {
    /// A stable id derived from the valuation time, e.g. `2026-06-30T1600Z`.
    pub id: String,
    /// Unix seconds. The valuation point, not when the command ran.
    pub valuation_time: i64,
    pub actor: String,
    /// How many journal entries this folds.
    pub journal_position: usize,
    /// SHA-256 of exactly those entries, as stored.
    pub journal_digest: String,
    pub net_asset_value: i64,
    pub trial_balance_difference: i64,
    /// The configuration in force at the last entry folded.
    pub config_digest: String,
}

/// What re-deriving a strike found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    pub id: String,
    /// The prefix still hashes the same: history was not rewritten.
    pub history_intact: bool,
    /// The fold landed on the same figures: the engine is deterministic.
    pub reproduced: bool,
    /// What re-derivation actually produced, so a mismatch can be read.
    pub net_asset_value: i64,
    pub journal_digest: String,
}

impl Replay {
    /// Both checks passed.
    pub fn ok(&self) -> bool {
        self.history_intact && self.reproduced
    }
}

/// Digest the first `n` journal entries, exactly as they are stored.
///
/// Hashes the SERIALIZED lines rather than the parsed values, so a change that
/// round-trips through the parser but alters the bytes — a reordered field, a
/// different memo encoding — is still visible. What is being attested is the
/// record, not an interpretation of it.
fn prefix_digest(entries: &[JournalEntry]) -> Result<String> {
    let mut buf = Vec::new();
    for e in entries {
        buf.extend_from_slice(serde_json::to_string(e).context("serializing an entry")?.as_bytes());
        buf.push(b'\n');
    }
    Ok(Digest::of(&buf).as_str().to_string())
}

/// Net asset value over a set of entries: assets minus liabilities.
///
/// One fold over both families. A liability's net is negative because it is
/// credit-normal, so summing them subtracts without a special case — and a sign
/// error is invisible in a screenshot and wrong by twice the liability.
fn fold_nav(book: &FileBook, entries: &[JournalEntry]) -> Result<(i64, i64)> {
    let types: std::collections::BTreeMap<i64, AccountTypeRecord> =
        book.accounts()?.into_iter().map(|a| (a.dim, a.account_type)).collect();

    // ⛔ ACCUMULATED IN `i128`, REPORTED IN `i64`. Summing a journal in `i64`
    // wraps — `debits` in particular adds the magnitude of EVERY posting ever
    // made, so it grows with history rather than with the fund, and it is the
    // first of these to go. A wrapped total does not look wrong; it looks like a
    // NAV.
    //
    // ⚠ And `-p.amount` is not always the magnitude: `-i64::MIN` overflows. In
    // `i128` it does not, which is the second reason this is not just about
    // headroom.
    let mut nav = 0i128;
    let mut debits = 0i128;
    let mut credits = 0i128;
    for e in entries {
        for p in &e.postings {
            let amount = p.amount as i128;
            if amount >= 0 {
                debits += amount;
            } else {
                credits += -amount;
            }
            if matches!(
                types.get(&p.dim),
                Some(AccountTypeRecord::Asset) | Some(AccountTypeRecord::Liability)
            ) {
                nav += amount;
            }
        }
    }
    // ⛔ The FIGURES must fit to be reported. A NAV that cannot be represented is
    // refused rather than truncated — `Ratio.Bounded`: an operation either agrees
    // with the theorem or declines, and there is no third answer.
    let nav = i64::try_from(nav)
        .map_err(|_| anyhow::anyhow!("this book's net asset value does not fit in 64 bits"))?;
    let diff = i64::try_from(debits - credits)
        .map_err(|_| anyhow::anyhow!("this book's trial-balance difference does not fit in 64 bits"))?;
    Ok((nav, diff))
}

/// Strike a NAV over the whole journal as it stands.
pub fn strike(book_path: &std::path::Path, valuation_time: i64, actor: &str) -> Result<Strike> {
    if actor.trim().is_empty() {
        bail!("a NAV is signed by somebody — pass --actor or set RATIO_ACTOR");
    }
    let book = FileBook::open(book_path)?;
    let entries = book.entries()?;
    if entries.is_empty() {
        bail!("nothing to strike: this book has no entries");
    }
    let (nav, tb) = fold_nav(&book, &entries)?;

    Ok(Strike {
        id: id_for(valuation_time),
        valuation_time,
        actor: actor.to_string(),
        journal_position: entries.len(),
        journal_digest: prefix_digest(&entries)?,
        net_asset_value: nav,
        trial_balance_difference: tb,
        // The configuration the LAST entry was posted under, which is what was
        // in force at the valuation point. Not `ACTIVE`: that is what is in
        // force NOW, and a strike re-read after a later approval would silently
        // claim to have been computed under a configuration that did not exist
        // when it was taken.
        config_digest: entries
            .last()
            .map(|e| e.config.as_str().to_string())
            .unwrap_or_default(),
    })
}

/// Re-derive a strike and report what was found.
pub fn replay(book_path: &std::path::Path, s: &Strike) -> Result<Replay> {
    let book = FileBook::open(book_path)?;
    let all = book.entries()?;

    // A journal that has since grown is normal and expected — a strike folds a
    // PREFIX. One that has SHRUNK cannot be reconciled with the strike at all,
    // and is reported as broken history rather than allowed to panic on a slice.
    if all.len() < s.journal_position {
        return Ok(Replay {
            id: s.id.clone(),
            history_intact: false,
            reproduced: false,
            net_asset_value: 0,
            journal_digest: String::new(),
        });
    }

    let prefix = &all[..s.journal_position];
    let digest = prefix_digest(prefix)?;
    let (nav, tb) = fold_nav(&book, prefix)?;

    Ok(Replay {
        id: s.id.clone(),
        history_intact: digest == s.journal_digest,
        reproduced: nav == s.net_asset_value && tb == s.trial_balance_difference,
        net_asset_value: nav,
        journal_digest: digest,
    })
}

/// `2026-06-30T16:00:00Z` → `2026-06-30T1600Z`.
///
/// Stable, sortable, and safe as a filename and a URL segment — a strike is
/// addressable, and an id needing escaping is an id that eventually is not.
pub fn id_for(unix_seconds: i64) -> String {
    let (y, mo, d, h, mi) = civil_from_unix(unix_seconds);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}{mi:02}Z")
}

/// RFC 3339, for the wire.
pub fn rfc3339(unix_seconds: i64) -> String {
    let (y, mo, d, h, mi) = civil_from_unix(unix_seconds);
    let s = unix_seconds.rem_euclid(60);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Unix seconds → civil date, by Howard Hinnant's `civil_from_days`.
///
/// Hand-rolled rather than pulling in a date library: two formatters and no
/// parsing is not worth a dependency, and this algorithm is exact for every
/// date in the proleptic Gregorian calendar rather than approximately right
/// near leap years — which is the kind of "approximately" that shows up as a
/// NAV filed under the wrong day, once, in March.
fn civil_from_unix(t: i64) -> (i64, u32, u32, u32, u32) {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };

    (y, m, d, (secs / 3600) as u32, ((secs % 3600) / 60) as u32)
}

// ── persistence ────────────────────────────────────────────────────────────
//
// Strikes live in the book, beside the journal they attest to. Tab-separated
// rather than a proto: this file is the one an auditor is most likely to be
// handed on its own, and a format they can read without a toolchain is worth
// more here than one that is a byte smaller.

fn line(s: &Strike) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        s.id, s.valuation_time, s.actor, s.journal_position, s.journal_digest,
        s.net_asset_value, s.trial_balance_difference, s.config_digest
    )
}

fn parse(l: &str) -> Option<Strike> {
    let f: Vec<&str> = l.split('\t').collect();
    if f.len() < 8 {
        return None; // a truncated line is skipped, never guessed at
    }
    Some(Strike {
        id: f[0].into(),
        valuation_time: f[1].parse().ok()?,
        actor: f[2].into(),
        journal_position: f[3].parse().ok()?,
        journal_digest: f[4].into(),
        net_asset_value: f[5].parse().ok()?,
        trial_balance_difference: f[6].parse().ok()?,
        config_digest: f[7].into(),
    })
}

/// Every strike on a book, newest first.
pub fn list(book_path: &std::path::Path) -> Result<Vec<Strike>> {
    let text = std::fs::read_to_string(book_path.join("NAVS")).unwrap_or_default();
    let mut out: Vec<Strike> = text.lines().filter_map(parse).collect();
    out.reverse();
    Ok(out)
}

/// One strike by id.
pub fn get(book_path: &std::path::Path, id: &str) -> Result<Strike> {
    list(book_path)?
        .into_iter()
        .find(|s| s.id == id)
        .with_context(|| format!("no NAV strike {id:?}"))
}

/// Strike and record it.
///
/// Refuses to overwrite an existing strike at the same valuation point. A NAV
/// struck twice for one moment is two answers to a question that has one, and
/// silently replacing the first is how the earlier number stops existing.
pub fn strike_and_record(
    book_path: &std::path::Path,
    valuation_time: i64,
    actor: &str,
) -> Result<Strike> {
    let s = strike(book_path, valuation_time, actor)?;
    if list(book_path)?.iter().any(|e| e.id == s.id) {
        bail!(
            "a NAV is already struck for {} — a valuation point has one answer, \
             and replacing it would remove the first",
            s.id
        );
    }
    let path = book_path.join("NAVS");
    let mut prior = std::fs::read_to_string(&path).unwrap_or_default();
    prior.push_str(&line(&s));
    std::fs::write(&path, prior).context("recording the strike")?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::{Account, AccountTypeRecord as A, PostingRecord};

    fn book(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ratio-nav-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
            Account { dim: 20, display_name: "Capital".into(), account_type: A::Equity },
            Account { dim: 40, display_name: "Payable".into(), account_type: A::Liability },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        for (id, legs) in [
            ("c1", vec![(2, 30_000_000i64), (20, -30_000_000)]),
            ("t1", vec![(1, 25_000_000), (2, -25_000_000)]),
        ] {
            b.append(&JournalEntry {
                id: id.into(),
                memo: id.into(),
                config: c.clone(),
                postings: legs.into_iter().map(|(dim, amount)| PostingRecord::new(dim, amount)).collect(),
            
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    const NOON: i64 = 1_782_662_400; // 2026-06-28T16:00:00Z

    #[test]
    fn a_strike_pins_the_journal_it_folded() {
        let d = book("pins");
        let s = strike(&d, NOON, "e.marsh").unwrap();
        assert_eq!(s.journal_position, 2);
        assert_eq!(s.net_asset_value, 30_000_000, "assets only; equity is outside the fold");
        assert_eq!(s.trial_balance_difference, 0);
        assert_eq!(s.journal_digest.len(), 64);
        assert_eq!(s.actor, "e.marsh");
    }

    #[test]
    fn a_strike_replays_identically() {
        let d = book("replay");
        let s = strike(&d, NOON, "e.marsh").unwrap();
        let r = replay(&d, &s).unwrap();
        assert!(r.history_intact, "history should be intact");
        assert!(r.reproduced, "the fold should reproduce");
        assert!(r.ok());
        assert_eq!(r.net_asset_value, s.net_asset_value);
    }

    #[test]
    fn a_strike_still_replays_after_the_journal_grows() {
        // The whole point of pinning a POSITION rather than a state: a NAV
        // struck at 16:00 must keep replaying after 17:00's trades land.
        let d = book("grows");
        let s = strike(&d, NOON, "e.marsh").unwrap();

        let mut b = FileBook::open(&d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: "t2".into(),
            memo: "later".into(),
            config: c,
            postings: vec![
                PostingRecord::new(1, 5_000_000),
                PostingRecord::new(2, -5_000_000),
            ],
        
            announcement: None,
        })
        .unwrap();

        let r = replay(&d, &s).unwrap();
        assert!(r.ok(), "a later entry must not disturb an earlier strike");
        assert_eq!(r.net_asset_value, s.net_asset_value);

        // And a strike taken now folds more and is a different number.
        let s2 = strike(&d, NOON + 3600, "e.marsh").unwrap();
        assert_eq!(s2.journal_position, 3);
        assert_ne!(s2.journal_digest, s.journal_digest);
    }

    #[test]
    fn rewritten_history_is_caught_and_named_as_such() {
        // The accusation about the DATA. An append-only journal should make
        // this impossible, which is exactly why it is worth checking: the
        // interesting failure is the one that was ruled out.
        let d = book("tamper");
        let s = strike(&d, NOON, "e.marsh").unwrap();

        let jp = d.join("journal.jsonl");
        let text = std::fs::read_to_string(&jp).unwrap();
        // Change an amount inside the prefix, keeping the entry balanced so the
        // journal would still accept it — the tamper a naive check would miss.
        let doctored = text.replacen("30000000", "31000000", 2);
        assert_ne!(doctored, text, "the fixture did not actually change");
        std::fs::write(&jp, doctored).unwrap();

        let r = replay(&d, &s).unwrap();
        assert!(!r.history_intact, "a rewritten prefix must not read as intact");
        assert!(!r.reproduced, "and the figures must not agree either");
        assert!(!r.ok());
    }

    #[test]
    fn a_shortened_journal_is_broken_history_not_a_panic() {
        // Slicing [..position] on a shorter journal would panic. A truncated
        // journal is exactly when a system must report rather than crash.
        let d = book("short");
        let s = strike(&d, NOON, "e.marsh").unwrap();
        std::fs::write(d.join("journal.jsonl"), "").unwrap();
        let r = replay(&d, &s).unwrap();
        assert!(!r.history_intact);
        assert!(!r.ok());
    }

    #[test]
    fn nothing_strikes_itself() {
        let d = book("actor");
        assert!(strike(&d, NOON, "  ").is_err(), "a NAV is signed by somebody");
    }

    #[test]
    fn an_empty_book_cannot_be_struck() {
        let d = std::env::temp_dir().join("ratio-nav-empty");
        let _ = std::fs::remove_dir_all(&d);
        FileBook::open(&d).unwrap().put_accounts(&[]).unwrap();
        assert!(strike(&d, NOON, "e.marsh").is_err());
    }

    #[test]
    fn a_recorded_strike_round_trips() {
        let d = book("record");
        let s = strike_and_record(&d, NOON, "e.marsh").unwrap();
        let back = get(&d, &s.id).unwrap();
        assert_eq!(back, s, "what was written is not what comes back");
        assert!(replay(&d, &back).unwrap().ok());
    }

    #[test]
    fn a_valuation_point_cannot_be_struck_twice() {
        // Two answers to a question that has one. Overwriting silently is how
        // the first number stops existing.
        let d = book("twice");
        strike_and_record(&d, NOON, "e.marsh").unwrap();
        let e = strike_and_record(&d, NOON, "e.marsh").unwrap_err().to_string();
        assert!(e.contains("already struck"), "{e}");
        assert_eq!(list(&d).unwrap().len(), 1);
    }

    #[test]
    fn strikes_come_back_newest_first() {
        let d = book("order");
        strike_and_record(&d, NOON, "a").unwrap();
        strike_and_record(&d, NOON + 3600, "b").unwrap();
        let all = list(&d).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].valuation_time > all[1].valuation_time);
    }

    #[test]
    fn dates_are_exact_not_approximate() {
        // Hand-rolled calendar arithmetic earns its place only if it is right
        // at the edges — a leap day, a year boundary, and the epoch.
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_709_164_800), "2024-02-29T00:00:00Z", "leap day");
        assert_eq!(rfc3339(1_735_689_599), "2024-12-31T23:59:59Z");
        assert_eq!(rfc3339(1_735_689_600), "2025-01-01T00:00:00Z");
        assert_eq!(id_for(1_782_662_400), "2026-06-28T1600Z");
        // Before the epoch, where a naive `/` instead of div_euclid goes wrong.
        assert_eq!(rfc3339(-1), "1969-12-31T23:59:59Z");
    }
}
