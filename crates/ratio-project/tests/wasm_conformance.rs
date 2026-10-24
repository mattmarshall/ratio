//! Test-only ABI: no second accounting implementation or production transport.
use ratio_store::{
    ConfigStore, Digest, FileBook, JournalEntry, MemoryStore, ObjectStore, PostingRecord,
};
use std::sync::Arc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn ratio_result_ptr() -> *const u8 {
    OUTPUT.lock().unwrap().as_ptr()
}
#[no_mangle]
pub extern "C" fn ratio_result_len() -> usize {
    OUTPUT.lock().unwrap().len()
}

#[no_mangle]
pub extern "C" fn ratio_wasm_conformance() -> u32 {
    let objects = Arc::new(MemoryStore::new());
    let result =
        backend_fixture::exercise_with(|| FileBook::on_store("wasm", objects.clone()).unwrap());
    assert_eq!(result.0.len(), 3);
    assert_eq!(result.1.len(), 6);
    assert!(result.1.iter().all(|plane| plane.len() == 2));
    assert_eq!(
        result.2[3],
        "923bede0abad250c1ea53a8b12f2fe8fa7fbc17abf77de476b535545bbe04502"
    );
    assert_eq!(result.3, (21, 21));
    for plane in [
        "journal",
        "deliveries",
        "entities",
        "facts",
        "actions",
        "explanations",
        "closes",
    ] {
        let log = ratio_store::SeqLog::new(objects.clone(), format!("claims/{plane}/"));
        assert!(log.claim(1, b"first").unwrap());
        assert!(!log.claim(1, b"overwrite").unwrap());
        assert_eq!(log.get(1).unwrap().unwrap(), b"first");
        let gap = ratio_store::SeqLog::new(objects.clone(), format!("holes/{plane}/"));
        assert!(gap.claim(2, b"later").unwrap());
        assert!(gap
            .for_each_since(0, &mut |_, _| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("gap"));
    }
    *OUTPUT.lock().unwrap() = serde_json::to_vec(&result).unwrap();
    1
}

extern "C" {
    fn ratio_fold_progress(height: u32);
}

/// A read-only generated journal avoids measuring a second in-memory copy of
/// all JSON bodies. It is test data, never a writable storage backend.
struct GeneratedJournal {
    metadata: MemoryStore,
    height: AtomicU64,
    config: Mutex<Option<Digest>>,
}
impl ObjectStore for GeneratedJournal {
    fn put_if_absent(&self, key: &str, body: &[u8]) -> anyhow::Result<bool> {
        anyhow::ensure!(
            !key.starts_with("capacity/journal/"),
            "generated journal is read-only"
        );
        self.metadata.put_if_absent(key, body)
    }
    fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        if let Some(seq) = key.strip_prefix("capacity/journal/") {
            let seq: u64 = seq.parse()?;
            if seq == 0 || seq > self.height.load(Ordering::Relaxed) {
                return Ok(None);
            }
            unsafe {
                ratio_fold_progress((seq - 1) as u32);
            }
            let entry = JournalEntry {
                id: format!("buy-{seq}"),
                memo: "capacity fixture".into(),
                config: self.config.lock().unwrap().clone().unwrap(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: 100,
                        currency: Some("USD".into()),
                        instrument: Some(format!("S{}", seq % 10_000)),
                        quantity: Some(1),
                    },
                    PostingRecord::of_currency(2, -100, "USD"),
                ],
                trade_date: Some("2024-01-02".into()),
                announcement: None,
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
                kind: None,
            };
            return Ok(Some(serde_json::to_vec(&entry)?));
        }
        self.metadata.get(key)
    }
    fn list(&self, prefix: &str) -> anyhow::Result<Vec<String>> {
        anyhow::ensure!(
            prefix != "capacity/journal/",
            "capacity fixture must stream its journal"
        );
        self.metadata.list(prefix)
    }
    fn max_sequence(&self, prefix: &str) -> anyhow::Result<u64> {
        if prefix == "capacity/journal/" {
            Ok(self.height.load(Ordering::Relaxed))
        } else {
            self.metadata.max_sequence(prefix)
        }
    }
}

#[no_mangle]
pub extern "C" fn ratio_wasm_capacity(height: u32, views: u32) -> u32 {
    assert!((1..=2).contains(&views));
    let objects = Arc::new(GeneratedJournal {
        metadata: MemoryStore::new(),
        height: AtomicU64::new(0),
        config: Mutex::new(None),
    });
    let mut book = FileBook::on_store("capacity", objects.clone()).unwrap();
    let mut config = String::new();
    for index in 0..views {
        config.push_str(&format!(
            "[[view]]\nid = 'view{index}'\nbasis = 'recorded'\n"
        ));
    }
    let digest = book.put(config.as_bytes()).unwrap();
    book.set_active(&digest).unwrap();
    *objects.config.lock().unwrap() = Some(digest);
    objects.height.store(u64::from(height), Ordering::Relaxed);
    let mut projection = ratio_project::Projection::new();
    assert_eq!(projection.follow_book(&book).unwrap(), height as usize);
    for index in 0..views {
        assert_eq!(
            projection.open_lots(&format!("view{index}")).unwrap(),
            i64::from(height)
        );
        let held = projection.positions(&format!("view{index}")).unwrap();
        assert_eq!(held.prefix, height as usize);
        assert_eq!(
            held.value
                .held
                .values()
                .map(|(_, quantity)| quantity)
                .sum::<i64>(),
            i64::from(height)
        );
    }
    unsafe {
        ratio_fold_progress(height);
    }
    height
}
