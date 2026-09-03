---------------------------- MODULE MinTaxEngine ----------------------------
(***************************************************************************)
(* Tax-minimising relief is a ranking at a PRICE, not a sort of the holding.*)
(*                                                                          *)
(* `Ratio.Lots.MinTax` proves the arithmetic of one sale: the same two lots,*)
(* two prices, two right answers, and that no `Order` reproduces both.      *)
(* `Ratio.Lots.Methods.a_tax_minimising_method_is_not_a_function_of_the_    *)
(* lots` is the inequality. Neither says what an ENGINE does if it treats   *)
(* MinTax as another Method — sort the holding once, walk, never look at    *)
(* the sale price.                                                          *)
(*                                                                          *)
(* ⛔ THAT IS THE MISTAKE THIS SPEC EXISTS TO CATCH. A sort-and-walk is a    *)
(* function of the lots. MinTax is not. The close-base holding from the     *)
(* Lean file (basis 10 short, basis 12 long) flips between 50 and 5; a      *)
(* price-blind sort cannot. `//tla:sort_and_walk_mintax_check` flips the    *)
(* dial and TheLotTakenMinimisesTax goes red.                               *)
(*                                                                          *)
(* ⚠ THE BASES ARE CLOSE ON PURPOSE. At a basis of 40 the long lot wins at  *)
(* both prices and a probe that used that holding would stay green on an    *)
(* engine that never saw the price. `Ratio.Lots.MinTax.far_bases_do_not_    *)
(* flip`.                                                                   *)
(*                                                                          *)
(* The two lots and two prices are CONSTANTS of the example, not a bound.   *)
(* ShortWeight is a jurisdiction's number, the same way `long_term_days`    *)
(* is. Nothing in Next assigns any of them.                                 *)
(***************************************************************************)
EXTENDS Integers

CONSTANTS
    TreatAsOrder     \* the bug. TRUE sorts the holding once and walks.

VARIABLES
    price,           \* the sale's per-unit proceeds. 0 = not yet sold
    took,            \* which lot was taken: 0 = none, 1 = A (short, 10), 2 = B (long, 12)
    lastOp

vars == <<price, took, lastOp>>

Unsold == 0
None   == 0
LotA   == 1
LotB   == 2

\* The close-base holding. ⛔ TWELVE, NOT FORTY.
BasisA == 10
BasisB == 12
ShortWeight == 2
Prices == {5, 50}

\* `Ratio.Lots.MinTax.taxAt` / `taxCost`: short-term is weighted.
Tax(basis, short, px) ==
    LET gain == px - basis
    IN IF short THEN gain * ShortWeight ELSE gain

\* Cheaper-tax lot at this price. A is short, B is long.
\* At 50: A = 80, B = 38 → B. At 5: A = −10, B = −7 → A.
MinTaxPick(px) ==
    IF Tax(BasisA, TRUE, px) < Tax(BasisB, FALSE, px) THEN LotA ELSE LotB

\* A price-blind sort. Prefer-long — the method that looks like MinTax and
\* is not. `Ratio.Lots.MinTax.preferring_long_term_is_not_minimising_tax`.
\* HIFO by basis would pick B too (12 > 10). Either way it cannot flip.
OrderPick == LotB

TypeOK ==
    /\ price \in {Unsold} \cup Prices
    /\ took \in {None, LotA, LotB}
    /\ lastOp \in {"init", "sell"}

Init ==
    /\ price = Unsold
    /\ took = None
    /\ lastOp = "init"

Sell(px) ==
    /\ px \in Prices
    /\ price' = px
    /\ took' = IF TreatAsOrder THEN OrderPick ELSE MinTaxPick(px)
    /\ lastOp' = "sell"

Next ==
    \/ \E px \in Prices : Sell(px)
    \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* ⭐ THE OBLIGATION. The lot taken is the one that costs less in tax at    *)
(* THIS sale's price. Drop the price — TreatAsOrder — and the engine takes  *)
(* B at both prices. At 5 that is the wrong lot, and the books still tie.   *)
(*                                                                          *)
(* ⚠ NOT VACUOUS. `took` is written by Sell from one of two arms. An        *)
(* engine that "just sets the right lot" in one assignment has no dial to   *)
(* flip; this one does, and `//tla:sort_and_walk_mintax_check` flips it.    *)
(***************************************************************************)
TheLotTakenMinimisesTax ==
    took = None \/ took = MinTaxPick(price)

(***************************************************************************)
(* The two prices rank the same holding differently. Stated so a model that *)
(* only ever sells at 50 — where prefer-long and MinTax agree — cannot      *)
(* pass by never asking the question. `Ratio.Lots.MinTax.the_two_prices_    *)
(* rank_the_holding_differently`.                                           *)
(***************************************************************************)
TheTwoPricesDisagree ==
    MinTaxPick(50) # MinTaxPick(5)

=============================================================================
