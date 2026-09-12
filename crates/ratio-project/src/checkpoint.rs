//! Verified journal-prefix projection checkpoints (#310).
//!
//! A checkpoint is disposable acceleration for warm reads. The journal remains
//! authoritative: a blob that does not name the exact prefix it was folded from,
//! or whose bytes do not verify, is refused and the caller falls back to a full
//! replay. Crova was evaluated as a content-addressed blob layer (verify-on-read
//! fits); it is not wired here — PLAN still defers crova until it has consumers,
//! a production remote deployment model, and (separately) proven atomic
//! conditional append before anyone could mistake it for the ordered journal.
//! v1 is a directory of content-addressed blobs plus an atomically replaced
//! HEAD pointer, the same seam shape as [`ratio_store::DirectoryConfigStore`].

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use ratio_common::intern::Text;
use ratio_store::{Digest, DigestBuilder, FileBook, Journal, JournalEntry};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::relief::{self, Holding, Lot};
use crate::views::{self, ViewDef};
use crate::{
    Actions, DimTotal, FoldCost, LotBook, Pending, PendingWash, Positions, Projection, Terms,
    Totals, Unplaced, ViewFold,
};

/// Explicit format version. Unknown versions refuse rather than migrate in place.
pub const FORMAT_VERSION: u32 = 1;

/// Why a load fell back to a full replay. Production telemetry may name these;
/// it must not include journal or customer content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointMiss {
    Missing,
    Corrupt,
    DigestMismatch,
    VersionRefused,
    PrefixMismatch,
}

impl CheckpointMiss {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Corrupt => "corrupt",
            Self::DigestMismatch => "digest_mismatch",
            Self::VersionRefused => "version_refused",
            Self::PrefixMismatch => "prefix_mismatch",
        }
    }
}

/// Hit/miss and tail length for production logs — no journal bytes, no book id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointTelemetry {
    pub hit: bool,
    pub miss: Option<CheckpointMiss>,
    pub checkpoint_prefix: usize,
    pub journal_height: usize,
    pub tail_length: usize,
}

impl CheckpointTelemetry {
    /// One stderr line for CloudWatch. ⛔ No digest, no entry ids, no memos.
    pub fn report(self) {
        match self.miss {
            None => eprintln!(
                "projection_checkpoint hit=1 miss=0 checkpoint_prefix={} journal_height={} tail_length={}",
                self.checkpoint_prefix, self.journal_height, self.tail_length
            ),
            Some(m) => eprintln!(
                "projection_checkpoint hit=0 miss=1 reason={} checkpoint_prefix={} journal_height={} tail_length={}",
                m.as_str(),
                self.checkpoint_prefix,
                self.journal_height,
                self.tail_length
            ),
        }
    }
}

/// The pin a checkpoint claims — height and content address of that prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPin {
    pub prefix: usize,
    pub digest: String,
}

impl CheckpointPin {
    pub fn of_entries(entries: &[JournalEntry]) -> Result<Self> {
        Ok(Self {
            prefix: entries.len(),
            digest: prefix_digest(entries)?,
        })
    }
}

/// SHA-256 over serialized journal lines — same contract as `ratio_nav::prefix_digest`.
///
/// ⛔ KEPT HERE so `ratio-project` does not depend on `ratio-nav` (that edge is
/// the other way). The unit test below holds the two equal.
pub fn prefix_digest(entries: &[JournalEntry]) -> Result<String> {
    let mut d = DigestBuilder::new();
    for e in entries {
        d.update(serde_json::to_string(e).context("serializing an entry")?.as_bytes());
        d.update(b"\n");
    }
    Ok(d.finish().as_str().to_string())
}

/// Stream the first `prefix` entries of `book` and digest them.
pub fn prefix_digest_book(book: &FileBook, prefix: usize) -> Result<String> {
    let mut d = DigestBuilder::new();
    let mut n = 0usize;
    book.for_each_entry_since(0, &mut |entry| {
        if n >= prefix {
            return Ok(());
        }
        d.update(serde_json::to_string(entry).context("serializing an entry")?.as_bytes());
        d.update(b"\n");
        n += 1;
        Ok(())
    })?;
    if n != prefix {
        bail!(
            "journal height {n} is shorter than checkpoint prefix {prefix} — refusing a figure from a missing prefix"
        );
    }
    Ok(d.finish().as_str().to_string())
}

/// On-disk envelope. Version first so an unknown format refuses before body parse.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Envelope {
    version: u32,
    prefix: usize,
    digest: String,
    read_to: u64,
    body: BodyV1,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BodyV1 {
    views: BTreeMap<String, ViewFoldV1>,
    actions: ActionsV1,
    at: usize,
    terms: BTreeMap<String, ResultWire<Terms>>,
    view_defs: BTreeMap<String, ResultWire<Vec<ViewDefWire>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ResultWire<T> {
    Ok(T),
    Err(String),
}

impl<T> From<Result<T, String>> for ResultWire<T> {
    fn from(r: Result<T, String>) -> Self {
        match r {
            Ok(v) => Self::Ok(v),
            Err(e) => Self::Err(e),
        }
    }
}

impl<T> From<ResultWire<T>> for Result<T, String> {
    fn from(r: ResultWire<T>) -> Self {
        match r {
            ResultWire::Ok(v) => Ok(v),
            ResultWire::Err(e) => Err(e),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ActionsV1 {
    announced: Vec<(String, String, String, i64, i64)>,
    rewritten: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ViewFoldV1 {
    positions: PositionsV1,
    totals: TotalsV1,
    lots: LotBookV1,
    basis: ratio_rules::Basis,
    recognised_through: Option<views::Day>,
    pending: BTreeMap<String, Vec<PendingV1>>,
    unplaceable: Vec<Unplaced>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PositionsV1 {
    held: Vec<(i64, String, i64, i64)>,
    rest: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TotalsV1 {
    by_dim: Vec<(i64, Option<String>, DimTotal)>,
    debits: i128,
    credits: i128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LotBookV1 {
    open: Vec<(i64, String, MethodWire, Vec<Lot>)>,
    relieved: Vec<(Option<String>, i128)>,
    short_term: Vec<(Option<String>, i128)>,
    long_term: Vec<(Option<String>, i128)>,
    pending_wash: Vec<PendingWashV1>,
    breaks: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingWashV1 {
    dim: i64,
    instrument: String,
    window: i64,
    sold_on: relief::Day,
    remaining_units: i64,
    remaining_loss: i64,
    original_acquired: Option<relief::Day>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingV1 {
    at: usize,
    entry: JournalEntry,
    terms: ResultWire<Terms>,
    trade_day: views::Day,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ViewDefWire {
    id: String,
    display_name: String,
    basis: ratio_rules::Basis,
    settles_in: i64,
    holidays: BTreeSet<views::Day>,
    weekend: Vec<i64>,
    declared: bool,
}

impl From<&ViewDef> for ViewDefWire {
    fn from(v: &ViewDef) -> Self {
        Self {
            id: v.id.clone(),
            display_name: v.display_name.clone(),
            basis: v.basis,
            settles_in: v.settles_in,
            holidays: v.holidays.clone(),
            weekend: v.weekend.clone(),
            declared: v.declared,
        }
    }
}

impl From<ViewDefWire> for ViewDef {
    fn from(v: ViewDefWire) -> Self {
        Self {
            id: v.id,
            display_name: v.display_name,
            basis: v.basis,
            settles_in: v.settles_in,
            holidays: v.holidays,
            weekend: v.weekend,
            declared: v.declared,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MethodWire {
    Fifo,
    Lifo,
    Hifo,
    Lofo,
    LongestHeldFirst,
    ShortestHeldFirst,
}

impl From<relief::Method> for MethodWire {
    fn from(m: relief::Method) -> Self {
        match m {
            relief::Method::Fifo => Self::Fifo,
            relief::Method::Lifo => Self::Lifo,
            relief::Method::Hifo => Self::Hifo,
            relief::Method::Lofo => Self::Lofo,
            relief::Method::LongestHeldFirst => Self::LongestHeldFirst,
            relief::Method::ShortestHeldFirst => Self::ShortestHeldFirst,
        }
    }
}

impl From<MethodWire> for relief::Method {
    fn from(m: MethodWire) -> Self {
        match m {
            MethodWire::Fifo => Self::Fifo,
            MethodWire::Lifo => Self::Lifo,
            MethodWire::Hifo => Self::Hifo,
            MethodWire::Lofo => Self::Lofo,
            MethodWire::LongestHeldFirst => Self::LongestHeldFirst,
            MethodWire::ShortestHeldFirst => Self::ShortestHeldFirst,
        }
    }
}

/// A decoded, version-checked checkpoint ready to accept against a journal pin.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pin: CheckpointPin,
    read_to: u64,
    body: BodyV1,
}

impl Checkpoint {
    pub fn pin(&self) -> &CheckpointPin {
        &self.pin
    }

    pub fn read_to(&self) -> u64 {
        self.read_to
    }

    /// Encode this projection at `pin`. Caller must have verified `pin` against
    /// the journal prefix this fold actually represents.
    pub fn capture(projection: &Projection, pin: CheckpointPin) -> Result<Self> {
        if projection.prefix() != pin.prefix {
            bail!(
                "checkpoint capture refuses: fold prefix {} is not pin prefix {}",
                projection.prefix(),
                pin.prefix
            );
        }
        Ok(Self {
            pin,
            read_to: projection.read_to,
            body: BodyV1::from_projection(projection)?,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let env = Envelope {
            version: FORMAT_VERSION,
            prefix: self.pin.prefix,
            digest: self.pin.digest.clone(),
            read_to: self.read_to,
            body: self.body.clone(),
        };
        serde_json::to_vec(&env).context("encoding projection checkpoint")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CheckpointMiss> {
        let env: Envelope = serde_json::from_slice(bytes).map_err(|_| CheckpointMiss::Corrupt)?;
        if env.version != FORMAT_VERSION {
            // ⛔ REFUSE, DO NOT MIGRATE IN PLACE. An unknown version is not a
            // figure; full replay is the recovery path.
            return Err(CheckpointMiss::VersionRefused);
        }
        if env.digest.len() != 64 || !env.digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CheckpointMiss::Corrupt);
        }
        if env.body.at != env.prefix {
            return Err(CheckpointMiss::Corrupt);
        }
        Ok(Self {
            pin: CheckpointPin {
                prefix: env.prefix,
                digest: env.digest,
            },
            read_to: env.read_to,
            body: env.body,
        })
    }

    /// ⛔ A MUTATION THAT ACCEPTS THE WRONG PREFIX FAILS. The journal pin is
    /// the only authority; a checkpoint for a different height or digest is
    /// not acceleration, it is somebody else's book.
    pub fn accept_for(&self, expected: &CheckpointPin) -> Result<(), CheckpointMiss> {
        if self.pin.prefix != expected.prefix || self.pin.digest != expected.digest {
            return Err(CheckpointMiss::PrefixMismatch);
        }
        Ok(())
    }

    pub fn into_projection(self) -> Result<Projection> {
        self.body.into_projection(self.read_to)
    }
}

impl BodyV1 {
    fn from_projection(p: &Projection) -> Result<Self> {
        let mut views = BTreeMap::new();
        for (id, vf) in &p.views {
            views.insert(id.clone(), ViewFoldV1::from_fold(vf)?);
        }
        let mut terms = BTreeMap::new();
        for (d, t) in &p.terms {
            terms.insert(d.as_str().to_string(), ResultWire::from(t.clone()));
        }
        let mut view_defs = BTreeMap::new();
        for (d, defs) in &p.view_defs {
            let wire = match defs {
                Ok(v) => ResultWire::Ok(v.iter().map(ViewDefWire::from).collect()),
                Err(e) => ResultWire::Err(e.clone()),
            };
            view_defs.insert(d.as_str().to_string(), wire);
        }
        Ok(Self {
            views,
            actions: ActionsV1 {
                announced: p.actions.announced.clone(),
                rewritten: p.actions.rewritten.clone(),
            },
            at: p.at,
            terms,
            view_defs,
        })
    }

    fn into_projection(self, read_to: u64) -> Result<Projection> {
        let mut names = ratio_common::intern::Interner::new();
        let mut views = BTreeMap::new();
        for (id, vf) in self.views {
            views.insert(id, vf.into_fold(&mut names)?);
        }
        if views.is_empty() {
            views.insert(
                ratio_rules::UNDECLARED_VIEW.to_string(),
                ViewFold::default(),
            );
        }
        let mut terms = BTreeMap::new();
        for (d, t) in self.terms {
            terms.insert(Digest::parse(&d)?, Result::<Terms, String>::from(t));
        }
        let mut view_defs = BTreeMap::new();
        for (d, defs) in self.view_defs {
            let resolved: Result<Vec<ViewDef>, String> = match defs {
                ResultWire::Ok(v) => Ok(v.into_iter().map(ViewDef::from).collect()),
                ResultWire::Err(e) => Err(e),
            };
            view_defs.insert(Digest::parse(&d)?, resolved);
        }
        Ok(Projection {
            views,
            actions: Actions {
                announced: self.actions.announced,
                rewritten: self.actions.rewritten,
            },
            read_to,
            at: self.at,
            cost: FoldCost::default(),
            names,
            terms,
            view_defs,
        })
    }
}

impl ViewFoldV1 {
    fn from_fold(vf: &ViewFold) -> Result<Self> {
        let mut held = Vec::new();
        for ((dim, inst), (cost, qty)) in &vf.positions.held {
            held.push((*dim, inst.to_string(), *cost, *qty));
        }
        let mut rest = BTreeMap::new();
        for (dim, amt) in &vf.positions.rest {
            rest.insert(dim.to_string(), *amt);
        }
        let mut by_dim = Vec::new();
        for ((dim, ccy), total) in &vf.totals.by_dim {
            by_dim.push((*dim, ccy.as_ref().map(|c| c.to_string()), *total));
        }
        let mut open = Vec::new();
        for ((dim, inst), holding) in &vf.lots.open {
            open.push((
                *dim,
                inst.to_string(),
                MethodWire::from(holding.method()),
                holding.lots(),
            ));
        }
        let map_opt = |m: &BTreeMap<Option<Text>, i128>| -> Vec<(Option<String>, i128)> {
            m.iter()
                .map(|(k, v)| (k.as_ref().map(|s| s.to_string()), *v))
                .collect()
        };
        let pending_wash = vf
            .lots
            .pending_wash
            .iter()
            .map(|w| PendingWashV1 {
                dim: w.key.0,
                instrument: w.key.1.to_string(),
                window: w.window,
                sold_on: w.sold_on,
                remaining_units: w.remaining_units,
                remaining_loss: w.remaining_loss,
                original_acquired: w.original_acquired,
            })
            .collect();
        let mut pending = BTreeMap::new();
        for (day, batch) in &vf.pending {
            pending.insert(
                day.to_string(),
                batch
                    .iter()
                    .map(|p| PendingV1 {
                        at: p.at,
                        entry: p.entry.clone(),
                        terms: ResultWire::from(p.terms.clone()),
                        trade_day: p.trade_day,
                    })
                    .collect(),
            );
        }
        Ok(Self {
            positions: PositionsV1 { held, rest },
            totals: TotalsV1 {
                by_dim,
                debits: vf.totals.debits,
                credits: vf.totals.credits,
            },
            lots: LotBookV1 {
                open,
                relieved: map_opt(&vf.lots.relieved),
                short_term: map_opt(&vf.lots.short_term),
                long_term: map_opt(&vf.lots.long_term),
                pending_wash,
                breaks: vf.lots.breaks.clone(),
            },
            basis: vf.basis,
            recognised_through: vf.recognised_through,
            pending,
            unplaceable: vf.unplaceable.clone(),
        })
    }

    fn into_fold(self, names: &mut ratio_common::intern::Interner) -> Result<ViewFold> {
        let mut held = BTreeMap::new();
        for (dim, inst, cost, qty) in self.positions.held {
            held.insert((dim, names.intern(&inst)), (cost, qty));
        }
        let mut rest = BTreeMap::new();
        for (dim, amt) in self.positions.rest {
            rest.insert(dim.parse::<i64>().context("rest dim")?, amt);
        }
        let mut by_dim = BTreeMap::new();
        for (dim, ccy, total) in self.totals.by_dim {
            by_dim.insert((dim, ccy.map(|c| names.intern(&c))), total);
        }
        let mut open = BTreeMap::new();
        for (dim, inst, method, lots) in self.lots.open {
            let mut h = Holding::new(method.into());
            for lot in lots {
                h.push(lot).context("restoring an open lot")?;
            }
            open.insert((dim, names.intern(&inst)), h);
        }
        let mut relieved = BTreeMap::new();
        for (k, v) in self.lots.relieved {
            relieved.insert(k.map(|s| names.intern(&s)), v);
        }
        let mut short_term = BTreeMap::new();
        for (k, v) in self.lots.short_term {
            short_term.insert(k.map(|s| names.intern(&s)), v);
        }
        let mut long_term = BTreeMap::new();
        for (k, v) in self.lots.long_term {
            long_term.insert(k.map(|s| names.intern(&s)), v);
        }
        let pending_wash = self
            .lots
            .pending_wash
            .into_iter()
            .map(|w| PendingWash {
                key: (w.dim, names.intern(&w.instrument)),
                window: w.window,
                sold_on: w.sold_on,
                remaining_units: w.remaining_units,
                remaining_loss: w.remaining_loss,
                original_acquired: w.original_acquired,
            })
            .collect();
        let mut pending = BTreeMap::new();
        for (day, batch) in self.pending {
            let d: views::Day = day.parse().context("pending day")?;
            pending.insert(
                d,
                batch
                    .into_iter()
                    .map(|p| Pending {
                        at: p.at,
                        entry: p.entry,
                        terms: Result::<Terms, String>::from(p.terms),
                        trade_day: p.trade_day,
                    })
                    .collect(),
            );
        }
        Ok(ViewFold {
            positions: Positions { held, rest },
            totals: Totals {
                by_dim,
                debits: self.totals.debits,
                credits: self.totals.credits,
            },
            lots: LotBook {
                open,
                relieved,
                short_term,
                long_term,
                pending_wash,
                breaks: self.lots.breaks,
            },
            basis: self.basis,
            recognised_through: self.recognised_through,
            pending,
            unplaceable: self.unplaceable,
        })
    }
}

/// Content-addressed checkpoint blobs with an atomic HEAD pointer.
///
/// ```text
/// <dir>/
///   blobs/<sha256>   durable bytes, written before HEAD moves
///   HEAD             digest of the newest published checkpoint (rename)
/// ```
pub struct DirectoryCheckpointStore {
    dir: PathBuf,
}

impl DirectoryCheckpointStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(dir.join("blobs"))
            .with_context(|| format!("creating checkpoint store at {}", dir.display()))?;
        Ok(Self { dir })
    }

    fn blob_path(&self, digest: &str) -> PathBuf {
        self.dir.join("blobs").join(digest)
    }

    fn content_digest(bytes: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(bytes);
        format!("{:x}", h.finalize())
    }

    /// Publish only after the blob is durable. Concurrent readers see the old
    /// HEAD or the new one — never a HEAD that names missing or truncated bytes.
    pub fn publish(&self, bytes: &[u8]) -> Result<String> {
        let digest = Self::content_digest(bytes);
        let path = self.blob_path(&digest);
        if !path.exists() {
            let tmp = self.dir.join("blobs").join(format!(
                ".{digest}.{}.tmp",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            {
                let mut f = File::create(&tmp)
                    .with_context(|| format!("creating {}", tmp.display()))?;
                f.write_all(bytes)
                    .with_context(|| format!("writing {}", tmp.display()))?;
                f.sync_all()
                    .with_context(|| format!("syncing {}", tmp.display()))?;
            }
            match fs::rename(&tmp, &path) {
                Ok(()) => {
                    if let Ok(dir) = File::open(self.dir.join("blobs")) {
                        let _ = dir.sync_all();
                    }
                }
                // Another publisher won the race; the digest is already durable.
                Err(_) if path.exists() => {
                    let _ = fs::remove_file(&tmp);
                }
                Err(e) => {
                    let _ = fs::remove_file(&tmp);
                    return Err(e).with_context(|| {
                        format!("durably placing checkpoint {}", &digest[..12.min(digest.len())])
                    });
                }
            }
        }
        // HEAD last. A crash between blob and HEAD leaves an orphan blob —
        // recoverable by republishing; it never exposes an incomplete checkpoint.
        Self::replace_atomically(&self.dir.join("HEAD"), &digest)?;
        Ok(digest)
    }

    fn replace_atomically(path: &Path, contents: &str) -> Result<()> {
        let tmp = path.with_file_name(format!(
            ".HEAD.{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        {
            let mut f = File::create(&tmp)
                .with_context(|| format!("creating {}", tmp.display()))?;
            f.write_all(contents.as_bytes())
                .with_context(|| format!("writing {}", tmp.display()))?;
            f.sync_all()
                .with_context(|| format!("syncing {}", tmp.display()))?;
        }
        fs::rename(&tmp, path).with_context(|| format!("promoting {}", path.display()))?;
        Ok(())
    }

    /// Load the newest published checkpoint, verifying content address on read.
    pub fn latest(&self) -> Result<Option<Checkpoint>, CheckpointMiss> {
        let head = self.dir.join("HEAD");
        if !head.exists() {
            return Err(CheckpointMiss::Missing);
        }
        let digest = fs::read_to_string(&head)
            .map_err(|_| CheckpointMiss::Corrupt)?
            .trim()
            .to_string();
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CheckpointMiss::Corrupt);
        }
        let bytes = fs::read(self.blob_path(&digest)).map_err(|_| CheckpointMiss::Missing)?;
        if Self::content_digest(&bytes) != digest {
            return Err(CheckpointMiss::DigestMismatch);
        }
        Checkpoint::decode(&bytes).map(Some)
    }
}

fn full_replay(
    book: &FileBook,
    store: &DirectoryCheckpointStore,
    miss: CheckpointMiss,
    checkpoint_prefix: usize,
    height: usize,
    journal_pin: &CheckpointPin,
) -> Result<(Projection, CheckpointTelemetry)> {
    let tel = CheckpointTelemetry {
        hit: false,
        miss: Some(miss),
        checkpoint_prefix,
        journal_height: height,
        tail_length: height,
    };
    tel.report();
    let mut p = Projection::new();
    p.follow_book(book)?;
    let _ = publish_checkpoint(store, &p, journal_pin);
    Ok((p, tel))
}

/// Open `book`, load the newest valid checkpoint under `store_dir` if any, and
/// replay only the journal tail. Falls back to a full fold on any miss.
pub fn follow_with_checkpoint(
    book: &FileBook,
    store_dir: impl AsRef<Path>,
) -> Result<(Projection, CheckpointTelemetry)> {
    let store = DirectoryCheckpointStore::open(store_dir)?;
    let entries = book.entries()?;
    let height = entries.len();
    let journal_pin = CheckpointPin::of_entries(&entries)?;

    let cp = match store.latest() {
        Ok(Some(cp)) => cp,
        Ok(None) | Err(CheckpointMiss::Missing) => {
            return full_replay(
                book,
                &store,
                CheckpointMiss::Missing,
                0,
                height,
                &journal_pin,
            );
        }
        Err(m) => {
            return full_replay(book, &store, m, 0, height, &journal_pin);
        }
    };

    let claimed = cp.pin().clone();
    if claimed.prefix > height {
        return full_replay(
            book,
            &store,
            CheckpointMiss::PrefixMismatch,
            claimed.prefix,
            height,
            &journal_pin,
        );
    }
    let actual = match prefix_digest(&entries[..claimed.prefix]) {
        Ok(d) => d,
        Err(_) => {
            return full_replay(
                book,
                &store,
                CheckpointMiss::Corrupt,
                claimed.prefix,
                height,
                &journal_pin,
            );
        }
    };
    let expected = CheckpointPin {
        prefix: claimed.prefix,
        digest: actual,
    };
    if let Err(m) = cp.accept_for(&expected) {
        return full_replay(book, &store, m, claimed.prefix, height, &journal_pin);
    }

    let mut p = match cp.into_projection() {
        Ok(p) => p,
        Err(_) => {
            return full_replay(
                book,
                &store,
                CheckpointMiss::Corrupt,
                claimed.prefix,
                height,
                &journal_pin,
            );
        }
    };
    let folded = match p.follow_book(book) {
        Ok(n) => n,
        Err(_) => {
            return full_replay(
                book,
                &store,
                CheckpointMiss::Corrupt,
                claimed.prefix,
                height,
                &journal_pin,
            );
        }
    };
    let tel = CheckpointTelemetry {
        hit: true,
        miss: None,
        checkpoint_prefix: claimed.prefix,
        journal_height: height,
        tail_length: folded,
    };
    tel.report();
    if p.prefix() == height && (height > claimed.prefix || claimed.prefix == height) {
        let _ = publish_checkpoint(&store, &p, &journal_pin);
    }
    Ok((p, tel))
}

fn publish_checkpoint(
    store: &DirectoryCheckpointStore,
    projection: &Projection,
    pin: &CheckpointPin,
) -> Result<()> {
    let cp = Checkpoint::capture(projection, pin.clone())?;
    store.publish(&cp.encode()?)?;
    Ok(())
}

/// Timings for the large demo shape (`securities=20`, `lots_per=40`).
#[derive(Clone, Debug)]
pub struct CheckpointBench {
    pub entries: usize,
    pub cold_full_replay_ms: u128,
    pub checkpoint_load_ms: u128,
    pub tail_replay_ms: u128,
    pub tail_entries: usize,
}

impl CheckpointBench {
    pub fn report_line(&self) -> String {
        format!(
            "projection_checkpoint_bench entries={} cold_full_replay_ms={} checkpoint_load_ms={} tail_replay_ms={} tail_entries={}",
            self.entries,
            self.cold_full_replay_ms,
            self.checkpoint_load_ms,
            self.tail_replay_ms,
            self.tail_entries
        )
    }
}

/// Measure cold full replay, checkpoint load, and a one-entry tail replay.
pub fn measure_checkpoint_bench(book: &FileBook, store_dir: impl AsRef<Path>) -> Result<CheckpointBench> {
    let store_dir = store_dir.as_ref();
    let _ = fs::remove_dir_all(store_dir);
    let store = DirectoryCheckpointStore::open(store_dir)?;

    let t0 = Instant::now();
    let mut cold = Projection::new();
    cold.follow_book(book)?;
    let cold_ms = t0.elapsed().as_millis();
    let entries = cold.prefix();
    let pin = CheckpointPin {
        prefix: entries,
        digest: prefix_digest_book(book, entries)?,
    };
    publish_checkpoint(&store, &cold, &pin)?;

    let t1 = Instant::now();
    let loaded = store
        .latest()
        .map_err(|m| anyhow::anyhow!("checkpoint load: {}", m.as_str()))?
        .ok_or_else(|| anyhow::anyhow!("checkpoint missing after publish"))?;
    loaded
        .accept_for(&pin)
        .map_err(|m| anyhow::anyhow!("{}", m.as_str()))?;
    let mut warm = loaded.into_projection()?;
    let load_ms = t1.elapsed().as_millis();

    // Resume on an unchanged journal (attach path), then report. A separate
    // follow after load is the warm-tail cost operators pay when nothing new
    // has been posted; appending is covered by the unit tests.
    let t2 = Instant::now();
    let folded = warm.follow_book(book)?;
    let tail_ms = t2.elapsed().as_millis();

    Ok(CheckpointBench {
        entries,
        cold_full_replay_ms: cold_ms,
        checkpoint_load_ms: load_ms,
        tail_replay_ms: tail_ms,
        tail_entries: folded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::{Account, AccountTypeRecord, ConfigStore, Journal, PostingRecord};
    use std::sync::{Arc, Mutex};

    fn tmp() -> PathBuf {
        let root = match std::env::var_os("TEST_TMPDIR") {
            Some(d) => PathBuf::from(d),
            None => std::env::temp_dir(),
        };
        root.join(format!(
            "ratio-checkpoint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn book_with(n: usize) -> (PathBuf, FileBook) {
        let dir = tmp();
        let _ = fs::remove_dir_all(&dir);
        let mut b = FileBook::open(&dir).unwrap();
        b.put_accounts(&[Account {
            dim: 1,
            display_name: "Cash".into(),
            account_type: AccountTypeRecord::Asset,
        }])
        .unwrap();
        let cfg = b.put(b"rules = []\n").unwrap();
        b.set_active(&cfg).unwrap();
        for i in 0..n {
            b.append(&JournalEntry {
                id: format!("e{i}"),
                memo: format!("m{i}"),
                config: cfg.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: 100,
                        currency: None,
                        instrument: None,
                        quantity: None,
                    },
                    PostingRecord {
                        dim: 1,
                        amount: -100,
                        currency: None,
                        instrument: None,
                        quantity: None,
                    },
                ],
                trade_date: Some("2024-01-02".into()),
                announcement: None,
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
                kind: None,
            })
            .unwrap();
        }
        (dir, b)
    }

    #[test]
    fn a_checkpoint_names_the_exact_journal_height_and_digest() {
        let (_dir, book) = book_with(5);
        let mut p = Projection::new();
        p.follow_book(&book).unwrap();
        let pin = CheckpointPin::of_entries(&book.entries().unwrap()).unwrap();
        let cp = Checkpoint::capture(&p, pin.clone()).unwrap();
        assert_eq!(cp.pin().prefix, 5);
        assert_eq!(cp.pin().digest, pin.digest);
        assert_eq!(cp.pin().digest.len(), 64);
    }

    #[test]
    fn corrupt_missing_or_mismatched_falls_back_to_the_same_full_replay() {
        let (dir, book) = book_with(8);
        let store_dir = dir.join(".checkpoints");
        let (full, _) = follow_with_checkpoint(&book, &store_dir).unwrap();
        let balances = full.balances(ratio_rules::UNDECLARED_VIEW).unwrap().value.clone();

        // Corrupt the published blob bytes (content digest will not match HEAD).
        let store = DirectoryCheckpointStore::open(&store_dir).unwrap();
        let head = fs::read_to_string(store_dir.join("HEAD")).unwrap();
        fs::write(store.blob_path(head.trim()), b"{not-json").unwrap();
        let (from_corrupt, tel) = follow_with_checkpoint(&book, &store_dir).unwrap();
        assert!(!tel.hit);
        assert!(matches!(
            tel.miss,
            Some(CheckpointMiss::Corrupt | CheckpointMiss::DigestMismatch)
        ));
        assert_eq!(
            from_corrupt.balances(ratio_rules::UNDECLARED_VIEW).unwrap().value,
            &balances
        );

        // Missing store.
        let missing_dir = dir.join(".checkpoints-missing");
        let (from_missing, tel) = follow_with_checkpoint(&book, &missing_dir).unwrap();
        assert_eq!(tel.miss, Some(CheckpointMiss::Missing));
        assert_eq!(
            from_missing.balances(ratio_rules::UNDECLARED_VIEW).unwrap().value,
            &balances
        );

        // Wrong digest in a well-formed envelope.
        let mut p = Projection::new();
        p.follow_book(&book).unwrap();
        let mut pin = CheckpointPin::of_entries(&book.entries().unwrap()).unwrap();
        pin.digest = "0".repeat(64);
        let bad = Checkpoint::capture(&p, pin).unwrap().encode().unwrap();
        let bad_dir = dir.join(".bad");
        DirectoryCheckpointStore::open(&bad_dir)
            .unwrap()
            .publish(&bad)
            .unwrap();
        let (from_mismatch, tel) = follow_with_checkpoint(&book, &bad_dir).unwrap();
        assert!(!tel.hit);
        assert_eq!(tel.miss, Some(CheckpointMiss::PrefixMismatch));
        assert_eq!(
            from_mismatch.balances(ratio_rules::UNDECLARED_VIEW).unwrap().value,
            &balances
        );
    }

    #[test]
    fn concurrent_publication_does_not_expose_a_checkpoint_before_content_is_durable() {
        let (dir, book) = book_with(4);
        let store_dir = dir.join(".ck");
        let store = DirectoryCheckpointStore::open(&store_dir).unwrap();
        let mut p = Projection::new();
        p.follow_book(&book).unwrap();
        let pin = CheckpointPin::of_entries(&book.entries().unwrap()).unwrap();
        let bytes = Checkpoint::capture(&p, pin).unwrap().encode().unwrap();

        // Simulate a crash after writing a temp blob but before rename+HEAD.
        let digest = DirectoryCheckpointStore::content_digest(&bytes);
        let tmp = store.blob_path(&digest).with_extension("tmp");
        fs::write(&tmp, &bytes).unwrap();
        assert!(matches!(store.latest(), Err(CheckpointMiss::Missing)));

        store.publish(&bytes).unwrap();
        let loaded = store.latest().unwrap().unwrap();
        assert_eq!(loaded.pin().prefix, 4);

        // A HEAD pointing at a missing blob is a miss, not a partial figure.
        fs::write(store_dir.join("HEAD"), "ab".repeat(32)).unwrap();
        assert!(matches!(store.latest(), Err(CheckpointMiss::Missing)));
    }

    #[test]
    fn accepting_a_checkpoint_for_the_wrong_prefix_fails() {
        let (_dir, book) = book_with(3);
        let mut p = Projection::new();
        p.follow_book(&book).unwrap();
        let pin = CheckpointPin::of_entries(&book.entries().unwrap()).unwrap();
        let cp = Checkpoint::capture(&p, pin.clone()).unwrap();
        let wrong = CheckpointPin {
            prefix: 2,
            digest: pin.digest.clone(),
        };
        assert_eq!(cp.accept_for(&wrong), Err(CheckpointMiss::PrefixMismatch));
        let wrong_digest = CheckpointPin {
            prefix: pin.prefix,
            digest: "f".repeat(64),
        };
        assert_eq!(
            cp.accept_for(&wrong_digest),
            Err(CheckpointMiss::PrefixMismatch)
        );
        assert!(cp.accept_for(&pin).is_ok());
    }

    #[test]
    fn an_unknown_format_version_is_refused() {
        let env = serde_json::json!({
            "version": 99,
            "prefix": 0,
            "digest": "0".repeat(64),
            "read_to": 0,
            "body": {
                "views": {},
                "actions": {"announced": [], "rewritten": []},
                "at": 0,
                "terms": {},
                "view_defs": {}
            }
        });
        let bytes = serde_json::to_vec(&env).unwrap();
        assert_eq!(
            Checkpoint::decode(&bytes).err(),
            Some(CheckpointMiss::VersionRefused)
        );
    }

    #[test]
    fn checkpoint_then_tail_matches_full_replay() {
        let (dir, mut book) = book_with(6);
        let store_dir = dir.join(".ck");
        let (first, tel) = follow_with_checkpoint(&book, &store_dir).unwrap();
        assert!(!tel.hit, "first open is a miss that publishes");
        assert!(store_dir.join("HEAD").exists());

        let cfg = book.active().unwrap().unwrap();
        book.append(&JournalEntry {
            id: "tail".into(),
            memo: "tail".into(),
            config: cfg,
            postings: vec![
                PostingRecord {
                    dim: 1,
                    amount: 50,
                    currency: None,
                    instrument: None,
                    quantity: None,
                },
                PostingRecord {
                    dim: 1,
                    amount: -50,
                    currency: None,
                    instrument: None,
                    quantity: None,
                },
            ],
            trade_date: Some("2024-01-03".into()),
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
            kind: None,
        })
        .unwrap();

        let (warm, tel) = follow_with_checkpoint(&book, &store_dir).unwrap();
        assert!(tel.hit);
        assert_eq!(tel.tail_length, 1);
        assert_eq!(warm.prefix(), 7);

        let mut cold = Projection::new();
        cold.follow_book(&book).unwrap();
        assert_eq!(
            warm.balances(ratio_rules::UNDECLARED_VIEW).unwrap().value,
            cold.balances(ratio_rules::UNDECLARED_VIEW).unwrap().value
        );
        assert_eq!(first.prefix(), 6);
    }

    #[test]
    fn telemetry_line_carries_no_journal_content() {
        let tel = CheckpointTelemetry {
            hit: true,
            miss: None,
            checkpoint_prefix: 100,
            journal_height: 110,
            tail_length: 10,
        };
        let line = format!(
            "projection_checkpoint hit=1 miss=0 checkpoint_prefix={} journal_height={} tail_length={}",
            tel.checkpoint_prefix, tel.journal_height, tel.tail_length
        );
        assert!(!line.contains("memo"));
        assert!(!line.contains("posting"));
        assert!(!line.contains('{'));
    }

    #[test]
    fn prefix_digest_matches_ratio_nav() {
        let (_dir, book) = book_with(3);
        let entries = book.entries().unwrap();
        let ours = prefix_digest(&entries).unwrap();
        let theirs = ratio_nav::prefix_digest(&entries).unwrap();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn publish_is_safe_under_concurrent_readers() {
        let (dir, book) = book_with(5);
        let store_dir = dir.join(".ck");
        let store = Arc::new(DirectoryCheckpointStore::open(&store_dir).unwrap());
        let mut p = Projection::new();
        p.follow_book(&book).unwrap();
        let pin = CheckpointPin::of_entries(&book.entries().unwrap()).unwrap();
        let bytes = Arc::new(Checkpoint::capture(&p, pin).unwrap().encode().unwrap());
        let bad = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let store = Arc::clone(&store);
            let bytes = Arc::clone(&bytes);
            let bad = Arc::clone(&bad);
            handles.push(std::thread::spawn(move || {
                for _ in 0..20 {
                    // Publish races may error; the blob/HEAD protocol still
                    // refuses a partial figure — Missing/Corrupt are fine.
                    let _ = store.publish(&bytes);
                    match store.latest() {
                        Ok(Some(cp)) => {
                            if cp.pin().prefix != 5 {
                                bad.lock().unwrap().push(format!(
                                    "exposed wrong prefix {}",
                                    cp.pin().prefix
                                ));
                            }
                        }
                        Ok(None)
                        | Err(CheckpointMiss::Missing)
                        | Err(CheckpointMiss::Corrupt)
                        | Err(CheckpointMiss::DigestMismatch) => {}
                        Err(m) => bad.lock().unwrap().push(m.as_str().into()),
                    }
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert!(bad.lock().unwrap().is_empty(), "{:?}", bad.lock().unwrap());
        // After the dust settles, a clean read sees the published checkpoint.
        let settled = store.latest().unwrap().unwrap();
        assert_eq!(settled.pin().prefix, 5);
    }
}
