//! Benchmark: cold full replay, checkpoint load, and tail replay for the
//! large demo shape (`securities=20`, `lots_per=40` — ~5.4k journal entries).
//!
//! Related: #310. Prints one `projection_checkpoint_bench` line so a CI log
//! or operator can cite the three numbers without opening a figure.

use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use ratio_gen::Shape;
use ratio_project::checkpoint::{
    follow_with_checkpoint_store, measure_checkpoint_bench, CheckpointBench,
    ObjectCheckpointStore,
};
use ratio_store::{DirStore, FileBook, ObjectStore};

#[test]
fn large_demo_checkpoint_bench_reports_cold_load_and_tail() -> Result<()> {
    let root = match std::env::var_os("TEST_TMPDIR") {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::temp_dir(),
    };
    let book_dir = root.join(format!(
        "ratio-checkpoint-bench-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&book_dir);
    // ⛔ SAME DIALS AS deploy/seed-demo-funds.sh's ashcombe / bellwether books.
    let shape = Shape {
        securities: 20,
        lots_per: 40,
        currencies: 3,
        seed: 1,
        ..Shape::default()
    };
    ratio_gen::generate(&book_dir, shape)?;
    let book = ratio_store::FileBook::open(&book_dir)?;
    let store = book_dir.join(".projection-checkpoints-bench");
    let report: CheckpointBench = measure_checkpoint_bench(&book, &store)?;
    // Sanity: the large demo is thousands of entries, not a dozen.
    assert!(
        report.entries >= 4_000,
        "expected the large demo shape, got {} entries",
        report.entries
    );
    // Checkpoint load must beat a cold full replay on this shape — otherwise
    // the acceleration is a no-op we would still be measuring as success.
    assert!(
        report.checkpoint_load_ms <= report.cold_full_replay_ms,
        "checkpoint load {}ms should not exceed cold replay {}ms",
        report.checkpoint_load_ms,
        report.cold_full_replay_ms
    );
    eprintln!("{}", report.report_line());

    // The same generated shape through the configured object seam. Timings
    // include prefix verification, so they do not imply zero journal GETs.
    let objects_dir = root.join(format!("ratio-checkpoint-objects-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&objects_dir);
    let objects: Arc<dyn ObjectStore> = Arc::new(DirStore::at(&objects_dir));
    let durable_book = FileBook::open_with(&book_dir, Some(objects.clone()))?;
    let checkpoint = ObjectCheckpointStore::new(objects.clone(), "checkpoint-bench-book");
    let t0 = Instant::now();
    let (_, first) = follow_with_checkpoint_store(&durable_book, &checkpoint)?;
    let cold_ms = t0.elapsed().as_millis();
    assert!(!first.hit);
    drop(durable_book);
    let reopened = FileBook::open_with(&book_dir, Some(objects))?;
    let t1 = Instant::now();
    let (_, second) = follow_with_checkpoint_store(&reopened, &checkpoint)?;
    let warm_ms = t1.elapsed().as_millis();
    assert!(second.hit);
    assert_eq!(second.tail_length, 0);
    eprintln!(
        "projection_checkpoint_object_bench entries={} cold_ms={} checkpoint_ms={} tail_entries={}",
        report.entries, cold_ms, warm_ms, second.tail_length
    );
    Ok(())
}
