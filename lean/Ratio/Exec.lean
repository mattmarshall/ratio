set_option warningAsError true
/-! `Ratio.Exec` — two compartments that do not substitute for each other.

`Ratio.Closure` counts READS and stops there, which is enough to prove the thing
that matters most — a NAV never reads the tax lots — but not enough to say what
adding workers does. This is the model that answers that, and the answer is
mostly NO.

⛔ IO AND CPU ARE DIFFERENT RESOURCES AND ONLY ONE OF THEM PARTITIONS. There is
one store, so the bytes come off it serially: splitting a scan across ten
workers does not make the store ten times faster. The work done ON the rows once
they arrive does divide. So a plan's elapsed time is

    max (all the IO, added up)  (the CPU of the SLOWEST partition)

⚠ AND THAT MEANS PARTITIONING A PURE SCAN BUYS NOTHING. If a row costs one unit
to read and one unit to process, the IO term already equals the total and no
number of workers moves it. Partitioning helps exactly when the per-row work
exceeds the per-row read, and it stops helping the moment the slowest partition
drops below the IO floor. `more_workers_never_beat_the_io_floor` is the theorem;
it is also the shape of every disappointing scaling result I have measured.

⭐ AND THE FLOOR IS WHY THE FACTOR REDESIGN MATTERS MORE THAN THE EXECUTOR.
`Ratio.Actions.Factor` removes forty thousand lot REWRITES — that is IO, and IO
is the term no amount of concurrency reaches. Making the executor wider would
not have helped; not doing the work is the only thing that does.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Exec

/-- What a piece of a plan costs, in the two compartments.

⛔ A PAIR, NOT A SUM. Adding them would say a plan can trade one for the other,
and the entire content of this file is that it cannot. -/
structure Work where
  /-- Time on the store. There is one store, so these add up across partitions. -/
  io : Nat
  /-- Time on the rows once they have arrived. Divides across workers. -/
  cpu : Nat
deriving Repr, DecidableEq

/-- Scanning `rows` rows, at `perRow` units of work each. -/
def scan (rows perRow : Nat) : Work := { io := rows, cpu := rows * perRow }

/-- A plan is a partition: how many rows each worker takes. -/
def plan (chunks : List Nat) (perRow : Nat) : List Work :=
  chunks.map (scan · perRow)

/-- All the IO, added up. One store, read serially. -/
def totalIo : List Work → Nat
  | [] => 0
  | w :: rest => w.io + totalIo rest

/-- The CPU of the slowest partition. Workers run at once, so the last one to
finish is the one that matters. -/
def slowestCpu : List Work → Nat
  | [] => 0
  | w :: rest => max w.cpu (slowestCpu rest)

/-- What the plan takes. -/
def elapsed (ws : List Work) : Nat := max (totalIo ws) (slowestCpu ws)

/-- The rows in a partition. -/
def rows : List Nat → Nat
  | [] => 0
  | n :: rest => n + rows rest

/- ── Correctness: every row is read exactly once ───────────────────────── -/

/-- **A partition reads the rows it was given — all of them, once each.**

The correctness statement in the IO compartment. A partitioning that lost a
chunk would read fewer, and one that overlapped would read some twice; both are
invisible in a total that was never checked against the whole. -/
theorem partitioning_reads_every_row_once (chunks : List Nat) (perRow : Nat) :
    totalIo (plan chunks perRow) = rows chunks := by
  induction chunks with
  | nil => rfl
  | cons n rest ih => simp [plan, totalIo, scan, rows] at *; omega

/- ── The floor ─────────────────────────────────────────────────────────── -/

/-- **⭐ NO PARTITION BEATS THE IO FLOOR.**

Whatever the split, the plan takes at least as long as reading the rows. This is
the statement that makes concurrency an optimization of the SECOND term only,
and it is why the answer to "can we make the NAV faster with more workers" is
no — and does not need to be yes. -/
theorem no_partition_beats_the_io_floor (ws : List Work) :
    totalIo ws ≤ elapsed ws := Nat.le_max_left _ _

/-- **⛔ AN IO-BOUND PLAN IGNORES ITS WORKERS ENTIRELY.**

Once the slowest partition's CPU is under the IO total, splitting further
changes nothing at all — not a little, nothing. Every disappointing scaling
result has this shape: the system was already against the floor, the extra
workers had nowhere to take time from, and the graph stayed flat while the bill
did not. -/
theorem an_io_bound_plan_ignores_its_workers (ws : List Work)
    (h : slowestCpu ws ≤ totalIo ws) : elapsed ws = totalIo ws :=
  Nat.max_eq_left h

/-- **A pure scan is io-bound**, so partitioning it buys nothing.

One unit of work per row read. The slowest partition can never have more CPU
than the whole has IO, so the previous theorem applies and the elapsed time is
the read. This is the case people expect concurrency to fix. -/
theorem a_pure_scan_is_io_bound (chunks : List Nat) :
    elapsed (plan chunks 1) = rows chunks := by
  rw [← partitioning_reads_every_row_once chunks 1]
  apply an_io_bound_plan_ignores_its_workers
  induction chunks with
  | nil => simp [plan, slowestCpu, totalIo]
  | cons n rest ih =>
    simp [plan, slowestCpu, totalIo, scan] at *
    omega

/-- **And a plan with no work at all still pays for its reads**, which is the
degenerate case worth pinning: `elapsed` is not a CPU measure that happens to
mention IO. -/
theorem reading_costs_even_when_nothing_is_computed (chunks : List Nat) :
    elapsed (plan chunks 0) = rows chunks := by
  rw [← partitioning_reads_every_row_once chunks 0]
  apply an_io_bound_plan_ignores_its_workers
  induction chunks with
  | nil => simp [plan, slowestCpu, totalIo]
  | cons n rest ih =>
    simp [plan, slowestCpu, totalIo, scan] at *
    omega

/- ── Where partitioning does help ──────────────────────────────────────── -/

/-- **⭐ AND WHERE THE WORK IS HEAVY, SPLITTING IT DOES HELP — up to the floor.**

Eight rows at four units of work each: unpartitioned the CPU is 32 and the plan
takes 32. Split into two fours, the slowest partition is 16 and it takes 16.
Split into four twos, the slowest is 8 and it takes 8 — which is the IO total,
and splitting into eight ones still takes 8.

⚠ THE LAST TWO ARE THE SAME NUMBER. Doubling the workers from four to eight
bought exactly nothing, and a benchmark that stopped at four would have shown a
clean linear curve pointing at a speed-up that does not exist. -/
theorem splitting_helps_until_it_does_not :
    elapsed (plan [8] 4) = 32
    ∧ elapsed (plan [4, 4] 4) = 16
    ∧ elapsed (plan [2, 2, 2, 2] 4) = 8
    ∧ elapsed (plan [1, 1, 1, 1, 1, 1, 1, 1] 4) = 8 := by
  refine ⟨?_, ?_, ?_, ?_⟩ <;>
    simp [elapsed, plan, scan, totalIo, slowestCpu]

/- ── The decision a planner makes ──────────────────────────────────────── -/

/-- **Is another partition worth anything?**

The whole planner rule, and it is one comparison: splitting helps only while the
slowest partition is doing more CPU than the plan is doing IO. Below that the
IO floor is binding and another worker has nowhere to take time from.

⛔ THE DECISION IS HERE AND THE FOLDS ARE IN RUST, the same arrangement as
`Ratio.Ingest.kind_is_determined_by_count` — the running code counts rows and
sums IO with a loop, and asks this whether to go further. The bridge is
`a_plan_that_cannot_be_helped_takes_its_io` below: the answer depends on nothing
but the two totals, so a fold and this cannot disagree about a partition. -/
def splittingHelps (io cpu : Nat) : Bool := decide (io < cpu)

/-- **⭐ WHEN SPLITTING DOES NOT HELP, THE PLAN TAKES ITS IO — exactly.**

The bridge, and the theorem that makes the planner's one comparison sound. If
`splittingHelps` says no, the elapsed time is the IO total and nothing a planner
does afterwards can move it. -/
theorem a_plan_that_cannot_be_helped_takes_its_io (ws : List Work)
    (h : splittingHelps (totalIo ws) (slowestCpu ws) = false) :
    elapsed ws = totalIo ws := by
  apply an_io_bound_plan_ignores_its_workers
  simp [splittingHelps] at h
  omega

/-- And when it says yes, there is genuinely something to take: the plan is
currently paying for CPU it does not have to. Stated so the rule is not
vacuously safe by always saying no. -/
theorem a_plan_that_can_be_helped_is_paying_for_cpu (ws : List Work)
    (h : splittingHelps (totalIo ws) (slowestCpu ws) = true) :
    totalIo ws < elapsed ws := by
  simp [splittingHelps] at h
  simp [elapsed]
  omega

/-- **An uneven partition is as slow as its worst piece.**

Sixteen rows split seven ways is not sixteen sevenths of anything if one worker
took ten of them. Stated because a planner that splits by CONVENIENT boundaries
— a file, an instrument, a day — is splitting by something uncorrelated with
work, and the tail is what it will get. -/
theorem the_slowest_partition_sets_the_pace :
    elapsed (plan [10, 1, 1, 1, 1, 1, 1] 4) = 40 := by
  simp [elapsed, plan, scan, totalIo, slowestCpu]

/-- **The NAV is neither**, and that is the point of `Ratio.Closure`.

Five hundred and three reads at one unit each. There is nothing here to
partition and nothing to gain by it — the whole argument for this system's speed
is that the work was never done, not that it was spread out. -/
theorem the_nav_is_too_small_to_partition :
    elapsed (plan [503] 1) = 503 := by
  simp [elapsed, plan, scan, totalIo, slowestCpu]

end Ratio.Exec
