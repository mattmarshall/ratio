//! Build-enforced journal and side-plane equivalence across the local and
//! object-backed stores. The journal remains the book of record.

use ratio_project::checkpoint::{prefix_digest, prefix_digest_book};
use ratio_store::{ConfigStore, DirStore, FileBook, Journal, JournalEntry, MemoryStore,
    ObjectStore, Plane, PostingRecord, SeqLog};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join(format!("ratio-conformance-{label}-{}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)))
}

fn entry(id: &str, config: &ratio_store::Digest) -> JournalEntry {
    JournalEntry {
        id: id.into(), memo: "a cited posting".into(), config: config.clone(),
        postings: vec![
            PostingRecord { dim: 1, amount: 7, currency: None, instrument: None, quantity: None },
            PostingRecord { dim: 2, amount: -7, currency: None, instrument: None, quantity: None },
        ],
        trade_date: Some("2024-01-02".into()), announcement: None,
        due_date: None, application: None, identified_lots: None,
        special_allocations: None, kind: None,
    }
}

const PLANES: [Plane; 6] = [Plane::Deliveries, Plane::Entities, Plane::Facts,
    Plane::Actions, Plane::Explanations, Plane::Closes];

fn exercise(root: &PathBuf, store: Option<Arc<dyn ObjectStore>>) -> (Vec<String>, Vec<Vec<String>>, Vec<String>) {
    let mut book = FileBook::open_with(root, store.clone()).unwrap();
    let config = book.put(b"rules = []\n").unwrap();
    book.set_active(&config).unwrap();
    let mut digests = Vec::new();
    digests.push(prefix_digest(&[]).unwrap());
    for id in ["first", "second", "third"] {
        book.append(&entry(id, &config)).unwrap();
        let entries = book.entries().unwrap();
        let digest = prefix_digest(&entries).unwrap();
        assert_eq!(digest, prefix_digest_book(&book, entries.len()).unwrap());
        assert_eq!(digest, ratio_nav::prefix_digest(&entries).unwrap());
        digests.push(digest);
    }
    for (index, plane) in PLANES.iter().enumerate() {
        for sequence in 0..2 {
            book.append_record(*plane, &serde_json::json!({
                "plane": index, "sequence": sequence, "cite": "fixture"
            })).unwrap();
        }
    }
    drop(book);
    let reopened = FileBook::open_with(root, store).unwrap();
    let journal = reopened.entries().unwrap().iter().map(|e| serde_json::to_string(e).unwrap()).collect();
    let planes = PLANES.iter().map(|p| reopened.records::<serde_json::Value>(*p).unwrap()
        .iter().map(|v| serde_json::to_string(v).unwrap()).collect()).collect();
    assert_eq!(prefix_digest_book(&reopened, 3).unwrap(), digests[3]);
    (journal, planes, digests)
}

#[test]
fn journal_and_six_side_planes_replay_identically_across_backends() {
    let local = exercise(&root("local"), None);
    let memory = exercise(&root("memory"), Some(Arc::new(MemoryStore::new())));
    let dir = exercise(&root("objects"), Some(Arc::new(DirStore::at(root("object-files")))));
    assert_eq!(local, memory);
    assert_eq!(local, dir);
}

#[test]
fn both_object_backends_refuse_lost_claims_and_holes() {
    for store in [Arc::new(MemoryStore::new()) as Arc<dyn ObjectStore>,
                  Arc::new(DirStore::at(root("holes"))) as Arc<dyn ObjectStore>] {
        let log = SeqLog::new(store, "journal/");
        assert!(log.claim(2, b"later").unwrap());
        let err = log.for_each_since(0, &mut |_, _| Ok(())).unwrap_err().to_string();
        assert!(err.contains("missing") && err.contains("gap"), "{err}");
        assert!(log.claim(1, b"first").unwrap());
        assert!(!log.claim(1, b"lost").unwrap());
        assert_eq!(log.get(1).unwrap().as_deref(), Some(b"first".as_ref()));
    }
}
