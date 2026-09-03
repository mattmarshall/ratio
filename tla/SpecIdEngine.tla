---------------------------- MODULE SpecIdEngine ----------------------------
(***************************************************************************)
(* Specific identification is a SELECTION the taxpayer names, not a sort   *)
(* of the holding.                                                          *)
(*                                                                          *)
(* `Ratio.Lots.SpecId` proves the arithmetic of one sale: naming the       *)
(* middle lot of three gives up a basis no `Order` produces, and that an   *)
(* unknown, overspecified, insufficient or empty name is refused rather    *)
(* than walked. Neither says what an ENGINE does if it treats SpecID as    *)
(* another Method — sort the holding once, walk, never look at the name.   *)
(*                                                                          *)
(* ⛔ THAT IS THE MISTAKE THIS SPEC EXISTS TO CATCH. A sort-and-walk is a   *)
(* function of the lots. SpecID is not. The three-lot holding from the     *)
(* Lean file (basis 10 / 40 / 70) takes 40 when lot 2 is named; FIFO       *)
(* takes 10. A name-blind sort cannot take the middle.                     *)
(* `//tla:sort_and_walk_specid_check` flips the dial and                   *)
(* TheLotTakenIsTheOneNamed goes red.                                      *)
(*                                                                          *)
(* ⚠ THE NAMED LOT IS NOT THE FIFO LOT ON PURPOSE. Naming lot 1 would     *)
(* agree with FIFO and a probe that used that name would stay green on an  *)
(* engine that never saw it. `Ratio.Lots.SpecId.no_ordering_takes_the_     *)
(* middle`.                                                                *)
(*                                                                          *)
(* The three lots and the name are CONSTANTS of the example, not a bound.  *)
(* Nothing in Next assigns any of them.                                    *)
(***************************************************************************)
EXTENDS Integers

CONSTANTS
    TreatAsOrder     \* the bug. TRUE ignores the name and walks FIFO.

VARIABLES
    named,           \* the taxpayer's attested name. 0 = not yet sold
    took,            \* which lot was taken: 0 = none, 1 / 2 / 3
    lastOp

vars == <<named, took, lastOp>>

Unsold == 0
None   == 0
LotA   == 1
LotB   == 2
LotC   == 3

\* The three-lot holding. ⛔ THE MIDDLE ONE, NOT THE FIRST.
\* FIFO would take A. SpecID naming B takes B.
BasisA == 10
BasisB == 40
BasisC == 70
NamedLot == LotB

\* The taxpayer named this lot. Not a function of the holding.
SpecIdPick == NamedLot

\* A name-blind sort. FIFO — the method an engine falls through to
\* when the name is ignored, and the silent default this surface
\* exists to prevent. `Ratio.Lots.SpecId.selectFirst_of_nothing_is_fifo`.
OrderPick == LotA

TypeOK ==
    /\ named \in {Unsold, NamedLot}
    /\ took \in {None, LotA, LotB, LotC}
    /\ lastOp \in {"init", "sell"}

Init ==
    /\ named = Unsold
    /\ took = None
    /\ lastOp = "init"

Sell ==
    /\ named = Unsold
    /\ named' = NamedLot
    /\ took' = IF TreatAsOrder THEN OrderPick ELSE SpecIdPick
    /\ lastOp' = "sell"

Next ==
    \/ Sell
    \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* ⭐ THE OBLIGATION. The lot taken is the one the taxpayer named. Drop    *)
(* the name — TreatAsOrder — and the engine takes A. That is FIFO, and     *)
(* the books still tie.                                                    *)
(*                                                                          *)
(* ⚠ NOT VACUOUS. `took` is written by Sell from one of two arms. An       *)
(* engine that "just sets the right lot" in one assignment has no dial to  *)
(* flip; this one does, and `//tla:sort_and_walk_specid_check` flips it.   *)
(***************************************************************************)
TheLotTakenIsTheOneNamed ==
    took = None \/ took = SpecIdPick

(***************************************************************************)
(* The named lot is not the FIFO lot. Stated so a model that named lot 1   *)
(* — where FIFO and SpecID agree — cannot pass by never asking the         *)
(* question. `Ratio.Lots.SpecId.no_ordering_takes_the_middle`.             *)
(***************************************************************************)
TheNamedLotIsNotFifo ==
    SpecIdPick # OrderPick

=============================================================================
