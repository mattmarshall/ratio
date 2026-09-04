//! Measured 20M-lot Stage E fold of the HANDOFF geometry.
//!
//! ⛔ NOT A TLA PROBE AND NOT A SKIP. This target runs in `bazel test //...`.
//! It generates 10,000 × 2,000 lots, relieves every holding through
//! `relieve_by`, and publishes a digest CI can cite. The 140M-entry /
//! 40GB journal fold stays on Fargate ScaleTask — this is the open-lot
//! projection HANDOFF said the fold is bounded by.
//!
//! ⚠ SIZE IS `large` BECAUSE TWENTY MILLION LOTS ARE THE CLAIM. A
//! smaller stand-in would be the green suite that tests nothing.

use ratio_sql_project::{fold, Geometry};

/// Digest of `Geometry::HANDOFF` under `fold_scale/v1`. Locked after the
/// first measured run so a generator change cannot silently retell the
/// scale story.
const HANDOFF_DIGEST: &str =
    "bbf896400835916d0902f9ea175609bccd84be4801f71cc9fc57140f8a60a5d3";

#[test]
fn a_twenty_million_lot_fold_of_the_handoff_geometry() {
    let report = fold(Geometry::HANDOFF).expect("HANDOFF fold");
    println!("{}", report.to_json());

    assert_eq!(report.securities, 10_000, "not the 500-security dial");
    assert_eq!(report.lots_per, 2_000, "not the 40,000-lots-per dial");
    assert_eq!(report.lots, 20_000_000);
    assert_eq!(report.digest.len(), 64);
    assert_ne!(
        report.hifo_cost, report.seq_scan_cost,
        "HIFO must not be the seq scan — //tla:stale_method_relief_check"
    );
    assert_ne!(report.hifo_cost, report.fifo_cost);
    assert_eq!(
        report.hifo_cost, 2_000_000,
        "HIFO takes seq 1999 on security 0 (cost step 1000)"
    );
    assert_eq!(report.fifo_cost, 1_000);
    assert_eq!(report.seq_scan_cost, 1_000);
    assert_eq!(report.aggregate_units, 20_000_000 * 10);
    assert!(
        report.fold_ms > 0,
        "a zero-time claim about twenty million lots is the overclaim"
    );
    assert_eq!(
        report.digest, HANDOFF_DIGEST,
        "the HANDOFF fold digest moved — the citation is a different book"
    );
}
