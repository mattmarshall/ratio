//! Durable chart/configuration state for unpublished FileBooks (#363).
//! See FileBookState.tla: claim the exact successor, never retry stale state.
use crate::{Account, Digest, ObjectStore, SeqLog};
use anyhow::{ensure, Context, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{FileBookCachePin, FileBookState};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

pub const CACHE_PIN: &str = ".filebook-cache.pb";
static TEMP: AtomicU64 = AtomicU64::new(0);
#[derive(Clone)]
struct Version {
    state: FileBookState,
    digest: Digest,
}
pub(crate) struct StateStore {
    objects: Arc<dyn ObjectStore>,
    book: String,
}
impl StateStore {
    pub(crate) fn new(objects: Arc<dyn ObjectStore>, book: &str) -> Result<Self> {
        ensure!(
            !book.is_empty()
                && book != "."
                && book != ".."
                && !book.contains(['/', '\\'])
                && !book.chars().any(char::is_control),
            "invalid FileBook state identity"
        );
        Ok(Self {
            objects,
            book: book.into(),
        })
    }
    fn log(&self) -> SeqLog {
        SeqLog::new(
            self.objects.clone(),
            format!("_filebook/{}/state/", self.book),
        )
    }
    fn blob_key(&self, digest: &Digest) -> String {
        format!("_filebook/{}/config/{}", self.book, digest.as_str())
    }
    fn blob(&self, digest: &Digest) -> Result<Vec<u8>> {
        let bytes = self
            .objects
            .get(&self.blob_key(digest))?
            .with_context(|| format!("no durable FileBook configuration {}", digest.short()))?;
        ensure!(
            Digest::of(&bytes) == *digest,
            "durable FileBook configuration digest mismatch"
        );
        Ok(bytes)
    }
    fn stage(&self, bytes: &[u8]) -> Result<Digest> {
        let digest = Digest::of(bytes);
        if !self.objects.put_if_absent(&self.blob_key(&digest), bytes)? {
            ensure!(
                self.blob(&digest)? == bytes,
                "durable FileBook configuration conflicts"
            );
        }
        Ok(digest)
    }
    fn validate(&self, state: &FileBookState) -> Result<()> {
        ensure!(
            state.format_version == 1 && state.book_id == self.book,
            "unsupported or foreign FileBook state"
        );
        ensure!(
            state
                .config_digests
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "FileBook configuration addresses must be sorted and unique"
        );
        for digest in &state.config_digests {
            Digest::parse(digest)?;
        }
        for digest in state
            .promotion_history
            .iter()
            .chain((!state.active_digest.is_empty()).then_some(&state.active_digest))
        {
            ensure!(
                state.config_digests.binary_search(digest).is_ok(),
                "FileBook promotion names unstored configuration"
            );
        }
        if !state.chart.is_empty() {
            let _: Vec<Account> =
                serde_json::from_slice(&state.chart).context("invalid durable chart")?;
        }
        Ok(())
    }
    fn versions(&self) -> Result<Vec<Version>> {
        let mut versions: Vec<Version> = Vec::new();
        self.log().for_each_since(0, &mut |seq, bytes| {
            let state = FileBookState::decode(bytes).context("decoding FileBook state")?;
            self.validate(&state)?;
            ensure!(
                state.revision == i64::try_from(seq)?,
                "FileBook state revision differs from its slot"
            );
            let predecessor = versions.last().map(|v| v.digest.as_str()).unwrap_or("");
            ensure!(
                state.predecessor_digest == predecessor,
                "FileBook state predecessor mismatch"
            );
            if let Some(prior) = versions.last() {
                ensure!(
                    prior
                        .state
                        .config_digests
                        .iter()
                        .all(|d| state.config_digests.binary_search(d).is_ok()),
                    "FileBook state discarded a stored configuration"
                );
                if state.promotion_history == prior.state.promotion_history {
                    ensure!(
                        state.active_digest == prior.state.active_digest,
                        "FileBook active configuration changed without a promotion"
                    );
                } else {
                    ensure!(
                        state.promotion_history.len() == prior.state.promotion_history.len() + 1
                            && state.promotion_history[1..] == prior.state.promotion_history
                            && state.promotion_history[0] == state.active_digest,
                        "FileBook promotion history regressed or changed without its active pin"
                    );
                }
            }
            versions.push(Version {
                state,
                digest: Digest::of(bytes),
            });
            Ok(())
        })?;
        Ok(versions)
    }
    fn latest(&self) -> Result<Version> {
        self.versions()?
            .pop()
            .context("durable FileBook state is missing; refusing local fallback")
    }
    fn claim(&self, prior: Option<&Version>, mut state: FileBookState) -> Result<Version> {
        state.format_version = 1;
        state.book_id = self.book.clone();
        state.revision = prior
            .map(|v| v.state.revision)
            .unwrap_or(0)
            .checked_add(1)
            .context("FileBook state revision overflow")?;
        state.predecessor_digest = prior
            .map(|v| v.digest.as_str().to_owned())
            .unwrap_or_default();
        self.validate(&state)?;
        for digest in &state.config_digests {
            self.blob(&Digest::parse(digest)?)?;
        }
        let bytes = state.encode_to_vec();
        ensure!(
            self.log().claim(u64::try_from(state.revision)?, &bytes)?,
            "FileBook state changed concurrently; refusing a stale snapshot"
        );
        Ok(Version {
            state,
            digest: Digest::of(&bytes),
        })
    }
    fn initialized(&self, root: &Path) -> Result<Vec<Version>> {
        let mut versions = self.versions()?;
        if versions.is_empty() {
            ensure!(
                !root.join(CACHE_PIN).exists(),
                "durable FileBook state is unavailable; refusing local fallback"
            );
            let (seed, blobs) = capture(root)?;
            if seed.config_digests.is_empty() && !root.join("journal.jsonl").is_file() {
                ensure!(SeqLog::new(self.objects.clone(),format!("{}/journal/",self.book)).height()? == 0,
                    "durable FileBook metadata is missing for a nonempty journal; no legacy configuration source is available");
            }
            for (digest, bytes) in &blobs {
                ensure!(
                    self.stage(bytes)?.as_str() == digest,
                    "legacy configuration address mismatch"
                );
            }
            match self.claim(None, seed.clone()) {
                Ok(version) => versions.push(version),
                Err(error) => {
                    // An identical concurrent import may reuse the winning
                    // seed. A different source is never silently adopted.
                    versions = self.versions()?;
                    ensure!(
                        versions
                            .first()
                            .is_some_and(|v| same_content(&v.state, &seed)),
                        "concurrent FileBook seed conflicts: {error}"
                    );
                }
            }
        }
        Ok(versions)
    }
    pub(crate) fn publish_source(&self, root: &Path) -> Result<()> {
        let versions = self.initialized(root)?;
        if root.join(CACHE_PIN).exists() {
            let pin = FileBookCachePin::decode(fs::read(root.join(CACHE_PIN))?.as_slice())?;
            ensure!(
                pin.format_version == 1 && pin.book_id == self.book,
                "foreign FileBook cache pin"
            );
            let pinned = versions
                .iter()
                .find(|v| v.state.revision == pin.revision && v.digest.as_str() == pin.state_digest)
                .context("FileBook source pin has no durable revision")?;
            self.verify_cache(root, &pinned.state)
        } else {
            let (source, _) = capture(root)?;
            ensure!(
                same_content(&source, &versions[0].state),
                "FileBook seed metadata differs from its durable source"
            );
            Ok(())
        }
    }
    pub(crate) fn attach(&self, root: &Path) -> Result<()> {
        let versions = self.initialized(root)?;
        let current = versions.last().expect("seeded");
        if root.join(CACHE_PIN).exists() {
            let pin = FileBookCachePin::decode(fs::read(root.join(CACHE_PIN))?.as_slice())
                .context("decoding FileBook cache pin")?;
            ensure!(
                pin.format_version == 1 && pin.book_id == self.book,
                "foreign or unsupported FileBook cache pin"
            );
            let pinned = versions
                .iter()
                .find(|v| v.state.revision == pin.revision && v.digest.as_str() == pin.state_digest)
                .context("FileBook cache pin has no matching durable revision")?;
            self.verify_cache(root, &pinned.state)?;
            if pin.revision == current.state.revision {
                return Ok(());
            }
        } else {
            let (local, _) = capture(root)?;
            let empty = local.chart.is_empty()
                && local.config_digests.is_empty()
                && local.active_digest.is_empty()
                && local.promotion_history.is_empty();
            ensure!(
                empty || same_content(&local, &versions[0].state),
                "unpublished local cache conflicts with durable FileBook state"
            );
        }
        self.write_cache(root, current)
    }
    fn verify_cache(&self, root: &Path, state: &FileBookState) -> Result<()> {
        let (local, blobs) = capture(root)?;
        ensure!(
            same_content(&local, state),
            "unpublished local cache conflicts with its durable FileBook revision"
        );
        for (digest, bytes) in blobs {
            ensure!(
                self.blob(&Digest::parse(&digest)?)? == bytes,
                "FileBook cache bytes conflict with durable configuration"
            );
        }
        Ok(())
    }
    fn write_cache(&self, root: &Path, version: &Version) -> Result<()> {
        fs::create_dir_all(root.join("config"))?;
        if !version.state.chart.is_empty() {
            atomic_write(&root.join("accounts.json"), &version.state.chart)?;
        }
        for digest in &version.state.config_digests {
            let digest = Digest::parse(digest)?;
            atomic_write(
                &root.join("config").join(digest.as_str()),
                &self.blob(&digest)?,
            )?;
        }
        if !version.state.active_digest.is_empty() {
            atomic_write(
                &root.join("config/ACTIVE"),
                version.state.active_digest.as_bytes(),
            )?;
        }
        if !version.state.promotion_history.is_empty() {
            let history = version
                .state
                .promotion_history
                .iter()
                .rev()
                .map(|d| format!("{d}\n"))
                .collect::<String>();
            atomic_write(&root.join("config/HISTORY"), history.as_bytes())?;
        }
        // Last: interrupted cache publication cannot impersonate a verified
        // revision. The durable state remains available to a fresh root.
        atomic_write(
            &root.join(CACHE_PIN),
            &FileBookCachePin {
                format_version: 1,
                book_id: self.book.clone(),
                revision: version.state.revision,
                state_digest: version.digest.as_str().into(),
            }
            .encode_to_vec(),
        )
    }
    fn update(
        &self,
        root: &Path,
        mutate: impl FnOnce(&mut FileBookState) -> Result<bool>,
    ) -> Result<()> {
        self.attach(root)?;
        let prior = self.latest()?;
        let mut next = prior.state.clone();
        if !mutate(&mut next)? {
            return Ok(());
        }
        let version = self.claim(Some(&prior), next)?;
        // Validate the old cache again before replacing it. A concurrent
        // cache writer may make this refuse; it must never select a figure.
        self.verify_cache(root, &prior.state)?;
        self.write_cache(root, &version)
    }
    pub(crate) fn put(&self, root: &Path, bytes: &[u8]) -> Result<Digest> {
        self.attach(root)?;
        let digest = self.stage(bytes)?;
        self.update(root, |state| {
            if state
                .config_digests
                .binary_search(&digest.as_str().to_owned())
                .is_ok()
            {
                return Ok(false);
            }
            state.config_digests.push(digest.as_str().into());
            state.config_digests.sort();
            Ok(true)
        })?;
        Ok(digest)
    }
    pub(crate) fn get(&self, digest: &Digest) -> Result<Vec<u8>> {
        ensure!(
            self.latest()?
                .state
                .config_digests
                .iter()
                .any(|d| d == digest.as_str()),
            "configuration is not bound to this FileBook"
        );
        self.blob(digest)
    }
    pub(crate) fn activate(&self, root: &Path, digest: &Digest) -> Result<()> {
        self.get(digest)?;
        self.update(root, |state| {
            state.active_digest = digest.as_str().into();
            state.promotion_history.insert(0, digest.as_str().into());
            Ok(true)
        })
    }
    pub(crate) fn active(&self) -> Result<Option<Digest>> {
        let state = self.latest()?.state;
        if state.active_digest.is_empty() {
            Ok(None)
        } else {
            self.get(&Digest::parse(&state.active_digest)?)?;
            Ok(Some(Digest::parse(&state.active_digest)?))
        }
    }
    pub(crate) fn history(&self) -> Result<Vec<Digest>> {
        self.latest()?
            .state
            .promotion_history
            .iter()
            .map(|d| Digest::parse(d))
            .collect()
    }
    pub(crate) fn chart(&self) -> Result<Vec<Account>> {
        let chart = self.latest()?.state.chart;
        if chart.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(serde_json::from_slice(&chart)?)
        }
    }
    pub(crate) fn put_chart(&self, root: &Path, bytes: &[u8]) -> Result<()> {
        self.update(root, |state| {
            if state.chart == bytes {
                return Ok(false);
            }
            state.chart = bytes.into();
            Ok(true)
        })
    }
}
fn same_content(a: &FileBookState, b: &FileBookState) -> bool {
    a.chart == b.chart
        && a.active_digest == b.active_digest
        && a.promotion_history == b.promotion_history
        && a.config_digests == b.config_digests
}
fn read(path: &Path) -> Result<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}
fn capture(root: &Path) -> Result<(FileBookState, BTreeMap<String, Vec<u8>>)> {
    let chart = read(&root.join("accounts.json"))?;
    if !chart.is_empty() {
        let _: Vec<Account> = serde_json::from_slice(&chart).context("invalid cached chart")?;
    }
    let active = String::from_utf8(read(&root.join("config/ACTIVE"))?)?
        .trim()
        .to_owned();
    if !active.is_empty() {
        Digest::parse(&active)?;
    }
    let mut history = String::from_utf8(read(&root.join("config/HISTORY"))?)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Digest::parse(line.trim()).map(|d| d.as_str().to_owned()))
        .collect::<Result<Vec<_>>>()?;
    history.reverse();
    let mut blobs = BTreeMap::new();
    match fs::read_dir(root.join("config")) {
        Ok(entries) => {
            for entry in entries {
                let path = entry?.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if name.len() != 64 {
                    continue;
                }
                let digest = Digest::parse(name)?;
                let bytes = fs::read(&path)?;
                ensure!(
                    Digest::of(&bytes) == digest,
                    "cached configuration digest mismatch"
                );
                blobs.insert(name.into(), bytes);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok((
        FileBookState {
            chart,
            active_digest: active,
            promotion_history: history,
            config_digests: blobs.keys().cloned().collect(),
            ..Default::default()
        },
        blobs,
    ))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!(
        "state-{}-{}.tmp",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, path).with_context(|| format!("publishing verified cache {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccountTypeRecord, ConfigStore, DirStore, FileBook, Journal, JournalEntry, MemoryStore,
    };
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "ratio-file-state-{}-{}",
                std::process::id(),
                TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn chart() -> Vec<Account> {
        vec![
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
        ]
    }
    fn entry(config: &Digest) -> JournalEntry {
        serde_json::from_value(serde_json::json!({"id":"opening","config":config.as_str(),"postings":[{"dim":1,"amount":12345},{"dim":2,"amount":-12345}]})).unwrap()
    }
    #[test]
    fn chart_staged_configuration_promotions_and_journal_survive_cache_loss() {
        for directory in [false, true] {
            let temp = Temp::new();
            let path = temp.0.join("book");
            let objects: Arc<dyn ObjectStore> = if directory {
                Arc::new(DirStore::at(temp.0.join("objects")))
            } else {
                Arc::new(MemoryStore::new())
            };
            let mut book = FileBook::open_with(&path, Some(objects.clone())).unwrap();
            book.put_accounts(&chart()).unwrap();
            let first = book.put(b"rules = []\n").unwrap();
            book.set_active(&first).unwrap();
            let draft = book.put(b"# unpromoted\nrules = []\n").unwrap();
            assert_eq!(book.active().unwrap(), Some(first.clone()));
            assert_eq!(book.history().unwrap(), vec![first.clone()]);
            let second = book.put(b"# reviewed\nrules = []\n").unwrap();
            book.set_active(&second).unwrap();
            book.append(&entry(&first)).unwrap();
            let expected = book.entries().unwrap();
            drop(book);
            fs::remove_dir_all(&path).unwrap();
            let objects: Arc<dyn ObjectStore> = if directory {
                Arc::new(DirStore::at(temp.0.join("objects")))
            } else {
                objects
            };
            let recovered = FileBook::open_with(&path, Some(objects)).unwrap();
            assert_eq!(recovered.accounts().unwrap(), chart());
            assert_eq!(recovered.active().unwrap(), Some(second.clone()));
            assert_eq!(recovered.history().unwrap(), vec![second, first]);
            assert_eq!(
                recovered.get(&draft).unwrap(),
                b"# unpromoted\nrules = []\n"
            );
            assert_eq!(
                serde_json::to_vec(&recovered.entries().unwrap()).unwrap(),
                serde_json::to_vec(&expected).unwrap()
            );
            assert_eq!(recovered.trial_balance().unwrap().debits, 12345);
            assert!(FileBook::open(&path)
                .err()
                .unwrap()
                .to_string()
                .contains("refusing local fallback"));
        }
    }
    #[test]
    fn a_recognized_older_cache_refreshes_without_choosing_the_old_configuration() {
        let temp = Temp::new();
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let first_path = temp.0.join("first/book");
        let second_path = temp.0.join("second/book");
        let mut first = FileBook::open_with(&first_path, Some(objects.clone())).unwrap();
        first.put_accounts(&chart()).unwrap();
        let old = first.put(b"old").unwrap();
        first.set_active(&old).unwrap();
        drop(first);
        let mut second = FileBook::open_with(&second_path, Some(objects.clone())).unwrap();
        let new = second.put(b"new").unwrap();
        second.set_active(&new).unwrap();
        drop(second);
        assert_eq!(
            fs::read_to_string(first_path.join("config/ACTIVE")).unwrap(),
            old.as_str()
        );
        let refreshed = FileBook::open_with(&first_path, Some(objects)).unwrap();
        assert_eq!(refreshed.active().unwrap(), Some(new.clone()));
        assert_eq!(
            fs::read_to_string(first_path.join("config/ACTIVE")).unwrap(),
            new.as_str()
        );
    }
    #[test]
    fn an_unknown_cache_or_wrong_backend_refuses_without_resetting_local_bytes() {
        let temp = Temp::new();
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let path = temp.0.join("book");
        let mut book = FileBook::open_with(&path, Some(objects.clone())).unwrap();
        book.put_accounts(&chart()).unwrap();
        let config = book.put(b"rules=[]").unwrap();
        book.set_active(&config).unwrap();
        drop(book);
        let before = fs::read(path.join(CACHE_PIN)).unwrap();
        let wrong: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        assert!(FileBook::open_with(&path, Some(wrong))
            .err()
            .unwrap()
            .to_string()
            .contains("refusing local fallback"));
        assert_eq!(fs::read(path.join(CACHE_PIN)).unwrap(), before);
        fs::write(path.join("accounts.json"), b"[]").unwrap();
        assert!(FileBook::open_with(&path, Some(objects))
            .err()
            .unwrap()
            .to_string()
            .contains("conflicts"));
        assert_eq!(fs::read(path.join("accounts.json")).unwrap(), b"[]");
    }
    #[test]
    fn legacy_history_first_crash_state_is_preserved_and_a_draft_is_not_promoted() {
        let temp = Temp::new();
        let path = temp.0.join("book");
        let mut local = FileBook::open(&path).unwrap();
        local.put_accounts(&chart()).unwrap();
        let first = local.put(b"first").unwrap();
        local.set_active(&first).unwrap();
        let second = local.put(b"second").unwrap();
        local.set_active(&second).unwrap();
        let draft = local.put(b"draft").unwrap();
        fs::write(path.join("config/ACTIVE"), first.as_str()).unwrap();
        drop(local);
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let migrated = FileBook::open_with(&path, Some(objects.clone())).unwrap();
        assert_eq!(migrated.active().unwrap(), Some(first.clone()));
        assert_eq!(
            migrated.history().unwrap(),
            vec![second.clone(), first.clone()]
        );
        drop(migrated);
        fs::remove_dir_all(&path).unwrap();
        let recovered = FileBook::open_with(&path, Some(objects)).unwrap();
        assert_eq!(recovered.active().unwrap(), Some(first.clone()));
        assert_eq!(recovered.history().unwrap(), vec![second, first]);
        assert_eq!(recovered.get(&draft).unwrap(), b"draft");
    }
    #[test]
    fn a_stale_full_snapshot_cannot_claim_a_later_revision_and_lose_history() {
        let temp = Temp::new();
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let store = StateStore::new(objects, "book").unwrap();
        store.attach(&temp.0.join("book")).unwrap();
        let prior = store.latest().unwrap();
        let a = store.stage(b"a").unwrap();
        let b = store.stage(b"b").unwrap();
        let mut first = prior.state.clone();
        first.config_digests = vec![a.as_str().into()];
        first.active_digest = a.as_str().into();
        first.promotion_history = vec![a.as_str().into()];
        let mut stale = prior.state.clone();
        stale.config_digests = vec![b.as_str().into()];
        stale.active_digest = b.as_str().into();
        stale.promotion_history = vec![b.as_str().into()];
        store.claim(Some(&prior), first).unwrap();
        assert!(store
            .claim(Some(&prior), stale)
            .err()
            .unwrap()
            .to_string()
            .contains("changed concurrently"));
        assert_eq!(store.history().unwrap(), vec![a]);
    }
    #[test]
    fn corrupt_durable_configuration_is_refused_after_local_cache_loss() {
        let temp = Temp::new();
        let path = temp.0.join("book");
        let objects = Arc::new(DirStore::at(temp.0.join("objects")));
        let mut book = FileBook::open_with(&path, Some(objects)).unwrap();
        let digest = book.put(b"valid").unwrap();
        book.set_active(&digest).unwrap();
        drop(book);
        fs::remove_dir_all(&path).unwrap();
        fs::write(
            temp.0
                .join("objects/_filebook/book/config")
                .join(digest.as_str()),
            b"corrupt",
        )
        .unwrap();
        assert!(
            FileBook::open_with(&path, Some(Arc::new(DirStore::at(temp.0.join("objects")))))
                .err()
                .unwrap()
                .to_string()
                .contains("digest mismatch")
        );
    }
    #[test]
    fn missing_metadata_cannot_turn_an_existing_journal_into_an_empty_new_book() {
        let temp = Temp::new();
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        SeqLog::new(objects.clone(), "book/journal/")
            .claim(1, b"existing")
            .unwrap();
        let error = FileBook::open_with(temp.0.join("book"), Some(objects.clone()))
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("metadata is missing") && error.contains("nonempty journal"),
            "{error}"
        );
        assert!(objects.list("_filebook/book/state/").unwrap().is_empty());
    }

    #[test]
    fn no_store_retains_the_documented_layout_and_promotion_log() {
        let temp = Temp::new();
        let path = temp.0.join("book");
        let mut book = FileBook::open(&path).unwrap();
        book.put_accounts(&chart()).unwrap();
        let digest = book.put(b"rules=[]").unwrap();
        book.set_active(&digest).unwrap();
        book.append(&entry(&digest)).unwrap();
        assert!(!path.join(CACHE_PIN).exists());
        assert!(path.join("accounts.json").is_file());
        assert!(path.join("journal.jsonl").is_file());
        assert_eq!(
            fs::read_to_string(path.join("config/HISTORY")).unwrap(),
            format!("{digest}\n")
        );
        assert_eq!(
            FileBook::open(&path).unwrap().active().unwrap(),
            Some(digest)
        );
    }
}
