//! Durable proposal artifacts. A proposal is a draft, never active policy;
//! its exact reviewed TOML is immutable under one book/proposal id.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{ProposalMigration, StoredProposal};

use crate::{installed_object_store, Digest, ObjectStore};

fn book_id(book: &Path) -> Result<&str> {
    book.file_name().and_then(|part| part.to_str())
        .filter(|part| !part.is_empty())
        .context("a durable proposal needs a book directory name")
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id == "." || id == ".." || id.contains('/') || id.contains('\\')
        || id.chars().any(char::is_control)
    {
        bail!("invalid proposal id {id:?}");
    }
    Ok(())
}

fn prefix(book: &Path) -> Result<String> {
    Ok(format!("{}/proposals/", book_id(book)?))
}

fn key(book: &Path, id: &str) -> Result<String> {
    validate_id(id)?;
    Ok(format!("{}{}", prefix(book)?, Digest::of(id.as_bytes()).as_str()))
}

fn record(book: &Path, id: &str, toml: &str) -> Result<StoredProposal> {
    validate_id(id)?;
    Ok(StoredProposal {
        format_version: 1,
        book_id: book_id(book)?.into(),
        proposal_id: id.into(),
        toml_bytes: toml.as_bytes().into(),
    })
}

fn validate_record(book: &Path, key_name: &str, bytes: &[u8]) -> Result<(String, String)> {
    let proposed = StoredProposal::decode(bytes).context("decoding durable proposal")?;
    if proposed.format_version != 1 || proposed.book_id != book_id(book)? {
        bail!("proposal has an unsupported version or different book identity");
    }
    if key(book, &proposed.proposal_id)? != key_name {
        bail!("proposal object key does not match its id");
    }
    let text = String::from_utf8(proposed.toml_bytes).context("proposal TOML is not UTF-8")?;
    Ok((proposed.proposal_id, text))
}

fn local_proposals(book: &Path) -> Result<Vec<StoredProposal>> {
    let dir = book.join("proposals");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("listing {}", dir.display())),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.with_context(|| format!("listing {}", dir.display()))?.path();
        if path.extension().is_some_and(|extension| extension == "toml") {
            paths.push(path);
        }
    }
    paths.sort();
    paths.into_iter().map(|path| {
        let id = path.file_stem().and_then(|part| part.to_str())
            .context("proposal name is not UTF-8")?;
        let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        record(book, id, &text)
    }).collect()
}

fn migration_keys(book: &Path) -> Result<(String, String)> {
    let id = book_id(book)?;
    Ok((format!("_proposal-migration/{id}"),
        format!("_proposal-migration-complete/{id}")))
}

fn migrate_proposals(book: &Path, store: &dyn ObjectStore) -> Result<()> {
    let local = local_proposals(book)?;
    let (claim_key, complete_key) = migration_keys(book)?;
    let source = match store.get(&claim_key)? {
        Some(bytes) => {
            let source = ProposalMigration::decode(bytes.as_slice())
                .context("decoding durable proposal migration source")?;
            if source.format_version != 1 || source.book_id != book_id(book)? {
                bail!("proposal migration source has the wrong version or book");
            }
            if !local.is_empty() {
                let expected: BTreeMap<_, _> = source.proposals.iter()
                    .map(|draft| (draft.proposal_id.as_str(), draft.toml_bytes.as_slice())).collect();
                let actual: BTreeMap<_, _> = local.iter()
                    .map(|draft| (draft.proposal_id.as_str(), draft.toml_bytes.as_slice())).collect();
                if expected != actual { bail!("local proposals changed after durable migration began"); }
            }
            source
        }
        None if local.is_empty() => return Ok(()),
        None => {
            if !store.list(&prefix(book)?)?.is_empty() {
                bail!("local proposals cannot seed an already durable proposal set");
            }
            let source = ProposalMigration {
                format_version: 1,
                book_id: book_id(book)?.into(),
                proposals: local,
            };
            let bytes = source.encode_to_vec();
            if !store.put_if_absent(&claim_key, &bytes)?
                && store.get(&claim_key)?.as_deref() != Some(bytes.as_slice())
            {
                bail!("concurrent proposal migrations name different source bytes");
            }
            source
        }
    };
    let mut ids = BTreeSet::new();
    let source_bytes = source.encode_to_vec();
    let source_digest = Digest::of(&source_bytes);
    let claim = source_digest.as_str().as_bytes();
    let completed = store.get(&complete_key)?;
    if completed.as_deref().is_some_and(|body| body != claim) {
        bail!("completed proposal migration names different source bytes");
    }
    for draft in &source.proposals {
        if !ids.insert(draft.proposal_id.as_str()) {
            bail!("proposal migration names one id twice");
        }
        let key_name = key(book, &draft.proposal_id)?;
        let bytes = draft.encode_to_vec();
        validate_record(book, &key_name, &bytes)?;
        if completed.is_some() {
            if store.get(&key_name)?.as_deref() != Some(bytes.as_slice()) {
                bail!("completed proposal migration lost or changed {}", draft.proposal_id);
            }
        } else if !store.put_if_absent(&key_name, &bytes)?
            && store.get(&key_name)?.as_deref() != Some(bytes.as_slice())
        {
            bail!("legacy proposal {} conflicts with durable evidence", draft.proposal_id);
        }
    }
    if !store.put_if_absent(&complete_key, claim)?
        && store.get(&complete_key)?.as_deref() != Some(claim)
    {
        bail!("completed proposal migration names different source bytes");
    }
    Ok(())
}

pub fn write_with_store(book: &Path, id: &str, toml: &str,
    store: Option<Arc<dyn ObjectStore>>) -> Result<()> {
    let proposed = record(book, id, toml)?;
    if let Some(store) = store {
        migrate_proposals(book, store.as_ref())?;
        let key_name = key(book, id)?;
        let bytes = proposed.encode_to_vec();
        if !store.put_if_absent(&key_name, &bytes)?
            && store.get(&key_name)?.as_deref() != Some(bytes.as_slice())
        {
            bail!("proposal {id} already exists with different bytes; use a new id");
        }
        return Ok(());
    }
    let dir = book.join("proposals");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    fs::write(dir.join(format!("{id}.toml")), toml).context("writing proposal")
}

pub fn write(book: &Path, id: &str, toml: &str) -> Result<()> {
    write_with_store(book, id, toml, installed_object_store())
}

pub fn read_with_store(book: &Path, id: &str,
    store: Option<Arc<dyn ObjectStore>>) -> Result<Option<String>> {
    let key_name = key(book, id)?;
    let Some(store) = store else {
        return match fs::read_to_string(book.join("proposals").join(format!("{id}.toml"))) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("reading proposal"),
        };
    };
    migrate_proposals(book, store.as_ref())?;
    store.get(&key_name)?.map(|bytes| validate_record(book, &key_name, &bytes).map(|(_, text)| text)).transpose()
}

pub fn read(book: &Path, id: &str) -> Result<Option<String>> {
    read_with_store(book, id, installed_object_store())
}

pub fn list_with_store(book: &Path,
    store: Option<Arc<dyn ObjectStore>>) -> Result<Vec<(String, String)>> {
    let Some(store) = store else {
        return local_proposals(book)?.into_iter().map(|draft| {
            Ok((draft.proposal_id, String::from_utf8(draft.toml_bytes)?))
        }).collect();
    };
    migrate_proposals(book, store.as_ref())?;
    let mut out = Vec::new();
    for key_name in store.list(&prefix(book)?)? {
        let bytes = store.get(&key_name)?.context("listed proposal disappeared")?;
        out.push(validate_record(book, &key_name, &bytes)?);
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn list(book: &Path) -> Result<Vec<(String, String)>> {
    list_with_store(book, installed_object_store())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DirStore, MemoryStore};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn root(label: &str) -> PathBuf {
        let base = std::env::var_os("TEST_TMPDIR").map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join(format!("ratio-proposals-{label}-{}-{}", std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)))
    }

    #[test]
    fn a_proposal_is_one_immutable_reviewed_draft_after_cold_reopen() {
        let book = root("cold-book");
        let objects = root("cold-objects");
        for store in [
            Arc::new(MemoryStore::new()) as Arc<dyn ObjectStore>,
            Arc::new(DirStore::at(&objects)) as Arc<dyn ObjectStore>,
        ] {
            write_with_store(&book, "p1", "[[rule]]\nid = 'p1'", Some(store.clone())).unwrap();
            write_with_store(&book, "p1", "[[rule]]\nid = 'p1'", Some(store.clone())).unwrap();
            let err = write_with_store(&book, "p1", "[[rule]]\nid = 'changed'",
                Some(store.clone())).unwrap_err().to_string();
            assert!(err.contains("different bytes"), "{err}");
            let _ = fs::remove_dir_all(&book);
            assert_eq!(read_with_store(&book, "p1", Some(store.clone())).unwrap().as_deref(),
                Some("[[rule]]\nid = 'p1'"));
            assert_eq!(list_with_store(&book, Some(store)).unwrap().len(), 1);
        }
        assert_eq!(list_with_store(&book, Some(Arc::new(DirStore::at(&objects))))
            .unwrap().len(), 1);
    }

    #[test]
    fn a_legacy_proposal_source_survives_cold_reopen_and_refuses_loss() {
        let book = root("legacy-book");
        let objects = root("legacy-objects");
        write_with_store(&book, "a", "first", None).unwrap();
        write_with_store(&book, "b", "second", None).unwrap();
        let store = Arc::new(DirStore::at(&objects));
        assert_eq!(list_with_store(&book, Some(store.clone())).unwrap().len(), 2);
        fs::remove_dir_all(&book).unwrap();
        assert_eq!(list_with_store(&book, Some(store.clone())).unwrap().len(), 2);
        fs::remove_file(objects.join(key(&book, "a").unwrap())).unwrap();
        let err = list_with_store(&book, Some(store)).unwrap_err().to_string();
        assert!(err.contains("lost or changed"), "{err}");
    }

    #[test]
    fn an_interrupted_legacy_proposal_migration_resumes_from_its_source() {
        let book = root("resume-book");
        let store = Arc::new(MemoryStore::new());
        let source = ProposalMigration {
            format_version: 1,
            book_id: book_id(&book).unwrap().into(),
            proposals: vec![record(&book, "a", "first").unwrap(),
                record(&book, "b", "second").unwrap()],
        };
        let (claim_key, complete_key) = migration_keys(&book).unwrap();
        assert!(store.put_if_absent(&claim_key, &source.encode_to_vec()).unwrap());
        assert!(store.put_if_absent(&key(&book, "a").unwrap(),
            &source.proposals[0].encode_to_vec()).unwrap());
        assert!(store.get(&complete_key).unwrap().is_none());
        assert_eq!(list_with_store(&book, Some(store.clone())).unwrap().len(), 2);
        assert!(store.get(&complete_key).unwrap().is_some());
    }

    struct UnavailableStore;

    impl ObjectStore for UnavailableStore {
        fn put_if_absent(&self, _: &str, _: &[u8]) -> Result<bool> { bail!("store unavailable") }
        fn get(&self, _: &str) -> Result<Option<Vec<u8>>> { bail!("store unavailable") }
        fn list(&self, _: &str) -> Result<Vec<String>> { bail!("store unavailable") }
    }

    #[test]
    fn a_configured_proposal_writer_refuses_storage_failure_and_bad_ids() {
        let book = root("unavailable");
        let err = write_with_store(&book, "p", "draft", Some(Arc::new(UnavailableStore)))
            .unwrap_err().to_string();
        assert!(err.contains("store unavailable"), "{err}");
        assert!(!book.join("proposals/p.toml").exists());
        assert!(write_with_store(&book, "../escape", "draft", None).is_err());
    }
}
