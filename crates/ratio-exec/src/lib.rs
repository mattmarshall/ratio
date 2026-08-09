//! ratio-exec — choosing a partition, and running it.
//!
//! Two proved things made real, and neither is about arithmetic.
//!
//! # The planner
//!
//! `Ratio.Exec` proves that IO and CPU are different resources and only one of
//! them partitions. There is one store, so IO adds up across partitions while
//! CPU divides, and a plan takes `max(all the IO, the slowest partition's CPU)`.
//! Below the IO floor another worker has nowhere to take time from.
//!
//! ⛔ THE DECISION IS EMITTED, THE FOLDS ARE HERE. [`plan`] counts rows and sums
//! IO with a loop and asks `splitting_helps` whether to go further.
//! `Ratio.Exec.a_plan_that_cannot_be_helped_takes_its_io` is why the two cannot
//! disagree: the answer depends on nothing but the two totals.
//!
//! # The executor
//!
//! `//tla:executor_check` proves what a run of partitions across workers must
//! do, and both failures it rules out are arithmetically silent:
//!
//! * a partition run **twice** double-counts, and every number in the doubled
//!   total is correct arithmetic on real rows — the same shape as a split
//!   applied twice or a mark taken from cost. `//tla:unclaimed_partitions_check`
//! * a **partial** result read as a whole one is simply LOW, with no ragged
//!   edge and no null to notice. `//tla:partial_commit_check`
//!
//! So a worker claims before it runs, and results are published all at once.

use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{bail, Result};

mod generated;
pub use generated::{elapsed_of, splitting_helps};

/// How a scan was split: rows per partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub chunks: Vec<i64>,
    /// Work per row, as the planner was told.
    pub per_row: i64,
}

impl Plan {
    /// `Ratio.Exec.totalIo` — one store, so these add up.
    pub fn total_io(&self) -> i64 {
        self.chunks.iter().sum()
    }

    /// `Ratio.Exec.slowestCpu` — workers run at once, so the last to finish is
    /// the one that matters.
    pub fn slowest_cpu(&self) -> i64 {
        self.chunks.iter().map(|c| c * self.per_row).max().unwrap_or(0)
    }

    /// What it takes. The emitted `elapsed_of`, on totals folded here.
    pub fn elapsed(&self) -> i64 {
        elapsed_of(self.total_io(), self.slowest_cpu())
    }

    /// Whether another partition would buy anything.
    pub fn can_be_helped(&self) -> bool {
        splitting_helps(self.total_io(), self.slowest_cpu())
    }
}

/// Split `rows` for `workers`, stopping when splitting stops helping.
///
/// ⛔ IT STOPS EARLY ON PURPOSE, and will hand back fewer partitions than there
/// are workers. `Ratio.Exec.splitting_helps_until_it_does_not` is the reason:
/// eight rows at four units of work go 32, 16, 8, 8 — the last two are the SAME
/// NUMBER, and a planner that kept splitting would spend workers to buy nothing
/// while a benchmark stopping one step earlier showed a clean linear curve.
///
/// ⚠ EVEN CHUNKS, and that is a real limitation rather than a simplification.
/// `Ratio.Exec.the_slowest_partition_sets_the_pace`: an uneven split is as slow
/// as its worst piece, so splitting by something convenient — a file, an
/// instrument, a day — is splitting by something uncorrelated with work, and
/// the tail is what you get. Even chunks are only right when rows cost the same.
pub fn plan(rows: i64, per_row: i64, workers: usize) -> Result<Plan> {
    if rows < 0 || per_row < 0 {
        bail!("a plan over {rows} rows at {per_row} work per row is not a plan");
    }
    if workers == 0 {
        bail!("a plan needs at least one worker");
    }

    let mut best = Plan { chunks: vec![rows], per_row };
    for p in 2..=workers {
        if !best.can_be_helped() {
            break;
        }
        let p = p as i64;
        let base = rows / p;
        let extra = rows % p;
        let chunks: Vec<i64> =
            (0..p).map(|i| base + if i < extra { 1 } else { 0 }).filter(|c| *c > 0).collect();
        if chunks.is_empty() {
            break;
        }
        let next = Plan { chunks, per_row };
        // Only keep a split that actually shortens the plan.
        if next.elapsed() >= best.elapsed() {
            break;
        }
        best = next;
    }
    Ok(best)
}

/// What a run produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome<T> {
    /// Every partition ran, and here is the whole answer.
    Complete(T),
    /// The run was abandoned. ⛔ CARRIES NO PARTIAL RESULT — there is no
    /// variant holding "some of it", because a caller handed a partial total
    /// cannot tell it from a correct one. `//tla:partial_commit_check`.
    Cancelled,
}

/// Run a plan across `workers` threads and combine the results.
///
/// ⛔ CLAIM BEFORE WORK. Partitions are handed out through one atomic counter,
/// so no two workers can take the same one — `//tla:unclaimed_partitions_check`
/// is that guard removed, and the double-counted total it produces is built
/// entirely from correct arithmetic on real rows.
///
/// ⛔ AND THE COMBINE HAPPENS ONLY IF EVERY PARTITION RAN. `Outcome` has no
/// variant for a partial answer, so there is nothing for a caller to
/// accidentally read.
pub fn run<T, F>(
    plan: &Plan,
    workers: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
    work: F,
) -> Outcome<Vec<T>>
where
    T: Send,
    F: Fn(usize, i64) -> T + Sync,
{
    let n = plan.chunks.len();
    let next = AtomicUsize::new(0);
    let mut slots: Vec<Option<T>> = (0..n).map(|_| None).collect();

    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for _ in 0..workers.max(1) {
            let next = &next;
            let work = &work;
            let plan = &plan;
            handles.push(s.spawn(move || {
                let mut mine: Vec<(usize, T)> = Vec::new();
                loop {
                    if cancelled() {
                        return mine;
                    }
                    // ⛔ THE CLAIM. `fetch_add` hands each index to exactly one
                    // worker; there is no window in which two can read the same
                    // one, which is the whole content of the TLA guard.
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= plan.chunks.len() {
                        return mine;
                    }
                    mine.push((i, work(i, plan.chunks[i])));
                }
            }));
        }
        for h in handles {
            for (i, v) in h.join().expect("a worker panicked") {
                slots[i] = Some(v);
            }
        }
    });

    // All or nothing. A missing slot means a partition never ran, and a total
    // built from the rest would be low — which is indistinguishable from right.
    if slots.iter().any(Option::is_none) {
        return Outcome::Cancelled;
    }
    Outcome::Complete(slots.into_iter().map(Option::unwrap).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI64;

    fn never() -> bool {
        false
    }

    #[test]
    fn a_pure_scan_is_not_partitioned_at_all() {
        // ⛔ `Ratio.Exec.a_pure_scan_is_io_bound`. One unit of work per row read
        // means the IO floor already binds, so the planner declines to split —
        // and it is right to, because splitting would take exactly as long.
        let p = plan(1_000, 1, 16).unwrap();
        assert_eq!(p.chunks, vec![1_000], "one partition");
        assert_eq!(p.elapsed(), 1_000);
        assert!(!p.can_be_helped());
    }

    #[test]
    fn heavy_work_is_split_until_it_hits_the_floor() {
        // `Ratio.Exec.splitting_helps_until_it_does_not`: 8 rows × 4 work goes
        // 32, 16, 8, 8. The planner must stop at four, not eight.
        let p = plan(8, 4, 16).unwrap();
        assert_eq!(p.elapsed(), 8, "the IO floor");
        assert_eq!(p.total_io(), 8);
        assert!(
            p.chunks.len() <= 4,
            "stopped at the floor, not at the worker count: {:?}",
            p.chunks
        );
        assert!(!p.can_be_helped());
    }

    #[test]
    fn more_workers_than_the_work_needs_are_declined() {
        // ⚠ THE RESULT THAT COSTS MONEY WHEN IT IS MISSED. Sixteen workers
        // offered, four used — and the ones declined would have bought nothing.
        let four = plan(8, 4, 4).unwrap();
        let sixteen = plan(8, 4, 16).unwrap();
        assert_eq!(four.elapsed(), sixteen.elapsed(), "the extra twelve buy nothing");
    }

    #[test]
    fn the_plan_never_loses_or_duplicates_a_row() {
        // `Ratio.Exec.partitioning_reads_every_row_once`.
        for rows in [0i64, 1, 7, 8, 100, 1_001] {
            for per_row in [1i64, 4, 64] {
                let p = plan(rows, per_row, 7).unwrap();
                assert_eq!(p.total_io(), rows, "rows={rows} per_row={per_row} {:?}", p.chunks);
            }
        }
    }

    #[test]
    fn every_partition_runs_exactly_once() {
        // ⛔ `//tla:unclaimed_partitions_check` as a running program. The claim
        // is an atomic fetch_add, so no index is handed out twice — and a
        // doubled total would be built from correct arithmetic on real rows.
        let p = plan(1_000, 64, 8).unwrap();
        let runs = AtomicI64::new(0);
        let out = run(&p, 8, &never, |_, chunk| {
            runs.fetch_add(1, Ordering::SeqCst);
            chunk
        });

        match out {
            Outcome::Complete(v) => {
                assert_eq!(v.len(), p.chunks.len());
                assert_eq!(v.iter().sum::<i64>(), 1_000, "every row, once");
            }
            Outcome::Cancelled => panic!("nothing cancelled it"),
        }
        assert_eq!(runs.load(Ordering::SeqCst), p.chunks.len() as i64);
    }

    #[test]
    fn no_index_is_claimed_twice_under_contention() {
        // ⛔ THE TEST THAT ALMOST WAS NOT ONE.
        //
        // The first version of this checked `every_partition_runs_exactly_once`
        // on eight partitions, and a mutation replacing the atomic `fetch_add`
        // with a load-then-store race PASSED IT — eight slots and fast work
        // leave the window essentially never open. A test that cannot fail on
        // the bug it names is not coverage.
        //
        // Many partitions, more workers than partitions, and real work inside
        // the loop. A duplicated claim overwrites one slot and leaves another
        // empty, which surfaces as `Cancelled` — so `Complete` with the right
        // total is the assertion.
        //
        // ⛔ AND IT STILL DOES NOT CATCH THE RACE. Measured, not assumed: with
        // 400 partitions, 16 workers, and spin loops widening the window, the
        // load/store mutation PASSES this test on Apple silicon. SeqCst
        // load/store leaves a window of a few nanoseconds and the scheduler
        // essentially never lands two threads inside it.
        //
        // ⚠ SO THIS IS NOT THE GUARD, AND IT MUST NOT BE READ AS ONE.
        // `//tla:unclaimed_partitions_check` is the guard — it checks every
        // interleaving rather than the ones this machine happened to take. What
        // this test catches is a GROSS regression: a claim removed altogether,
        // a slot indexed wrong, a worker that returns early.
        //
        // I could have kept widening the window until the mutation failed. That
        // would have made the bug easier to find than it actually is, and left
        // a test whose green meant nothing about the real interleavings.
        let p = Plan { chunks: (1..=400).collect(), per_row: 1 };
        for _ in 0..8 {
            let out = run(&p, 16, &never, |i, chunk| {
                for _ in 0..64 {
                    std::hint::spin_loop();
                }
                (i as i64) + chunk
            });
            match out {
                Outcome::Complete(v) => {
                    assert_eq!(v.len(), 400);
                    let expect: i64 = (1..=400i64).map(|c| (c - 1) + c).sum();
                    assert_eq!(v.iter().sum::<i64>(), expect, "each index exactly once");
                }
                Outcome::Cancelled => {
                    panic!("a slot was never filled — an index was claimed twice")
                }
            }
        }
    }

    #[test]
    fn a_cancelled_run_yields_no_partial_answer() {
        // ⛔ `//tla:partial_commit_check`. A total built from some of the
        // partitions is LOW, and low is indistinguishable from right — so
        // `Outcome` has no variant that could carry one.
        let p = plan(1_000, 64, 8).unwrap();
        let out: Outcome<Vec<i64>> = run(&p, 8, &|| true, |_, chunk| chunk);
        assert_eq!(out, Outcome::Cancelled);
    }

    #[test]
    fn the_answer_does_not_depend_on_the_worker_count() {
        // Different partitionings, same total. A planner that changed the
        // ANSWER when it changed the plan would make every figure depend on
        // machine shape, and no replay could ever reproduce one.
        let mut totals = Vec::new();
        for w in [1usize, 2, 3, 8, 16] {
            let p = plan(1_001, 64, w).unwrap();
            match run(&p, w, &never, |_, chunk| chunk) {
                Outcome::Complete(v) => totals.push(v.iter().sum::<i64>()),
                Outcome::Cancelled => panic!("nothing cancelled it"),
            }
        }
        assert!(totals.iter().all(|t| *t == 1_001), "{totals:?}");
    }

    #[test]
    fn a_plan_with_no_rows_is_still_a_plan() {
        let p = plan(0, 4, 8).unwrap();
        assert_eq!(p.total_io(), 0);
        assert_eq!(p.elapsed(), 0);
        match run(&p, 8, &never, |_, c| c) {
            Outcome::Complete(v) => assert_eq!(v.iter().sum::<i64>(), 0),
            Outcome::Cancelled => panic!("an empty plan completes"),
        }
    }

    #[test]
    fn nonsense_is_refused_rather_than_clamped() {
        assert!(plan(-1, 4, 8).is_err());
        assert!(plan(8, -1, 8).is_err());
        assert!(plan(8, 4, 0).is_err(), "zero workers is not a small number of workers");
    }
}
