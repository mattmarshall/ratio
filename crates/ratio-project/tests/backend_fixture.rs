//! One fixture for every journal backend in the #359 Bazel gate.

use ratio_project::checkpoint::{prefix_digest, prefix_digest_book};
use ratio_store::{ConfigStore, FileBook, Journal, JournalEntry, ObjectStore, Plane, PostingRecord};
use std::path::Path;
use std::sync::Arc;

pub type ResultShape = (Vec<String>, Vec<Vec<String>>, Vec<String>, (i64, i64));

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

pub const PLANES: [Plane; 6] = [Plane::Deliveries, Plane::Entities, Plane::Facts,
    Plane::Actions, Plane::Explanations, Plane::Closes];

pub fn exercise(root: &Path, store: Option<Arc<dyn ObjectStore>>) -> ResultShape {
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
    let journal = reopened.entries().unwrap().iter()
        .map(|e| serde_json::to_string(e).unwrap()).collect();
    let planes = PLANES.iter().map(|p| reopened.records::<serde_json::Value>(*p).unwrap()
        .iter().map(|v| serde_json::to_string(v).unwrap()).collect()).collect();
    assert_eq!(prefix_digest_book(&reopened, 3).unwrap(), digests[3]);
    let balance = reopened.trial_balance().unwrap();
    assert_eq!(balance.debits, balance.credits);
    (journal, planes, digests, (balance.debits, balance.credits))
}
