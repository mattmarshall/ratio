--------------------------- MODULE Valuation ---------------------------
(***************************************************************************)
(* Marking a book to market, as a state machine.                           *)
(*                                                                         *)
(* `Ratio.Valuation` proves what ONE mark does: it conserves, it lands on   *)
(* market value, marking twice at one price posts nothing, and a later      *)
(* price posts only the difference. Those are theorems about a function.    *)
(*                                                                         *)
(* What they cannot say is what a SEQUENCE does — and valuation is where    *)
(* sequences bite, because the carrying value is shared state that marking, *)
(* trading and correcting all read and write:                              *)
(*                                                                         *)
(*   * a price correction arrives and the position is marked twice on one   *)
(*     day. Does the second mark start from the carrying value, or from     *)
(*     the original cost? The second double-counts, and the book still      *)
(*     TIES either way — a balanced entry of the wrong size is balanced.    *)
(*     Conservation cannot catch this. That is the whole point.             *)
(*                                                                         *)
(*   * a trade lands between two marks, so the quantity the second mark     *)
(*     values is not the quantity the first one did.                        *)
(*                                                                         *)
(*   * a NAV is struck and a backdated price arrives afterwards.            *)
(***************************************************************************)
EXTENDS Naturals, Integers, FiniteSets

CONSTANTS
    Units,              \* quantities a trade may move
    Prices,             \* prices that may be observed
    Days,               \* valuation days
    MarkFromCarrying    \* the fix. FALSE marks from COST and double-counts.

VARIABLES
    units,      \* how many are held
    cost,       \* what they were bought for — never revalued
    carrying,   \* what the book holds them at, after marks
    seen,       \* observations: a set of <<day, price>>
    lastMark,   \* the price the last mark used, or 0 for none
    struck,     \* NAV figures already struck, pinned and immutable
    lastOp

vars == <<units, cost, carrying, seen, lastMark, struck, lastOp>>

----------------------------------------------------------------------------
(* The price to mark at: the latest observation on or before the day.
   Mirrors `Ratio.Valuation.markPrice`. *)

Eligible(asOf) == {o \in seen : o[1] =< asOf}

MarkPrice(asOf) ==
    IF Eligible(asOf) = {} THEN 0
    ELSE (CHOOSE o \in Eligible(asOf) :
            \A p \in Eligible(asOf) : p[1] =< o[1])[2]

Market(price) == units * price

----------------------------------------------------------------------------

Init ==
    /\ units = 0
    /\ cost = 0
    /\ carrying = 0
    /\ seen = {}
    /\ lastMark = 0
    /\ struck = {}
    /\ lastOp = "init"

(* A trade. Cost and carrying both move by what was paid — a purchase is
   carried at cost until something marks it. *)
Trade(q, p) ==
    /\ units' = units + q
    /\ cost' = cost + q * p
    /\ carrying' = carrying + q * p
    /\ lastOp' = "trade"
    /\ UNCHANGED <<seen, lastMark, struck>>

(* A price arrives. Prices are observations, so they accumulate; a correction
   is a new observation, never an edit. *)
Observe(d, p) ==
    /\ <<d, p>> \notin seen
    /\ seen' = seen \cup {<<d, p>>}
    /\ lastOp' = "observe"
    /\ UNCHANGED <<units, cost, carrying, lastMark, struck>>

(* Mark to market.
   ⛔ THE DELTA IS FROM THE CARRYING VALUE, NOT FROM COST. Marking from cost
   posts the whole gain again every time, and the entry is still BALANCED, so
   the trial balance ties while the book is wrong. *)
Mark(d) ==
    /\ MarkPrice(d) # 0
    /\ LET price == MarkPrice(d)
           delta == IF MarkFromCarrying
                      THEN Market(price) - carrying
                      ELSE Market(price) - cost
       IN /\ carrying' = carrying + delta
          /\ lastMark' = price
    /\ lastOp' = "mark"
    /\ UNCHANGED <<units, cost, seen, struck>>

(* Strike a NAV. Pinned: what is struck is struck. *)
Strike ==
    /\ struck' = struck \cup {carrying}
    /\ lastOp' = "strike"
    /\ UNCHANGED <<units, cost, carrying, seen, lastMark>>

Next ==
    \/ \E q \in Units, p \in Prices : Trade(q, p)
    \/ \E d \in Days, p \in Prices : Observe(d, p)
    \/ \E d \in Days : Mark(d)
    \/ Strike

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(* ⚠ A BOUND, not a property. `Trade` grows the position every time it fires,
   so the state space is infinite and TLC explores it forever. This keeps the
   search finite; it says nothing about the system. *)
StateBound ==
    /\ units =< 2
    /\ cost =< 4
    /\ carrying =< 8
    /\ carrying >= 0 - 8
    /\ Cardinality(struck) =< 2

(***************************************************************************)
(* 1. AFTER A MARK, THE BOOK HOLDS MARKET VALUE.                           *)
(*                                                                         *)
(*    `Ratio.Valuation.mark_lands_on_market` proves this of one mark. This  *)
(*    says it survives every sequence — including a mark, then another mark *)
(*    at a corrected price on the same day, which is where marking from     *)
(*    cost goes wrong.                                                      *)
(*                                                                         *)
(*    ⚠ Stated only for the step that just marked. A trade afterwards       *)
(*    changes the quantity, and the book is then at cost for the new part   *)
(*    until something marks it again — which is correct, and is why this is *)
(*    an ACTION property rather than an invariant.                          *)
(***************************************************************************)
MarkLandsOnMarket ==
    [][ (lastOp' = "mark") => (carrying' = units * lastMark') ]_vars

(***************************************************************************)
(* 2. A STRUCK NAV IS IMMUTABLE.                                           *)
(*                                                                         *)
(*    Marks, trades and backdated prices all land AFTER it. A figure that   *)
(*    was struck stays struck, which is what makes a NAV citable at all.    *)
(***************************************************************************)
StrikesAreImmutable ==
    [][ \A n \in struck : n \in struck' ]_vars

(***************************************************************************)
(* 3. MARKING DOES NOT TOUCH COST.                                         *)
(*                                                                         *)
(*    Unrealized gain is market minus cost, so a valuation that quietly     *)
(*    moved cost would make the gain unrecoverable — and it is the figure   *)
(*    an investor is taxed on.                                              *)
(***************************************************************************)
CostIsNeverRevalued ==
    [][ (lastOp' = "mark") => (cost' = cost) ]_vars

(***************************************************************************)
(* 4. THE PRICE USED IS NEVER FROM THE FUTURE.                             *)
(***************************************************************************)
MarkUsesAnObservedPrice ==
    [][ (lastOp' = "mark") => (\E o \in seen : o[2] = lastMark') ]_vars

=============================================================================
