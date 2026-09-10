//! Actual publication and cold-root recovery, using explicit independent stores.
use super::*;
use prost::Message;
use ratio_store::{
    bootstrap::{BookBootstrap, BookPublication, BootstrapStore},
    control::{
        control_operation, membership_revision, ConfigPromotion, ControlOperation, ControlStore,
        MembershipRevision,
    },
    Digest, DirStore, MemoryStore, ObjectStore,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "ratio-bootstrap-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn member(sub: &str) -> Subject {
    auth::from_request_context(&serde_json::json!({"authorizer":{"jwt":{"claims":{"sub":sub,"email":"same@example.test","org_id":"same-org"}}}}).to_string()).unwrap()
}
fn console(root: &Path, objects: Arc<dyn ObjectStore>, sub: &str) -> Console {
    Console::scoped(root, member(sub)).with_object_store(objects)
}
fn request(id: &str, kind: book::BookKind) -> pb::CreateBookRequest {
    pb::CreateBookRequest {
        book_id: id.into(),
        book: Some(pb::Book {
            kind: kind.proto(),
            display_name: format!("Exact {id}"),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn entry(digest: &Digest) -> ratio_store::JournalEntry {
    serde_json::from_value(serde_json::json!({"id":"opening", "config":digest.as_str(), "postings":[{"dim":1,"amount":12345},{"dim":20,"amount":-12345}]})).unwrap()
}

fn control_operation(
    control: &ControlStore,
    book: &str,
    id: &str,
    actor: &str,
    change: control_operation::Change,
) -> ControlOperation {
    let state = control.read(book).unwrap();
    ControlOperation {
        format_version: 1,
        book_id: book.into(),
        bootstrap_digest: state.bootstrap_digest,
        expected_revision: state.revision,
        expected_predecessor_digest: state.predecessor_digest,
        operation_id: id.into(),
        actor_subject: actor.into(),
        actor_provenance: "verified-test-context".into(),
        change: Some(change),
    }
}

fn membership(
    action: membership_revision::Action,
    principal: membership_revision::Principal,
) -> control_operation::Change {
    control_operation::Change::MembershipRevision(MembershipRevision {
        action: action as i32,
        principal: Some(principal),
    })
}

#[test]
fn all_kinds_recover_exact_bytes_grants_and_the_journal_from_an_empty_root() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    let original = temp.0.join("first");
    let recovered = temp.0.join("second");
    for kind in [
        book::BookKind::Personal,
        book::BookKind::Investment,
        book::BookKind::Project,
        book::BookKind::Operating,
    ] {
        let id = kind.as_str();
        console(&original, objects.clone(), "creator")
            .create_book(request(id, kind))
            .unwrap();
        let (_, state) = BootstrapStore::new(objects.clone())
            .get(id)
            .unwrap()
            .unwrap();
        assert_eq!(
            state.chart,
            serde_json::to_vec_pretty(&book::chart_for(kind)).unwrap()
        );
        assert_eq!(state.config, book::config_for(kind).as_bytes());
        let mut b = FileBook::open_with(original.join(id), Some(objects.clone())).unwrap();
        let digest = b.active().unwrap().unwrap();
        b.append(&entry(&digest)).unwrap();
        assert_eq!(b.history().unwrap(), vec![digest]);
    }
    std::fs::remove_dir_all(&original).unwrap();
    let reader = console(&recovered, objects.clone(), "creator");
    let rows = reader.list_books().unwrap().books;
    assert_eq!(rows.len(), 4);
    for row in rows {
        let id = row.name.strip_prefix("books/").unwrap();
        let (_, state) = BootstrapStore::new(objects.clone())
            .get(id)
            .unwrap()
            .unwrap();
        assert_eq!(row.kind, state.kind);
        assert_eq!(row.display_name, state.display_name);
        assert_eq!(row.config_digest, state.config_digest);
        assert_eq!(row.entry_count, 1);
        assert!(row.fund.is_empty() && row.organization.is_empty());
        let b = FileBook::open_with(recovered.join(id), Some(objects.clone())).unwrap();
        assert_eq!(b.get(&b.active().unwrap().unwrap()).unwrap(), state.config);
        assert_eq!(
            b.entries().unwrap(),
            vec![entry(&b.active().unwrap().unwrap())]
        );
        assert_eq!(reader.get_book(&row.name).unwrap().entry_count, 1);
        assert_eq!(reader.projection(id).unwrap().prefix(), 1);
        let staged = console(&recovered, objects.clone(), "creator")
            .with_stage_e_store(ratio_sql_project::ProjectionReads::in_process());
        let parent = format!("funds/{id}/views/{}", row.default_view);
        let expected = reader.list_accounts(&parent, "").unwrap();
        assert_eq!(staged.list_accounts(&parent, "").unwrap(), expected);
        let (pin, _) = staged.stage_e_pin(id).unwrap().unwrap();
        assert_eq!(pin.prefix, 1);
        assert_eq!(
            pin.digest,
            ratio_nav::prefix_digest(&b.entries().unwrap()).unwrap()
        );
    }
    assert!(!recovered.join("MEMBERSHIP.tsv").exists());
    assert!(console(&recovered, objects.clone(), "stranger")
        .list_books()
        .unwrap()
        .books
        .is_empty());
    assert!(Console::open(&recovered, member("stranger"))
        .with_object_store(objects.clone())
        .list_books()
        .unwrap()
        .books
        .is_empty());
    assert!(console(&recovered, objects, "stranger")
        .get_book("books/personal")
        .is_err());
}

#[test]
fn one_console_resolves_durable_membership_again_at_every_operation_boundary() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    console(&temp.0, objects.clone(), "creator")
        .create_book(request("book", book::BookKind::Personal))
        .unwrap();
    let creator = console(&temp.0, objects.clone(), "creator");
    let guest = console(&temp.0, objects.clone(), "guest");
    assert!(creator.get_book("books/book").is_ok());
    assert!(guest.get_book("books/book").is_err());

    let control = ControlStore::new(objects.clone());
    control
        .commit(&control_operation(
            &control,
            "book",
            "grant-guest",
            "creator",
            membership(
                membership_revision::Action::Grant,
                membership_revision::Principal::AuthkitSubject("guest".into()),
            ),
        ))
        .unwrap();
    assert!(guest.get_book("books/book").is_ok());
    control
        .commit(&control_operation(
            &control,
            "book",
            "revoke-creator",
            "guest",
            membership(
                membership_revision::Action::Revoke,
                membership_revision::Principal::AuthkitSubject("creator".into()),
            ),
        ))
        .unwrap();
    assert!(creator.get_book("books/book").is_err());
}

#[test]
fn explicit_organization_membership_never_delegates_to_connect() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    console(&temp.0, objects.clone(), "creator")
        .create_book(request("book", book::BookKind::Personal))
        .unwrap();
    let control = ControlStore::new(objects.clone());
    control
        .commit(&control_operation(
            &control,
            "book",
            "grant-org",
            "creator",
            membership(
                membership_revision::Action::Grant,
                membership_revision::Principal::OrganizationId("same-org".into()),
            ),
        ))
        .unwrap();
    assert!(console(&temp.0, objects.clone(), "other-authkit-sub")
        .get_book("books/book")
        .is_ok());

    let mut connect = member("connect-sub");
    if let Subject::Member { connect, .. } = &mut connect {
        *connect = true;
    }
    let client = Console::scoped(&temp.0, connect).with_object_store(objects);
    assert!(client.get_book("books/book").is_err());
}

#[test]
fn a_posting_keeps_the_configuration_digest_captured_before_a_promotion() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    console(&temp.0, objects.clone(), "creator")
        .create_book(request("book", book::BookKind::Personal))
        .unwrap();
    let mut in_flight =
        FileBook::open_with(temp.0.join("book"), Some(objects.clone())).unwrap();
    let pinned = in_flight.active().unwrap().unwrap();
    let control = ControlStore::new(objects);
    let next = control
        .stage_config(b"rules = []\n# promoted later\n")
        .unwrap();
    control
        .commit(&control_operation(
            &control,
            "book",
            "promotion",
            "creator",
            control_operation::Change::ConfigPromotion(ConfigPromotion {
                config_digest: next.as_str().into(),
            }),
        ))
        .unwrap();
    assert_eq!(in_flight.active().unwrap(), Some(next));
    in_flight.append(&entry(&pinned)).unwrap();
    assert_eq!(in_flight.entries().unwrap()[0].config, pinned);
}

#[test]
fn persisted_identity_escapes_round_trip_and_a_creator_does_not_delegate_to_connect() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    let mut req = request("identity", book::BookKind::Personal);
    let display = "A \"quoted\" name\nwith a backslash \\ and λ";
    req.book.as_mut().unwrap().display_name = display.into();
    let c = console(&temp.0.join("first"), objects.clone(), "creator");
    assert_eq!(c.create_book(req).unwrap().display_name, display);
    let reader = console(&temp.0.join("second"), objects.clone(), "creator");
    assert_eq!(reader.list_books().unwrap().books[0].display_name, display);
    let mut delegated = member("creator");
    if let Subject::Member { connect, .. } = &mut delegated {
        *connect = true;
    }
    let client = Console::scoped(&temp.0.join("second"), delegated).with_object_store(objects);
    assert!(client.list_books().unwrap().books.is_empty());
    assert!(client
        .create_book(request("client-created", book::BookKind::Personal))
        .is_err());
}

#[test]
fn staging_and_crash_debris_stay_invisible_during_concurrent_creation_and_listing() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    let debris = temp.0.join(".bootstrap-crashed-0");
    std::fs::create_dir(&debris).unwrap();
    std::fs::write(debris.join("accounts.json"), b"[]").unwrap();
    std::fs::write(debris.join("BOOTSTRAP.pb"), b"incomplete").unwrap();
    let reader = console(&temp.0, objects.clone(), "creator");
    assert!(reader.list_books().unwrap().books.is_empty());
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for n in 0..8 {
                console(&temp.0, objects.clone(), "creator")
                    .create_book(request(&format!("created-{n}"), book::BookKind::Personal))
                    .unwrap();
            }
        });
        for _ in 0..24 {
            let rows = reader.list_books().unwrap().books;
            assert!(rows
                .iter()
                .all(|row| row.name.starts_with("books/created-")));
        }
    });
    assert_eq!(reader.list_books().unwrap().books.len(), 8);
}

#[test]
fn published_books_keep_their_kind_beside_a_legacy_single_book_root() {
    let temp = Temp::new();
    book::initialize(&temp.0, "demo", "Legacy", book::BookKind::Investment).unwrap();
    std::fs::remove_file(temp.0.join("book.toml")).unwrap();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    console(&temp.0, objects.clone(), "creator")
        .create_book(request("personal", book::BookKind::Personal))
        .unwrap();
    let reader = Console::new(&temp.0).with_object_store(objects);
    assert_eq!(reader.list_books().unwrap().books.len(), 2);
    let funds = reader.list_funds().unwrap().funds;
    assert_eq!(funds.len(), 1);
    assert_eq!(funds[0].name, "funds/demo");
}

fn race(objects: Arc<dyn ObjectStore>, root: &Path) {
    let start = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (sub, kind) in [
            ("alice", book::BookKind::Personal),
            ("bob", book::BookKind::Project),
        ] {
            let (objects, start, path) = (objects.clone(), start.clone(), root.join(sub));
            handles.push(scope.spawn(move || {
                let c = console(&path, objects, sub);
                start.wait();
                (sub, kind, c.create_book(request("shared", kind)).is_ok())
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        results.iter().filter(|(_, _, ok)| *ok).count(),
        1,
        "only one creator may succeed: {results:?}"
    );
    let winner = results.iter().find(|(_, _, ok)| *ok).unwrap();
    let loser = results.iter().find(|(_, _, ok)| !*ok).unwrap();
    let (_, state) = BootstrapStore::new(objects.clone())
        .get("shared")
        .unwrap()
        .unwrap();
    assert_eq!(state.kind, winner.1.proto());
    assert_eq!(state.creator_grant.unwrap().subject, winner.0);
    assert!(!root.join(loser.0).join("shared").exists());
    let reader_root = root.join("third");
    assert_eq!(
        console(&reader_root, objects.clone(), winner.0)
            .list_books()
            .unwrap()
            .books
            .len(),
        1
    );
    assert!(console(&reader_root, objects.clone(), loser.0)
        .list_books()
        .unwrap()
        .books
        .is_empty());
    assert!(console(&reader_root, objects, winner.0)
        .create_book(request("shared", winner.1))
        .is_err());
}
#[test]
fn racing_creators_cannot_replace_the_winner_or_inherit_its_grant() {
    let temp = Temp::new();
    race(Arc::new(MemoryStore::new()), &temp.0.join("memory"));
    race(
        Arc::new(DirStore::at(temp.0.join("objects"))),
        &temp.0.join("directory"),
    );
}

#[derive(Default)]
struct FaultStore {
    objects: MemoryStore,
    reads: Mutex<BTreeMap<String, Option<Vec<u8>>>>,
    fail_publication: bool,
}
impl ObjectStore for FaultStore {
    fn put_if_absent(&self, key: &str, bytes: &[u8]) -> Result<bool> {
        if self.fail_publication && key.starts_with("_bootstrap/publications/") {
            bail!("injected publication failure");
        }
        self.objects.put_if_absent(key, bytes)
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(answer) = self.reads.lock().unwrap().get(key) {
            return Ok(answer.clone());
        }
        self.objects.get(key)
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        self.objects.list(prefix)
    }
}
fn unchecked(objects: &dyn ObjectStore, mut state: BookBootstrap, mutation: usize) {
    match mutation {
        0 => state.chart.clear(),
        1 => state.config.clear(),
        2 => state.active.clear(),
        3 => state.history.clear(),
        4 => state.creator_grant = None,
        5 => state.book_id = "other".into(),
        6 => state.format_version = 2,
        7 => state.kind = 0,
        8 => state.chart_digest = "0".repeat(64),
        9 => state.config_digest = "0".repeat(64),
        10 => state.creator_grant.as_mut().unwrap().book_id = "other".into(),
        11 => state.creator_subject.clear(),
        12 => state.display_name.clear(),
        13 => {
            state.config = b"[broken".to_vec();
            state.config_digest = Digest::of(&state.config).as_str().into();
            state.active = state.config_digest.clone();
            state.history = vec![state.active.clone()];
        }
        _ => unreachable!(),
    }
    let bytes = state.encode_to_vec();
    let digest = Digest::of(&bytes).as_str().to_string();
    objects
        .put_if_absent(&format!("_bootstrap/blobs/{digest}"), &bytes)
        .unwrap();
    objects
        .put_if_absent(
            "_bootstrap/publications/broken",
            &BookPublication {
                format_version: 1,
                book_id: "broken".into(),
                bootstrap_digest: digest,
            }
            .encode_to_vec(),
        )
        .unwrap();
}
#[test]
fn every_required_plane_and_relationship_refuses_without_creating_a_cache() {
    let temp = Temp::new();
    for mutation in 0..14 {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        unchecked(
            objects.as_ref(),
            book::bootstrap("broken", "Broken", book::BookKind::Personal, "creator").unwrap(),
            mutation,
        );
        let root = temp.0.join(mutation.to_string());
        let c = console(&root, objects, "creator");
        assert!(
            c.list_books().is_err(),
            "mutation {mutation} produced defaults"
        );
        assert!(
            c.get_book("books/broken").is_err(),
            "mutation {mutation} opened a book"
        );
        assert!(
            !root.exists(),
            "mutation {mutation} created a partial cache"
        );
    }
}
#[test]
fn missing_corrupt_and_version_invalid_objects_refuse_even_after_a_prior_read() {
    let temp = Temp::new();
    let objects = Arc::new(FaultStore::default());
    let state = book::bootstrap("book", "Book", book::BookKind::Personal, "creator").unwrap();
    let pubrec = BootstrapStore::new(objects.clone())
        .publish(&state)
        .unwrap();
    let c = console(&temp.0.join("cache"), objects.clone(), "creator");
    assert_eq!(c.list_books().unwrap().books.len(), 1);
    let publication_key = "_bootstrap/publications/book".to_string();
    let blob_key = format!("_bootstrap/blobs/{}", pubrec.bootstrap_digest);
    let mut invalid = pubrec.clone();
    invalid.format_version = 2;
    let mut mismatched = pubrec.clone();
    mismatched.book_id = "other".into();
    for (key, response) in [
        (blob_key.clone(), None),
        (blob_key, Some(vec![0, 255, 42])),
        (publication_key.clone(), None),
        (publication_key.clone(), Some(vec![255])),
        (publication_key.clone(), Some(invalid.encode_to_vec())),
        (publication_key, Some(mismatched.encode_to_vec())),
    ] {
        objects.reads.lock().unwrap().insert(key.clone(), response);
        assert!(c.list_books().is_err(), "{key} did not refuse");
        assert!(
            c.get_book("books/book").is_err(),
            "{key} opened cached defaults"
        );
        objects.reads.lock().unwrap().clear();
    }
}
#[test]
fn failed_publication_has_neither_a_local_book_nor_a_grant() {
    let temp = Temp::new();
    let objects = Arc::new(FaultStore {
        fail_publication: true,
        ..Default::default()
    });
    let c = console(&temp.0, objects.clone(), "creator");
    assert!(c
        .create_book(request("unpublished", book::BookKind::Personal))
        .is_err());
    assert!(!temp.0.join("unpublished").exists());
    assert!(BootstrapStore::new(objects.clone())
        .ids()
        .unwrap()
        .is_empty());
    assert!(c.list_books().unwrap().books.is_empty());
    assert!(
        !objects.list("_bootstrap/blobs/").unwrap().is_empty(),
        "failure was injected after staging"
    );
}
#[test]
fn legacy_seeds_are_not_published_and_conflicting_or_changed_caches_are_not_reset() {
    let temp = Temp::new();
    let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
    let seed = temp.0.join("seed");
    book::initialize(&seed, "seed", "Original seed", book::BookKind::Investment).unwrap();
    let before = std::fs::read(seed.join("accounts.json")).unwrap();
    let c = console(&temp.0, objects.clone(), "creator");
    assert!(c
        .create_book(request("seed", book::BookKind::Personal))
        .is_err());
    FileBook::open_with(&seed, Some(objects.clone())).unwrap();
    assert!(BootstrapStore::new(objects.clone())
        .ids()
        .unwrap()
        .is_empty());
    assert_eq!(std::fs::read(seed.join("accounts.json")).unwrap(), before);
    c.create_book(request("durable", book::BookKind::Personal))
        .unwrap();
    let path = temp.0.join("durable");
    let mut b = FileBook::open_with(&path, Some(objects.clone())).unwrap();
    let staged = b.put(b"rules = []").unwrap();
    assert_ne!(b.active().unwrap(), Some(staged));
    assert!(b.set_active(&Digest::of(b"later")).is_err());
    assert!(b.put_accounts(&[]).is_err());
    std::fs::write(path.join("config/ACTIVE"), b"later").unwrap();
    assert!(c.get_book("books/durable").is_err());
    assert_eq!(std::fs::read(path.join("config/ACTIVE")).unwrap(), b"later");
    std::fs::remove_dir_all(&path).unwrap();
    book::initialize(
        &path,
        "durable",
        "Conflicting seed",
        book::BookKind::Project,
    )
    .unwrap();
    assert!(c.get_book("books/durable").is_err());
    assert_eq!(
        book::BookMeta::load(&path, "durable").display_name,
        "Conflicting seed"
    );
    objects
        .put_if_absent("unregistered/journal/00000000000000000001", b"legacy")
        .unwrap();
    assert!(c
        .create_book(request("unregistered", book::BookKind::Personal))
        .is_err());
}

#[test]
fn bootstrap_process() {
    let Ok(mode) = std::env::var("RATIO_BOOTSTRAP_TEST_MODE") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("RATIO_BOOTSTRAP_TEST_ROOT").unwrap());
    let objects = PathBuf::from(std::env::var_os("RATIO_BOOTSTRAP_TEST_OBJECTS").unwrap());
    ratio_store::install_object_store(Arc::new(DirStore::at(&objects)));
    let c = Console::scoped(&root, member("creator"));
    if mode == "create" {
        c.create_book(request("cold", book::BookKind::Personal))
            .unwrap();
        let control = ControlStore::new(Arc::new(DirStore::at(&objects)));
        for (id, bytes, actor) in [
            ("first", b"rules = []\n# first\n".as_slice(), "creator"),
            ("second", b"rules = []\n# second\n".as_slice(), "guest"),
        ] {
            if id == "second" {
                control
                    .commit(&control_operation(
                        &control,
                        "cold",
                        "grant",
                        "creator",
                        membership(
                            membership_revision::Action::Grant,
                            membership_revision::Principal::AuthkitSubject("guest".into()),
                        ),
                    ))
                    .unwrap();
            }
            let digest = control.stage_config(bytes).unwrap();
            control
                .commit(&control_operation(
                    &control,
                    "cold",
                    id,
                    actor,
                    control_operation::Change::ConfigPromotion(ConfigPromotion {
                        config_digest: digest.as_str().into(),
                    }),
                ))
                .unwrap();
        }
        control
            .commit(&control_operation(
                &control,
                "cold",
                "revoke",
                "creator",
                membership(
                    membership_revision::Action::Revoke,
                    membership_revision::Principal::AuthkitSubject("guest".into()),
                ),
            ))
            .unwrap();
        let mut b = FileBook::open(root.join("cold")).unwrap();
        let digest = b.active().unwrap().unwrap();
        b.append(&entry(&digest)).unwrap();
    } else if mode == "reinitialize" {
        assert!(!root.exists());
        assert!(book::initialize(
            &root.join("cold"),
            "cold",
            "Replacement",
            book::BookKind::Personal
        )
        .is_err());
        assert_eq!(
            book::BookMeta::load(&root.join("cold"), "cold").display_name,
            "Exact cold"
        );
    } else {
        assert!(
            !root.exists(),
            "recovery requires an entirely fresh serving root"
        );
        let rows = c.list_books().unwrap().books;
        assert_eq!(rows.len(), 1);
        let book = c.get_book("books/cold").unwrap();
        assert_eq!(book.kind, book::BookKind::Personal.proto());
        assert_eq!(book.entry_count, 1);
        let b = FileBook::open(root.join("cold")).unwrap();
        assert_eq!(
            b.entries().unwrap(),
            vec![entry(&b.active().unwrap().unwrap())]
        );
        assert_eq!(b.history().unwrap().len(), 3);
        assert!(Console::scoped(&root, member("guest"))
            .list_books()
            .unwrap()
            .books
            .is_empty());
        assert_eq!(
            Console::scoped(&root, member("stranger"))
                .list_books()
                .unwrap()
                .books
                .len(),
            0
        );
    }
}
#[test]
fn a_new_process_recovers_promotions_and_membership_after_the_serving_root_is_lost() {
    let temp = Temp::new();
    let root = temp.0.join("serving");
    for mode in ["create", "reinitialize", "recover"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bootstrap_tests::bootstrap_process",
                "--nocapture",
            ])
            .env("RATIO_BOOTSTRAP_TEST_MODE", mode)
            .env("RATIO_BOOTSTRAP_TEST_ROOT", &root)
            .env("RATIO_BOOTSTRAP_TEST_OBJECTS", temp.0.join("objects"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
