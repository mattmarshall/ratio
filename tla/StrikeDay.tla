------------------------------ MODULE StrikeDay ------------------------------
(***************************************************************************)
(* Which day a strike's price check asks about, against the day it lands.  *)
(*                                                                         *)
(* `Ratio.Valuation.strike_refuses_exactly_when_something_is_unpriced`     *)
(* proves the refusal is exactly the unpriced list AT A DAY, and           *)
(* `Ratio.Period.strike_lands_on_the_day_being_valued` proves a strike     *)
(* lands on the day being valued. Each is about ONE day. The CLI had two:  *)
(* `ratio strike --as-of D` ran the price check at D and struck at the     *)
(* clock's day. Asking for 30 June in July recorded July's NAV under       *)
(* July's id, spent July's one answer, and was checked against prices for  *)
(* a day it was not about. Both theorems held; the composition did not,    *)
(* and the books tied throughout.                                          *)
(*                                                                         *)
(* ⭐ THE FIX IS A REFUSAL, NOT A BACKDATED STRIKE. Taking the valuation    *)
(* point from --as-of would change the strike id, what the NAVS ledger     *)
(* means and what `one_answer_per_view_per_day` is about, and a fold cut   *)
(* at a past day over today's journal is a question no theorem here        *)
(* answers. So --as-of must name the day the strike lands on, or nothing   *)
(* is struck.                                                              *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Days,                   \* valuation days, numbered from 1
    AsOfMustBeTheStrikeDay, \* the fix. FALSE checks the asked day and strikes the clock's.
    BareStrikeChecksPrices  \* the fix. FALSE strikes without --as-of unchecked.

\* `ratio strike` with no --as-of. Days are numbered from 1, so this is none.
NoDay == 0

VARIABLES
    today,   \* the clock's day: wall time, or RATIO_SEED_EPOCH
    held,    \* whether the fund holds a position
    seen,    \* the days a price was observed on
    struck,  \* the strikes taken, each [asked, day, priced]
    lastOp

vars == <<today, held, seen, struck, lastOp>>

Asks == Days \cup {NoDay}

(* Priced on day d: nothing is held, or a price was observed on or before d.
   Mirrors `Ratio.Valuation.priced` over `markPrice`. *)
PricedOn(d) == ~held \/ \E o \in seen : o =< d

(* The gate in front of the strike, as `blocking_text` runs it. *)
Gate(asked) ==
    IF asked = NoDay
    THEN BareStrikeChecksPrices => PricedOn(today)
    ELSE /\ AsOfMustBeTheStrikeDay => asked = today
         /\ PricedOn(asked)

TypeOK ==
    /\ today \in Days
    /\ held \in BOOLEAN
    /\ seen \subseteq Days
    /\ struck \subseteq [asked : Asks, day : Days, priced : BOOLEAN]

Init ==
    /\ today = CHOOSE d \in Days : \A e \in Days : d =< e
    /\ held = FALSE
    /\ seen = {}
    /\ struck = {}
    /\ lastOp = "init"

Trade(h) ==
    /\ held' = h
    /\ lastOp' = "trade"
    /\ UNCHANGED <<today, seen, struck>>

(* ⚠ A price may carry a day after the clock's. A seeded book runs on a fixed
   epoch, and a vendor dates its own file. That is how a check run at the
   wrong day passes a position that is unpriced on the day actually struck. *)
Observe(d) ==
    /\ d \notin seen
    /\ seen' = seen \cup {d}
    /\ lastOp' = "observe"
    /\ UNCHANGED <<today, held, struck>>

Advance ==
    /\ today + 1 \in Days
    /\ today' = today + 1
    /\ lastOp' = "advance"
    /\ UNCHANGED <<held, seen, struck>>

(* A strike lands on the clock's day, and once per day: one view's
   `Ratio.Period.one_answer_per_view_per_day`. *)
Strike(asked) ==
    /\ Gate(asked)
    /\ \A s \in struck : s.day # today
    /\ struck' = struck \cup {[asked |-> asked, day |-> today, priced |-> PricedOn(today)]}
    /\ lastOp' = "strike"
    /\ UNCHANGED <<today, held, seen>>

Next ==
    \/ \E h \in BOOLEAN : Trade(h)
    \/ \E d \in Days : Observe(d)
    \/ Advance
    \/ \E a \in Asks : Strike(a)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. A STRIKE ASKED FOR A DAY LANDS ON THAT DAY.                          *)
(*                                                                         *)
(*    The operator's question and the recorded answer name the same day.   *)
(*    Violated, a NAV asked for the 30th is filed under the 3rd, and the   *)
(*    3rd's one answer is spent on a question nobody asked.                *)
(***************************************************************************)
AStrikeLandsOnTheDayItWasAskedFor ==
    \A s \in struck : s.asked \in {NoDay, s.day}

(***************************************************************************)
(* 2. A CHECKED STRIKE IS PRICED ON THE DAY IT LANDS.                      *)
(*                                                                         *)
(*    Passing the price check is only evidence about the day it was run    *)
(*    for. Violated, a holding with no price on the struck day is in a NAV *)
(*    that went through the check.                                         *)
(***************************************************************************)
ACheckedStrikeIsPricedOnItsOwnDay ==
    \A s \in struck : s.asked # NoDay => s.priced

(***************************************************************************)
(* 3. EVERY STRIKE IS PRICED ON THE DAY IT LANDS.                          *)
(*                                                                         *)
(*    What `strike_refuses_exactly_when_something_is_unpriced` promises:   *)
(*    no path to a struck NAV with an unvalued holding.                    *)
(*                                                                         *)
(*    ⭐ THE STRIKE CHOOSES THE DAY, SO IT CAN CHECK IT. The bare strike  *)
(*    used to skip the check because "a book does not know what day it  *)
(*    is" — but the strike stamps a day either way. It now checks the    *)
(*    day it lands on; BareStrikeIsUnchecked.cfg is the old behavior.    *)
(***************************************************************************)
EveryStrikeIsPricedOnItsOwnDay ==
    \A s \in struck : s.priced

=============================================================================
