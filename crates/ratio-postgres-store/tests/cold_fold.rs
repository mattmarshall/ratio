//! Manual equal-height companion to the retained live S3 missing-checkpoint run.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_postgres_store::PgStore;
use ratio_project::{
    checkpoint::{follow_with_checkpoint_store, prefix_digest_book, ObjectCheckpointStore},
    Projection,
};
use ratio_proto::ratio::storage::v1::{BackendConformanceCheck, BackendConformanceEvidence};
use ratio_store::{FileBook, Journal, ObjectStore};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};

#[test]
fn native_cold_fold_uses_the_same_5396_entry_journal_as_live_s3() -> Result<()> {
    let root = PathBuf::from(std::env::var("RATIO_CONFORMANCE_ROOT")?);
    let phase = std::env::var("RATIO_CONFORMANCE_PHASE")?;
    let attempt = std::env::var("RATIO_CONFORMANCE_ATTEMPT")?;
    ensure!(
        attempt
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "invalid attempt"
    );
    let objects = Arc::new(PgStore::connect_local(
        &std::env::var("RATIO_NATIVE_PG_URL")?,
        "ratio_cold_362",
        "seeded-demo",
    )?);
    let book_path = root.join("seeded-demo");
    if phase == "seed" {
        ensure!(
            !root.exists() && objects.list("")?.is_empty(),
            "new namespace required; preserve previous attempt"
        );
        for name in ["reference", "seeded-demo"] {
            ratio_gen::generate(
                &root.join(name),
                ratio_gen::Shape {
                    securities: 20,
                    lots_per: 40,
                    currencies: 3,
                    seed: 1,
                    ..Default::default()
                },
            )?;
        }
        let book = FileBook::open_with(&book_path, Some(objects.clone()))?;
        ensure!(
            book.entries()?.len() == 5396,
            "S3 comparison height differs"
        );
        fs::remove_file(book_path.join("journal.jsonl"))?;
    } else {
        ensure!(
            phase == "cold" && !book_path.join("journal.jsonl").exists(),
            "fresh process must use native journal"
        );
    }
    let book = FileBook::open_with(&book_path, Some(objects.clone()))?;
    let expected = FileBook::open(root.join("reference"))?;
    let digest = prefix_digest_book(&expected, 5396)?;
    ensure!(
        digest == "70e476310d24354c03de924285f7df0fe5e3c1ad40fb44d6ca51748b60ae9138",
        "journal differs from retained live S3 comparison"
    );
    let mut evidence=BackendConformanceEvidence{run_id:"20261023-f2fdf6e8".into(),kernel_revision:std::env::var("RATIO_CONFORMANCE_REVISION")?,backend_location:"loopback PostgreSQL 16 / ratio_cold_362 / seeded-demo".into(),journal_height:5396,journal_digest:digest.clone(),limitations:vec!["Same 5396-entry geometry, exact prefix digest, and missing-checkpoint function as retained S3 cold run (792031 ms). PostgreSQL runs on the operator host; S3 is remote. Single observations, not deployed latency or cost guarantees. Database OS cache is not purged; process/projection/checkpoint are cold.".into()],..Default::default()};
    if phase == "cold" {
        let checkpoint = ObjectCheckpointStore::new(objects, &format!("native-{attempt}"));
        let start = Instant::now();
        let (projection, telemetry) = follow_with_checkpoint_store(&book, &checkpoint)?;
        let elapsed = start.elapsed();
        ensure!(
            !telemetry.hit && projection.prefix() == 5396,
            "cold comparison must miss checkpoint"
        );
        let full = Projection::of_file_book(&expected)?;
        for view in full.views() {
            ensure!(
                full.balances(view)?.value == projection.balances(view)?.value,
                "cold view balance differs"
            );
        }
        ensure!(
            prefix_digest_book(&book, 5396)? == digest,
            "native prefix digest differs"
        );
        evidence.checks.push(BackendConformanceCheck{check_id:format!("native-{attempt}-missing-checkpoint-fold"),passed:true,elapsed:Some(ratio_proto::duration_proto::google::protobuf::Duration{seconds:elapsed.as_secs() as i64,nanos:elapsed.subsec_nanos() as i32}),detail:"Same missing-checkpoint load/prefix verification/full fold/publication as live S3; every declared-view balance matches full local fold and native digest matches S3".into(),..Default::default()});
        eprintln!(
            "native_pg_cold height=5396 elapsed_ms={} digest={digest} full_view_equivalence=true",
            elapsed.as_millis()
        );
    }
    fs::write(root.join(format!("{attempt}.pb")), evidence.encode_to_vec())?;
    Ok(())
}
