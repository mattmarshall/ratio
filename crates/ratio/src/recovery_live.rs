//! Manual, synthetic AWS recovery evidence. All figures are decided by the
//! existing kernel; the constants here are independent fixture expectations.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_console::{book::BookKind, BreakExplanation, Console, Subject};
use ratio_proto::ratio::{
    console::v1 as pb,
    storage::v1::{
        RecoveryAppState, RecoveryDrillBook, RecoveryDrillEvidence, RecoveryDrillPhase,
        RecoveryObject,
    },
    v1::BreakReport,
};
use ratio_proto::{
    duration_proto::google::protobuf::Duration as ProtoDuration,
    timestamp_proto::google::protobuf::Timestamp,
};
use ratio_store::{
    changes,
    control::{
        control_operation, membership_revision, ConfigPromotion, ControlOperation, ControlStore,
        MembershipRevision,
    },
    proposals, reports, CloseRecord, ConfigStore, Digest, FileBook, Journal, JournalEntry,
    ObjectStore, Plane, PostingRecord,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("required operator setting {name}"))
}
fn millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
fn hash(bytes: &[u8]) -> Vec<u8> {
    let hex = Digest::of(bytes);
    (0..64)
        .step_by(2)
        .map(|n| u8::from_str_radix(&hex.as_str()[n..n + 2], 16).unwrap())
        .collect()
}
fn member(id: &str) -> Subject {
    ratio_console::auth::from_request_context(&serde_json::json!({"authorizer":{"jwt":{"claims":{"sub":id,"org_id":"synthetic-shared-org"}}}}).to_string()).unwrap()
}
fn entry(id: &str, config: &Digest, amount: i64) -> JournalEntry {
    JournalEntry {
        id: id.into(),
        memo: "Approved synthetic recovery capital".into(),
        config: config.clone(),
        postings: vec![
            PostingRecord::new(2, amount),
            PostingRecord::new(20, -amount),
        ],
        trade_date: Some("2026-06-01".into()),
        announcement: None,
        due_date: None,
        application: None,
        identified_lots: None,
        special_allocations: None,
        kind: None,
    }
}
fn inventory(store: &dyn ObjectStore) -> Result<Vec<RecoveryObject>> {
    store
        .list("")?
        .into_iter()
        .map(|key| {
            let bytes = store
                .get(&key)?
                .expect("listed object disappeared during quiescent inventory");
            Ok(RecoveryObject {
                key,
                size: bytes.len() as i64,
                sha256: hash(&bytes),
            })
        })
        .collect()
}
fn commit(control: &ControlStore, change: control_operation::Change, id: &str) -> Result<()> {
    let state = control.read("alpha")?;
    control.commit(&ControlOperation {
        format_version: 1,
        book_id: "alpha".into(),
        bootstrap_digest: state.bootstrap_digest,
        expected_revision: state.revision,
        expected_predecessor_digest: state.predecessor_digest,
        operation_id: id.into(),
        actor_subject: "alice".into(),
        actor_provenance: "approved-synthetic-fixture".into(),
        change: Some(change),
    })?;
    Ok(())
}
fn snapshot(
    root: &PathBuf,
    store: Arc<dyn ObjectStore>,
    id: &str,
    expected: i64,
) -> Result<RecoveryDrillBook> {
    let book = FileBook::open_with(root.join(id), Some(store.clone()))?;
    let entries = book.entries()?;
    let strikes = ratio_nav::list_with_store(&root.join(id), Some(store.clone()))?;
    ensure!(
        strikes.len() == 1,
        "fixture requires one signed NAV per book"
    );
    let strike = &strikes[0];
    ensure!(
        strike.net_asset_value == expected,
        "independent fixture figure differs for {id}"
    );
    ensure!(
        ratio_nav::replay_with_store(&root.join(id), strike, Some(store.clone()))?.ok(),
        "strike replay refused"
    );
    let all = inventory(store.as_ref())?;
    Ok(RecoveryDrillBook {
        book_id: id.into(),
        journal_height: entries.len() as i64,
        journal_digest: ratio_nav::prefix_digest(&entries)?,
        config_history: book
            .history()?
            .into_iter()
            .map(|d| d.as_str().to_string())
            .collect(),
        expected_nav: expected,
        reproduced_nav: strike.net_asset_value,
        objects: all
            .into_iter()
            .filter(|o| o.key.starts_with(&format!("{id}/")))
            .collect(),
    })
}
fn seed(root: &PathBuf, store: Arc<dyn ObjectStore>) -> Result<Vec<RecoveryDrillBook>> {
    ensure!(store.list("")?.is_empty(), "refusing a nonempty lab source");
    for (id, owner, kind) in [
        ("alpha", "alice", BookKind::Investment),
        ("beta", "bob", BookKind::Personal),
    ] {
        Console::scoped(root, member(owner))
            .with_object_store(store.clone())
            .create_book(pb::CreateBookRequest {
                book_id: id.into(),
                book: Some(pb::Book {
                    kind: kind.proto(),
                    ..Default::default()
                }),
            })?;
    }
    let mut alpha = FileBook::open_with(root.join("alpha"), Some(store.clone()))?;
    let first = alpha.active()?.unwrap();
    alpha.append(&entry("capital-1", &first, 50_000))?;
    let control = ControlStore::new(store.clone());
    for n in 1..=2 {
        let mut rules = alpha.get(&alpha.active()?.unwrap())?;
        rules.extend_from_slice(format!("\n# approved synthetic promotion {n}\n").as_bytes());
        let next = control.stage_config(&rules)?;
        commit(
            &control,
            control_operation::Change::ConfigPromotion(ConfigPromotion {
                config_digest: next.as_str().into(),
            }),
            &format!("promotion-{n}"),
        )?;
        if n == 1 {
            alpha.append(&entry("capital-2", &next, 10_000))?;
        }
    }
    for (action, id) in [
        (membership_revision::Action::Grant, "grant-guest"),
        (membership_revision::Action::Revoke, "revoke-guest"),
    ] {
        commit(
            &control,
            control_operation::Change::MembershipRevision(MembershipRevision {
                action: action as i32,
                principal: Some(membership_revision::Principal::AuthkitSubject(
                    "guest".into(),
                )),
            }),
            id,
        )?;
    }
    let active = alpha.active()?.unwrap();
    let delivery_bytes = b"reference,amount\ncapital-1,500.00\ncapital-2,100.00\n";
    let delivery = ratio_ingest::Delivery {
        digest: Digest::of(delivery_bytes).as_str().into(),
        origin: "synthetic://approved-capital.csv".into(),
        received: 1_782_800_000,
        bytes: delivery_bytes.len() as i64,
    };
    alpha.append_record(Plane::Deliveries, &delivery)?;
    alpha.append_record(
        Plane::Entities,
        &ratio_ingest::Entity {
            id: "fixture-instrument".into(),
            kind: ratio_ingest::EntityKind::Instrument,
            display_name: "Synthetic inventory instrument".into(),
            attributes: BTreeMap::new(),
        },
    )?;
    alpha.append_record(
        Plane::Facts,
        &ratio_ingest::Fact {
            id: "fixture-provenance".into(),
            kind: "recovery-note".into(),
            reference: "capital-1".into(),
            entities: BTreeMap::new(),
            values: BTreeMap::new(),
            provenance: ratio_ingest::Provenance {
                delivery: delivery.digest.clone(),
                row: 2,
                template: active.as_str().into(),
                template_id: "synthetic-recovery".into(),
                received: delivery.received,
            },
        },
    )?;
    alpha.append_record(
        Plane::Actions,
        &ratio_ingest::actions::Announced {
            id: "future-fixture-split".into(),
            instrument: "fixture-instrument".into(),
            split: ratio_ingest::actions::Split { num: 2, den: 1 },
            ex_date: "2030-01-01".into(),
            announced: delivery.received,
        },
    )?;
    let digest = ratio_nav::prefix_digest(&alpha.entries()?)?;
    for n in 0..2 {
        alpha.append_record(
            Plane::Explanations,
            &BreakExplanation {
                break_id: format!("synthetic-cash-{n}"),
                text: "Synthetic source review; not customer acceptance".into(),
                actor: "alice".into(),
                accept_time: 1_782_800_001 + n,
                difference: 25,
                config_digest: active.as_str().into(),
                journal_position: 2,
                journal_digest: digest.clone(),
            },
        )?;
        alpha.append_record(
            Plane::Closes,
            &CloseRecord {
                view: ratio_rules::UNDECLARED_VIEW.into(),
                closed_date: "2026-06-30".into(),
                journal_position: 2,
                journal_digest: digest.clone(),
                config_digest: active.as_str().into(),
                closing_entry: None,
                actor: "alice".into(),
                recorded: 1_782_800_005 + n,
                equity_destination: 20,
                surplus: None,
            },
        )?;
        let report = BreakReport {
            name: format!("books/alpha/breakReports/report-{n}"),
            config_digest: active.as_str().into(),
            entries_posted: 2,
            book_ties: true,
            ..Default::default()
        }
        .encode_to_vec();
        reports::write_report_with_store(
            &root.join("alpha"),
            &format!("report-{n}.pb"),
            &report,
            Some(store.clone()),
        )?;
        changes::append_with_store(
            &root.join("alpha"),
            &format!("{n}\talice\tsynthetic-review\tcapital\tdigest\n"),
            Some(store.clone()),
        )?;
    }
    proposals::write_with_store(
        &root.join("alpha"),
        "reviewed-draft",
        "rules = []\n",
        Some(store.clone()),
    )?;
    ensure!(store.put_if_absent(
        &format!("external/deliveries/{}.csv", delivery.digest),
        delivery_bytes
    )?);
    for (app, id) in [("bank-feed", "plaid"), ("sheets-export", "google")] {
        let app_state = RecoveryAppState {
            app_id: app.into(),
            book_id: "alpha".into(),
            subject: "alice".into(),
            registration_id: format!("synthetic-{id}-registration"),
            credential_reference: format!("synthetic-only://{id}/alice/alpha"),
            retry_cursor: "synthetic-cursor-2".into(),
            posting_correspondence: [("pending-1".into(), "posted-1".into())]
                .into_iter()
                .collect(),
            google_sync_token: "synthetic-sync-2".into(),
            sheet_row_index: [("capital-1".into(), "row-2".into())].into_iter().collect(),
            revoked: false,
            outbound_writes_disabled: true,
        };
        ensure!(store.put_if_absent(
            &format!("external/connect/{id}.pb"),
            &app_state.encode_to_vec()
        )?);
    }
    drop(alpha);
    let mut beta = FileBook::open_with(root.join("beta"), Some(store.clone()))?;
    beta.append(&entry("beta-capital", &beta.active()?.unwrap(), 7_000))?;
    drop(beta);
    for id in ["alpha", "beta"] {
        ratio_nav::strike_and_record_with_store(
            &root.join(id),
            ratio_rules::UNDECLARED_VIEW,
            1_782_800_000,
            if id == "alpha" { "alice" } else { "bob" },
            Some(store.clone()),
        )?;
    }
    Ok(vec![
        snapshot(root, store.clone(), "alpha", 60_000)?,
        snapshot(root, store, "beta", 7_000)?,
    ])
}

fn verify(
    root: &PathBuf,
    store: Arc<dyn ObjectStore>,
    baseline: &RecoveryDrillEvidence,
) -> Result<Vec<RecoveryDrillBook>> {
    let expected = &baseline.external_inventory;
    ensure!(
        inventory(store.as_ref())? == *expected,
        "complete restored object inventory differs from quiescent source"
    );
    let mut books = Vec::new();
    for (id, expected) in [("alpha", 60_000), ("beta", 7_000)] {
        let now = snapshot(root, store.clone(), id, expected)?;
        let before = baseline.books.iter().find(|b| b.book_id == id).unwrap();
        ensure!(
            now == *before,
            "figure, prefix, config history or object bytes differ for {id}"
        );
        books.push(now);
    }
    let alpha = root.join("alpha");
    ensure!(
        reports::newest_report_with_store(&alpha, Some(store.clone()))?
            .unwrap()
            .name
            == "books/alpha/breakReports/report-1",
        "report selection changed"
    );
    ensure!(
        proposals::read_with_store(&alpha, "reviewed-draft", Some(store.clone()))?.as_deref()
            == Some("rules = []\n")
    );
    // CreateBook records its own attributed creation before the two reviews.
    // The first live attempt exposed this mistaken fixture expectation.
    let audit = changes::read_with_store(&alpha, Some(store.clone()))?;
    ensure!(
        audit.len() == 3,
        "creation plus both review audit records must survive"
    );
    ensure!(
        audit[0].contains("\talice\tcreated\talpha\t"),
        "creation attribution differs"
    );
    for n in 0..2 {
        ensure!(
            audit[n + 1].trim_end() == format!("{n}\talice\tsynthetic-review\tcapital\tdigest"),
            "review audit ordering or attribution differs"
        );
    }
    let mut book = FileBook::open_with(&alpha, Some(store.clone()))?;
    ensure!(
        book.closes()?.len() == 2
            && book.records::<BreakExplanation>(Plane::Explanations)?.len() == 2
    );
    ensure!(
        book.history()?.len() == 3,
        "two configuration promotions were not restored"
    );
    for (who, id, allowed) in [
        ("alice", "alpha", true),
        ("bob", "beta", true),
        ("alice", "beta", false),
        ("bob", "alpha", false),
        ("guest", "alpha", false),
        ("outsider", "alpha", false),
    ] {
        let console = Console::scoped(root, member(who)).with_object_store(store.clone());
        ensure!(
            console.get_book(&format!("books/{id}")).is_ok() == allowed,
            "read boundary changed for {who}/{id}"
        );
        if !allowed {
            let error = console
                .apply_event(&pb::ApplyEventRequest {
                    parent: format!("funds/{id}"),
                    event_id: "isolation-attempt".into(),
                    rule_id: "contribution".into(),
                    amount: "1.00".into(),
                    ..Default::default()
                })
                .unwrap_err();
            ensure!(
                error.to_string().contains("no fund"),
                "write failed for a reason other than membership: {error:#}"
            );
        }
    }
    ensure!(
        inventory(store.as_ref())? == *expected,
        "denied operations changed authoritative bytes"
    );
    let original = book
        .records::<ratio_ingest::Delivery>(Plane::Deliveries)?
        .pop()
        .unwrap();
    let bytes = store
        .get(&format!("external/deliveries/{}.csv", original.digest))?
        .unwrap();
    ensure!(
        Digest::of(&bytes).as_str() == original.digest && bytes.len() as i64 == original.bytes,
        "original delivery custody differs"
    );
    for id in ["plaid", "google"] {
        let state = RecoveryAppState::decode(
            store
                .get(&format!("external/connect/{id}.pb"))?
                .unwrap()
                .as_slice(),
        )?;
        ensure!(
            state.book_id == "alpha"
                && state.subject == "alice"
                && state.outbound_writes_disabled
                && state.credential_reference.starts_with("synthetic-only://"),
            "Connect custody differs"
        );
    }
    let mut next = entry("after-recovery", &book.active()?.unwrap(), 10_000);
    ensure!(book
        .append(&next)
        .unwrap_err()
        .to_string()
        .contains("closed through"));
    next.trade_date = Some("2026-07-01".into());
    book.append(&next)?;
    ensure!(
        book.entries()?.len() == 3,
        "restored journal did not claim the next slot"
    );
    ensure!(
        FileBook::open_with(root.join("beta"), Some(store))?
            .entries()?
            .len()
            == 1,
        "alpha append changed beta"
    );
    Ok(books)
}

#[test]
#[ignore = "requires an explicitly approved, isolated real AWS lab"]
fn the_approved_synthetic_s3_rehearsal() -> Result<()> {
    ensure!(
        std::env::var_os("AWS_ENDPOINT_URL").is_none()
            && std::env::var_os("AWS_ENDPOINT_URL_S3").is_none(),
        "live rehearsal refuses endpoint overrides"
    );
    let bucket = env("RATIO_DRILL_BUCKET");
    let prefix = env("RATIO_DRILL_PREFIX");
    ensure!(
        bucket.starts_with("ratio-recovery-lab-")
            && prefix.starts_with("recovery-lab/")
            && prefix.ends_with('/'),
        "refusing a non-lab namespace"
    );
    let root = PathBuf::from(env("RATIO_DRILL_ROOT"));
    ensure!(!root.exists(), "serving root must start absent");
    let store: Arc<dyn ObjectStore> = Arc::new(crate::scale::S3::open(&bucket, &prefix)?);
    let phase = env("RATIO_DRILL_PHASE");
    let started = millis();
    let timer = Instant::now();
    let mut record = if phase == "seed" {
        RecoveryDrillEvidence {
            run_id: env("RATIO_DRILL_ID"),
            kernel_revision: env("RATIO_DRILL_REVISION"),
            operator: "Codex under Matt Marshall's approved AWS session".into(),
            verifier: "Matt Marshall".into(),
            source_location: format!("s3://{bucket}/{prefix}"),
            backup_location: env("RATIO_DRILL_BACKUP"),
            restore_location: env("RATIO_DRILL_RESTORE"),
            rpo_target: Some(ProtoDuration { seconds: 0, nanos: 0 }),
            rto_target: Some(ProtoDuration { seconds: 3600, nanos: 0 }),
            evidence_retention_days: 7,
            verifier_accepted: false,
            limitations: vec![
                "Synthetic subjects exercise application membership, not gateway JWT signature validation".into(),
                "Synthetic Connect custody has no live provider tokens; no outbound provider writes are permitted".into(),
            ],
            ..Default::default()
        }
    } else {
        RecoveryDrillEvidence::decode(fs::read(env("RATIO_DRILL_BASELINE"))?.as_slice())?
    };
    let result = match phase.as_str() {
        "seed" => seed(&root, store.clone()),
        "verify" => verify(&root, store.clone(), &record),
        _ => anyhow::bail!("unknown rehearsal phase"),
    };
    let passed = result.is_ok();
    let detail = match &result {
        Ok(_) => {
            "existing kernel figures, exact inventory, replay and membership assertions passed"
                .into()
        }
        Err(error) => format!("{error:#}"),
    };
    if phase == "seed" {
        record.external_inventory = inventory(store.as_ref())?;
    }
    if let Ok(books) = &result {
        record.books = books.clone();
    }
    record.phases.push(RecoveryDrillPhase {
        phase_id: phase.clone(),
        started_time: Some(Timestamp {
            seconds: started / 1000,
            nanos: ((started % 1000) * 1_000_000) as i32,
        }),
        elapsed: Some(ProtoDuration {
            seconds: timer.elapsed().as_secs() as i64,
            nanos: timer.elapsed().subsec_nanos() as i32,
        }),
        passed,
        detail,
    });
    fs::write(env("RATIO_DRILL_RECORD"), record.encode_to_vec())?;
    result?;
    println!("LIVE_AWS_S3 phase={phase} objects={} alpha_nav=60000 beta_nav=7000 verifier_acceptance=pending",record.external_inventory.len());
    Ok(())
}
