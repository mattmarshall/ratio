//! Benchmark: cold full replay, checkpoint load, and tail replay for the
//! large demo shape (`securities=20`, `lots_per=40` — ~5.4k journal entries).
//!
//! Related: #310. Prints one `projection_checkpoint_bench` line so a CI log
//! or operator can cite the three numbers without opening a figure.

use anyhow::Result;
use ratio_gen::Shape;
use ratio_project::checkpoint::{measure_checkpoint_bench, CheckpointBench};

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
    Ok(())
}
