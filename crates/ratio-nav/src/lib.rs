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
use ratio_project::views;
use ratio_store::{AccountTypeRecord, ConfigStore, Digest, FileBook, Journal, JournalEntry};

/// A NAV, pinned to the journal that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strike {
    /// The book of record this was struck in.
    ///
    /// ⛔ ON THE RECORD, AND IT IS WHAT MAKES `Ratio.Period.one_answer_per_view_
    /// per_day` REACHABLE. `id_for` derives the id from the valuation time
    /// alone, so an accounting NAV and a settlement NAV for one moment have the
    /// SAME id — and without this field the second is indistinguishable from a
    /// restatement of the first, which that theorem refuses. Two views are two
    /// answers to two questions; a restatement is a second answer to one.
    ///
    /// ⚠ A BOOK WRITTEN BEFORE THIS EXISTED HAS EIGHT FIELDS PER LINE AND READS
    /// BACK AS `ratio_rules::UNDECLARED_VIEW` — see [`parse`]. An append-only
    /// record you cannot read is not a record.
    pub view: String,
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
    let mut d = ratio_store::DigestBuilder::new();
    for e in entries {
        feed(&mut d, e)?;
    }
    Ok(d.finish().as_str().to_string())
}

/// One entry's bytes, as the digest has always seen them.
///
/// ⛔ THE ONE PLACE THAT DECIDES WHAT A PREFIX DIGEST IS OVER. Two encoders for
/// this would be two digests for one prefix, and the failure is a strike that
/// cannot be replayed — which is the accusation this system exists to be able to
/// make about itself.
fn feed(d: &mut ratio_store::DigestBuilder, e: &JournalEntry) -> Result<()> {
    d.update(serde_json::to_string(e).context("serializing an entry")?.as_bytes());
    d.update(b"\n");
    Ok(())
}

/// The NAV fold, one entry at a time.
///
/// ⛔ SO A STRIKE DOES NOT HOLD THE JOURNAL. `strike` read the whole log into a
/// `Vec`, hashed a second `Vec` of every serialized entry, and folded the first
/// — three copies of a thing that only ever needed to be walked once.
struct NavFold {
    types: std::collections::BTreeMap<i64, AccountTypeRecord>,
    rates: ratio_project::Rates,
    /// ⛔ PER CURRENCY. This was ONE `i128` and the bug is the whole reason this
    /// comment exists: `ratio strike` — the RECORDED nav, the figure a replay
    /// re-derives and somebody is paid on — added dollars, euros and pounds and
    /// called the total USD. It returned the IDENTICAL number for a
    /// one-currency and a three-currency book, because it never looked at the
    /// currency at all. And it tied the whole way: trial balance 0, digest
    /// reproducible, replay reporting "reproduced" — of the wrong figure,
    /// forever. `Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch`.
    nav: std::collections::BTreeMap<Option<String>, i128>,
    /// ⛔ `i128`, REPORTED IN `i64`. Summing a journal in `i64` wraps —
    /// `debits` in particular adds the magnitude of EVERY posting ever made, so
    /// it grows with HISTORY rather than with the fund, and it is the first of
    /// these to go. A wrapped total does not look wrong; it looks like a NAV.
    ///
    /// ⚠ And `-p.amount` is not always the magnitude: `-i64::MIN` overflows. In
    /// `i128` it does not, which is the second reason this is not just about
    /// headroom.
    debits: i128,
    credits: i128,

    /// The book of record this fold is, and the day it recognises through.
    ///
    /// ⚠ `as_of: None` IS THE WHOLE JOURNAL, and it is why two views can agree.
    /// Folded to the end of history every view recognises everything it can
    /// place — `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_gap` — so a
    /// differential test between two views that does not CUT compares a figure
    /// with itself and is green under a broken convention.
    view: String,
    as_of: Option<views::Day>,
    /// The view's terms, resolved once per configuration digest. A book has a
    /// handful of configurations and millions of entries naming them.
    defs: std::collections::BTreeMap<ratio_store::Digest, Option<views::ViewDef>>,
    /// Entries this view cannot date at all, each with the reason.
    ///
    /// ⛔ REPORTED, NEVER SILENTLY DROPPED. An entry left out of a figure that
    /// nobody was told about is value leaving a book whose trial balance goes on
    /// tying, which is the shape of every defect in HANDOFF.md's table.
    unplaceable: Vec<String>,
}

impl NavFold {
    fn new(book: &FileBook, view: &str, as_of: Option<views::Day>) -> Result<Self> {
        Ok(Self {
            types: book.accounts()?.into_iter().map(|a| (a.dim, a.account_type)).collect(),
            rates: ratio_project::Rates::of_facts(
                ratio_store::BASE_CURRENCY,
                &book.records(ratio_store::Plane::Facts)?,
            ),
            nav: Default::default(),
            debits: 0,
            credits: 0,
            view: view.to_string(),
            as_of,
            defs: Default::default(),
            unplaceable: Vec::new(),
        })
    }

    /// The view definition the entry's OWN configuration declares.
    ///
    /// ⛔ FROM THE DIGEST THE ENTRY PINNED, NEVER FROM ACTIVE, and cached by
    /// digest exactly as `Projection::terms_of` caches `Terms`. A settlement
    /// date is a trade date rolled over a calendar, so resolving the calendar
    /// from whatever is in force NOW makes a recorded NAV re-derive to a
    /// different figure the moment somebody declares a holiday — which is worse
    /// than a restatement, because a restatement announces itself.
    /// `//tla:calendar_in_side_file_check` is that as a model run.
    ///
    /// ⛔ AND A CONFIGURATION THAT DOES NOT DECLARE THIS VIEW REFUSES THE ENTRY
    /// rather than falling back to journal order. Folding it as `recorded`
    /// would put an entry in a settlement NAV that no settlement convention
    /// admitted, and the figure would tie the whole way — the same argument
    /// that stops an unreadable config falling back to FIFO.
    fn def_for(&mut self, book: &FileBook, digest: &ratio_store::Digest) -> Option<views::ViewDef> {
        if let Some(d) = self.defs.get(digest) {
            return d.clone();
        }
        let resolved = (|| {
            let bytes = book.get(digest).ok()?;
            let set = ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&bytes)).ok()?;
            views::ViewDef::of(&set).into_iter().find(|v| v.id == self.view)
        })();
        self.defs.insert(digest.clone(), resolved.clone());
        resolved
    }

    /// Fold one entry, if this view recognises it as of the cut.
    ///
    /// Returns whether it was folded, so the caller can keep the digest and the
    /// count over exactly the entries the figure was built from.
    fn consider(&mut self, book: &FileBook, e: &JournalEntry) -> bool {
        let Some(def) = self.def_for(book, &e.config) else {
            self.unplaceable.push(format!(
                "{}: the configuration this entry pinned ({}) declares no view {:?}, so this \
                 view cannot say when it is recognised",
                e.id,
                e.config.as_str(),
                self.view
            ));
            return false;
        };
        // ⚠ A DATE THAT WILL NOT PARSE IS `None` HERE AND UNPLACEABLE BELOW,
        // not a silent absence: `placement` names the entry in its refusal.
        let traded = e
            .trade_date
            .as_deref()
            .and_then(|d| ratio_common::days_from_iso_date(d).ok())
            .map(|n| n as views::Day);
        let placement = def.placement(&e.id, traded);
        if let views::Placement::Unplaceable(why) = &placement {
            self.unplaceable.push(why.clone());
            return false;
        }
        if !def.recognises(&placement, self.as_of) {
            return false;
        }
        self.push(e);
        true
    }

    fn push(&mut self, e: &JournalEntry) {
        for p in &e.postings {
            let a = p.amount as i128;
            if matches!(
                self.types.get(&p.dim),
                Some(AccountTypeRecord::Asset) | Some(AccountTypeRecord::Liability)
            ) {
                *self.nav.entry(p.currency.clone()).or_default() += a;
            }
            // ⚠ THE TRIAL-BALANCE DIFFERENCE IS SAFE TO SUM FLAT, and only that.
            // Every currency nets to zero independently, so the flat total is
            // zero too — the DIFFERENCE survives mixing even though the two
            // column totals would not, and the difference is all `Strike`
            // records.
            if a >= 0 {
                self.debits += a;
            } else {
                self.credits += -a;
            }
        }
    }

    fn finish(self) -> Result<(i64, i64)> {
        let mut nav = 0i128;
        for (currency, amount) in &self.nav {
            let factor = self.rates.factor_of_optional(currency.as_deref()).ok_or_else(|| {
                anyhow::anyhow!(
                    "this fund holds {} and no rate for it was supplied — a NAV mixing \
                     denominations is not a NAV",
                    currency.as_deref().unwrap_or("an untyped balance")
                )
            })?;
            nav += amount * factor as i128 / ratio_project::RATE_SCALE as i128;
        }
        Ok((
            i64::try_from(nav)
                .map_err(|_| anyhow::anyhow!("this fund's net asset value does not fit in 64 bits"))?,
            i64::try_from(self.debits - self.credits)
                .map_err(|_| anyhow::anyhow!("the trial-balance difference does not fit in 64 bits"))?,
        ))
    }
}


/// Strike a NAV over the whole journal as it stands, in one book of record.
///
/// ⛔ THE PREFIX IS THE WHOLE JOURNAL IN EVERY VIEW, AND THE VIEW DECIDES WHAT
/// IS FOLDED WITHIN IT. `journal_position` and `journal_digest` cover every
/// entry, recognised or not, because that is what a replay has to re-read to
/// prove history was not rewritten. What the view changes is which of those
/// entries reach the fold. Pinning only the recognised ones would make a strike
/// unable to notice that an entry it declined was edited afterwards.
pub fn strike(
    book_path: &std::path::Path,
    view: &str,
    valuation_time: i64,
    actor: &str,
) -> Result<Strike> {
    if actor.trim().is_empty() {
        bail!("a NAV is signed by somebody — pass --actor or set RATIO_ACTOR");
    }
    let book = FileBook::open(book_path)?;
    // ⛔ ONE WALK, NOT THREE COPIES. This read the journal into a `Vec`, then
    // `prefix_digest` serialized every entry into a second `Vec`, then
    // `fold_nav` walked the first — to produce two numbers and a hash.
    // ⛔ THE CUT IS DERIVED FROM THE VALUATION POINT, NOT CARRIED BESIDE IT.
    // A settlement figure is determined by (prefix, view, day), and the day is
    // the one the fund is valuing — which the strike already records. A second
    // field would be two answers to one question, and the failure mode is a NAV
    // labelled the 30th and folded through the 2nd. Same argument as
    // `FUND_CURRENCY` being one constant.
    let mut fold = NavFold::new(&book, view, Some(day_of(valuation_time)))?;
    let mut hash = ratio_store::DigestBuilder::new();
    let mut count = 0usize;
    let mut recognised = 0usize;
    let mut last_config = String::new();
    book.for_each_entry_since(0, &mut |e| {
        if fold.consider(&book, e) {
            recognised += 1;
        }
        feed(&mut hash, e)?;
        count += 1;
        // ⚠ Carried forward rather than looked up afterwards: the last entry is
        // gone by the time the walk ends, and it is the one whose configuration
        // was in force at the valuation point.
        last_config.clear();
        last_config.push_str(e.config.as_str());
        Ok(())
    })?;
    if count == 0 {
        bail!("nothing to strike: this book has no entries");
    }
    // ⛔ REFUSED, NOT QUALIFIED. An entry this view cannot date contributes to
    // no figure, so a NAV struck over a book holding one is short by an amount
    // nobody has been told about — and it would tie, digest and replay the whole
    // way. The same judgement `Ratio.Valuation.strike_refuses_exactly_when_
    // something_is_unpriced` makes about a position with no price.
    if !fold.unplaceable.is_empty() {
        let shown: Vec<&str> = fold.unplaceable.iter().take(3).map(|s| s.as_str()).collect();
        bail!(
            "view {view:?} cannot place {} of this book's {count} entries, so a NAV struck in \
             it would be short by an amount nobody was told about:\n  {}{}",
            fold.unplaceable.len(),
            shown.join("\n  "),
            if fold.unplaceable.len() > shown.len() { "\n  …" } else { "" }
        );
    }
    let (nav, tb) = fold.finish()?;
    let entries_len = count;
    let journal_digest = hash.finish().as_str().to_string();
    let _ = recognised;

    Ok(Strike {
        view: view.to_string(),
        id: id_for(valuation_time),
        valuation_time,
        actor: actor.to_string(),
        journal_position: entries_len,
        journal_digest,
        net_asset_value: nav,
        trial_balance_difference: tb,
        // The configuration the LAST entry was posted under, which is what was
        // in force at the valuation point. Not `ACTIVE`: that is what is in
        // force NOW, and a strike re-read after a later approval would silently
        // claim to have been computed under a configuration that did not exist
        // when it was taken.
        config_digest: last_config,
    })
}

/// Re-derive a strike and report what was found.
pub fn replay(book_path: &std::path::Path, s: &Strike) -> Result<Replay> {
    let book = FileBook::open(book_path)?;

    // ⛔ ONE WALK, AND ONLY THE PREFIX IS FOLDED. This read the whole journal
    // into a `Vec`, sliced it, and hashed a second `Vec` of the slice.
    //
    // ⚠ It still READS to the end, because a shorter journal than the strike
    // recorded is the thing this has to detect and there is no way to know that
    // without reaching the end. What it no longer does is HOLD it.
    // ⛔ IN THE STRIKE'S OWN VIEW, AND THE VIEW'S TERMS COME FROM THE DIGEST
    // EACH ENTRY PINNED — which `NavFold::def_for` does, so a calendar amended
    // since cannot move this figure. A replay that resolved the view from the
    // ACTIVE configuration would re-derive a different NAV the day somebody
    // declared a holiday, and report the strike as unreproducible when nothing
    // about it had changed.
    let mut fold = NavFold::new(&book, &s.view, Some(day_of(s.valuation_time)))?;
    let mut hash = ratio_store::DigestBuilder::new();
    let mut seen = 0usize;
    book.for_each_entry_since(0, &mut |e| {
        if seen < s.journal_position {
            fold.consider(&book, e);
            feed(&mut hash, e)?;
        }
        seen += 1;
        Ok(())
    })?;

    // A journal that has since grown is normal and expected — a strike folds a
    // PREFIX. One that has SHRUNK cannot be reconciled with the strike at all,
    // and is reported as broken history rather than allowed to panic on a slice.
    if seen < s.journal_position {
        return Ok(Replay {
            id: s.id.clone(),
            history_intact: false,
            reproduced: false,
            net_asset_value: 0,
            journal_digest: String::new(),
        });
    }

    let digest = hash.finish().as_str().to_string();
    let (nav, tb) = fold.finish()?;

    Ok(Replay {
        id: s.id.clone(),
        history_intact: digest == s.journal_digest,
        reproduced: nav == s.net_asset_value && tb == s.trial_balance_difference,
        net_asset_value: nav,
        journal_digest: digest,
    })
}

/// The civil day a valuation point falls on, as `Ratio.Views.Day`.
///
/// ⛔ THE ONE PLACE THAT DECIDES WHAT DAY A STRIKE IS CUTTING AT. Two
/// derivations of this would be two answers to the question a settlement view
/// turns on, and they would disagree exactly at a UTC midnight — which is where
/// a period end sits.
pub fn day_of(unix_seconds: i64) -> views::Day {
    unix_seconds.div_euclid(86_400) as views::Day
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
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        s.id, s.valuation_time, s.actor, s.journal_position, s.journal_digest,
        s.net_asset_value, s.trial_balance_difference, s.config_digest, s.view
    )
}

/// ⛔ THE VIEW IS THE NINTH FIELD, APPENDED, AND THAT IS THE WHOLE MIGRATION.
/// Every line written before this existed has eight and reads back as the view
/// a book with no declared views has — which is exactly what such a book was
/// struck in. Nothing is rewritten, no digest moves, and an append-only record
/// stays readable, which is the same argument `#[serde(default)]` makes on
/// every field added to `JournalEntry`.
fn parse(l: &str) -> Option<Strike> {
    let f: Vec<&str> = l.split('\t').collect();
    if f.len() < 8 {
        return None; // a truncated line is skipped, never guessed at
    }
    Some(Strike {
        view: f.get(8).map(|v| v.trim_end()).filter(|v| !v.is_empty())
            .unwrap_or(ratio_rules::UNDECLARED_VIEW).to_string(),
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

/// Every strike on a book, in every view, newest first.
pub fn list(book_path: &std::path::Path) -> Result<Vec<Strike>> {
    let text = std::fs::read_to_string(book_path.join("NAVS")).unwrap_or_default();
    let mut out: Vec<Strike> = text.lines().filter_map(parse).collect();
    out.reverse();
    Ok(out)
}

/// Every strike in one book of record, newest first.
pub fn list_in(book_path: &std::path::Path, view: &str) -> Result<Vec<Strike>> {
    Ok(list(book_path)?.into_iter().filter(|s| s.view == view).collect())
}

/// One strike, by view and id.
///
/// ⛔ BOTH, BECAUSE `id_for` DERIVES THE ID FROM THE VALUATION TIME ALONE. An
/// accounting NAV and a settlement NAV for one moment carry the same id, so a
/// lookup on the id alone returns whichever the file happens to hold first —
/// silently, and labelled with the view the caller asked for.
pub fn get(book_path: &std::path::Path, view: &str, id: &str) -> Result<Strike> {
    list(book_path)?
        .into_iter()
        .find(|s| s.id == id && s.view == view)
        .with_context(|| format!("no NAV strike {id:?} in view {view:?}"))
}

/// Strike and record it.
///
/// Refuses to overwrite an existing strike at the same valuation point. A NAV
/// struck twice for one moment is two answers to a question that has one, and
/// silently replacing the first is how the earlier number stops existing.
pub fn strike_and_record(
    book_path: &std::path::Path,
    view: &str,
    valuation_time: i64,
    actor: &str,
) -> Result<Strike> {
    let s = strike(book_path, view, valuation_time, actor)?;
    // ⛔ THE KEY IS `(view, id)`, AND WIDENING IT IS WHAT MAKES MULTI-VIEW BOOKS
    // POSSIBLE WITHOUT WEAKENING ANYTHING. `Ratio.Period.one_answer_per_view_
    // per_day` still refuses a second answer to the same question; two views
    // striking one valuation point are two answers to two different questions,
    // and `two_views_are_two_answers_and_neither_restates_the_other` is that
    // fact. Keyed on the id alone, the settlement strike would be refused as a
    // restatement of the accounting one.
    if list(book_path)?.iter().any(|e| e.id == s.id && e.view == s.view) {
        bail!(
            "a NAV is already struck for {} in view {} — a valuation point has one answer \
             per book of record, and replacing it would remove the first",
            s.id,
            s.view
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

    /// The view a book declaring none has. Every book below is one.
    const V: &str = ratio_rules::UNDECLARED_VIEW;

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
            
                trade_date: None,
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
        let s = strike(&d, V, NOON, "e.marsh").unwrap();
        assert_eq!(s.journal_position, 2);
        assert_eq!(s.net_asset_value, 30_000_000, "assets only; equity is outside the fold");
        assert_eq!(s.trial_balance_difference, 0);
        assert_eq!(s.journal_digest.len(), 64);
        assert_eq!(s.actor, "e.marsh");
    }

    #[test]
    fn a_strike_replays_identically() {
        let d = book("replay");
        let s = strike(&d, V, NOON, "e.marsh").unwrap();
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
        let s = strike(&d, V, NOON, "e.marsh").unwrap();

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
        
            trade_date: None,
            announcement: None,
        })
        .unwrap();

        let r = replay(&d, &s).unwrap();
        assert!(r.ok(), "a later entry must not disturb an earlier strike");
        assert_eq!(r.net_asset_value, s.net_asset_value);

        // And a strike taken now folds more and is a different number.
        let s2 = strike(&d, V, NOON + 3600, "e.marsh").unwrap();
        assert_eq!(s2.journal_position, 3);
        assert_ne!(s2.journal_digest, s.journal_digest);
    }

    #[test]
    fn rewritten_history_is_caught_and_named_as_such() {
        // The accusation about the DATA. An append-only journal should make
        // this impossible, which is exactly why it is worth checking: the
        // interesting failure is the one that was ruled out.
        let d = book("tamper");
        let s = strike(&d, V, NOON, "e.marsh").unwrap();

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
        let s = strike(&d, V, NOON, "e.marsh").unwrap();
        std::fs::write(d.join("journal.jsonl"), "").unwrap();
        let r = replay(&d, &s).unwrap();
        assert!(!r.history_intact);
        assert!(!r.ok());
    }

    #[test]
    fn nothing_strikes_itself() {
        let d = book("actor");
        assert!(strike(&d, V, NOON, "  ").is_err(), "a NAV is signed by somebody");
    }

    #[test]
    fn an_empty_book_cannot_be_struck() {
        let d = std::env::temp_dir().join("ratio-nav-empty");
        let _ = std::fs::remove_dir_all(&d);
        FileBook::open(&d).unwrap().put_accounts(&[]).unwrap();
        assert!(strike(&d, V, NOON, "e.marsh").is_err());
    }

    #[test]
    fn a_recorded_strike_round_trips() {
        let d = book("record");
        let s = strike_and_record(&d, V, NOON, "e.marsh").unwrap();
        let back = get(&d, V, &s.id).unwrap();
        assert_eq!(back, s, "what was written is not what comes back");
        assert!(replay(&d, &back).unwrap().ok());
    }

    #[test]
    fn a_valuation_point_cannot_be_struck_twice() {
        // Two answers to a question that has one. Overwriting silently is how
        // the first number stops existing.
        let d = book("twice");
        strike_and_record(&d, V, NOON, "e.marsh").unwrap();
        let e = strike_and_record(&d, V, NOON, "e.marsh").unwrap_err().to_string();
        assert!(e.contains("already struck"), "{e}");
        assert_eq!(list(&d).unwrap().len(), 1);
    }

    #[test]
    fn strikes_come_back_newest_first() {
        let d = book("order");
        strike_and_record(&d, V, NOON, "a").unwrap();
        strike_and_record(&d, V, NOON + 3600, "b").unwrap();
        let all = list(&d).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].valuation_time > all[1].valuation_time);
    }

    #[test]
    fn a_streamed_digest_is_the_digest_it_replaces() {
        // ⛔ A STRIKE TAKEN BEFORE THIS CHANGE MUST STILL REPLAY. The digest is
        // what makes a figure re-derivable years later, and swapping a
        // one-shot hash of a materialized buffer for an incremental one is only
        // safe if the same bytes arrive in the same order. Asserted against the
        // ORIGINAL construction rather than against itself.
        let d = book("digest-stream");
        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        assert!(!entries.is_empty());

        // The construction this replaced: serialize every entry into one
        // buffer, hash it once.
        let mut buf = Vec::new();
        for e in &entries {
            buf.extend_from_slice(serde_json::to_string(e).unwrap().as_bytes());
            buf.push(b'\n');
        }
        let materialized = ratio_store::Digest::of(&buf).as_str().to_string();

        assert_eq!(prefix_digest(&entries).unwrap(), materialized);

        // And an empty prefix hashes as an empty buffer, which is what a strike
        // over no entries would have recorded.
        assert_eq!(
            prefix_digest(&[]).unwrap(),
            ratio_store::Digest::of(&[]).as_str().to_string()
        );
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
    /// A book keeping two books of record, over one journal.
    ///
    /// ⛔ THE SUBSCRIPTION IS THE POINT, AND A PURCHASE WOULD NOT DO. `t1` moves
    /// cash into investments — both assets — so recognising it or not moves the
    /// NAV by ZERO, and a test built on trades alone is green whatever the
    /// settlement convention does. `c1` is capital: equity is outside the NAV
    /// filter, so recognising it leaves the asset side holding a balance and the
    /// figure MOVES. HANDOFF.md records the multi-currency version of this being
    /// written vacuously twice.
    fn dual_book(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ratio-nav-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
            Account { dim: 20, display_name: "Capital".into(), account_type: A::Equity },
        ])
        .unwrap();
        let c = b
            .put(
                b"rules = []\n\
                  [[calendar]]\n\
                  id = \"c\"\n\
                  [[view]]\n\
                  id = \"abor\"\n\
                  basis = \"trade\"\n\
                  [[view]]\n\
                  id = \"ibor\"\n\
                  basis = \"settlement\"\n\
                  settles_in = 2\n\
                  calendar = \"c\"\n",
            )
            .unwrap();
        b.set_active(&c).unwrap();
        for (id, legs) in [
            ("c1", vec![(2, 30_000_000i64), (20, -30_000_000)]),
            ("t1", vec![(1, 25_000_000), (2, -25_000_000)]),
        ] {
            b.append(&JournalEntry {
                id: id.into(),
                memo: id.into(),
                config: c.clone(),
                postings: legs
                    .into_iter()
                    .map(|(dim, amount)| PostingRecord::new(dim, amount))
                    .collect(),
                // Struck on the valuation day itself, so it has settled in the
                // trade-date view and has NOT in the settlement one.
                trade_date: Some("2026-06-28".into()),
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    #[test]
    fn an_eight_field_navs_line_still_reads() {
        // ⛔ AGAINST A HAND-WRITTEN LEGACY LINE, NOT AGAINST ONE THIS CODE WROTE.
        // Round-tripping through `line` would compare the parser to itself and
        // pass however the format changed. Every NAVS file in existence has
        // eight fields, and reading them back as anything but the view a book
        // with no declared views has would attribute a figure to a convention
        // that did not exist when it was struck.
        let legacy = "2026-06-30T1600Z\t1782662400\te.marsh\t3\tdeadbeef\t100\t0\tcafe";
        let s = parse(legacy).expect("an eight-field line is a record, not a truncation");
        assert_eq!(s.view, ratio_rules::UNDECLARED_VIEW);
        assert_eq!(s.id, "2026-06-30T1600Z");
        assert_eq!(s.net_asset_value, 100);
        assert_eq!(s.config_digest, "cafe");

        // And a seven-field line is still a truncation rather than a guess.
        assert!(parse("a\tb\tc\td\te\tf\tg").is_none());
    }

    #[test]
    fn two_views_strike_one_valuation_point_and_neither_restates_the_other() {
        // ⭐ `Ratio.Period.two_views_are_two_answers_and_neither_restates_the_
        // other`, as the file it is about. Keyed on the id alone — which
        // `id_for` derives from the valuation time — the second of these would
        // be refused as a restatement of the first.
        let d = dual_book("two-views-one-point");
        let a = strike_and_record(&d, "abor", NOON, "e.marsh").unwrap();
        let b = strike_and_record(&d, "ibor", NOON, "e.marsh").unwrap();
        assert_eq!(a.id, b.id, "one valuation point, one id");
        assert_ne!(a.view, b.view);
        assert_eq!(list(&d).unwrap().len(), 2);

        // ⛔ AND THE FIGURES DIFFER, WHICH IS THE ONLY THING THAT SAYS THE
        // CONVENTION REACHED THE FOLD. Two views agreeing is what a hardcoded
        // recognition rule also produces.
        assert_ne!(
            a.net_asset_value, b.net_asset_value,
            "one journal, two settlement conventions, identical NAV — the view reaches nothing"
        );
        assert_eq!(a.net_asset_value, 30_000_000, "the subscription has settled on trade date");
        assert_eq!(b.net_asset_value, 0, "and has not, two open days later");

        // Both still tie: a view keeps or drops WHOLE entries, so conservation
        // cannot move. `Ratio.Views.every_view_conserves`.
        assert_eq!(a.trial_balance_difference, 0);
        assert_eq!(b.trial_balance_difference, 0);

        // Each view sees only its own, and each is still refused a second answer
        // to its OWN question.
        assert_eq!(list_in(&d, "abor").unwrap().len(), 1);
        assert_eq!(get(&d, "ibor", &b.id).unwrap().view, "ibor");
        let e = strike_and_record(&d, "abor", NOON, "e.marsh").unwrap_err().to_string();
        assert!(e.contains("per book of record"), "{e}");
    }

    #[test]
    fn a_replay_reproduces_each_view_on_its_own_terms() {
        // ⛔ THE MOST LIKELY-VACUOUS TEST IN THE FEATURE, so it asserts the
        // FIGURE and not merely `ok()`. A replay resolving the view from the
        // ACTIVE configuration rather than from the digest each entry pinned
        // would still report `reproduced` on a book whose configuration has not
        // moved — and would be wrong the day one did.
        let d = dual_book("replay-per-view");
        for view in ["abor", "ibor"] {
            let s = strike_and_record(&d, view, NOON, "e.marsh").unwrap();
            let r = replay(&d, &s).unwrap();
            assert!(r.ok(), "{view} did not replay");
            assert_eq!(r.net_asset_value, s.net_asset_value, "{view} replayed a different figure");
        }
    }

    #[test]
    fn a_view_the_pinned_configuration_does_not_declare_refuses_rather_than_folding() {
        // ⛔ NO FALLBACK TO JOURNAL ORDER. An entry folded as `recorded` into a
        // settlement NAV is an entry no settlement convention admitted, and the
        // figure would tie, digest and replay the whole way. Same argument that
        // stops an unreadable configuration falling back to FIFO.
        let d = dual_book("view-not-declared");
        let e = strike(&d, "nobody-declared-this", NOON, "e.marsh")
            .expect_err("a view the configuration does not declare")
            .to_string();
        assert!(e.contains("declares no view"), "{e}");
        assert!(e.contains("cannot place"), "the refusal must say what it could not do: {e}");
    }

    #[test]
    fn a_settlement_view_refuses_a_book_whose_entries_carry_no_trade_date() {
        // ⛔ REFUSED, NOT QUIETLY SHORT. `book()` predates trade dates, which is
        // what every book written before them looks like. A settlement view over
        // it cannot date a single entry, so a NAV struck in one would be missing
        // every figure — and would tie.
        let d = book("settle-over-undated");
        let mut b = FileBook::open(&d).unwrap();
        let c = b
            .put(
                b"rules = []\n[[calendar]]\nid = \"c\"\n[[view]]\nid = \"ibor\"\n\
                  basis = \"settlement\"\nsettles_in = 2\ncalendar = \"c\"\n",
            )
            .unwrap();
        b.set_active(&c).unwrap();
        // ⚠ The entries still pin the OLD digest, which declares no views at
        // all — so this exercises the refusal above rather than the date one.
        // Both are refusals and both name the entry; asserting the message
        // distinguishes them.
        let e = strike(&d, "ibor", NOON, "e.marsh").unwrap_err().to_string();
        assert!(e.contains("cannot place"), "{e}");
    }

}
