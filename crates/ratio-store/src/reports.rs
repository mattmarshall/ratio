//! Reconciliation reports beside the journal. The report remains the exact
//! `ratio.v1.BreakReport` bytes that the writer emitted; object storage keeps
//! one immutable, ordered envelope per report.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{ReportMigration, StoredReport};
use ratio_proto::ratio::v1::BreakReport;
use ratio_proto::timestamp_proto::google::protobuf::Timestamp;

use crate::{installed_object_store, Digest, ObjectStore, SeqLog};

fn book_id(book: &Path) -> Result<&str> {
    book.file_name().and_then(|part| part.to_str())
        .filter(|part| !part.is_empty())
        .context("a durable report needs a book directory name")
}

fn prefix(book: &Path) -> Result<String> {
    Ok(format!("{}/reports/", book_id(book)?))
}

fn report_log(book: &Path, store: Arc<dyn ObjectStore>) -> Result<SeqLog> {
    Ok(SeqLog::new(store, prefix(book)?))
}

fn validate_report(book: &Path, record: &StoredReport) -> Result<BreakReport> {
    if record.format_version != 1 || record.book_id != book_id(book)? {
        bail!("stored report has an unsupported version or different book identity");
    }
    let name = record.filename.as_str();
    if name.is_empty() || !name.ends_with(".pb") || name.contains('/') || name.contains('\\')
        || name == ".pb"
    {
        bail!("invalid stored report file name {name:?}");
    }
    record.record_time.as_ref().context("stored report has no recording time")?;
    BreakReport::decode(record.report_bytes.as_slice())
        .with_context(|| format!("decoding report {name}"))
}

fn timestamp(when: SystemTime) -> Result<Timestamp> {
    match when.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(elapsed) => Ok(Timestamp {
            seconds: i64::try_from(elapsed.as_secs()).context("report time exceeds i64")?,
            nanos: i32::try_from(elapsed.subsec_nanos()).context("report nanos exceed i32")?,
        }),
        Err(before) => {
            let elapsed = before.duration();
            let whole = i64::try_from(elapsed.as_secs()).context("report time exceeds i64")?;
            if elapsed.subsec_nanos() == 0 {
                Ok(Timestamp { seconds: -whole, nanos: 0 })
            } else {
                Ok(Timestamp {
                    seconds: -whole - 1,
                    nanos: i32::try_from(1_000_000_000 - elapsed.subsec_nanos())?,
                })
            }
        }
    }
}

fn local_reports(book: &Path) -> Result<Vec<StoredReport>> {
    let dir = book.join("reports");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("listing {}", dir.display())),
    };
    let mut paths: Vec<(SystemTime, PathBuf)> = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "pb") {
            let modified = fs::metadata(&path)?.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            paths.push((modified, path));
        }
    }
    // ⚠ MTIME, THEN PATH: preserve the existing newest-report rule when
    // migrating a local directory. The object sequence keeps that order.
    paths.sort();
    paths.into_iter().map(|(modified, path)| {
        let name = path.file_name().and_then(|part| part.to_str())
            .context("report name is not UTF-8")?.to_string();
        let record = StoredReport {
            format_version: 1,
            book_id: book_id(book)?.into(),
            filename: name,
            report_bytes: fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
            record_time: Some(timestamp(modified)?),
        };
        validate_report(book, &record)?;
        Ok(record)
    }).collect()
}

fn migration_keys(book: &Path) -> Result<(String, String)> {
    let id = book_id(book)?;
    Ok((format!("_report-migration/{id}"), format!("_report-migration-complete/{id}")))
}

fn migrate_reports(book: &Path, store: Arc<dyn ObjectStore>) -> Result<()> {
    let local = local_reports(book)?;
    let (claim_key, complete_key) = migration_keys(book)?;
    let manifest = match store.get(&claim_key)? {
        Some(bytes) => {
            let manifest = ReportMigration::decode(bytes.as_slice())
                .context("decoding durable report migration source")?;
            if manifest.format_version != 1 || manifest.book_id != book_id(book)? {
                bail!("report migration source has the wrong version or book");
            }
            if !local.is_empty() {
                let expected: BTreeMap<_, _> = manifest.reports.iter()
                    .map(|record| (record.filename.as_str(), record.report_bytes.as_slice())).collect();
                let actual: BTreeMap<_, _> = local.iter()
                    .map(|record| (record.filename.as_str(), record.report_bytes.as_slice())).collect();
                if expected != actual { bail!("local reports changed after durable migration began"); }
            }
            manifest
        }
        None if local.is_empty() => return Ok(()),
        None => {
            let log = report_log(book, store.clone())?;
            if log.height()? != 0 {
                bail!("local reports cannot seed an already durable report log");
            }
            let manifest = ReportMigration {
                format_version: 1,
                book_id: book_id(book)?.into(),
                reports: local,
            };
            let bytes = manifest.encode_to_vec();
            if !store.put_if_absent(&claim_key, &bytes)?
                && store.get(&claim_key)?.as_deref() != Some(bytes.as_slice())
            {
                bail!("concurrent report migrations name different source bytes");
            }
            manifest
        }
    };
    let source = manifest.encode_to_vec();
    let mut names = std::collections::BTreeSet::new();
    for report in &manifest.reports {
        if !names.insert(report.filename.as_str()) {
            bail!("report migration names one filename twice");
        }
    }
    let digest = Digest::of(&source);
    let claim = digest.as_str().as_bytes();
    let completed = store.get(&complete_key)?;
    if completed.as_deref().is_some_and(|body| body != claim) {
        bail!("completed report migration names different source bytes");
    }
    let log = report_log(book, store.clone())?;
    for (index, report) in manifest.reports.iter().enumerate() {
        validate_report(book, report)?;
        let seq = u64::try_from(index + 1).context("too many legacy reports")?;
        let bytes = report.encode_to_vec();
        if completed.is_some() {
            if log.get(seq)?.as_deref() != Some(bytes.as_slice()) {
                bail!("completed report migration lost or changed report sequence {seq}");
            }
        } else if !log.claim(seq, &bytes)?
            && log.get(seq)?.as_deref() != Some(bytes.as_slice())
        {
            bail!("legacy report sequence {seq} conflicts with durable evidence");
        }
    }
    if !store.put_if_absent(&complete_key, claim)?
        && store.get(&complete_key)?.as_deref() != Some(claim)
    {
        bail!("completed report migration names different source bytes");
    }
    Ok(())
}

/// Store the exact protobuf report. Configured storage refuses failures
/// instead of falling back to a serving container's filesystem.
pub fn write_report_with_store(
    book: &Path,
    name: &str,
    bytes: &[u8],
    store: Option<Arc<dyn ObjectStore>>,
) -> Result<()> {
    let record = StoredReport {
        format_version: 1,
        book_id: book_id(book)?.into(),
        filename: name.into(),
        report_bytes: bytes.into(),
        record_time: Some(timestamp(SystemTime::now())?),
    };
    validate_report(book, &record)?;
    if let Some(store) = store {
        migrate_reports(book, store.clone())?;
        report_log(book, store)?.append(&record.encode_to_vec())?;
        return Ok(());
    }
    let dir = book.join("reports");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    fs::write(dir.join(name), bytes).context("storing the report")
}

pub fn write_report(book: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    write_report_with_store(book, name, bytes, installed_object_store())
}

/// Latest report by append order. A hole in the object log refuses a read.
pub fn newest_report_with_store(
    book: &Path,
    store: Option<Arc<dyn ObjectStore>>,
) -> Result<Option<BreakReport>> {
    let Some(store) = store else {
        let mut reports = local_reports(book)?;
        return reports.pop().map(|record| validate_report(book, &record)).transpose();
    };
    migrate_reports(book, store.clone())?;
    let log = report_log(book, store)?;
    let mut newest = None;
    log.for_each_since(0, &mut |_, bytes| {
        let record = StoredReport::decode(bytes).context("decoding durable report envelope")?;
        newest = Some(validate_report(book, &record)?);
        Ok(())
    })?;
    Ok(newest)
}

pub fn newest_report(book: &Path) -> Result<Option<BreakReport>> {
    newest_report_with_store(book, installed_object_store())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DirStore, MemoryStore};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn root(label: &str) -> PathBuf {
        let base = std::env::var_os("TEST_TMPDIR").map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join(format!("ratio-reports-{label}-{}-{}", std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)))
    }

    fn bytes(digest: &str) -> Vec<u8> {
        BreakReport { name: format!("books/test/breakReports/{digest}"),
            config_digest: digest.into(), ..Default::default() }.encode_to_vec()
    }

    #[test]
    fn reports_reopen_from_each_object_backend_in_append_order() {
        let book = root("cold-book");
        let objects = root("cold-objects");
        for store in [
            Arc::new(MemoryStore::new()) as Arc<dyn ObjectStore>,
            Arc::new(DirStore::at(&objects)) as Arc<dyn ObjectStore>,
        ] {
            write_report_with_store(&book, "a.pb", &bytes("a"), Some(store.clone())).unwrap();
            write_report_with_store(&book, "b.pb", &bytes("b"), Some(store.clone())).unwrap();
            assert_eq!(newest_report_with_store(&book, Some(store.clone())).unwrap()
                .unwrap().config_digest, "b");
            let _ = fs::remove_dir_all(&book);
            assert_eq!(newest_report_with_store(&book, Some(store)).unwrap()
                .unwrap().config_digest, "b");
        }
        assert_eq!(newest_report_with_store(&book,
            Some(Arc::new(DirStore::at(&objects)))).unwrap().unwrap().config_digest, "b");
    }

    #[test]
    fn legacy_reports_resume_from_a_fenced_source_and_refuse_a_missing_object() {
        let book = root("migration-book");
        let objects = root("migration-objects");
        write_report_with_store(&book, "a.pb", &bytes("a"), None).unwrap();
        write_report_with_store(&book, "b.pb", &bytes("b"), None).unwrap();
        let store = Arc::new(DirStore::at(&objects));
        assert_eq!(newest_report_with_store(&book, Some(store.clone())).unwrap()
            .unwrap().config_digest, "b");
        let (claim_key, _) = migration_keys(&book).unwrap();
        let source = ReportMigration::decode(store.get(&claim_key).unwrap().unwrap().as_slice()).unwrap();
        assert_eq!(source.reports.iter().map(|report| report.filename.as_str())
            .collect::<Vec<_>>(), vec!["a.pb", "b.pb"]);
        assert!(source.reports.iter().all(|report| report.record_time.is_some()));
        assert_eq!(source.reports[0].report_bytes, bytes("a"));
        fs::remove_dir_all(&book).unwrap();
        assert_eq!(newest_report_with_store(&book, Some(store.clone())).unwrap()
            .unwrap().config_digest, "b");
        let log = report_log(&book, store.clone()).unwrap();
        assert_eq!(log.height().unwrap(), 2);
        fs::remove_file(objects.join(format!("{}{:020}", prefix(&book).unwrap(), 1))).unwrap();
        let err = newest_report_with_store(&book, Some(store)).unwrap_err().to_string();
        assert!(err.contains("lost or changed"), "{err}");
    }

    #[test]
    fn a_hole_in_report_evidence_refuses_the_latest_report() {
        let book = root("hole");
        let store = Arc::new(MemoryStore::new());
        let log = report_log(&book, store.clone()).unwrap();
        let report = StoredReport { format_version: 1, book_id: book_id(&book).unwrap().into(),
            filename: "later.pb".into(), report_bytes: bytes("later"),
            record_time: Some(timestamp(SystemTime::now()).unwrap()) };
        assert!(log.claim(2, &report.encode_to_vec()).unwrap());
        let err = newest_report_with_store(&book, Some(store)).unwrap_err().to_string();
        assert!(err.contains("gap"), "{err}");
    }

    struct UnavailableStore;

    impl ObjectStore for UnavailableStore {
        fn put_if_absent(&self, _: &str, _: &[u8]) -> Result<bool> {
            bail!("object store unavailable")
        }
        fn get(&self, _: &str) -> Result<Option<Vec<u8>>> {
            bail!("object store unavailable")
        }
        fn list(&self, _: &str) -> Result<Vec<String>> {
            bail!("object store unavailable")
        }
    }

    #[test]
    fn a_configured_report_writer_refuses_an_unavailable_store() {
        let book = root("unavailable");
        let err = write_report_with_store(&book, "r.pb", &bytes("r"),
            Some(Arc::new(UnavailableStore))).unwrap_err().to_string();
        assert!(err.contains("object store unavailable"), "{err}");
        assert!(!book.join("reports/r.pb").exists());
    }
}
