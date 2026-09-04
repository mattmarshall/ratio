-------------------------- MODULE PoolPeriodEngine ---------------------------
(***************************************************************************)
(* The date a pooled holding carries, not a category.                       *)
(*                                                                          *)
(* `Ratio.Lots.AverageCost` pools the basis. It does not say what DATE     *)
(* the remainder and the slice carry. US single-category invents FIFO's    *)
(* oldest date on a mixed pool and classifies the sale long-term.          *)
(* Double-category invents two pools. Both invent a short-vs-long          *)
(* answer the lots do not support.                                         *)
(*                                                                          *)
(* ⛔ THE HONEST RULE IS A DATE, NOT A CATEGORY. When every lot agrees,     *)
(* that date is carried. Mixed or missing dates stay unset. Treating the   *)
(* rule as an Order — FIFO on mixed dates — invents long-term.             *)
(* `//tla:sort_and_walk_pool_period_check` flips the dial and              *)
(* ThePoolDateStaysUnset goes red.                                         *)
(*                                                                          *)
(* ⚠ THE DATES ARE THE LEAN EXAMPLE ON PURPOSE. Day 0 and day 400,         *)
(* dispose 400, threshold 365. FIFO is long; the other lot is short. A     *)
(* model that used dates that do not flip would stay green on an engine    *)
(* that never asked the question.                                          *)
(* `Ratio.Lots.PoolPeriod.treating_mixed_dates_as_an_order_invents_a_      *)
(* category`.                                                              *)
(*                                                                          *)
(* ⚠ NOT AN `Order` / `Method` / `lot_method` variant. The rule is which   *)
(* DATE a later classification reads — or that it reads none. A            *)
(* sort-and-walk has no place to put "unset".                              *)
(*                                                                          *)
(* The two dates and the threshold are CONSTANTS of the example, not a     *)
(* bound. Nothing in Next assigns any of them.                             *)
(***************************************************************************)
EXTENDS Integers

CONSTANTS
    TreatAsOrder     \* the bug. TRUE takes the FIFO (oldest) date.

VARIABLES
    classified,      \* the date a later isLongTerm is asked. -1 = unset
    lastOp

vars == <<classified, lastOp>>

Unset == -1

\* The Lean example. ⛔ THESE DATES FLIP. Day 0 / 400 / dispose 400 / 365.
DateA      == 0
DateB      == 400
DisposedOn == 400
Threshold  == 365

\* This spec IS the honest rule. Mixed dates stay unset.
\* FIFO invents DateA and classifies long.
PoolDate  == Unset
OrderDate == DateA

IsLong(acq) == DisposedOn - acq >= Threshold

TypeOK ==
    /\ classified \in {Unset, DateA, DateB}
    /\ lastOp \in {"init", "classify"}

Init ==
    /\ classified = Unset
    /\ lastOp = "init"

\* Classify the pool for a later disposal. The dial is the whole
\* thing: FALSE leaves the date unset; TRUE writes FIFO's oldest
\* date and invents a category.
Classify ==
    /\ classified = Unset
    /\ classified' = IF TreatAsOrder THEN OrderDate ELSE PoolDate
    /\ lastOp' = "classify"

Next ==
    \/ Classify
    \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* ⭐ THE OBLIGATION. Mixed dates stay unset. Treat the rule as an Order   *)
(* — TreatAsOrder — and the engine writes FIFO's oldest date. Same units,  *)
(* same basis, same proceeds. The books still tie. The figure that goes    *)
(* wrong is the RATE.                                                      *)
(*                                                                          *)
(* ⚠ NOT VACUOUS. `classified` is written by Classify from one of two      *)
(* arms. An engine that "just leaves the date unset" in one assignment     *)
(* has no dial to flip; this one does, and                                 *)
(* `//tla:sort_and_walk_pool_period_check` flips it.                       *)
(***************************************************************************)
ThePoolDateStaysUnset ==
    classified = Unset

(***************************************************************************)
(* The two dates disagree on the rate. Stated so a model that used dates   *)
(* that do not flip — both already long — cannot pass by never asking      *)
(* the question. `Ratio.Lots.PoolPeriod.treating_mixed_dates_as_an_order_  *)
(* invents_a_category`.                                                    *)
(***************************************************************************)
TreatingMixedDatesAsAnOrderInventsACategory ==
    IsLong(DateA) # IsLong(DateB)

=============================================================================
