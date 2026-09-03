//! The object-store seam under the journal, and the append that claims a slot.
//!
//! `tla/S3Journal.tla` is the spec: one object per entry, LIST then PUT at
//! `max + 1`, and the PUT is conditional (`If-None-Match:*`). A second writer
//! to the same slot is refused and retries. Turn the condition off and the
//! second PUT overwrites the first — the losing entry was acked and is gone,
//! and the shortened journal still ties, digests and replays. Issue #24 is
//! that fork, arriving as a truncation rather than as two `/tmp`s.
//!
//! This crate does not talk to S3. The deployed adapter lives in `ratio`
//! (which already holds the SDK for the scale runner) and is installed once
//! at process start. Tests use [`MemoryStore`] and [`DirStore`].

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{bail, Context, Result};

/// Bytes at a key, with one atomic create.
///
/// ⛔ `put_if_absent` IS THE CLAIM. Everything the sequence log proves rests
/// on it being genuinely atomic: create the key, or report that somebody
/// already has, and never overwrite. A store that implements it as "write
/// anyway" will pass a test that only checks the happy path and lose an
/// acked entry the first time two writers LIST the same height.
pub trait ObjectStore: Send + Sync {
    /// Create `key` with `body`, or report that the key is already held.
    ///
    /// Returns `true` when this caller created it. Returns `false` when the
    /// key exists — the caller has NOT acked and must retry at the next slot.
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool>;
    /// The bytes at `key`, or `None` if it is absent.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    /// Keys under `prefix`, in lex order. For the sequence log that is also
    /// numeric order, because the keys are zero-padded.
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

impl<S: ObjectStore + ?Sized> ObjectStore for Arc<S> {
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        (**self).put_if_absent(key, body)
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        (**self).get(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        (**self).list(prefix)
    }
}

/// The process-wide store, installed once by the binary when the demo is
/// wired to a durable backend. [`FileBook`][`super::FileBook`] reads it on
/// open so every caller — console, MCP, projection — sees the same journal
/// without threading a handle through every `open`.
///
/// Unset is the local shape: one JSON line per entry on disk, which is what
/// every test and every `ratio` invocation without the env uses.
static INSTALLED: OnceLock<Arc<dyn ObjectStore>> = OnceLock::new();

/// Install the durable journal backend for this process.
///
/// ⚠ FIRST CALL WINS. A second install is ignored rather than swapping the
/// store under a book that has already hydrated from the first — two stores
/// for one book is the fork this exists to close.
pub fn install_object_store(store: Arc<dyn ObjectStore>) {
    let _ = INSTALLED.set(store);
}

/// The store the process was wired to, if any.
pub fn installed_object_store() -> Option<Arc<dyn ObjectStore>> {
    INSTALLED.get().cloned()
}

/// In-memory objects. The test double, and the dial in the spec.
///
/// `conditional: true` is the production claim. `false` is `LostWrite.cfg`:
/// a PUT to an occupied key overwrites, and an acked entry vanishes.
pub struct MemoryStore {
    inner: Mutex<BTreeMap<String, Vec<u8>>>,
    /// ⛔ THE DIAL. Matching `ConditionalPut` in `S3Journal.tla`.
    conditional: bool,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(BTreeMap::new()),
            conditional: true,
        }
    }

    /// The probe: PUTs overwrite. Used only to show that a test which
    /// accepts overwrite would stay green while an acked write disappears.
    pub fn unconditional() -> Self {
        Self {
            inner: Mutex::new(BTreeMap::new()),
            conditional: false,
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectStore for MemoryStore {
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        let mut m = self.inner.lock().expect("memory store mutex");
        if self.conditional && m.contains_key(key) {
            return Ok(false);
        }
        let occupied = m.contains_key(key);
        m.insert(key.to_string(), body.to_vec());
        Ok(!occupied || !self.conditional)
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let m = self.inner.lock().expect("memory store mutex");
        Ok(m.get(key).cloned())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let m = self.inner.lock().expect("memory store mutex");
        Ok(m.keys().filter(|k| k.starts_with(prefix)).cloned().collect())
    }
}

/// One object per file, `create_new` as the claim. For `RATIO_JOURNAL_LOCAL`
/// and for tests that want a real filesystem race rather than a mutex.
pub struct DirStore {
    root: PathBuf,
}

impl DirStore {
    pub fn at(root: impl AsRef<Path>) -> Self {
        DirStore {
            root: root.as_ref().to_path_buf(),
        }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }
}

impl ObjectStore for DirStore {
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        let path = self.path(key);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(body)
                    .with_context(|| format!("writing {}", path.display()))?;
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e).with_context(|| format!("claiming {}", path.display())),
        }
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match fs::read(self.path(key)) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {key}")),
        }
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        walk_keys(&self.root, &self.root, prefix, &mut out)?;
        out.sort();
        Ok(out)
    }
}

fn walk_keys(dir: &Path, root: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
    let rd = match fs::read_dir(dir) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("listing {}", dir.display())),
    };
    for entry in rd {
        let entry = entry.with_context(|| format!("listing {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            walk_keys(&path, root, prefix, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with(prefix) {
            out.push(rel);
        }
    }
    Ok(())
}

/// One object per sequence number under a prefix: the append in the spec.
///
/// Keys are `{prefix}{seq:020}`. Zero-padded so a LIST is already in order,
/// and so "the objects present form a gapless prefix" is a statement about
/// the keys themselves, not about a side index.
#[derive(Clone)]
pub struct SeqLog {
    store: Arc<dyn ObjectStore>,
    prefix: String,
}

impl SeqLog {
    pub fn new(store: Arc<dyn ObjectStore>, prefix: impl Into<String>) -> Self {
        SeqLog {
            store,
            prefix: prefix.into(),
        }
    }

    fn key(&self, seq: u64) -> String {
        format!("{}{seq:020}", self.prefix)
    }

    fn seq_of(&self, key: &str) -> Option<u64> {
        key.strip_prefix(&self.prefix)?.parse().ok()
    }

    /// Highest sequence present, or zero on an empty log. `Height` in the spec.
    pub fn height(&self) -> Result<u64> {
        let mut max = 0u64;
        for k in self.store.list(&self.prefix)? {
            if let Some(n) = self.seq_of(&k) {
                max = max.max(n);
            }
        }
        Ok(max)
    }

    /// PUT `body` at `seq` only if that key is absent.
    ///
    /// ⛔ THIS IS `Put(w)` WITH THE DIAL ON. `false` means the slot was taken
    /// and this writer has not acked — `Reserve` again at the next height.
    pub fn claim(&self, seq: u64, body: &[u8]) -> Result<bool> {
        if seq == 0 {
            bail!("journal sequence numbers start at 1");
        }
        self.store.put_if_absent(&self.key(seq), body)
    }

    /// LIST, PUT at height+1, retry on a lost claim.
    ///
    /// The read and the write are separate steps on purpose: two callers can
    /// LIST the same height. The condition on the PUT is what makes that
    /// safe rather than a silent overwrite.
    pub fn append(&self, body: &[u8]) -> Result<u64> {
        // ⚠ BOUNDED, because a store that always reports "occupied" would
        // otherwise spin. A working store makes progress on every retry: the
        // winner filled the slot, the height moved.
        for _ in 0..1_024 {
            let seq = self
                .height()?
                .checked_add(1)
                .context("the journal sequence does not fit in 64 bits")?;
            if self.claim(seq, body)? {
                return Ok(seq);
            }
        }
        bail!("could not claim a journal slot — every candidate was occupied")
    }

    pub fn get(&self, seq: u64) -> Result<Option<Vec<u8>>> {
        self.store.get(&self.key(seq))
    }

    /// Every object after `after`, in sequence order. Returns the height.
    ///
    /// ⛔ A HOLE IS A REFUSAL. `ContiguousNoHoles` is an invariant of the
    /// spec, not a courtesy: a fold that skipped a missing key would report
    /// a figure over a prefix that never existed. The cursor is the sequence
    /// number itself, so `after` is "the last seq this reader has folded".
    pub fn for_each_since(
        &self,
        after: u64,
        f: &mut dyn FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        let height = self.height()?;
        if after > height {
            bail!(
                "the journal is {height} entries and was read to {after} — an append-only \
                 log does not shrink, so this is not the book that was read"
            );
        }
        for seq in (after + 1)..=height {
            let body = match self.get(seq)? {
                Some(b) => b,
                None => bail!(
                    "journal sequence {seq} is missing under {} (height {height}) — a gap \
                     in an append-only log is a different book, not a skip",
                    self.prefix
                ),
            };
            f(seq, &body)?;
        }
        Ok(height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_to_a_sequence_slot_is_refused() {
        // ⭐ THE CLAIM IN THE SPEC. Two writers that LIST the same height both
        // aim at slot 1; the second PUT must not succeed, and must not replace
        // the first. A `put_if_absent` that overwrites keeps this green until
        // the body check — so the body check is the test.
        let log = SeqLog::new(Arc::new(MemoryStore::new()), "journal/");
        assert!(log.claim(1, b"first").unwrap());
        assert!(
            !log.claim(1, b"second").unwrap(),
            "a second claim to the same slot must fail closed"
        );
        assert_eq!(
            log.get(1).unwrap().as_deref(),
            Some(b"first".as_ref()),
            "the first writer is still readable — NoWriteIsLost"
        );
    }

    #[test]
    fn an_unconditional_put_loses_the_losing_writer() {
        // ⛔ THE PROBE. `LostWrite.cfg` flips ConditionalPut to FALSE; this is
        // that dial on the store. Both claims "succeed", the first body is
        // gone, the journal is one object shorter than the writes that believe
        // they are in it. Kept so a store that overwrites cannot hide behind
        // a test that never looks.
        let log = SeqLog::new(Arc::new(MemoryStore::unconditional()), "journal/");
        assert!(log.claim(1, b"first").unwrap());
        assert!(
            log.claim(1, b"second").unwrap(),
            "the probe store overwrites and reports success"
        );
        assert_eq!(
            log.get(1).unwrap().as_deref(),
            Some(b"second".as_ref()),
            "the first writer was acked and is gone"
        );
        assert_eq!(log.height().unwrap(), 1, "the log is a prefix short");
    }

    #[test]
    fn two_appenders_retrying_keep_both_writes() {
        // `append` is Reserve+Put with retry. Two threads racing still land
        // two objects; if the PUT overwrote, height would be 1 and one body
        // would vanish.
        let log = SeqLog::new(Arc::new(MemoryStore::new()), "journal/");
        std::thread::scope(|s| {
            let a = log.clone();
            let b = log.clone();
            s.spawn(move || a.append(b"alpha").unwrap());
            s.spawn(move || b.append(b"beta").unwrap());
        });
        assert_eq!(log.height().unwrap(), 2);
        let mut bodies = [
            log.get(1).unwrap().expect("slot 1"),
            log.get(2).unwrap().expect("slot 2"),
        ];
        bodies.sort();
        assert_eq!(bodies, [b"alpha".to_vec(), b"beta".to_vec()]);
    }

    #[test]
    fn a_dir_store_refuses_a_second_create() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ratio-dirstore-{n}"));
        let _ = fs::remove_dir_all(&dir);
        let log = SeqLog::new(Arc::new(DirStore::at(&dir)), "fund/journal/");
        assert!(log.claim(1, b"one").unwrap());
        assert!(!log.claim(1, b"two").unwrap());
        assert_eq!(log.get(1).unwrap().as_deref(), Some(b"one".as_ref()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hole_is_refused_rather_than_skipped() {
        let store = Arc::new(MemoryStore::new());
        // Occupy slot 2 with nothing at 1 — a prefix that LIST would report
        // as height 2 with a gap. The fold must not skip it.
        store.put_if_absent("journal/00000000000000000002", b"later").unwrap();
        let log = SeqLog::new(store, "journal/");
        let err = log
            .for_each_since(0, &mut |_, _| Ok(()))
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing"), "{err}");
        assert!(err.contains("gap"), "{err}");
    }

    #[test]
    fn for_each_since_walks_a_gapless_prefix() {
        let log = SeqLog::new(Arc::new(MemoryStore::new()), "journal/");
        log.append(b"a").unwrap();
        log.append(b"b").unwrap();
        log.append(b"c").unwrap();
        let mut seen = Vec::new();
        let height = log
            .for_each_since(1, &mut |seq, body| {
                seen.push((seq, body.to_vec()));
                Ok(())
            })
            .unwrap();
        assert_eq!(height, 3);
        assert_eq!(seen, vec![(2, b"b".to_vec()), (3, b"c".to_vec())]);
    }
}
