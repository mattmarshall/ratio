-------------------------- MODULE AverageCostEngine --------------------------
(***************************************************************************)
(* Average cost is a POOL of the holding, not a sort of the lots.           *)
(*                                                                          *)
(* `Ratio.Lots.AverageCost` proves the arithmetic of one sale: three lots  *)
(* at 10 / 20 / 60 pool to 30, a basis no `Order` produces, and that a     *)
(* figure that will not divide is refused rather than rounded. Neither     *)
(* says what an ENGINE does if it treats average cost as another Method —  *)
(* sort the holding once, walk, never pool.                                *)
(*                                                                          *)
(* ⛔ THAT IS THE MISTAKE THIS SPEC EXISTS TO CATCH. A sort-and-walk is a   *)
(* function of the lots. Average cost is not. The 10 / 20 / 60 holding     *)
(* from the Lean file gives up 30 when pooled; FIFO gives up 10. A         *)
(* walk that never pooled cannot give up 30.                               *)
(* `//tla:sort_and_walk_average_cost_check` flips the dial and             *)
(* TheBasisTakenIsThePooledBasis goes red.                                 *)
(*                                                                          *)
(* ⚠ THE POOL IS NOT ANY LOT'S BASIS ON PURPOSE. 10 / 40 / 70 pools to    *)
(* 40, which equals the middle lot, and a probe that used that holding     *)
(* would stay green on an engine that walked SpecID-style to lot 2.        *)
(* `Ratio.Lots.AverageCost.the_pooled_basis_is_not_any_lots_basis`.        *)
(*                                                                          *)
(* The three lots and the pool are CONSTANTS of the example, not a bound.  *)
(* Nothing in Next assigns any of them.                                    *)
(***************************************************************************)
EXTENDS Integers

CONSTANTS
    TreatAsOrder     \* the bug. TRUE ignores the pool and walks FIFO.

VARIABLES
    sold,            \* 0 = not yet sold, 1 = sold
    took,            \* basis given up: 0 = none, else the figure
    lastOp

vars == <<sold, took, lastOp>>

Unsold == 0
None   == 0

\* The three-lot holding. ⛔ THIRTY, NOT FORTY.
\* 10 / 40 / 70 pools to a lot's own basis. This one does not.
BasisA == 10
BasisB == 20
BasisC == 60
PooledBasis == (BasisA + BasisB + BasisC) \div 3

\* The pool. Not a function of which lot is first.
AverageCostPick == PooledBasis

\* A name-blind / pool-blind sort. FIFO — the method an engine falls
\* through to when the pool is ignored, and the silent default this
\* surface exists to prevent.
\* `Ratio.Lots.AverageCost.an_ordering_leaves_the_other_lots`.
OrderPick == BasisA

TypeOK ==
    /\ sold \in {Unsold, 1}
    /\ took \in {None, BasisA, BasisB, BasisC, PooledBasis}
    /\ lastOp \in {"init", "sell"}

Init ==
    /\ sold = Unsold
    /\ took = None
    /\ lastOp = "init"

Sell ==
    /\ sold = Unsold
    /\ sold' = 1
    /\ took' = IF TreatAsOrder THEN OrderPick ELSE AverageCostPick
    /\ lastOp' = "sell"

Next ==
    \/ Sell
    \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* ⭐ THE OBLIGATION. The basis taken is the pooled basis. Drop the pool   *)
(* — TreatAsOrder — and the engine takes 10. That is FIFO, and the books   *)
(* still tie.                                                              *)
(*                                                                          *)
(* ⚠ NOT VACUOUS. `took` is written by Sell from one of two arms. An       *)
(* engine that "just sets the right basis" in one assignment has no dial   *)
(* to flip; this one does, and `//tla:sort_and_walk_average_cost_check`    *)
(* flips it.                                                               *)
(***************************************************************************)
TheBasisTakenIsThePooledBasis ==
    took = None \/ took = AverageCostPick

(***************************************************************************)
(* The pooled basis is not any lot's basis. Stated so a model that used    *)
(* 10 / 40 / 70 — where the pool equals lot B — cannot pass by never       *)
(* asking the question. `Ratio.Lots.AverageCost.the_pooled_basis_is_not_   *)
(* any_lots_basis`.                                                        *)
(***************************************************************************)
ThePooledBasisIsNotAnOrdering ==
    /\ AverageCostPick # OrderPick
    /\ AverageCostPick # BasisA
    /\ AverageCostPick # BasisB
    /\ AverageCostPick # BasisC

=============================================================================
