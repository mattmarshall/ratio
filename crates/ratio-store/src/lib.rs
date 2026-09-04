//! ratio-store — the append-only record, and the content-addressed config seam.
//!
//! PLAN.md Stage 0: *a ledger you can query*. The kernel proves conservation
//! over vectors in memory; until something durably records what was posted and
//! under which rules, neither the demo nor the shadow wedge can be built.
//!
//! **The log is the database.** Entries are immutable and appended; a
//! correction is a new entry with its own provenance, never an edit. That is
//! the same access pattern the platform page describes, and it is why this
//! needs no relational engine at MVP scale: the trial balance is a fold of the
//! log, which is the definition rather than an optimisation of it.
//!
//! Two seams are stated as traits so the implementation can change without the
//! callers noticing:
//!
//! * [`ConfigStore`] — exactly the five methods PLAN.md specifies. Backed here
//!   by a directory and a pointer file; later by crova for the blobs and geetch
//!   for review and history. Every entry records the digest it was posted
//!   under, so nothing downstream changes when the implementation does.
//! * [`Journal`] — the append-only record. Backed here by one JSON line per
//!   entry, or by one object per entry on an [`ObjectStore`] when the process
//!   is wired to a durable backend (`tla/S3Journal.tla`, issue #24). The
//!   trigger to put a real engine underneath is indexed lookup (postings for
//!   one account over a period, without a full scan), which the MVP does not
//!   need and the wedge might.
//!
//! **An unbalanced entry cannot be appended.** [`Journal::append`] runs the
//! kernel's conservation check first and refuses. This is the product's actual
//! claim, so it is enforced at the door rather than reported at month-end.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod objects;
pub use objects::{
    install_object_store, installed_object_store, DirStore, MemoryStore, ObjectStore, SeqLog,
};

use anyhow::{anyhow, bail, Context, Result};
use ratio_chart::{AccountType, Posting, TrialBalance};
use ratio_kernel::{transaction_is_balanced, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// The currency a book reports in, and the base every other is translated into.
///
/// ⛔ ONE DEFINITION, BECAUSE IT WAS TWO ANSWERS TO ONE QUESTION AND THEY
/// DISAGREED. `Fund.currency_code` labeled the console's figures USD while
/// `ratio strike` — the RECORDED nav, the one a replay re-derives — summed
/// dollars, euros and pounds without translating any of them. Both were "the
/// base currency"; only one of them knew it.
///
/// ⚠ HARDCODED, AND THAT IS A REAL LIMITATION rather than a placeholder. A
/// fund's reporting currency is a property of the fund, and when a second one
/// arrives this becomes a field on the book.
pub const BASE_CURRENCY: &str = "USD";

/// A content address: the SHA-256 of some bytes, lowercase hex.
///
/// Names the artifact by what it *is* rather than where it sits, which is what
/// makes "which rules produced this figure?" answerable years later.
/// ⛔ INTERNED, BECAUSE EVERY JOURNAL LINE CARRIES ONE AND THEY ARE NEARLY ALL
/// THE SAME. A digest is 64 hex characters; a book with one configuration
/// repeats those 64 bytes on every entry, which is **22% of the journal's bytes**
/// and one heap allocation per line on the read path. At the shape issue #6 asks
/// for that is 140 million allocations of a value that has one distinct
/// instance.
///
/// `Text` is `Arc<str>`, so `Ord`, `Eq`, `Hash` and the serialized form are
/// exactly what `String` gave — a clone is a refcount bump, and N copies become
/// one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(ratio_common::intern::Text);

thread_local! {
    /// One table per thread. ⚠ It keeps every digest the thread has seen alive,
    /// which is the right trade for a value with a handful of distinct
    /// instances per book and one per line.
    static DIGESTS: std::cell::RefCell<ratio_common::intern::Interner> =
        std::cell::RefCell::new(ratio_common::intern::Interner::new());
}

fn intern_digest(s: &str) -> ratio_common::intern::Text {
    DIGESTS.with(|d| d.borrow_mut().intern(s))
}

impl serde::Serialize for Digest {
    /// ⛔ THE WIRE FORM IS UNCHANGED — a plain string, exactly as the derive
    /// produced. Every journal ever written must still read, and every digest
    /// ever computed must still compare equal.
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Digest {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Digest;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a sha-256 hex digest")
            }
            /// ⭐ TAKES `&str`, SO A BORROWED INPUT NEVER BECOMES A `String`. A
            /// hex digest contains no escapes, so `serde_json` hands the slice
            /// straight out of the line buffer and this allocates nothing at
            /// all after the first sight of a given digest.
            fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<Digest, E> {
                Ok(Digest(intern_digest(s)))
            }
        }
        d.deserialize_str(V)
    }
}

/// A digest computed over bytes that arrive a piece at a time.
///
/// ⛔ BECAUSE THE THING BEING HASHED IS A JOURNAL PREFIX. `prefix_digest` built
/// a `Vec<u8>` of every serialized entry and hashed it in one call — about
/// twenty-four gigabytes at the shape issue #6 asks for, to compute a hash that
/// is streaming by construction.
///
/// ⚠ THE BYTES AND THEIR ORDER ARE THE CONTRACT, not this type. Feeding the
/// same bytes in the same order gives the same digest as hashing them in one
/// go, which is what makes a strike struck before this change still replay
/// after it — asserted by `a_streamed_digest_is_the_digest_it_replaces`.
#[derive(Default)]
pub struct DigestBuilder {
    hasher: Sha256,
}

impl DigestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    pub fn finish(self) -> Digest {
        Digest(intern_digest(&format!("{:x}", self.hasher.finalize())))
    }
}

impl Digest {
    /// The content address of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        let mut h = Sha256::new();
        h.update(bytes);
        Digest(intern_digest(&format!("{:x}", h.finalize())))
    }

    /// The full hex digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first seven characters, for display. Long enough to be unambiguous
    /// in a report, short enough to read aloud on a call.
    pub fn short(&self) -> &str {
        &self.0[..7]
    }

    /// Parse a hex digest, rejecting anything that is not one — a malformed
    /// digest in a journal line means the provenance is not trustworthy, and
    /// silently accepting it would defeat the point of recording it.
    pub fn parse(s: &str) -> Result<Self> {
        if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("not a sha-256 hex digest: {s:?}");
        }
        Ok(Digest(intern_digest(&s.to_ascii_lowercase())))
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One posting as it is written down: a conserved dimension and an exact
/// integer amount in minor units.
///
/// Deliberately distinct from `ratio_kernel::Posting`, which is Lean-emitted
/// and carries no dependencies. Serialization is an I/O concern and does not
/// belong on the proven kernel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostingRecord {
    pub dim: i64,
    pub amount: i64,

    /// What this posting is denominated in.
    ///
    /// ⛔ THE CONSERVED DIMENSION. `Ratio.Chart.Dimensions` — currency is what
    /// must net to zero, and two currencies are two INDEPENDENT conservation
    /// laws rather than one law over a sum. Without this field the kernel could
    /// only check the sum, and `[USD +100, EUR −100]` passed as balanced:
    /// nothing exchanged, no rate recorded, each side out by a hundred.
    ///
    /// ⚠ `None` IS ITS OWN GROUP, NOT A DEFAULT TO THE BASE CURRENCY. Every
    /// posting written before this field existed has `None`, so an untyped book
    /// forms exactly one group and behaves precisely as it did. A book that
    /// mixes typed and untyped postings has TWO groups and will refuse — which
    /// is right: an entry that names a currency on one leg and not the other is
    /// ambiguous, and guessing which base the untyped leg meant is how the hole
    /// above got in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    /// Which instrument this posting concerns, if any.
    ///
    /// An account does not name a conserved quantity — it PARTITIONS one. The
    /// kernel conserves value; accounts say where the value sits. Recording the
    /// instrument partitions it further, so conservation is untouched by
    /// construction: `Ratio.Ingest.partition_preserves_conservation` proves the
    /// parts of zero sum to zero, and `positions_roll_up` proves a position
    /// breakdown cannot contradict the account it breaks down.
    ///
    /// `#[serde(default)]` so every journal written before this field existed
    /// still reads — an append-only log you cannot read is not a record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument: Option<String>,

    /// The share count, in whole units.
    ///
    /// ⚠ NOT CONSERVED, and deliberately not claimed to be. Buying a hundred
    /// shares creates a hundred here and destroys them at the counterparty, who
    /// is outside this book's boundary. Quantity is a MEASURE, checked against
    /// the custodian by reconciliation — which is what reconciliation is for.
    /// Calling it a conserved dimension would be a better story and a false
    /// one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i64>,
}

impl PostingRecord {
    /// A posting that concerns no particular instrument — a fee, a
    /// subscription, a transfer.
    pub fn new(dim: i64, amount: i64) -> Self {
        PostingRecord { dim, amount, currency: None, instrument: None, quantity: None }
    }

    /// A posting against an instrument, with the quantity it moved.
    pub fn of(dim: i64, amount: i64, instrument: &str, quantity: Option<i64>) -> Self {
        PostingRecord {
            dim,
            amount,
            currency: None,
            instrument: Some(instrument.to_string()),
            quantity,
        }
    }

    /// A posting denominated in a named currency.
    ///
    /// ⚠ SEPARATE FROM [`new`] RATHER THAN AN ARGUMENT ON IT. `None` is its own
    /// conservation group, not a default, so a caller that has a currency and
    /// one that has none are making genuinely different statements — and the
    /// door refuses an entry that mixes them. A single constructor taking
    /// `Option` would make forgetting it look like declaring nothing.
    ///
    /// [`new`]: PostingRecord::new
    pub fn of_currency(dim: i64, amount: i64, currency: &str) -> Self {
        PostingRecord {
            dim,
            amount,
            currency: Some(currency.to_string()),
            instrument: None,
            quantity: None,
        }
    }
}

impl From<&PostingRecord> for Posting {
    /// The kernel sees only the conserved quantity. An instrument partitions
    /// where the value sits; it does not change how much there is.
    fn from(p: &PostingRecord) -> Self {
        Posting {
            dim: p.dim,
            amount: p.amount,
        }
    }
}

/// Appended to a conservation error when the currencies are what failed.
///
/// ⚠ The flat total can be ZERO while this is the real fault — that is the
/// whole point of the check — so the message has to say which law was broken.
const CURRENCY_HINT: &str = " — and its currencies do not net independently. \
A hundred dollars against a hundred euros sums to zero and exchanges nothing; \
each currency is its own conservation law";

/// A corporate action as announced, recorded IN THE JOURNAL.
///
/// ⛔ IN THE JOURNAL, NOT IN `Plane::Actions`, AND THAT IS THE WHOLE POINT.
/// `Ratio.Actions.Factor.replay_is_determined_by_the_prefix` is the obligation:
/// under the factor representation nothing is applied, so a strike's figure
/// depends on the announcements in force — and unless those are inside the
/// prefix a strike pins, a replay run later reads whatever has arrived since
/// and answers differently.
///
/// `Ratio.Actions.Factor.an_unpinned_announcement_changes_the_answer` is the
/// counterexample and `//tla:announcements_in_side_log_check` is the run: same
/// pin, same day, a different number. Worse than a restatement, because a
/// restatement announces itself.
///
/// ⚠ FLAT, and duplicating the shape of `ratio_ingest::actions::Announced`
/// rather than importing it. The store owns the format of what it persists; a
/// journal that could only be read by linking the ingest layer would be a
/// record with a dependency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnouncementRecord {
    pub id: String,
    pub instrument: String,
    /// Units received for units given up: a 2-for-1 is `2` and `1`.
    pub numerator: i64,
    pub denominator: i64,
    /// `YYYY-MM-DD`. ISO so it compares in date order as a string.
    pub ex_date: String,
    /// When we were told, which is not when it took effect.
    pub announced: i64,
}

/// An entry in the journal: a balanced transaction, plus the provenance that
/// makes the figures it produces reproducible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Client-assigned identifier, unique within the book.
    pub id: String,
    /// What this entry is, in a human's words.
    #[serde(default)]
    pub memo: String,
    /// The configuration version in force when this was posted.
    pub config: Digest,
    /// The postings. Must net to zero on every dimension.
    pub postings: Vec<PostingRecord>,

    /// The day the trade was struck, `YYYY-MM-DD`.
    ///
    /// ⛔ ON THE ENTRY, NOT THE POSTING. A trade date is a property of the
    /// transaction; every leg of it happened on the same day, and putting it on
    /// each posting would create the possibility of them disagreeing.
    ///
    /// ⚠ AND IT IS OPTIONAL BECAUSE EVERY EXISTING ENTRY LACKS ONE. A lot opened
    /// by an entry with no date has no acquisition date, and the holding-period
    /// methods REFUSE such a holding rather than guessing — both defaults are
    /// wrong in the direction that matters. Defaulting to the epoch makes
    /// everything long-term, which is the favorable rate on records that do not
    /// support it; defaulting to today makes everything short-term, which is
    /// punitive on a holding held for years.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trade_date: Option<String>,

    /// A corporate action being ANNOUNCED, if that is what this entry is.
    ///
    /// ⛔ AN ANNOUNCEMENT MOVES NO MONEY, so such an entry carries no postings
    /// and nets to zero trivially — it passes the balance check at the door
    /// because it has nothing to unbalance. What it is doing in the journal is
    /// taking a POSITION in the order, so that every strike after it pins it.
    ///
    /// `#[serde(default)]` so every journal written before this field existed
    /// still reads. An append-only log you cannot read is not a record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announcement: Option<AnnouncementRecord>,

    /// When an invoice or bill is due, `YYYY-MM-DD`.
    ///
    /// ⛔ ABSENT MEANS THE ITEM CANNOT BE AGED. Treating a missing due date
    /// as current would put undated AR in the current bucket and misstate
    /// the aged schedule while the control account still ties. Same
    /// honesty bar as `trade_date`: both defaults are wrong, so there is
    /// no default.
    ///
    /// `#[serde(default)]` so every journal written before this field
    /// existed still reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,

    /// The open item (invoice or bill entry id) a collection or payment
    /// applies to.
    ///
    /// ⛔ ABSENT ON A REDUCTION MEANS THE SIDE CANNOT BE AGED. A collection
    /// that does not name its invoice cannot be allocated, so remaining
    /// per item is unknown. Inventing FIFO or an equal split would make
    /// the buckets look exact while being somebody else's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<String>,

    /// Lots the taxpayer named for this sale. `Ratio.Lots.SpecId`.
    ///
    /// ⛔ NOT A `lot_method`. Specific identification is an attested per-sale
    /// choice, not a sort of the holding. `lot_method = "specific_id"` stays
    /// refused. `None` means this sale is not SpecID; `Some([])` means the
    /// election is present and the lots are unnamed — that refuses, it does
    /// not walk FIFO. FIFO is a method real funds elect.
    ///
    /// `#[serde(default)]` so every journal written before this field
    /// existed still reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identified_lots: Option<Vec<u64>>,

    /// Exact partner specials this entry names. `Ratio.Partners.Fact`.
    ///
    /// ⛔ NOT A WEIGHT, AND NOT A SILENT 1/N. Each row is an amount
    /// somebody attested. `None` means this entry carries no special.
    /// `Some([])` is elected and unnamed and refuses — the SpecID
    /// shape. `Ratio.Partners.unnamed_facts_refuse`.
    ///
    /// `#[serde(default)]` so every journal written before this field
    /// existed still reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_allocations: Option<Vec<SpecialAllocationFact>>,
}

/// An exact amount named on an entry. Not a configuration weight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialAllocationFact {
    pub partner: String,
    /// `income` / `expense` / `unrealized`.
    pub kind: String,
    pub amount: i64,
}

/// A period close: the citeable boundary per book of record.
///
/// ⛔ NOT THE CLOSING POSTING. That is a journal entry — conserved, dated,
/// pinned to a configuration — and is what makes the roll-forward citeable
/// from ledger postings. This record is the evidence an operator opens:
/// which view, through which day, at which journal prefix, under which
/// configuration, by whom, when. `Ratio.Close`. `//tla:period_close_check`.
///
/// ⚠ `surplus` AND `closing_entry` ARE OPTIONAL. A period with no income or
/// expense activity can still be closed (the door holds); there is then
/// nothing to roll. Empty is unset, not a measured zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseRecord {
    pub view: String,
    /// `YYYY-MM-DD`. ISO so it compares in date order as a string.
    pub closed_date: String,
    pub journal_position: u64,
    pub journal_digest: String,
    pub config_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closing_entry: Option<String>,
    pub actor: String,
    /// Unix seconds. When the operator closed it, not the period end.
    pub recorded: i64,
    pub equity_destination: i64,
    /// Raw surplus posted to the destination. Absent when nothing closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surplus: Option<i64>,
}

impl JournalEntry {
    /// The kernel's view of this entry.
    pub fn transaction(&self) -> Transaction {
        Transaction {
            postings: self.postings.iter().map(Posting::from).collect(),
        }
    }

    /// Whether the entry conserves value, IN EVERY CURRENCY.
    ///
    /// ⛔ `Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch`. A sum
    /// over everything is one conservation law; a multi-currency book has one
    /// PER CURRENCY, and `[USD +100, EUR −100]` satisfies the first and neither
    /// of the second. Nothing was exchanged and no rate was recorded.
    ///
    /// ⚠ `None` is its own group, so a book whose postings predate the currency
    /// field forms exactly one group and behaves as it always did. A book mixing
    /// typed and untyped legs has two, and refuses — an entry naming a currency
    /// on one leg and not the other is ambiguous, and guessing which base the
    /// untyped leg meant is how the hole got in.
    pub fn conserves_every_currency(&self) -> bool {
        let mut nets: std::collections::BTreeMap<Option<&str>, i128> = Default::default();
        for p in &self.postings {
            *nets.entry(p.currency.as_deref()).or_default() += p.amount as i128;
        }
        nets.values().all(|n| *n == 0)
    }

    /// Whether the entry conserves value.
    pub fn is_balanced(&self) -> bool {
        transaction_is_balanced(&self.transaction()) && self.conserves_every_currency()
    }

    /// Refuse an unnamed or unparseable special-allocation election.
    ///
    /// ⭐ `None` IS SILENCE. `Some([])` is elected and unnamed — that
    /// refuses rather than inventing 1/N.
    /// `Ratio.Partners.unnamed_facts_refuse`.
    pub fn refuse_special_allocations(&self) -> Result<()> {
        let Some(facts) = &self.special_allocations else {
            return Ok(());
        };
        if facts.is_empty() {
            bail!(
                "entry {:?} names special_allocations and the list is empty — \
                 that is an unnamed election, not silence. Omit the field. A \
                 silent 1/N is the defect. Ratio.Partners.unnamed_facts_refuse",
                self.id
            );
        }
        for f in facts {
            if f.partner.trim().is_empty() {
                bail!(
                    "entry {:?} names a special allocation without a partner",
                    self.id
                );
            }
            if !matches!(f.kind.as_str(), "income" | "expense" | "unrealized") {
                bail!(
                    "entry {:?} special allocation kind {:?} is not income, \
                     expense, or unrealized",
                    self.id,
                    f.kind
                );
            }
        }
        Ok(())
    }
}

/// A named dimension — what an accountant calls an account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub dim: i64,
    pub display_name: String,
    /// Serialized as the lowercase type name: `asset`, `liability`, …
    pub account_type: AccountTypeRecord,
}

/// `AccountType` as it is written down. Mirrors the Lean-emitted enum, which
/// carries no serde dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountTypeRecord {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

/// The object-store prefix for a book: its directory name.
///
/// Demo funds are unique ids under one root (`/tmp/funds/acme`). Two books
/// that shared a directory name in different parents would share a journal;
/// the live path does not do that, and inventing a hash of the full path
/// would make a rename a silent fork.
fn book_key(root: &Path) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("book")
        .to_string()
}

/// PUT each non-empty line at its 1-based sequence, starting after the
/// store's current height.
///
/// Occupied slots are a contiguous prefix (`ContiguousNoHoles`). If the
/// store is already at or past the seed, this is a no-op — a second open
/// must not re-PutObject seq 1..=height. A timeout mid-hydrate leaves a
/// partial prefix; the next open resumes at height+1 rather than burning
/// the budget on 412s from seq 1.
fn hydrate_jsonl(path: &Path, log: &SeqLog) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(path).with_context(|| format!("seeding from {}", path.display()))?;
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let seed_line_count = u64::try_from(lines.len())
        .ok()
        .context("the seed is longer than a journal can hold")?;
    let height = log.height()?;
    if height >= seed_line_count {
        return Ok(());
    }
    // Contiguous prefix: slots 1..=height are already claimed.
    for (i, line) in lines.iter().enumerate().skip(height as usize) {
        let seq = (i as u64)
            .checked_add(1)
            .context("the seed is longer than a journal can hold")?;
        log.claim(seq, line.as_bytes())?;
    }
    Ok(())
}

impl From<AccountTypeRecord> for AccountType {
    fn from(t: AccountTypeRecord) -> Self {
        match t {
            AccountTypeRecord::Asset => AccountType::Asset,
            AccountTypeRecord::Liability => AccountType::Liability,
            AccountTypeRecord::Equity => AccountType::Equity,
            AccountTypeRecord::Income => AccountType::Income,
            AccountTypeRecord::Expense => AccountType::Expense,
        }
    }
}

/// The content-addressed configuration store — PLAN.md's seam, verbatim.
///
/// ⭐ THIS TRAIT IS THE CONTROL PLANE. v1 is [`DirectoryConfigStore`] (a
/// directory, SHA-256, and an atomic pointer file). The destination is crova
/// behind `put`/`get` and geetch behind `history` and the review that gates
/// `set_active`. Callers depend on these five methods, so that swap is a new
/// `impl`, not a rewrite. geetch/crova stay out until PLAN's trigger: a second
/// customer, or compliance-reviewed change control a pointer file cannot satisfy.
pub trait ConfigStore {
    /// Store `bytes` and return their content address.
    fn put(&mut self, bytes: &[u8]) -> Result<Digest>;
    /// Retrieve the bytes at `digest`.
    fn get(&self, digest: &Digest) -> Result<Vec<u8>>;
    /// Promote `digest` to active. The only way policy moves.
    ///
    /// ⛔ ATOMIC ON THE POINTER. A half-written ACTIVE would make "which rules
    /// produced this figure?" unanswerable — every posting names a digest, and
    /// the pointer is how a reader finds the one in force now.
    fn set_active(&mut self, digest: &Digest) -> Result<()>;
    /// The active configuration, if one has been promoted.
    fn active(&self) -> Result<Option<Digest>>;
    /// Every configuration ever promoted, newest first.
    fn history(&self) -> Result<Vec<Digest>>;
}

/// v1 ConfigStore: a directory, SHA-256, and a pointer file.
///
/// PLAN.md's afternoon shape. `FileBook` delegates here so the journal and the
/// control plane do not share one type — later crova/geetch sit behind this
/// struct, not behind the book.
///
/// ```text
/// <dir>/
///   <digest>     configuration bytes, addressed by content
///   ACTIVE       the digest currently in force (replaced by rename)
///   HISTORY      every promotion, one digest per line, oldest first
/// ```
pub struct DirectoryConfigStore {
    dir: PathBuf,
}

impl DirectoryConfigStore {
    /// Open a store, creating the directory if it is not there.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating config store at {}", dir.display()))?;
        Ok(Self { dir })
    }

    fn blob_path(&self, digest: &Digest) -> PathBuf {
        self.dir.join(digest.as_str())
    }

    /// Write `contents` over `path` by creating a sibling and renaming.
    ///
    /// ⛔ RENAME, NOT `fs::write`. A crash in the middle of a write leaves a
    /// truncated ACTIVE, which `Digest::parse` then refuses, which looks like
    /// "no configuration is in force" on a book that has one. rename(2) on
    /// POSIX replaces the destination atomically; a reader sees the old
    /// pointer or the new one, never a prefix of the hex.
    fn replace_atomically(path: &Path, contents: &str) -> Result<()> {
        let tmp = path.with_extension("tmp");
        {
            let mut f = File::create(&tmp)
                .with_context(|| format!("creating {}", tmp.display()))?;
            f.write_all(contents.as_bytes())
                .with_context(|| format!("writing {}", tmp.display()))?;
            f.sync_all()
                .with_context(|| format!("syncing {}", tmp.display()))?;
        }
        fs::rename(&tmp, path)
            .with_context(|| format!("promoting {} into place", path.display()))?;
        Ok(())
    }
}

impl ConfigStore for DirectoryConfigStore {
    fn put(&mut self, bytes: &[u8]) -> Result<Digest> {
        let digest = Digest::of(bytes);
        let path = self.blob_path(&digest);
        // Content-addressed: identical bytes are already stored, and rewriting
        // them would be a no-op at best and a corruption window at worst.
        if !path.exists() {
            fs::write(&path, bytes)
                .with_context(|| format!("writing config {}", digest.short()))?;
        }
        Ok(digest)
    }

    fn get(&self, digest: &Digest) -> Result<Vec<u8>> {
        fs::read(self.blob_path(digest)).with_context(|| format!("no config {}", digest.short()))
    }

    fn set_active(&mut self, digest: &Digest) -> Result<()> {
        // Promoting something that was never stored would leave every entry
        // posted afterwards pointing at nothing.
        if !self.blob_path(digest).exists() {
            bail!("cannot promote {}: not stored", digest.short());
        }
        // HISTORY first, then the pointer. A crash between them leaves a
        // promotion that is not yet active — re-running set_active is how that
        // recovers. The other order leaves ACTIVE naming a digest HISTORY
        // never recorded, which is the one a reader cannot reconstruct.
        let mut hist = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("HISTORY"))
            .context("opening HISTORY")?;
        writeln!(hist, "{digest}").context("appending to HISTORY")?;
        hist.sync_all().context("syncing HISTORY")?;
        Self::replace_atomically(&self.dir.join("ACTIVE"), digest.as_str())
            .context("writing ACTIVE")?;
        Ok(())
    }

    fn active(&self) -> Result<Option<Digest>> {
        let path = self.dir.join("ACTIVE");
        if !path.exists() {
            return Ok(None);
        }
        let s = fs::read_to_string(&path).context("reading ACTIVE")?;
        Ok(Some(Digest::parse(s.trim())?))
    }

    fn history(&self) -> Result<Vec<Digest>> {
        let path = self.dir.join("HISTORY");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let s = fs::read_to_string(&path).context("reading HISTORY")?;
        let mut out: Vec<Digest> = s
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Digest::parse(l.trim()))
            .collect::<Result<_>>()?;
        out.reverse(); // newest first
        Ok(out)
    }
}

/// The append-only record of what happened.
pub trait Journal {
    /// Append an entry. **Refuses an unbalanced one.**
    fn append(&mut self, entry: &JournalEntry) -> Result<()>;
    /// Append many, opening the journal once. **Same checks, one file handle.**
    fn append_all(&mut self, entries: &[JournalEntry]) -> Result<()>;
    /// Every entry, in the order they were posted.
    fn entries(&self) -> Result<Vec<JournalEntry>>;

    /// Entries appended after `offset` bytes, and where the journal now ends.
    ///
    /// ⛔ THE POINT OF THE BYTE OFFSET IS THAT IT DOES NOT READ WHAT IT SKIPS.
    /// `entries()` reads and parses the whole file, so a "maintained"
    /// projection built on it would still pay O(journal) just to discover
    /// nothing had changed — which is not maintenance, it is a rebuild with a
    /// cache in front of it.
    fn entries_since(&self, offset: u64) -> Result<(Vec<JournalEntry>, u64)>;

    /// Hand every entry after `offset` to `f`, one at a time. Returns where the
    /// journal now ends.
    ///
    /// ⛔ THE ONLY WAY TO READ A BIG JOURNAL. Everything else here MATERIALIZES:
    /// `entries()` and `entries_since()` build a `Vec` of the whole log before
    /// the caller sees the first row. Measured at 1.77M entries that is **1.85 GB
    /// resident** to fold a journal whose projection is 8 MB of lots — and the
    /// twenty-million-lot book issue #6 asks for is ~80M entries, where the
    /// vector alone would be about eighty gigabytes. The fold was never going to
    /// run out of time; it was going to run out of memory.
    ///
    /// ⚠ AND IT REUSES ONE LINE BUFFER. `BufReader::lines()` allocates a fresh
    /// `String` per line — 1.8M allocations of ~295 bytes before serde is even
    /// called.
    fn for_each_entry_since(
        &self,
        offset: u64,
        f: &mut dyn FnMut(&JournalEntry) -> Result<()>,
    ) -> Result<u64>;
}

/// A book on disk.
///
/// ```text
/// <root>/
///   journal.jsonl      one entry per line, appended, never rewritten
///                      (the local backend; an installed ObjectStore is
///                      `{book}/journal/{seq}` instead — issue #24)
///   accounts.json      the chart of accounts
///   config/<digest>    configuration bytes, addressed by content
///   config/ACTIVE      the digest currently in force
///   config/HISTORY     every promotion, one digest per line, oldest first
///   deliveries.jsonl   every file received, by content digest
///   entities.jsonl     the master: instruments, counterparties
///   facts.jsonl        what the templates read out of those files
/// ```
///
/// The last three are append-only for the same reason the journal is: a
/// correction is a NEW record, never an edit, so what was believed at the time
/// a figure was struck is still there to be read afterwards.
pub struct FileBook {
    root: PathBuf,
    /// When set, the journal and the append-only planes are one object per
    /// entry on this store, not `journal.jsonl` on disk. `/tmp` is still
    /// where the seeded chart and config live; it is no longer the system
    /// of record for a write. `tla/S3Journal.tla`.
    objects: Option<(Arc<dyn ObjectStore>, String)>,
}

/// An append-only plane beside the journal.
///
/// Named rather than free-form: these are the logs the data plane keeps, and a
/// store that would append to any filename a caller passed is a store that will
/// one day append to `journal.jsonl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// Files received, by content digest.
    Deliveries,
    /// The master: instruments, counterparties.
    Entities,
    /// What the templates read out of the files.
    Facts,
    /// Corporate actions announced, whether or not they have been applied.
    ///
    /// Announcement is not application. An action sits here from the moment
    /// somebody tells us about it, and the JOURNAL says whether it has gone
    /// through — which is how a strike taken before it can be identified
    /// afterwards without storing anything about staleness.
    Actions,
    /// Why somebody decided a difference was acceptable.
    ///
    /// ⛔ BESIDE THE JOURNAL AND NOT IN IT, which is the opposite of the call
    /// made for `Actions` and rests on the difference between them. An
    /// announcement changes what the books SAY once it is applied, so a side
    /// plane left it pinned by no strike and a replay answered differently as
    /// the world told us more. An explanation changes no figure at all: the
    /// difference is the same size after somebody explains it, and the NAV is
    /// the same NAV. What it changes is whether the break is WORKED.
    ///
    /// ⚠ SO NOTHING DOWNSTREAM MAY LET ONE MOVE A NUMBER. The moment an
    /// explanation adjusts a figure rather than annotating one, it belongs in
    /// the journal with everything else that does.
    Explanations,
    /// A period close: the citeable boundary, not the closing posting.
    ///
    /// ⛔ BESIDE THE JOURNAL, LIKE AN EXPLANATION, because the close record
    /// moves no money. The surplus roll-forward IS a journal entry — that is
    /// what makes the post-close trial balance cite the same postings as the
    /// P&L. This plane is the evidence: which view, through which day, at
    /// which prefix, under which configuration, by whom, when.
    ///
    /// ⚠ AND IT IS WHAT THE APPEND DOOR READS. Append-only storage is not a
    /// close; `Journal::append` refuses a dated entry on or before the
    /// closed-through day. `Ratio.Close.a_dated_entry_on_or_before_close_
    /// is_refused`. `//tla:period_close_check`.
    Closes,
}

impl Plane {
    fn file(self) -> &'static str {
        match self {
            Plane::Deliveries => "deliveries.jsonl",
            Plane::Entities => "entities.jsonl",
            Plane::Facts => "facts.jsonl",
            Plane::Actions => "actions.jsonl",
            Plane::Explanations => "explanations.jsonl",
            Plane::Closes => "closes.jsonl",
        }
    }
}

impl FileBook {
    /// Open a book, creating the layout if it is not there.
    ///
    /// When a process-wide [`ObjectStore`] has been installed, the journal
    /// (and the append-only planes) are read and written there. The seeded
    /// `journal.jsonl` is copied into the store once, by conditional PUT at
    /// the line's sequence number, so two containers hydrating the same seed
    /// converge rather than fork.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(root, installed_object_store())
    }

    /// How many journal entries the index may report.
    ///
    /// Prefers the local `journal.jsonl` line count — the demo entrypoint
    /// copies the seed onto `/tmp`, and that file is authoritative for the
    /// list. Falls back to [`SeqLog::height`] (one LIST, no body GETs) when
    /// the local file is missing and a store is in play.
    ///
    /// ⛔ NOT A FOLD. `trial_balance` and `for_each_entry_since` GET every
    /// object; a generated fund cannot finish that inside API Gateway's 30s.
    /// The books index needs a count, not a trial balance.
    pub fn list_entry_count(root: &Path, store: Option<Arc<dyn ObjectStore>>) -> Result<u64> {
        let journal = root.join("journal.jsonl");
        if journal.is_file() {
            let text = fs::read_to_string(&journal)
                .with_context(|| format!("counting {}", journal.display()))?;
            return Ok(text.lines().filter(|l| !l.trim().is_empty()).count() as u64);
        }
        if let Some(store) = store {
            return SeqLog::new(store, format!("{}/journal/", book_key(root))).height();
        }
        Ok(0)
    }

    /// Open a book against an explicit store. Tests use this so they do not
    /// have to mutate process-global state; the demo binary installs once
    /// and calls [`open`][`FileBook::open`].
    pub fn open_with(
        root: impl AsRef<Path>,
        store: Option<Arc<dyn ObjectStore>>,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("config"))
            .with_context(|| format!("creating book at {}", root.display()))?;
        let objects = store.map(|s| (s, book_key(&root)));
        let book = FileBook { root, objects };
        book.hydrate_objects()?;
        Ok(book)
    }

    fn journal_log(&self) -> Option<SeqLog> {
        let (store, key) = self.objects.as_ref()?;
        Some(SeqLog::new(store.clone(), format!("{key}/journal/")))
    }

    fn plane_log(&self, plane: Plane) -> Option<SeqLog> {
        let (store, key) = self.objects.as_ref()?;
        let name = plane.file().trim_end_matches(".jsonl");
        Some(SeqLog::new(store.clone(), format!("{key}/{name}/")))
    }

    /// Copy seeded jsonl files into the object store, slot by slot.
    ///
    /// ⛔ CLAIM AT THE LINE'S OWN SEQUENCE, NOT APPEND. Two containers that
    /// both find an empty store must PUT seed line 1 at slot 1. If they
    /// `append`, the loser retries at slot 2 with the same seed line and the
    /// journal double-counts the opening position — the books still tie.
    /// `put_if_absent` on a predetermined key makes a 412 mean "already
    /// seeded", not "try the next slot".
    fn hydrate_objects(&self) -> Result<()> {
        if self.objects.is_none() {
            return Ok(());
        }
        if let Some(log) = self.journal_log() {
            hydrate_jsonl(&self.journal_path(), &log)?;
        }
        for plane in [
            Plane::Deliveries,
            Plane::Entities,
            Plane::Facts,
            Plane::Actions,
            Plane::Explanations,
            Plane::Closes,
        ] {
            if let Some(log) = self.plane_log(plane) {
                hydrate_jsonl(&self.root.join(plane.file()), &log)?;
            }
        }
        Ok(())
    }

    fn journal_path(&self) -> PathBuf {
        self.root.join("journal.jsonl")
    }
    fn accounts_path(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    /// Append one record to a plane beside the journal.
    ///
    /// Generic over the record so the store does not need to know what a fact
    /// or an instrument looks like — those belong to `ratio-ingest`, and a
    /// storage layer that had to be edited every time the data plane learned a
    /// new fact type would be the wrong shape.
    pub fn append_record<T: serde::Serialize>(&mut self, log: Plane, record: &T) -> Result<()> {
        let line = serde_json::to_string(record).context("serializing the record")?;
        if line.contains('\n') {
            bail!("a record with a newline in it would break the log");
        }
        if let Some(seq) = self.plane_log(log) {
            seq.append(line.as_bytes())?;
            return Ok(());
        }
        let path = self.root.join(log.file());
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        use std::io::Write;
        writeln!(f, "{line}").with_context(|| format!("appending to {}", path.display()))?;
        Ok(())
    }

    /// Every record on a plane, in the order they were appended.
    pub fn records<T: serde::de::DeserializeOwned>(&self, log: Plane) -> Result<Vec<T>> {
        if let Some(seq) = self.plane_log(log) {
            let mut out = Vec::new();
            seq.for_each_since(0, &mut |n, bytes| {
                let s = std::str::from_utf8(bytes).with_context(|| {
                    format!("{} sequence {n} is not utf-8", log.file())
                })?;
                out.push(
                    serde_json::from_str(s)
                        .with_context(|| format!("{} sequence {n}", log.file()))?,
                );
                Ok(())
            })?;
            return Ok(out);
        }
        let path = self.root.join(log.file());
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                serde_json::from_str(l)
                    .with_context(|| format!("{} line {}", path.display(), i + 1))
            })
            .collect()
    }

    /// Replace the chart of accounts. The chart is current-state, not a log —
    /// it names dimensions rather than recording events.
    pub fn put_accounts(&mut self, accounts: &[Account]) -> Result<()> {
        let json = serde_json::to_string_pretty(accounts)?;
        fs::write(self.accounts_path(), json).context("writing accounts")?;
        Ok(())
    }

    /// The chart of accounts. Empty if none has been set.
    pub fn accounts(&self) -> Result<Vec<Account>> {
        let path = self.accounts_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let s = fs::read_to_string(&path).context("reading accounts")?;
        Ok(serde_json::from_str(&s).context("parsing accounts")?)
    }

    /// Every period close recorded on this book, oldest first.
    pub fn closes(&self) -> Result<Vec<CloseRecord>> {
        self.records(Plane::Closes)
    }

    /// The latest close of one view, if any.
    ///
    /// ⛔ PER VIEW. Two books of record are two answers; a settlement close
    /// does not borrow the accounting book's closed-through day.
    pub fn latest_close(&self, view: &str) -> Result<Option<CloseRecord>> {
        let mut closes = self.closes()?;
        closes.retain(|c| c.view == view);
        Ok(closes.into_iter().max_by(|a, b| a.closed_date.cmp(&b.closed_date)))
    }

    /// Refuse a dated entry that lands on or before any view's closed-through
    /// day.
    ///
    /// ⭐ THE DOOR STAGE 1 NAMED. Append-only storage is not a close.
    /// `Ratio.Close.a_dated_entry_on_or_before_close_is_refused`. An undated
    /// entry has no period (`an_undated_entry_is_not_in_a_period`) and is
    /// left alone — period folds already skip it.
    ///
    /// ⚠ CONSERVATIVE ACROSS VIEWS. The journal is shared. Refusing against
    /// the most restrictive close is the door that cannot silently put an
    /// entry into a view that already signed the period.
    fn refuse_closed_period(&self, entry: &JournalEntry) -> Result<()> {
        let Some(day) = entry.trade_date.as_deref() else {
            return Ok(());
        };
        let closes = self.closes()?;
        if let Some(c) = closes
            .iter()
            .filter(|c| day <= c.closed_date.as_str())
            .max_by(|a, b| a.closed_date.cmp(&b.closed_date))
        {
            bail!(
                "entry {:?} is dated {day}, and view {:?} is closed through {} \
                 (journal prefix {}, closed by {}). Posting into a closed period \
                 is refused — append-only storage is not a close, and this door is. \
                 Ratio.Close.a_dated_entry_on_or_before_close_is_refused",
                entry.id,
                c.view,
                c.closed_date,
                c.journal_position,
                c.actor
            );
        }
        Ok(())
    }

    /// The trial balance of the whole book — the fold of the log.
    ///
    /// Cannot report out of balance for a book built through [`Journal::append`]:
    /// every entry conserved on the way in, and conservation is closed under
    /// the fold (`Ratio.Core.ledger_conserves`), so total debits equal total
    /// credits by `Ratio.Chart.trial_balance_ties`.
    pub fn trial_balance(&self) -> Result<TrialBalance> {
        // ⛔ STREAMED, AND IT USED TO MATERIALIZE TWICE — the whole journal into
        // a `Vec<JournalEntry>`, and then every posting in it into a second
        // `Vec<Posting>`. At the shape issue #6 asks for that is tens of
        // gigabytes to add up two columns.
        //
        // ⚠ `i128` ACCUMULATORS, as `ratio_chart::debits`/`credits` use: these
        // grow with HISTORY rather than with the fund, and an `i64` that wrapped
        // would not look wrong — it would look like a trial balance.
        let (mut debits, mut credits) = (0i128, 0i128);
        self.for_each_entry_since(0, &mut |e| {
            for p in &e.postings {
                let a = p.amount as i128;
                if a >= 0 {
                    debits += a;
                } else {
                    credits += -a;
                }
            }
            Ok(())
        })?;
        Ok(TrialBalance {
            debits: i64::try_from(debits)
                .map_err(|_| anyhow!("total debits do not fit in 64 bits"))?,
            credits: i64::try_from(credits)
                .map_err(|_| anyhow!("total credits do not fit in 64 bits"))?,
        })
    }

    /// Value and quantity per (account, instrument), plus what is NOT
    /// attributed to any instrument.
    ///
    /// ⛔ THE UNATTRIBUTED PART IS RETURNED, NOT DROPPED. `positions_roll_up`
    /// proves that filtering a list and taking the rest gives the total back —
    /// BOTH halves. A positions view that showed only the attributed half
    /// would disagree with the trial balance by exactly the amount it hid,
    /// which is the one thing two views of one book must never do.
    pub fn positions(&self) -> Result<(BTreeMap<(i64, String), (i64, i64)>, BTreeMap<i64, i64>)> {
        let mut held: BTreeMap<(i64, String), (i64, i64)> = BTreeMap::new();
        let mut rest: BTreeMap<i64, i64> = BTreeMap::new();
        // ⛔ STREAMED. This is bounded by the CHART — a few hundred positions —
        // and used to hold the whole journal in memory to build it.
        self.for_each_entry_since(0, &mut |entry| {
            for p in &entry.postings {
                match &p.instrument {
                    Some(i) => {
                        let slot = held.entry((p.dim, i.clone())).or_insert((0, 0));
                        slot.0 += p.amount;
                        slot.1 += p.quantity.unwrap_or(0);
                    }
                    None => *rest.entry(p.dim).or_default() += p.amount,
                }
            }
            Ok(())
        })?;
        Ok((held, rest))
    }

    /// Debit and credit totals per dimension, for the report.
    /// ⛔ KEYED BY `(dimension, currency)`, AND THE PAIR IS THE POINT. This
    /// returned `BTreeMap<i64, _>` — it folded away the currency — and five
    /// callers printed the result as a trial balance, adding dollars to euros
    /// under one account label on every multi-currency book. `Projection::nav`
    /// and `ratio strike` were both moved OFF this fold for exactly that reason
    /// and the fold was left in place, still wrong, still called.
    ///
    /// ⚠ SO THE CURRENCY IS IN THE TYPE. A caller that wants one number per
    /// account now has to say what it did with the denominations — translate
    /// them, or show them apart — rather than getting a plausible total for
    /// free. `Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch`.
    pub fn balances_by_dim(&self) -> Result<BTreeMap<(i64, Option<String>), (i64, i64)>> {
        let mut out: BTreeMap<(i64, Option<String>), (i64, i64)> = BTreeMap::new();
        // ⛔ STREAMED. Bounded by the chart, not by the log it reads.
        self.for_each_entry_since(0, &mut |entry| {
            for p in &entry.postings {
                let slot = out.entry((p.dim, p.currency.clone())).or_insert((0, 0));
                if p.amount >= 0 {
                    slot.0 += p.amount;
                } else {
                    slot.1 += -p.amount;
                }
            }
            Ok(())
        })?;
        Ok(out)
    }
}

impl FileBook {
    fn configs(&self) -> Result<DirectoryConfigStore> {
        DirectoryConfigStore::open(self.config_dir())
    }
}

impl ConfigStore for FileBook {
    fn put(&mut self, bytes: &[u8]) -> Result<Digest> {
        self.configs()?.put(bytes)
    }
    fn get(&self, digest: &Digest) -> Result<Vec<u8>> {
        self.configs()?.get(digest)
    }
    fn set_active(&mut self, digest: &Digest) -> Result<()> {
        self.configs()?.set_active(digest)
    }
    fn active(&self) -> Result<Option<Digest>> {
        self.configs()?.active()
    }
    fn history(&self) -> Result<Vec<Digest>> {
        self.configs()?.history()
    }
}

impl Journal for FileBook {
    fn append(&mut self, entry: &JournalEntry) -> Result<()> {
        // The check at the door. An unbalanced entry never reaches the record,
        // so there is no state in which the book is out and somebody has to
        // find it.
        if !entry.is_balanced() {
            let net: i128 = entry.postings.iter().map(|p| p.amount as i128).sum();
            return Err(anyhow!(
                "entry {:?} does not conserve value: postings net to {net}, not 0{}",
                entry.id,
                if entry.conserves_every_currency() { "" } else { CURRENCY_HINT }
            ));
        }
        // The provenance has to resolve, or the entry is not reproducible.
        if self.get(&entry.config).is_err() {
            bail!(
                "entry {:?} names config {} which is not stored",
                entry.id,
                entry.config.short()
            );
        }
        self.refuse_closed_period(entry)?;
        entry.refuse_special_allocations()?;
        let line = serde_json::to_string(entry).context("serializing entry")?;
        if let Some(log) = self.journal_log() {
            log.append(line.as_bytes())?;
            return Ok(());
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.journal_path())
            .context("opening journal")?;
        writeln!(f, "{line}").context("appending to journal")?;
        Ok(())
    }

    /// Append many entries, opening the journal once.
    ///
    /// ⛔ THE DOOR CHECK IS STILL PER ENTRY. What is shared is the file handle
    /// and nothing else: every entry is balanced and has resolvable provenance
    /// before any of them is written, so a batch cannot smuggle in an entry that
    /// `append` would have refused. Sharing the VALIDATION would be a different
    /// function wearing this one's name.
    ///
    /// ⚠ AND IT IS ALL-OR-NOTHING ON VALIDATION, NOT ON WRITING. The checks run
    /// over the whole batch first, so a bad entry stops the write before it
    /// starts — but a failure mid-write (a full disk) leaves what was already
    /// appended. That is the same guarantee `append` gives one entry at a time,
    /// and an append-only log is readable to wherever it stops.
    ///
    /// Exists because `append` opens and closes the journal on every call, which
    /// is ~4 ms an entry — 549 seconds for 140,000, and twenty-one hours for the
    /// twenty million this system's central claim is about. Measured, not
    /// assumed.
    fn append_all(&mut self, entries: &[JournalEntry]) -> Result<()> {
        // ⚠ THE PROVENANCE CHECK IS PER DISTINCT CONFIG, NOT PER ENTRY, and
        // that is a correctness-preserving change rather than a shortcut:
        // `get` is a pure read of content-addressed bytes, so asking twice for
        // one digest cannot give two answers. Asking per entry read the config
        // file 140,000 times and was most of what this function cost.
        let mut checked: std::collections::BTreeSet<&Digest> = Default::default();
        for entry in entries {
            if !entry.is_balanced() {
                // ⛔ `i128`. Reporting the net of an entry that did not balance
                // must not itself overflow — the whole reason it did not balance
                // may be that the figures are enormous, which is exactly when an
                // `i64` sum wraps and the message says "nets to 0".
                let net: i128 = entry.postings.iter().map(|p| p.amount as i128).sum();
                return Err(anyhow!(
                    "entry {:?} does not conserve value: postings net to {net}, not 0{}",
                    entry.id,
                    if entry.conserves_every_currency() { "" } else { CURRENCY_HINT }
                ));
            }
            if checked.insert(&entry.config) && self.get(&entry.config).is_err() {
                bail!(
                    "entry {:?} names config {} which is not stored",
                    entry.id,
                    entry.config.short()
                );
            }
            self.refuse_closed_period(entry)?;
            entry.refuse_special_allocations()?;
        }
        if let Some(log) = self.journal_log() {
            // Each entry is its own claim. Sharing a file handle is the jsonl
            // optimisation; sharing a slot would be a fork.
            for entry in entries {
                let line = serde_json::to_string(entry).context("serializing entry")?;
                log.append(line.as_bytes())?;
            }
            return Ok(());
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.journal_path())
            .context("opening journal")?;
        let mut buf = String::new();
        for entry in entries {
            buf.clear();
            let line = serde_json::to_string(entry).context("serializing entry")?;
            buf.push_str(&line);
            buf.push('\n');
            f.write_all(buf.as_bytes()).context("appending to journal")?;
        }
        Ok(())
    }

    fn entries_since(&self, offset: u64) -> Result<(Vec<JournalEntry>, u64)> {
        // ⚠ MATERIALIZES. Kept for callers that genuinely want the whole log —
        // tests, and books small enough not to care. Anything folding a real
        // journal wants `for_each_entry_since`; see the note on the trait.
        let mut out = Vec::new();
        let end = self.for_each_entry_since(offset, &mut |e| {
            out.push(e.clone());
            Ok(())
        })?;
        Ok((out, end))
    }

    fn for_each_entry_since(
        &self,
        offset: u64,
        f: &mut dyn FnMut(&JournalEntry) -> Result<()>,
    ) -> Result<u64> {
        if let Some(log) = self.journal_log() {
            // ⚠ THE CURSOR IS A SEQUENCE NUMBER, not a byte offset. The jsonl
            // backend seeks by bytes because that is what a file is; an object
            // per entry has no bytes to skip. Mixing the two on one book is
            // a different book, which `follow` already refuses when the
            // journal appears to shrink.
            return log.for_each_since(offset, &mut |seq, bytes| {
                let entry: JournalEntry = serde_json::from_slice(bytes).with_context(|| {
                    format!("journal sequence {seq} is not an entry")
                })?;
                f(&entry)
            });
        }
        use std::io::{BufRead, BufReader, Seek, SeekFrom};
        let path = self.journal_path();
        if !path.exists() {
            return Ok(0);
        }
        let mut file =
            fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let end = file.metadata()?.len();

        // ⚠ A SHORTER FILE THAN THE OFFSET MEANS THE JOURNAL WAS REPLACED, not
        // that nothing happened. An append-only log does not shrink, so this is
        // a different book at the same path — reading from the stale offset
        // would splice two histories together and fold the result as one.
        if offset > end {
            bail!(
                "the journal at {} is {end} bytes and was read to {offset} — an append-only \
                 log does not shrink, so this is not the book that was read",
                path.display()
            );
        }
        file.seek(SeekFrom::Start(offset)).context("seeking the journal")?;

        let mut reader = BufReader::new(file);
        // ⛔ ONE BUFFER, REUSED. `lines()` allocates a `String` per line.
        let mut line = String::new();
        let mut consumed = offset;
        let mut i = 0usize;
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .with_context(|| format!("{} after byte {offset}", path.display()))?;
            if n == 0 {
                break;
            }
            consumed += n as u64;
            i += 1;
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(line.trim_end()).with_context(|| {
                format!("{} line {} after byte {offset}", path.display(), i)
            })?;
            f(&entry)?;
        }
        Ok(consumed)
    }

    fn entries(&self) -> Result<Vec<JournalEntry>> {
        if self.journal_log().is_some() {
            let (out, _) = self.entries_since(0)?;
            return Ok(out);
        }
        let path = self.journal_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let f = File::open(&path).context("opening journal")?;
        let mut out = Vec::new();
        for (i, line) in BufReader::new(f).lines().enumerate() {
            let line = line.context("reading journal")?;
            if line.trim().is_empty() {
                continue;
            }
            out.push(
                serde_json::from_str(&line)
                    .with_context(|| format!("journal line {} is not an entry", i + 1))?,
            );
        }
        Ok(out)
    }
}

#[cfg(test)]
mod position_tests {
    use super::*;

    /// ⛔ The property `Ratio.Ingest.positions_roll_up` proves, on a real book.
    ///
    /// A positions breakdown plus what it does not attribute must equal the
    /// account. My first check of this asserted the positions alone equalled
    /// the account and failed by 350,000 — I had forgotten the other half of
    /// the theorem I had just written.
    #[test]
    fn an_interned_digest_is_wire_identical_to_the_string_it_replaced() {
        // ⛔ EVERY JOURNAL EVER WRITTEN MUST STILL READ. `Digest` held a
        // `String` and derived its serde; it holds an `Arc<str>` now with
        // hand-written impls, and the ONLY thing that makes that safe is that
        // the bytes on the wire did not move.
        let d = Digest::of(b"some configuration bytes");
        let json = serde_json::to_string(&d).unwrap();

        // A plain JSON string, quoted, 64 hex characters — what the derive
        // produced.
        assert_eq!(json, format!("\"{}\"", d.as_str()));
        assert_eq!(d.as_str().len(), 64);

        // And it round-trips to something equal, from a line it did not create.
        let back: Digest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        assert_eq!(back.as_str(), d.as_str());

        // ⭐ THE POINT: two sightings of one digest share one allocation, which
        // is what a journal repeating it on every line is for.
        let a: Digest = serde_json::from_str(&json).unwrap();
        let b: Digest = serde_json::from_str(&json).unwrap();
        assert!(std::sync::Arc::ptr_eq(&a.0, &b.0));

        // ⚠ Ordering and equality are still by VALUE, not by pointer — a
        // `BTreeMap` keyed on digests is consulted by content.
        let other = Digest::of(b"different bytes");
        assert_ne!(other, d);
        assert_eq!(other.cmp(&d), other.as_str().cmp(d.as_str()));
    }

    #[test]
    fn positions_and_what_they_do_not_attribute_equal_the_account() {
        let dir = std::env::temp_dir().join("ratio-store-positions");
        let _ = fs::remove_dir_all(&dir);
        let mut b = FileBook::open(&dir).unwrap();
        let cfg = b.put(b"rules = []\n").unwrap();
        b.set_active(&cfg).unwrap();

        // One entry attributed to an instrument, one not — a fee has no
        // security on it, and a book has both.
        b.append(&JournalEntry {
            id: "t1".into(),
            memo: "buy".into(),
            config: cfg.clone(),
            postings: vec![
                PostingRecord::of(1, 25_000_00, "inst-vti", Some(100)),
                PostingRecord::new(2, -25_000_00),
            ],
        
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        })
        .unwrap();
        b.append(&JournalEntry {
            id: "f1".into(),
            memo: "an adjustment with no security on it".into(),
            config: cfg.clone(),
            postings: vec![
                PostingRecord::new(1, 1_000_00),
                PostingRecord::new(2, -1_000_00),
            ],
        
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        })
        .unwrap();

        let (held, rest) = b.positions().unwrap();
        assert_eq!(held[&(1, "inst-vti".to_string())], (25_000_00, 100));
        assert_eq!(rest[&1], 1_000_00);

        let bal = b.balances_by_dim().unwrap();
        let account: i64 = bal[&(1, None)].0 - bal[&(1, None)].1;
        let attributed: i64 = held.iter().filter(|((d, _), _)| *d == 1).map(|(_, v)| v.0).sum();
        assert_eq!(
            attributed + rest[&1],
            account,
            "the breakdown must equal the figure it breaks down",
        );

        // …and conservation is untouched: an instrument partitions where the
        // value sits, not how much there is.
        let tb = b.trial_balance().unwrap();
        assert_eq!(tb.debits, tb.credits, "an instrument partitions value, it does not create any");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("ratio-store-test-{n}-{:?}", std::thread::current().id()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn book() -> (FileBook, Digest) {
        let mut b = FileBook::open(tmp()).unwrap();
        let d = b.put(b"rules = []\n").unwrap();
        b.set_active(&d).unwrap();
        (b, d)
    }

    fn entry(id: &str, cfg: &Digest, postings: &[(i64, i64)]) -> JournalEntry {
        JournalEntry {
            id: id.into(),
            memo: String::new(),
            config: cfg.clone(),
            postings: postings
                .iter()
                .map(|(dim, amount)| PostingRecord::new(*dim, *amount))
                .collect(),
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        }
    }

    /// The same entry, every leg in one currency.
    fn entry_in(id: &str, cfg: &Digest, ccy: &str, postings: &[(i64, i64)]) -> JournalEntry {
        JournalEntry {
            postings: postings.iter().map(|(d, a)| in_ccy(*d, *a, ccy)).collect(),
            ..entry(id, cfg, &[])
        }
    }

    fn in_ccy(dim: i64, amount: i64, ccy: &str) -> PostingRecord {
        PostingRecord {
            dim,
            amount,
            currency: Some(ccy.into()),
            instrument: None,
            quantity: None,
        }
    }

    #[test]
    fn unnamed_special_allocations_are_refused_not_split() {
        // `Ratio.Partners.unnamed_facts_refuse`. Some([]) is elected
        // and unnamed — that refuses, it does not invent 1/N.
        let (mut b, cfg) = book();
        let mut e = entry("s", &cfg, &[(1, 100), (2, -100)]);
        e.special_allocations = Some(vec![]);
        let err = b.append(&e).unwrap_err().to_string();
        assert!(err.contains("unnamed"), "{err}");
        e.special_allocations = Some(vec![crate::SpecialAllocationFact {
            partner: String::new(),
            kind: "income".into(),
            amount: 100,
        }]);
        let err = b.append(&e).unwrap_err().to_string();
        assert!(err.contains("without a partner"), "{err}");
        e.special_allocations = Some(vec![crate::SpecialAllocationFact {
            partner: "LP".into(),
            kind: "fee".into(),
            amount: 100,
        }]);
        let err = b.append(&e).unwrap_err().to_string();
        assert!(err.contains("not income"), "{err}");
        e.special_allocations = Some(vec![crate::SpecialAllocationFact {
            partner: "LP".into(),
            kind: "income".into(),
            amount: 100,
        }]);
        b.append(&e).unwrap();
    }

    #[test]
    fn a_flat_total_of_zero_across_two_currencies_is_refused() {
        // ⛔ `Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch`. A
        // hundred dollars against a hundred euros sums to zero and exchanges
        // NOTHING — no rate was recorded and each side is out by a hundred.
        // This passed the door until the posting carried a currency.
        let (mut b, cfg) = book();
        let e = JournalEntry {
            id: "fx".into(),
            memo: "not an exchange".into(),
            config: cfg,
            postings: vec![in_ccy(1, 10_000, "USD"), in_ccy(2, -10_000, "EUR")],
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        };
        assert!(!e.conserves_every_currency());
        let err = b.append(&e).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("own conservation law"), "names WHICH law: {msg}");
    }

    #[test]
    fn a_real_exchange_conserves_both_sides() {
        // ⚠ The same shape done properly needs four legs — the two sides of an
        // actual exchange, at a rate somebody recorded.
        let (mut b, cfg) = book();
        assert!(b
            .append(&JournalEntry {
                id: "fx-ok".into(),
                memo: "an exchange".into(),
                config: cfg,
                postings: vec![
                    in_ccy(1, 10_000, "USD"),
                    in_ccy(2, -10_000, "USD"),
                    in_ccy(1, 9_000, "EUR"),
                    in_ccy(2, -9_000, "EUR"),
                ],
                trade_date: None,
                announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
            })
            .is_ok());
    }

    #[test]
    fn an_untyped_book_behaves_exactly_as_it_did() {
        // ⚠ THE MIGRATION. Every posting written before the currency field has
        // `None`, so an untyped book forms exactly ONE group and the check is
        // the one it always was. Nothing that balanced yesterday refuses today.
        let (mut b, cfg) = book();
        assert!(b
            .append(&entry("plain", &cfg, &[(1, 500), (2, -500)]))
            .is_ok());
    }

    #[test]
    fn mixing_a_typed_leg_with_an_untyped_one_is_refused() {
        // ⛔ TWO GROUPS, AND THAT IS RIGHT. An entry naming a currency on one
        // leg and not the other is ambiguous, and guessing which base the
        // untyped leg meant is exactly how the hole got in.
        let (mut b, cfg) = book();
        let e = JournalEntry {
            id: "mixed".into(),
            memo: "half-declared".into(),
            config: cfg,
            postings: vec![in_ccy(1, 500, "USD"), PostingRecord::new(2, -500)],
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        };
        assert!(!e.conserves_every_currency());
        assert!(b.append(&e).is_err());
    }

    #[test]
    fn digest_is_content_addressed() {
        assert_eq!(Digest::of(b"abc"), Digest::of(b"abc"));
        assert_ne!(Digest::of(b"abc"), Digest::of(b"abd"));
        // Known SHA-256 of "abc" — pins that this is the real hash, not a
        // stand-in that happens to be deterministic.
        assert_eq!(
            Digest::of(b"abc").as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_malformed_digest_is_rejected() {
        assert!(Digest::parse("nope").is_err());
        assert!(Digest::parse(&"z".repeat(64)).is_err());
        assert!(Digest::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn config_round_trips_and_promotes() {
        let mut b = FileBook::open(tmp()).unwrap();
        assert_eq!(b.active().unwrap(), None);
        let d = b.put(b"fee = 75bp").unwrap();
        assert_eq!(b.get(&d).unwrap(), b"fee = 75bp");
        b.set_active(&d).unwrap();
        assert_eq!(b.active().unwrap(), Some(d.clone()));
        assert_eq!(b.history().unwrap(), vec![d]);
    }

    #[test]
    fn promoting_something_unstored_is_refused() {
        let mut b = FileBook::open(tmp()).unwrap();
        let ghost = Digest::of(b"never stored");
        assert!(b.set_active(&ghost).is_err());
    }

    #[test]
    fn history_is_newest_first() {
        let mut b = FileBook::open(tmp()).unwrap();
        let d1 = b.put(b"one").unwrap();
        let d2 = b.put(b"two").unwrap();
        b.set_active(&d1).unwrap();
        b.set_active(&d2).unwrap();
        assert_eq!(b.history().unwrap(), vec![d2.clone(), d1]);
        assert_eq!(b.active().unwrap(), Some(d2));
    }

    #[test]
    fn put_does_not_promote_and_identical_bytes_store_once() {
        // ⭐ PLAN's seam: put is content-addressed and is not set_active.
        // A store that promoted on write would make "which rules are in force"
        // the last thing anybody saved, including a draft.
        let dir = tmp();
        let mut s = DirectoryConfigStore::open(&dir).unwrap();
        let d = s.put(b"fee = 75bp").unwrap();
        assert_eq!(s.active().unwrap(), None, "put must not move the pointer");
        assert_eq!(s.put(b"fee = 75bp").unwrap(), d, "identical bytes, one digest");
        assert!(
            dir.join(d.as_str()).exists(),
            "the blob is a file named by the hash",
        );
        s.set_active(&d).unwrap();
        assert_eq!(s.active().unwrap(), Some(d.clone()));
        assert_eq!(s.history().unwrap(), vec![d]);
    }

    #[test]
    fn promoting_something_unstored_does_not_move_the_pointer() {
        let mut s = DirectoryConfigStore::open(tmp()).unwrap();
        let ghost = Digest::of(b"never stored");
        assert!(s.set_active(&ghost).is_err());
        assert_eq!(s.active().unwrap(), None);
        assert!(s.history().unwrap().is_empty(), "a refused promotion is not history");
    }

    #[test]
    fn set_active_replaces_the_pointer_atomically() {
        // After a promotion the pointer is a complete digest, never a prefix:
        // replace_atomically renames into place. A reader that saw a truncated
        // ACTIVE would report "no configuration" on a book that has one.
        let dir = tmp();
        let mut s = DirectoryConfigStore::open(&dir).unwrap();
        let d1 = s.put(b"one").unwrap();
        let d2 = s.put(b"two").unwrap();
        s.set_active(&d1).unwrap();
        s.set_active(&d2).unwrap();
        let raw = fs::read_to_string(dir.join("ACTIVE")).unwrap();
        assert_eq!(raw, d2.as_str(), "ACTIVE is the digest, not a partial write");
        assert_eq!(s.active().unwrap(), Some(d2.clone()));
        assert_eq!(s.history().unwrap()[0], d2);
        assert!(!dir.join("ACTIVE.tmp").exists(), "the temp does not linger");
    }

    #[test]
    fn a_balanced_entry_is_appended_and_read_back() {
        let (mut b, cfg) = book();
        let e = entry("t1", &cfg, &[(1, 500), (2, -500)]);
        b.append(&e).unwrap();
        assert_eq!(b.entries().unwrap(), vec![e]);
    }

    #[test]
    fn an_unbalanced_entry_is_refused_at_the_door() {
        let (mut b, cfg) = book();
        let err = b
            .append(&entry("bad", &cfg, &[(1, 500), (2, -499)]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not conserve"), "{err}");
        // And nothing reached the record.
        assert!(b.entries().unwrap().is_empty());
    }

    #[test]
    fn an_entry_naming_an_unstored_config_is_refused() {
        let (mut b, _) = book();
        let ghost = Digest::of(b"never stored");
        assert!(b.append(&entry("t1", &ghost, &[(1, 1), (2, -1)])).is_err());
    }

    #[test]
    fn the_book_ties_however_many_entries_are_posted() {
        let (mut b, cfg) = book();
        for i in 0..50 {
            b.append(&entry(
                &format!("t{i}"),
                &cfg,
                &[(1, 100 + i), (2, -(100 + i))],
            ))
            .unwrap();
        }
        let tb = b.trial_balance().unwrap();
        assert_eq!(tb.debits, tb.credits);
        assert!(ratio_chart::trial_balance_ties(tb));
        assert_eq!(tb.debits, (0..50).map(|i| 100 + i).sum::<i64>());
    }

    #[test]
    fn multi_leg_entries_tie_too() {
        let (mut b, cfg) = book();
        b.append(&entry("fee", &cfg, &[(10, 61_240_18), (20, -61_240_00), (21, -18)]))
            .unwrap();
        let tb = b.trial_balance().unwrap();
        assert!(ratio_chart::trial_balance_ties(tb));
        assert_eq!(tb.debits, 61_240_18);
    }

    #[test]
    fn balances_are_reported_per_dimension() {
        let (mut b, cfg) = book();
        b.append(&entry("a", &cfg, &[(1, 300), (2, -300)])).unwrap();
        b.append(&entry("b", &cfg, &[(1, 200), (3, -200)])).unwrap();
        let by = b.balances_by_dim().unwrap();
        assert_eq!(by[&(1, None)], (500, 0));
        assert_eq!(by[&(2, None)], (0, 300));
        assert_eq!(by[&(3, None)], (0, 200));
    }

    #[test]
    fn balances_keep_the_currencies_apart() {
        // ⛔ THE ROW THIS FOLD USED TO MERGE. Keyed on the dimension alone, one
        // account holding 300 dollars and 200 euros reported "500" — a number
        // in no currency, printed under a currency-free column header by five
        // separate callers. Splitting the key is what makes that unsayable.
        let (mut b, cfg) = book();
        b.append(&entry_in("a", &cfg, "USD", &[(1, 300), (2, -300)])).unwrap();
        b.append(&entry_in("b", &cfg, "EUR", &[(1, 200), (2, -200)])).unwrap();
        let by = b.balances_by_dim().unwrap();
        assert_eq!(by[&(1, Some("USD".into()))], (300, 0));
        assert_eq!(by[&(1, Some("EUR".into()))], (200, 0));
        assert_eq!(by.get(&(1, None)), None, "nothing lands in the untyped slot");
    }

    #[test]
    fn the_journal_survives_reopening() {
        let root = tmp();
        let cfg = {
            let mut b = FileBook::open(&root).unwrap();
            let d = b.put(b"cfg").unwrap();
            b.set_active(&d).unwrap();
            b.append(&entry("t1", &d, &[(1, 7), (2, -7)])).unwrap();
            d
        };
        let reopened = FileBook::open(&root).unwrap();
        assert_eq!(reopened.entries().unwrap().len(), 1);
        assert_eq!(reopened.active().unwrap(), Some(cfg));
        assert!(ratio_chart::trial_balance_ties(
            reopened.trial_balance().unwrap()
        ));
    }

    #[test]
    fn accounts_round_trip() {
        let mut b = FileBook::open(tmp()).unwrap();
        assert!(b.accounts().unwrap().is_empty());
        let chart = vec![
            Account {
                dim: 1,
                display_name: "Cash".into(),
                account_type: AccountTypeRecord::Asset,
            },
            Account {
                dim: 2,
                display_name: "Capital".into(),
                account_type: AccountTypeRecord::Equity,
            },
        ];
        b.put_accounts(&chart).unwrap();
        assert_eq!(b.accounts().unwrap(), chart);
        // The record type maps onto the Lean-emitted classification.
        assert_eq!(
            ratio_chart::normal_side(chart[0].account_type.into()),
            ratio_chart::Side::Debit
        );
    }

    #[test]
    fn a_journal_on_an_object_store_survives_dropping_the_local_file() {
        // ⭐ ISSUE #24. The write is on the store, not in `/tmp`. Deleting
        // journal.jsonl — a cold start, a new container — must not lose it.
        let store: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let root = tmp();
        let cfg = {
            let mut b = FileBook::open_with(&root, Some(store.clone())).unwrap();
            let d = b.put(b"cfg").unwrap();
            b.set_active(&d).unwrap();
            b.append(&entry("t1", &d, &[(1, 7), (2, -7)])).unwrap();
            d
        };
        let _ = fs::remove_file(root.join("journal.jsonl"));
        let reopened = FileBook::open_with(&root, Some(store)).unwrap();
        let got = reopened.entries().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "t1");
        assert_eq!(reopened.active().unwrap(), Some(cfg));
        assert!(ratio_chart::trial_balance_ties(
            reopened.trial_balance().unwrap()
        ));
    }

    #[test]
    fn a_seeded_jsonl_is_claimed_at_its_own_slots() {
        // Hydration writes line i at sequence i. Two hydrations of the same
        // seed therefore PUT the same keys, and the second is a 412, not a
        // duplicate append.
        let root = tmp();
        let cfg = {
            let mut b = FileBook::open(&root).unwrap();
            let d = b.put(b"cfg").unwrap();
            b.set_active(&d).unwrap();
            b.append(&entry("seed", &d, &[(1, 3), (2, -3)])).unwrap();
            d
        };
        let store: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let first = FileBook::open_with(&root, Some(store.clone())).unwrap();
        assert_eq!(first.entries().unwrap().len(), 1);
        let second = FileBook::open_with(&root, Some(store)).unwrap();
        let got = second.entries().unwrap();
        assert_eq!(got.len(), 1, "a second hydrate must not duplicate the seed");
        assert_eq!(got[0].id, "seed");
        assert_eq!(got[0].config, cfg);
    }

    /// Records every `put_if_absent` attempt, including those that return
    /// false (occupied). The production defect was re-claiming seq 1..=height
    /// on every open — a 412 still costs a PutObject.
    struct CountingStore {
        inner: MemoryStore,
        puts: std::sync::Mutex<Vec<String>>,
    }

    impl CountingStore {
        fn new() -> Self {
            Self {
                inner: MemoryStore::new(),
                puts: std::sync::Mutex::new(Vec::new()),
            }
        }
        fn puts(&self) -> Vec<String> {
            self.puts.lock().expect("puts").clone()
        }
        fn clear(&self) {
            self.puts.lock().expect("puts").clear();
        }
    }

    impl ObjectStore for CountingStore {
        fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
            self.puts.lock().expect("puts").push(key.to_string());
            self.inner.put_if_absent(key, body)
        }
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            self.inner.get(key)
        }
        fn list(&self, prefix: &str) -> Result<Vec<String>> {
            self.inner.list(prefix)
        }
    }

    #[test]
    fn a_second_hydrate_against_a_full_seed_does_not_put() {
        // ⭐ THE SHORT-CIRCUIT. Height already covers the seed, so a second
        // open must not call put for any existing seq — a 412 on every line
        // is what burned the Lambda budget on ashcombe.
        let root = tmp();
        {
            let mut b = FileBook::open(&root).unwrap();
            let d = b.put(b"cfg").unwrap();
            b.set_active(&d).unwrap();
            b.append(&entry("seed", &d, &[(1, 3), (2, -3)])).unwrap();
        }
        let store = Arc::new(CountingStore::new());
        let first = FileBook::open_with(&root, Some(store.clone())).unwrap();
        assert_eq!(first.entries().unwrap().len(), 1);
        store.clear();
        let second = FileBook::open_with(&root, Some(store.clone())).unwrap();
        assert_eq!(second.entries().unwrap().len(), 1, "a second hydrate must not duplicate the seed");
        assert!(
            store.puts().is_empty(),
            "a full seed must not re-claim: {:?}",
            store.puts()
        );
    }

    #[test]
    fn a_partial_hydrate_claims_only_past_the_store_height() {
        // A timeout mid-seed leaves a contiguous prefix. The next open
        // resumes at height+1; re-Putting seq 1 is the hang.
        let root = tmp();
        fs::write(root.join("journal.jsonl"), "one\ntwo\nthree\n").unwrap();
        let store = Arc::new(CountingStore::new());
        let log = SeqLog::new(store.clone(), format!("{}/journal/", super::book_key(&root)));
        assert!(log.claim(1, b"one").unwrap());
        store.clear();
        let _ = FileBook::open_with(&root, Some(store.clone())).unwrap();
        let puts = store.puts();
        assert!(
            puts.iter().all(|k| !k.ends_with("00000000000000000001")),
            "seq 1 is already held and must not be re-Put: {puts:?}"
        );
        assert!(
            puts.iter().any(|k| k.ends_with("00000000000000000002")),
            "seq 2 must be claimed: {puts:?}"
        );
        assert!(
            puts.iter().any(|k| k.ends_with("00000000000000000003")),
            "seq 3 must be claimed: {puts:?}"
        );
        assert_eq!(log.height().unwrap(), 3);
    }

    #[test]
    fn list_entry_count_prefers_the_local_jsonl() {
        let root = tmp();
        fs::write(root.join("journal.jsonl"), "a\n\nb\n").unwrap();
        let store: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        assert_eq!(FileBook::list_entry_count(&root, Some(store)).unwrap(), 2);
    }

    #[test]
    fn list_entry_count_falls_back_to_store_height() {
        let root = tmp();
        let store: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let log = SeqLog::new(store.clone(), format!("{}/journal/", super::book_key(&root)));
        assert!(log.claim(1, b"one").unwrap());
        assert!(log.claim(2, b"two").unwrap());
        assert_eq!(FileBook::list_entry_count(&root, Some(store)).unwrap(), 2);
    }

    #[test]
    fn concurrent_object_appends_keep_both_entries() {
        // Two FileBooks on one store, as two Lambda containers under one URL.
        let store: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let dir = tmp();
        let cfg = {
            let mut seed = FileBook::open_with(&dir, Some(store.clone())).unwrap();
            let d = seed.put(b"rules = []\n").unwrap();
            seed.set_active(&d).unwrap();
            d
        };
        std::thread::scope(|s| {
            let store_a = store.clone();
            let store_b = store.clone();
            let dir_a = dir.clone();
            let dir_b = dir.clone();
            let cfg_a = cfg.clone();
            let cfg_b = cfg.clone();
            s.spawn(move || {
                let mut b = FileBook::open_with(&dir_a, Some(store_a)).unwrap();
                b.append(&entry("w1", &cfg_a, &[(1, 10), (2, -10)]))
                    .unwrap();
            });
            s.spawn(move || {
                let mut b = FileBook::open_with(&dir_b, Some(store_b)).unwrap();
                b.append(&entry("w2", &cfg_b, &[(1, 11), (2, -11)]))
                    .unwrap();
            });
        });
        let b = FileBook::open_with(&dir, Some(store)).unwrap();
        let mut ids: Vec<String> = b.entries().unwrap().into_iter().map(|e| e.id).collect();
        ids.sort();
        assert_eq!(ids, vec!["w1".to_string(), "w2".to_string()]);
        assert!(ratio_chart::trial_balance_ties(b.trial_balance().unwrap()));
    }

    #[test]
    fn a_plane_on_an_object_store_survives_dropping_the_local_file() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
        struct Rec {
            id: String,
        }
        let store: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let root = tmp();
        {
            let mut b = FileBook::open_with(&root, Some(store.clone())).unwrap();
            b.append_record(Plane::Facts, &Rec { id: "f1".into() })
                .unwrap();
        }
        let _ = fs::remove_file(root.join("facts.jsonl"));
        let b = FileBook::open_with(&root, Some(store)).unwrap();
        let got: Vec<Rec> = b.records(Plane::Facts).unwrap();
        assert_eq!(got, vec![Rec { id: "f1".into() }]);
    }

    #[test]
    fn a_dated_entry_on_or_before_a_close_is_refused() {
        // ⭐ THE DOOR. A close recorded, then a later append dated inside the
        // closed period. Append-only storage would have accepted it.
        // `Ratio.Close.a_dated_entry_on_or_before_close_is_refused`.
        let (mut b, cfg) = book();
        let mut open = entry("open", &cfg, &[(1, 500), (2, -500)]);
        open.trade_date = Some("2026-03-15".into());
        b.append(&open).unwrap();
        b.append_record(
            Plane::Closes,
            &CloseRecord {
                view: "book".into(),
                closed_date: "2026-03-31".into(),
                journal_position: 1,
                journal_digest: "d".into(),
                config_digest: cfg.as_str().to_string(),
                closing_entry: None,
                actor: "tester".into(),
                recorded: 1,
                equity_destination: 25,
                surplus: None,
            },
        )
        .unwrap();

        let mut back = entry("back", &cfg, &[(1, 100), (2, -100)]);
        back.trade_date = Some("2026-03-31".into());
        let err = b.append(&back).unwrap_err().to_string();
        assert!(err.contains("closed through"), "{err}");
        assert!(err.contains("2026-03-31"), "{err}");
        assert_eq!(b.entries().unwrap().len(), 1, "the back-date must not land");

        let mut earlier = entry("earlier", &cfg, &[(1, 50), (2, -50)]);
        earlier.trade_date = Some("2026-03-01".into());
        assert!(b.append(&earlier).is_err(), "on or before, not only before");

        let mut later = entry("later", &cfg, &[(1, 25), (2, -25)]);
        later.trade_date = Some("2026-04-01".into());
        b.append(&later).unwrap();

        let undated = entry("undated", &cfg, &[(1, 10), (2, -10)]);
        b.append(&undated).unwrap();
        assert_eq!(b.entries().unwrap().len(), 3);
    }

    #[test]
    fn a_close_record_without_a_posting_stays_unset() {
        // ⭐ MISSING CLOSE POSTING IS UNSET, NOT ZERO. A period with no
        // income or expense can still be closed; the door holds and the
        // surplus field is absent.
        let (mut b, cfg) = book();
        b.append_record(
            Plane::Closes,
            &CloseRecord {
                view: "book".into(),
                closed_date: "2026-03-31".into(),
                journal_position: 0,
                journal_digest: "empty".into(),
                config_digest: cfg.as_str().to_string(),
                closing_entry: None,
                actor: "tester".into(),
                recorded: 1,
                equity_destination: 25,
                surplus: None,
            },
        )
        .unwrap();
        let got = b.latest_close("book").unwrap().expect("the close is there");
        assert!(got.closing_entry.is_none());
        assert!(got.surplus.is_none());
        assert!(b.latest_close("settlement").unwrap().is_none());
    }
}
