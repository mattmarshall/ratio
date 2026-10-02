//! Disposable object-store recovery for #300. A fresh object namespace and
//! empty book root must reproduce the cited evidence and membership boundary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use prost::Message;
use ratio_console::{book::BookKind, BreakExplanation, Console, Subject};
use ratio_proto::ratio::{console::v1 as pb, v1::BreakReport};
use ratio_store::{changes, proposals, reports, CloseRecord, ConfigStore, DirStore, FileBook,
    Journal, ObjectStore, Plane};

fn scratch() -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR").map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let root = base.join(format!("ratio-operational-recovery-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn copy_tree(source: &Path, target: &Path) {
    assert!(!target.exists(), "restore destination must start empty");
    fn copy(source: &Path, target: &Path) {
        fs::create_dir_all(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let from = entry.path();
            let to = target.join(entry.file_name());
            if from.is_dir() { copy(&from, &to); }
            else { fs::copy(from, to).unwrap(); }
        }
    }
    copy(source, target);
}

fn member(sub: &str) -> Subject {
    ratio_console::auth::from_request_context(&serde_json::json!({
        "authorizer":{"jwt":{"claims":{"sub":sub,"org_id":"shared-org-is-not-a-grant"}}}
    }).to_string()).unwrap()
}

fn create(root: &Path, store: Arc<dyn ObjectStore>, id: &str, sub: &str, kind: BookKind) {
    Console::scoped(root, member(sub)).with_object_store(store)
        .create_book(pb::CreateBookRequest {
            book_id: id.into(),
            book: Some(pb::Book { kind: kind.proto(), ..Default::default() }),
        }).unwrap();
}

#[test]
fn an_object_backup_restores_evidence_and_refuses_missing_planes() {
    let root = scratch();
    let live = root.join("live");
    let source_objects = root.join("source-objects");
    let source: Arc<dyn ObjectStore> = Arc::new(DirStore::at(&source_objects));
    create(&live, source.clone(), "alpha", "alice", BookKind::Investment);
    create(&live, source.clone(), "beta", "bob", BookKind::Personal);
    let alpha = live.join("alpha");
    let mut book = FileBook::open_with(&alpha, Some(source.clone())).unwrap();
    let config = book.active().unwrap().unwrap();
    let entry: ratio_store::JournalEntry = serde_json::from_value(serde_json::json!({
        "id":"capital", "config":config.as_str(),
        "postings":[{"dim":1,"amount":12345},{"dim":20,"amount":-12345}]
    })).unwrap();
    book.append(&entry).unwrap();
    let digest = ratio_nav::prefix_digest(&book.entries().unwrap()).unwrap();
    let explanation = BreakExplanation {
        break_id: "cash".into(), text: "Reviewed source statement".into(),
        actor: "alice".into(), accept_time: 1_782_800_000, difference: 0,
        config_digest: config.as_str().into(), journal_position: 1,
        journal_digest: digest.clone(),
    };
    book.append_record(Plane::Explanations, &explanation).unwrap();
    book.append_record(Plane::Explanations, &explanation).unwrap();
    let close = CloseRecord {
        view: ratio_rules::UNDECLARED_VIEW.into(), closed_date: "2026-06-30".into(),
        journal_position: 1, journal_digest: digest.clone(),
        config_digest: config.as_str().into(), closing_entry: None,
        actor: "alice".into(), recorded: 1_782_800_001,
        equity_destination: 20, surplus: None,
    };
    book.append_record(Plane::Closes, &close).unwrap();
    book.append_record(Plane::Closes, &close).unwrap();
    drop(book);
    let strike = ratio_nav::strike_and_record_with_store(&alpha,
        ratio_rules::UNDECLARED_VIEW, 1_782_800_000, "alice", Some(source.clone())).unwrap();
    let first = BreakReport {
        name: "books/alpha/breakReports/first".into(),
        config_digest: config.as_str().into(), entries_posted: 1,
        book_ties: true, ..Default::default()
    }.encode_to_vec();
    let second = BreakReport {
        name: "books/alpha/breakReports/second".into(),
        config_digest: config.as_str().into(), entries_posted: 1,
        book_ties: true, ..Default::default()
    }.encode_to_vec();
    reports::write_report_with_store(&alpha, "first.pb", &first, Some(source.clone())).unwrap();
    reports::write_report_with_store(&alpha, "second.pb", &second, Some(source.clone())).unwrap();
    proposals::write_with_store(&alpha, "draft", "rules = []\n", Some(source.clone())).unwrap();
    changes::append_with_store(&alpha, "1\talice\taccepted\tcash\tdigest\n",
        Some(source.clone())).unwrap();
    assert!(!alpha.join("NAVS").exists());
    assert!(!alpha.join("reports").exists());
    assert!(!alpha.join("proposals").exists());
    assert!(!alpha.join("CHANGELOG").exists());

    // Quiesce the writers. Copy object bytes into an independent backup, then
    // remove both original roots before restoring to a fresh object namespace.
    let backup = root.join("backup");
    copy_tree(&source_objects, &backup);
    fs::remove_dir_all(&live).unwrap();
    fs::remove_dir_all(&source_objects).unwrap();
    let objects = root.join("restored-objects");
    copy_tree(&backup, &objects);
    let restored = root.join("restored-books");
    let alpha = restored.join("alpha");
    let recovered: Arc<dyn ObjectStore> = Arc::new(DirStore::at(&objects));
    let alice = Console::scoped(&restored, member("alice")).with_object_store(recovered.clone());
    let bob = Console::scoped(&restored, member("bob")).with_object_store(recovered.clone());
    assert_eq!(alice.list_books().unwrap().books.len(), 1);
    assert_eq!(bob.list_books().unwrap().books.len(), 1);
    assert!(alice.get_book("books/beta").is_err());
    assert!(bob.get_book("books/alpha").is_err());
    assert!(Console::scoped(&restored, member("outsider"))
        .with_object_store(recovered.clone()).list_books().unwrap().books.is_empty());
    let book = FileBook::open_with(&alpha, Some(recovered.clone())).unwrap();
    assert_eq!(book.entries().unwrap().len(), 1);
    assert_eq!(ratio_nav::prefix_digest(&book.entries().unwrap()).unwrap(), digest);
    assert_eq!(book.closes().unwrap(), vec![close.clone(), close]);
    assert_eq!(book.records::<BreakExplanation>(Plane::Explanations).unwrap().len(), 2);
    assert_eq!(ratio_nav::list_with_store(&alpha, Some(recovered.clone())).unwrap(), vec![strike.clone()]);
    assert!(ratio_nav::replay_with_store(&alpha, &strike, Some(recovered.clone())).unwrap().ok());
    assert_eq!(reports::newest_report_with_store(&alpha, Some(recovered.clone()))
        .unwrap().unwrap().encode_to_vec(), second);
    assert_eq!(proposals::read_with_store(&alpha, "draft", Some(recovered.clone()))
        .unwrap().as_deref(), Some("rules = []\n"));
    assert!(changes::read_with_store(&alpha, Some(recovered.clone())).unwrap()
        .iter().any(|line| line.contains("alice\taccepted\tcash")));

    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "a_fresh_process_replays_the_restored_object_book", "--nocapture"])
        .env("RATIO_RECOVERY_CHILD_ROOT", &restored)
        .env("RATIO_RECOVERY_CHILD_OBJECTS", &objects)
        .env("RATIO_RECOVERY_CHILD_DIGEST", &digest)
        .output().unwrap();
    assert!(child.status.success(), "fresh process: {}",
        String::from_utf8_lossy(&child.stdout));
    assert!(String::from_utf8_lossy(&child.stdout).contains("1 passed"),
        "fresh process test was not selected");

    // Removing an earlier sequence cannot turn a partial plane into a valid
    // shorter history. Corrupting claim objects cannot become an empty list.
    for plane in ["closes", "explanations", "reports", "changes"] {
        let key = format!("alpha/{plane}/{:020}", 1);
        let path = objects.join(&key);
        let bytes = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        let result = match plane {
            "closes" => FileBook::open_with(&alpha, Some(recovered.clone()))
                .and_then(|b| b.closes().map(|_| ())),
            "explanations" => FileBook::open_with(&alpha, Some(recovered.clone()))
                .and_then(|b| b.records::<BreakExplanation>(Plane::Explanations).map(|_| ())),
            "reports" => reports::newest_report_with_store(&alpha, Some(recovered.clone()))
                .map(|_| ()),
            _ => changes::read_with_store(&alpha, Some(recovered.clone())).map(|_| ()),
        };
        assert!(result.unwrap_err().to_string().contains("missing"), "{plane}");
        fs::write(path, bytes).unwrap();
    }
    for prefix in ["alpha/nav-strikes/", "alpha/proposals/"] {
        let key = recovered.list(prefix).unwrap().pop().unwrap();
        let path = objects.join(key);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, b"corrupt").unwrap();
        let result = if prefix.contains("nav") {
            ratio_nav::list_with_store(&alpha, Some(recovered.clone())).map(|_| ())
        } else {
            proposals::read_with_store(&alpha, "draft", Some(recovered.clone())).map(|_| ())
        };
        assert!(result.is_err(), "{prefix} must refuse corrupt evidence");
        fs::write(path, bytes).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_fresh_process_replays_the_restored_object_book() {
    let Some(root) = std::env::var_os("RATIO_RECOVERY_CHILD_ROOT") else { return; };
    let objects = PathBuf::from(std::env::var_os("RATIO_RECOVERY_CHILD_OBJECTS").unwrap());
    let expected = std::env::var("RATIO_RECOVERY_CHILD_DIGEST").unwrap();
    let root = PathBuf::from(root);
    let alpha = root.join("alpha");
    let store: Arc<dyn ObjectStore> = Arc::new(DirStore::at(objects));
    let book = FileBook::open_with(&alpha, Some(store.clone())).unwrap();
    assert_eq!(ratio_nav::prefix_digest(&book.entries().unwrap()).unwrap(), expected);
    assert_eq!(book.closes().unwrap().len(), 2);
    assert_eq!(book.records::<BreakExplanation>(Plane::Explanations).unwrap().len(), 2);
    let strike = ratio_nav::list_with_store(&alpha, Some(store.clone())).unwrap().pop().unwrap();
    assert!(ratio_nav::replay_with_store(&alpha, &strike, Some(store.clone())).unwrap().ok());
    assert_eq!(reports::newest_report_with_store(&alpha, Some(store.clone()))
        .unwrap().unwrap().name, "books/alpha/breakReports/second");
    assert_eq!(proposals::read_with_store(&alpha, "draft", Some(store.clone()))
        .unwrap().as_deref(), Some("rules = []\n"));
    assert!(changes::read_with_store(&alpha, Some(store.clone())).unwrap()
        .iter().any(|line| line.contains("alice\taccepted\tcash")));
    assert!(Console::scoped(&root, member("bob"))
        .with_object_store(store).get_book("books/alpha").is_err());
}
