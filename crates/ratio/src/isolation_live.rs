//! Manual #360 provider proof: one process, two explicitly selected backends.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_console::{book::BookKind, Console, Subject};
use ratio_proto::ratio::{
    console::v1 as pb,
    storage::v1::{BackendConformanceCheck, BackendConformanceEvidence},
};
use ratio_store::{
    ConfigStore, DirStore, FileBook, Journal, JournalEntry, ObjectStore, PostingRecord,
};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
fn member(id: &str) -> Subject {
    ratio_console::auth::from_request_context(&serde_json::json!({"authorizer":{"jwt":{"claims":{"sub":id,"org_id":"shared-synthetic-org"}}}}).to_string()).unwrap()
}
#[test]
#[ignore = "requires isolated live S3 and filesystem namespaces"]
fn two_books_remain_isolated_across_live_backends_and_cold_processes() -> Result<()> {
    let bucket = std::env::var("RATIO_CONFORMANCE_BUCKET")?;
    let prefix = std::env::var("RATIO_CONFORMANCE_PREFIX")?;
    ensure!(
        bucket.starts_with("ratio-recovery-lab-")
            && prefix.starts_with("conformance-lab/")
            && prefix.ends_with("/360/"),
        "isolated #360 namespace required"
    );
    ensure!(
        !std::env::vars().any(|(k, _)| k.starts_with("AWS_ENDPOINT_URL")),
        "live provider required"
    );
    let root = PathBuf::from(std::env::var("RATIO_CONFORMANCE_ROOT")?);
    let phase = std::env::var("RATIO_CONFORMANCE_PHASE")?;
    let alpha: Arc<dyn ObjectStore> = Arc::new(crate::scale::S3::open(&bucket, &prefix)?);
    let beta: Arc<dyn ObjectStore> = Arc::new(DirStore::at(root.join("beta-objects")));
    let stores = BTreeMap::from([
        ("alpha".into(), alpha.clone()),
        ("beta".into(), beta.clone()),
    ]);
    let serving = root.join("serving");
    let client =
        |who: &str| Console::scoped(&serving, member(who)).with_book_object_stores(stores.clone());
    if phase == "seed" {
        ensure!(
            !root.exists() && alpha.list("")?.is_empty(),
            "fresh namespace required"
        );
        fs::create_dir_all(&root)?;
        for (id, who, kind, objects, amount) in [
            ("alpha", "alice", BookKind::Investment, alpha.clone(), 12345),
            ("beta", "bob", BookKind::Personal, beta.clone(), 7000),
        ] {
            client(who).create_book(pb::CreateBookRequest {
                book_id: id.into(),
                book: Some(pb::Book {
                    kind: kind.proto(),
                    display_name: id.into(),
                    ..Default::default()
                }),
                ..Default::default()
            })?;
            let path = serving.join(id);
            let mut b = FileBook::open_with(&path, Some(objects.clone()))?;
            let digest = b.active()?.unwrap();
            b.append(&JournalEntry {
                id: "opening".into(),
                memo: "synthetic isolation".into(),
                config: digest,
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
            })?;
            ratio_store::proposals::write_with_store(
                &path,
                "review",
                &format!("# {id} proposal\n"),
                Some(objects.clone()),
            )?;
            ratio_nav::strike_and_record_with_store(
                &path,
                ratio_rules::UNDECLARED_VIEW,
                1_780_000_000,
                who,
                Some(objects),
            )?;
        }
        // A binding mismatch must refuse while the local publication marker exists.
        let error = FileBook::open_with(serving.join("alpha"), Some(beta.clone()))
            .err()
            .unwrap()
            .to_string();
        ensure!(
            error.contains("durable book publication is unavailable"),
            "wrong binding refusal: {error}"
        );
        fs::remove_dir_all(&serving)?;
    } else {
        ensure!(
            phase == "cold" && root.exists() && !serving.exists(),
            "cold phase requires removed serving root"
        );
        for (id, who, other, kind, objects, amount) in [
            (
                "alpha",
                "alice",
                "bob",
                BookKind::Investment,
                alpha.clone(),
                12345,
            ),
            (
                "beta",
                "bob",
                "alice",
                BookKind::Personal,
                beta.clone(),
                7000,
            ),
        ] {
            ensure!(
                client(who).get_book(&format!("books/{id}"))?.kind == kind.proto(),
                "kind recovered"
            );
            ensure!(
                client(other).get_book(&format!("books/{id}")).is_err(),
                "foreign membership admitted"
            );
            ensure!(
                client("outsider").get_book(&format!("books/{id}")).is_err(),
                "outsider admitted"
            );
            let path = serving.join(id);
            let b = FileBook::open_with(&path, Some(objects.clone()))?;
            let entries = b.entries()?;
            ensure!(
                entries.len() == 1 && entries[0].postings[0].amount == amount,
                "journal identity"
            );
            let projection = client(who).projection(id)?;
            let _ = projection;
            let strikes = ratio_nav::list_with_store(&path, Some(objects.clone()))?;
            ensure!(
                strikes.len() == 1
                    && ratio_nav::replay_with_store(&path, &strikes[0], Some(objects.clone()))?
                        .ok(),
                "NAV replay differs"
            );
            ensure!(
                ratio_store::proposals::read_with_store(&path, "review", Some(objects))?
                    == Some(format!("# {id} proposal\n")),
                "proposal identity"
            );
            let audit = client(who)
                .list_change_log_entries(&format!("funds/{id}"))?
                .change_log_entries;
            ensure!(
                audit.len() == 2
                    && audit
                        .iter()
                        .any(|row| row.action == "created" && row.actor == who)
                    && audit
                        .iter()
                        .any(|row| row.action == "drafted" && row.subject == "review"),
                "creation and proposal audit identity differs"
            );
            let name = client(who)
                .list_nav_strikes(&format!(
                    "funds/{id}/views/{}",
                    ratio_rules::UNDECLARED_VIEW
                ))?
                .nav_strikes[0]
                .name
                .clone();
            ensure!(
                client(who).replay_nav_strike(&name)?.reproduced,
                "console NAV replay binding"
            );
        }
        ensure!(
            client("alice").get_book("books/unregistered").is_err(),
            "unmapped read admitted"
        );
    }
    for (objects, foreign) in [(&alpha, "beta"), (&beta, "alpha")] {
        let keys = objects.list("")?;
        ensure!(
            !keys
                .iter()
                .any(|k| k.split('/').any(|part| part == foreign)),
            "foreign book object on backend"
        );
    }
    let mut evidence=BackendConformanceEvidence{run_id:prefix.clone(),kernel_revision:std::env::var("RATIO_CONFORMANCE_REVISION")?,backend_location:format!("s3://{bucket}/{prefix} + isolated DirStore"),limitations:vec!["Synthetic live S3 plus independent filesystem backend; separate seed/cold processes. Not cross-account disaster recovery or production routing deployment.".into()],..Default::default()};
    evidence.checks.push(BackendConformanceCheck{check_id:format!("two-backend-{phase}"),passed:true,detail:"Per-book explicit store selection, independent subject grants, journal/NAV/proposal/audit recovery, no foreign backend keys; seed refuses wrong binding; cold refuses foreign and outsider reads".into(),..Default::default()});
    fs::write(root.join(format!("{phase}.pb")), evidence.encode_to_vec())?;
    eprintln!("two_backend_isolation phase={phase} passed");
    Ok(())
}
