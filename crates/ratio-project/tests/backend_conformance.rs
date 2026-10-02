//! Build-enforced journal and side-plane equivalence across the local and
//! object-backed stores. The journal remains the book of record.

use backend_fixture::exercise;
use ratio_store::{DirStore, MemoryStore, ObjectStore, SeqLog};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join(format!("ratio-conformance-{label}-{}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)))
}

const PREFIXES: [&str; 7] = ["journal/", "deliveries/", "entities/", "facts/",
    "actions/", "explanations/", "closes/"];

fn claim_contract(store: Arc<dyn ObjectStore>, prefix: &str) -> Result<(), String> {
    let log = SeqLog::new(store, prefix);
    if !log.claim(1, b"first").map_err(|e| e.to_string())? {
        return Err(format!("{prefix}: first conditional claim was refused"));
    }
    if log.claim(1, b"lost").map_err(|e| e.to_string())? {
        return Err(format!("{prefix}: conditional append accepted an overwrite"));
    }
    if log.get(1).map_err(|e| e.to_string())?.as_deref() != Some(b"first".as_ref()) {
        return Err(format!("{prefix}: an acknowledged entry was replaced"));
    }
    Ok(())
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
        for prefix in PREFIXES {
            let key = format!("{prefix}hole/");
            let log = SeqLog::new(store.clone(), &key);
            assert!(log.claim(2, b"later").unwrap());
            let err = log.for_each_since(0, &mut |_, _| Ok(())).unwrap_err().to_string();
            assert!(err.contains("missing") && err.contains("gap"), "{key}: {err}");
            claim_contract(store.clone(), &format!("{prefix}claim/"))
                .unwrap_or_else(|reason| panic!("{reason}"));
        }
    }
}

#[test]
fn unconditional_memory_backend_fails_for_the_conditional_claim_it_breaks() {
    let error = claim_contract(Arc::new(MemoryStore::unconditional()), "journal/")
        .unwrap_err();
    assert!(error.contains("conditional append accepted an overwrite"), "{error}");
}
