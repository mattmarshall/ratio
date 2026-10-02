//! Ordered audit lines beside the book. The verified actor is supplied by the
//! caller; this module preserves those bytes and refuses a broken prefix.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{ChangeMigration, StoredChange};

use crate::{Digest, ObjectStore, SeqLog};

fn book_id(book: &Path) -> Result<&str> {
    book.file_name().and_then(|part| part.to_str())
        .filter(|part| !part.is_empty())
        .context("a durable change needs a book directory name")
}

fn log(book: &Path, store: Arc<dyn ObjectStore>) -> Result<SeqLog> {
    Ok(SeqLog::new(store, format!("{}/changes/", book_id(book)?)))
}

fn local(book: &Path) -> Result<Vec<u8>> {
    match fs::read(book.join("CHANGELOG")) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).context("reading CHANGELOG"),
    }
}

fn lines(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bail!("CHANGELOG ends with a truncated line");
    }
    let text = std::str::from_utf8(bytes).context("CHANGELOG is not UTF-8")?;
    text.split_inclusive('\n').map(|line| {
        if line.trim_end_matches('\n').split('\t').count() != 5 {
            bail!("CHANGELOG contains a malformed audit line");
        }
        Ok(line.as_bytes().to_vec())
    }).collect()
}

fn stored(book: &Path, line: &[u8]) -> Result<Vec<u8>> {
    if lines(line)?.len() != 1 { bail!("audit append needs exactly one line"); }
    Ok(StoredChange {
        format_version: 1,
        book_id: book_id(book)?.into(),
        line_bytes: line.into(),
    }.encode_to_vec())
}

fn decode(book: &Path, bytes: &[u8]) -> Result<String> {
    let change = StoredChange::decode(bytes).context("decoding durable audit line")?;
    if change.format_version != 1 || change.book_id != book_id(book)? {
        bail!("audit line has an unsupported version or different book identity");
    }
    let mut parsed = lines(&change.line_bytes)?;
    if parsed.len() != 1 { bail!("audit object must contain exactly one line"); }
    String::from_utf8(parsed.pop().unwrap()).context("audit line is not UTF-8")
}

fn migrate(book: &Path, store: Arc<dyn ObjectStore>) -> Result<()> {
    let current = local(book)?;
    let id = book_id(book)?;
    let source_key = format!("_change-migration/{id}");
    let complete_key = format!("_change-migration-complete/{id}");
    let source = match store.get(&source_key)? {
        Some(bytes) => {
            let source = ChangeMigration::decode(bytes.as_slice())
                .context("decoding audit migration source")?;
            if source.format_version != 1 || source.book_id != id {
                bail!("audit migration source has the wrong version or book");
            }
            if !current.is_empty() && current != source.source_bytes {
                bail!("local CHANGELOG changed after durable migration began");
            }
            source
        }
        None if current.is_empty() => return Ok(()),
        None => {
            if log(book, store.clone())?.height()? != 0 {
                bail!("local CHANGELOG cannot seed an already durable audit log");
            }
            lines(&current)?;
            let source = ChangeMigration {
                format_version: 1, book_id: id.into(), source_bytes: current,
            };
            let bytes = source.encode_to_vec();
            if !store.put_if_absent(&source_key, &bytes)?
                && store.get(&source_key)?.as_deref() != Some(bytes.as_slice()) {
                bail!("concurrent audit migrations name different source bytes");
            }
            source
        }
    };
    let claim_bytes = source.encode_to_vec();
    let digest = Digest::of(&claim_bytes);
    let claim = digest.as_str().as_bytes();
    let completed = store.get(&complete_key)?;
    if completed.as_deref().is_some_and(|bytes| bytes != claim) {
        bail!("completed audit migration names different source bytes");
    }
    let audit = log(book, store.clone())?;
    for (index, line) in lines(&source.source_bytes)?.iter().enumerate() {
        let seq = u64::try_from(index + 1).context("too many legacy audit lines")?;
        let bytes = stored(book, line)?;
        if completed.is_some() {
            if audit.get(seq)?.as_deref() != Some(bytes.as_slice()) {
                bail!("completed audit migration lost or changed sequence {seq}");
            }
        } else if !audit.claim(seq, &bytes)?
            && audit.get(seq)?.as_deref() != Some(bytes.as_slice()) {
            bail!("legacy audit sequence {seq} conflicts with durable evidence");
        }
    }
    if !store.put_if_absent(&complete_key, claim)?
        && store.get(&complete_key)?.as_deref() != Some(claim) {
        bail!("completed audit migration names different source bytes");
    }
    Ok(())
}

pub fn append_with_store(book: &Path, line: &str,
    store: Option<Arc<dyn ObjectStore>>) -> Result<()> {
    let bytes = stored(book, line.as_bytes())?;
    if let Some(store) = store {
        migrate(book, store.clone())?;
        log(book, store)?.append(&bytes)?;
        return Ok(());
    }
    let mut file = fs::OpenOptions::new().create(true).append(true)
        .open(book.join("CHANGELOG")).context("opening CHANGELOG")?;
    file.write_all(line.as_bytes()).context("appending to CHANGELOG")
}

pub fn append(book: &Path, line: &str) -> Result<()> {
    append_with_store(book, line, None)
}

pub fn read_with_store(book: &Path, store: Option<Arc<dyn ObjectStore>>) -> Result<Vec<String>> {
    let Some(store) = store else {
        return lines(&local(book)?)?.into_iter()
            .map(|line| String::from_utf8(line).context("audit line is not UTF-8"))
            .collect();
    };
    migrate(book, store.clone())?;
    let audit = log(book, store)?;
    let mut out = Vec::new();
    audit.for_each_since(0, &mut |_, bytes| {
        out.push(decode(book, bytes)?);
        Ok(())
    })?;
    Ok(out)
}

pub fn read(book: &Path) -> Result<Vec<String>> {
    read_with_store(book, None)
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
        base.join(format!("ratio-changes-{label}-{}-{}", std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)))
    }

    #[test]
    fn audit_lines_survive_cold_reopen_and_refuse_a_hole() {
        let book = root("book");
        let objects = root("objects");
        let store = Arc::new(DirStore::at(&objects)) as Arc<dyn ObjectStore>;
        append_with_store(&book, "1\talice\tapproved\tp1\td1\n", Some(store.clone())).unwrap();
        append_with_store(&book, "2\tbob\tposted\te1\td1\n", Some(store.clone())).unwrap();
        assert_eq!(read_with_store(&book, Some(Arc::new(DirStore::at(&objects)))).unwrap().len(), 2);
        fs::remove_file(objects.join(format!("{}/changes/{:020}", book_id(&book).unwrap(), 1))).unwrap();
        assert!(read_with_store(&book, Some(store)).unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn legacy_audit_bytes_migrate_and_cannot_silently_disappear() {
        let book = root("legacy");
        fs::create_dir_all(&book).unwrap();
        fs::write(book.join("CHANGELOG"), b"1\talice\tapproved\tp1\td1\n").unwrap();
        let store = Arc::new(MemoryStore::new()) as Arc<dyn ObjectStore>;
        assert_eq!(read_with_store(&book, Some(store.clone())).unwrap().len(), 1);
        fs::remove_file(book.join("CHANGELOG")).unwrap();
        assert_eq!(read_with_store(&book, Some(store.clone())).unwrap().len(), 1);
        let audit = log(&book, store).unwrap();
        assert!(audit.get(1).unwrap().is_some());
    }

    #[test]
    fn interrupted_audit_import_resumes_from_the_claimed_source() {
        let book = root("resume");
        let store = Arc::new(MemoryStore::new()) as Arc<dyn ObjectStore>;
        let source = ChangeMigration {
            format_version: 1,
            book_id: book_id(&book).unwrap().into(),
            source_bytes: b"1\talice\tapproved\tp1\td1\n2\tbob\tposted\te1\td1\n".to_vec(),
        };
        let source_key = format!("_change-migration/{}", book_id(&book).unwrap());
        assert!(store.put_if_absent(&source_key, &source.encode_to_vec()).unwrap());
        let audit = log(&book, store.clone()).unwrap();
        assert!(audit.claim(1, &stored(&book, b"1\talice\tapproved\tp1\td1\n").unwrap()).unwrap());
        assert_eq!(read_with_store(&book, Some(store.clone())).unwrap().len(), 2);
        assert!(store.get(&format!("_change-migration-complete/{}", book_id(&book).unwrap()))
            .unwrap().is_some());
    }

    struct UnavailableStore;
    impl ObjectStore for UnavailableStore {
        fn put_if_absent(&self, _: &str, _: &[u8]) -> Result<bool> { bail!("store unavailable") }
        fn get(&self, _: &str) -> Result<Option<Vec<u8>>> { bail!("store unavailable") }
        fn list(&self, _: &str) -> Result<Vec<String>> { bail!("store unavailable") }
    }

    #[test]
    fn a_configured_audit_writer_refuses_a_failed_store() {
        let book = root("unavailable");
        let err = append_with_store(&book, "1\talice\tapproved\tp1\td1\n",
            Some(Arc::new(UnavailableStore))).unwrap_err().to_string();
        assert!(err.contains("store unavailable"), "{err}");
        assert!(!book.join("CHANGELOG").exists());
    }
}
