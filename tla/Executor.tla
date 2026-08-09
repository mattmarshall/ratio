--------------------------- MODULE Executor ---------------------------
(***************************************************************************)
(* Partitions handed to workers, and what a run of that does.               *)
(*                                                                          *)
(* `Ratio.Exec` proves what a partitioning COSTS — IO adds up across         *)
(* partitions while only CPU divides, so no split beats the IO floor. It     *)
(* says nothing about whether the partitions all ran, ran once, or were      *)
(* still running when somebody read the answer. Those are sequence           *)
(* problems.                                                                 *)
(*                                                                           *)
(* ⛔ AND BOTH FAILURES HERE ARE ARITHMETICALLY SILENT, which is the pattern  *)
(* this whole system keeps running into:                                     *)
(*                                                                           *)
(*   A partition run TWICE double-counts. The total is wrong and every       *)
(*   individual number in it is a real number computed correctly from real   *)
(*   rows. `Ratio.Exec.partitioning_reads_every_row_once` is the property    *)
(*   that fails, and nothing downstream can notice: the trial balance ties   *)
(*   on whatever it is handed.                                               *)
(*                                                                           *)
(*   A partial result read as a whole one is a SMALLER number that looks     *)
(*   exactly like a correct one. There is no ragged edge, no null, no error  *)
(*   — a NAV computed from six of eight partitions is simply low, and        *)
(*   `Ratio.Period.one_answer_per_day` will refuse to restate it.            *)
(*                                                                           *)
(* Two dials, one per failure.                                               *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Partitions,        \* the split, as `Ratio.Exec.plan` chose it
    Workers,
    NoWorker,          \* a model value, distinct from every worker
    ClaimBeforeWork,   \* the fix. FALSE lets two workers take the same one.
    CommitAtomically   \* the fix. FALSE makes a partial answer readable.

\* ⚠ A MODEL VALUE, not `CHOOSE x : x \notin Workers`. That is an unbounded
\* CHOOSE and TLC refuses to evaluate it — correctly, since it asks for a
\* witness from the whole universe.
ASSUME NoWorker \notin Workers

VARIABLES
    claim,      \* [Partitions -> Workers \cup {NoWorker}]
    ranBy,      \* [Partitions -> SUBSET Workers] — who ACTUALLY executed it
    committed,  \* SUBSET Partitions — results a reader can see
    cancelled,
    lastOp

vars == <<claim, ranBy, committed, cancelled, lastOp>>

----------------------------------------------------------------------------

Init ==
    /\ claim = [p \in Partitions |-> NoWorker]
    /\ ranBy = [p \in Partitions |-> {}]
    /\ committed = {}
    /\ cancelled = FALSE

    /\ lastOp = "init"

(* Take a partition.
   ⛔ THE CLAIM IS THE WHOLE GUARD. Without it nothing stops two workers
   picking the same one — and the arithmetic each performs is correct. *)
Claim(p, w) ==
    /\ ~cancelled
    /\ IF ClaimBeforeWork THEN claim[p] = NoWorker ELSE TRUE
    /\ claim' = [claim EXCEPT ![p] = w]
    /\ lastOp' = "claim"
    /\ UNCHANGED <<ranBy, committed, cancelled>>

(* Execute it. A worker may only run what it holds, when claiming is on. *)
Run(p, w) ==
    /\ ~cancelled
    /\ w \notin ranBy[p]
    /\ IF ClaimBeforeWork THEN claim[p] = w ELSE TRUE
    /\ ranBy' = [ranBy EXCEPT ![p] = @ \cup {w}]
    /\ lastOp' = "run"
    /\ UNCHANGED <<claim, committed, cancelled>>

Ran(p) == ranBy[p] # {}

(* Publish.
   ⛔ ALL OR NOTHING. A reader who can see six partitions of eight gets a
   number that is simply low, and low is indistinguishable from right. *)
Commit ==
    /\ ~cancelled
    /\ IF CommitAtomically
         THEN /\ \A p \in Partitions : Ran(p)
              /\ committed' = Partitions
         ELSE /\ \E p \in Partitions : Ran(p) /\ p \notin committed
              /\ committed' = committed \cup {p \in Partitions : Ran(p)}
    /\ lastOp' = "commit"
    /\ UNCHANGED <<claim, ranBy, cancelled>>

(* The query is abandoned — a timeout, a disconnect, an operator giving up. *)
Cancel ==
    /\ ~cancelled
    /\ committed # Partitions
    /\ cancelled' = TRUE
    /\ lastOp' = "cancel"
    /\ UNCHANGED <<claim, ranBy, committed>>

Settled == cancelled \/ committed = Partitions

Next ==
    \/ \E p \in Partitions, w \in Workers : Claim(p, w)
    \/ \E p \in Partitions, w \in Workers : Run(p, w)
    \/ Commit
    \/ Cancel
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. ⭐ NO PARTITION IS EXECUTED TWICE.                                    *)
(*                                                                         *)
(*    `Ratio.Exec.partitioning_reads_every_row_once` is proved of a         *)
(*    partition, as a list. This is that property surviving contact with    *)
(*    workers. Double execution double-counts, and the double-counted total *)
(*    is built entirely from correct arithmetic on real rows — which is the *)
(*    same shape as the split applied twice, the mark taken from cost, and  *)
(*    the partial lot relief that rounded. Balanced, and the wrong size.    *)
(***************************************************************************)
NoPartitionRunsTwice == \A p \in Partitions : Cardinality(ranBy[p]) =< 1

(***************************************************************************)
(* 2. ⭐ A READER SEES ALL OF IT OR NONE OF IT.                             *)
(*                                                                         *)
(*    A NAV folded from six partitions of eight is not an error, a null, or *)
(*    a ragged edge. It is a smaller number that looks exactly like a       *)
(*    correct one — and by the time anybody could tell, it has been struck  *)
(*    and `one_answer_per_day` refuses to replace it.                       *)
(***************************************************************************)
CommitIsAllOrNothing == committed = {} \/ committed = Partitions

(***************************************************************************)
(* 3. AND A CANCELLED QUERY LEAVES NOTHING BEHIND.                         *)
(*                                                                         *)
(*    Abandoning work must not be a way to publish half of it. Stated       *)
(*    separately from 2 because the interesting cancellation is the one     *)
(*    that arrives BETWEEN partitions, which is exactly when a non-atomic   *)
(*    commit has something partial to leak.                                 *)
(***************************************************************************)
CancelPublishesNothing == (cancelled /\ committed # Partitions) => committed = {}

(***************************************************************************)
(* 4. Nothing is committed that was never run — the direction that would    *)
(*    otherwise be assumed rather than checked.                             *)
(***************************************************************************)
CommittedWasRun == \A p \in committed : Ran(p)

TypeOK ==
    /\ claim \in [Partitions -> Workers \cup {NoWorker}]
    /\ committed \subseteq Partitions
    /\ cancelled \in BOOLEAN

=============================================================================
