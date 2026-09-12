//! Disposable local recovery evidence for #264. No AWS, credentials, or server.
//! These fixtures exercise directory recovery and characterize the legacy-book
//! gap when storage is attached after local creation; they make no production RPO claim.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{ensure, Result};
use ratio_console::{book::BookKind, Console, Subject};
use ratio_proto::ratio::console::v1 as pb;
use ratio_store::{ConfigStore, Digest, DirStore, FileBook, Journal, JournalEntry, PostingRecord};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("ratio-recovery-{}-{name}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    root
}

fn member(sub: &str) -> Subject {
    // Synthetic gateway claims, parsed through the same public boundary.
    // This local test does not claim to verify an OAuth token's signature.
    ratio_console::auth::from_request_context(&serde_json::json!({
        "authorizer": {"jwt": {"claims": {
            "sub": sub,
            "org_id": "same-org-is-not-a-grant"
        }}}
    }).to_string()).unwrap()
}

fn create(root: &Path, id: &str, who: &str, kind: BookKind) {
    Console::scoped(root, member(who))
        .create_book(pb::CreateBookRequest {
            book_id: id.into(),
            book: Some(pb::Book { kind: kind.proto(), ..Default::default() }),
        })
        .unwrap();
}

fn entry(id: &str, config: &Digest, amount: i64) -> JournalEntry {
    JournalEntry {
        id: id.into(),
        memo: "Recovery drill capital".into(),
        config: config.clone(),
        postings: vec![PostingRecord::new(2, amount), PostingRecord::new(20, -amount)],
        trade_date: Some("2026-06-01".into()),
        announcement: None,
        due_date: None,
        application: None,
        identified_lots: None,
        special_allocations: None,
        kind: None,
    }
}

// Runtime-only evidence, never serialized as a new interchange format.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for row in fs::read_dir(dir).unwrap() {
            let path = row.unwrap().path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // ⛔ DISPOSABLE ACCELERATION IS NOT CITEABLE BOOK CONTENT (#310).
            // Console projection checkpoints live under `.ratio-cache/` (and any
            // legacy `.projection-checkpoints/`). A restore drill that treated
            // them as part of the book would fail the moment a trial balance
            // warmed a fold — while the journal, config, and membership were
            // intact. Skip them here; they are not in the backup contract.
            if name == ".ratio-cache" || name == ".projection-checkpoints" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().into(), fs::read(path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn copy_tree(source: &Path, target: &Path) {
    assert!(!target.exists(), "a restore target must start empty");
    for (name, bytes) in files(source) {
        let path = target.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

fn verify_book(path: &Path, prefix: usize, digest: &str, configs: &[Digest]) -> Result<()> {
    let book = FileBook::open_with(path, None)?;
    let entries = book.entries()?;
    ensure!(entries.len() == prefix, "restored prefix differs");
    ensure!(ratio_nav::prefix_digest(&entries)? == digest, "restored journal digest differs");
    ensure!(book.active()? == configs.first().cloned(), "restored ACTIVE differs");
    ensure!(book.history()? == configs, "restored HISTORY differs");
    for config in configs {
        ensure!(Digest::of(&book.get(config)?) == *config, "restored config bytes differ");
    }
    Ok(())
}

#[test]
fn a_directory_restore_preserves_cited_history_and_isolates_books() {
    let root = scratch("whole-book");
    let live = root.join("live");
    create(&live, "alpha", "alice", BookKind::Investment);
    create(&live, "beta", "bob", BookKind::Personal);
    let alpha = live.join("alpha");
    let mut book = FileBook::open_with(&alpha, None).unwrap();
    let first = book.active().unwrap().unwrap();
    book.append(&entry("capital-1", &first, 50_000)).unwrap();
    let mut next = book.get(&first).unwrap();
    next.extend_from_slice(b"\n# Recovery drill promotion\n");
    let second = book.put(&next).unwrap();
    book.set_active(&second).unwrap();
    book.append(&entry("capital-2", &second, 10_000)).unwrap();
    let prefix = book.entries().unwrap().len();
    let digest = ratio_nav::prefix_digest(&book.entries().unwrap()).unwrap();
    assert_eq!(prefix, 2);
    let strike = ratio_nav::strike_and_record(
        &alpha, ratio_rules::UNDECLARED_VIEW, 1_781_784_000, "alice",
    ).unwrap();
    assert_eq!(strike.net_asset_value, 60_000);
    assert_eq!(strike.journal_position, prefix);
    assert_eq!(strike.config_digest, second.as_str());

    // Stop writers before the copy: this is a quiescent checkpoint, not an
    // atomic multi-plane snapshot invented by a directory traversal.
    drop(book);
    let captured = files(&live);
    let backup = root.join("backup");
    copy_tree(&live, &backup);
    fs::remove_dir_all(&live).unwrap();
    let restored = root.join("restored");
    copy_tree(&backup, &restored);
    assert_eq!(files(&restored), captured, "membership and metadata are part of the copy");
    let alpha = restored.join("alpha");
    let configs = [second, first];
    verify_book(&alpha, prefix, &digest, &configs).unwrap();
    let recovered = ratio_nav::list(&alpha).unwrap();
    assert_eq!(recovered, vec![strike.clone()]);
    assert!(ratio_nav::replay(&alpha, &recovered[0]).unwrap().ok());
    let alice = Console::scoped(&restored, member("alice"));
    let bob = Console::scoped(&restored, member("bob"));
    assert_eq!(alice.list_books().unwrap().books.len(), 1);
    assert_eq!(alice.get_book("books/alpha").unwrap().kind, BookKind::Investment.proto());
    assert!(alice.get_book("books/beta").is_err());
    assert_eq!(bob.get_book("books/beta").unwrap().kind, BookKind::Personal.proto());
    assert!(bob.get_book("books/alpha").is_err());
    assert!(Console::scoped(&restored, member("outsider")).list_books().unwrap().books.is_empty());

    // Sabotage changes provenance at the same prefix and leaves the NAV and
    // trial balance intact. A balance-only drill would accept this restore.
    let journal = alpha.join("journal.jsonl");
    let original = fs::read_to_string(&journal).unwrap();
    let changed = original.replacen("Recovery drill capital", "Wrong source attribution", 1);
    assert_ne!(changed, original);
    fs::write(&journal, changed).unwrap();
    let replay = ratio_nav::replay(&alpha, &strike).unwrap();
    assert_eq!(replay.net_asset_value, 60_000);
    assert!(!replay.history_intact);
    assert!(verify_book(&alpha, prefix, &digest, &configs).unwrap_err().to_string().contains("journal digest"));
    fs::write(&journal, original).unwrap();

    let blob = alpha.join("config").join(configs[0].as_str());
    let original = fs::read(&blob).unwrap();
    fs::write(&blob, b"rules = []\n").unwrap();
    assert!(verify_book(&alpha, prefix, &digest, &configs).unwrap_err().to_string().contains("config bytes"));
    fs::write(blob, original).unwrap();
    verify_book(&alpha, prefix, &digest, &configs).unwrap();
    assert_eq!(files(&restored), captured);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_legacy_book_attached_after_creation_does_not_gain_a_durable_bootstrap() {
    // Characterization of the legacy compatibility gap. Published CreateBook
    // state uses the durable bootstrap path and is covered by bootstrap tests.
    let root = scratch("journal-only");
    let live = root.join("live");
    create(&live, "household", "alice", BookKind::Personal);
    let local = live.join("household");
    let store = Arc::new(DirStore::at(root.join("objects")));
    let mut warm = FileBook::open_with(&local, Some(store.clone())).unwrap();
    let config = warm.active().unwrap().unwrap();
    warm.append(&entry("surviving-post", &config, 50_000)).unwrap();
    drop(warm);
    fs::remove_dir_all(live).unwrap();

    // Same basename reaches the same object keys; changing its parent does
    // not isolate a book in a shared object store.
    let cold_root = root.join("cold");
    let cold_path = cold_root.join("household");
    let cold = FileBook::open_with(&cold_path, Some(store)).unwrap();
    assert_eq!(cold.entries().unwrap()[0].id, "surviving-post");
    assert!(cold.active().unwrap().is_none());
    assert!(cold.history().unwrap().is_empty());
    assert!(cold.get(&config).is_err());
    assert!(cold.accounts().unwrap().is_empty());
    assert!(!cold_path.join("book.toml").exists());
    assert!(!cold_root.join("MEMBERSHIP.tsv").exists());
    assert!(Console::scoped(&cold_root, member("alice")).list_books().unwrap().books.is_empty());
    fs::remove_dir_all(root).unwrap();
}
