---------------------------- MODULE WashEngine ----------------------------
(***************************************************************************)
(* The write the wash rule is: a deferred loss lands on a lot the sale      *)
(* did not relieve.                                                         *)
(*                                                                          *)
(* `Ratio.Lots.Wash` proves the arithmetic of one sale and one repurchase   *)
(* considered together: how much is disallowed, that a gain is never        *)
(* washed, and that the two halves cancel over the life of the position.    *)
(* `//tla:wash_restatement_check` proves a struck figure that a later       *)
(* repurchase moved must not change silently. Neither says anything about   *)
(* the WRITE — attaching the deferral to a replacement that is still open.  *)
(*                                                                          *)
(* ⛔ AND THE WRITE IS THE WHOLE SECOND HALF OF THE RULE. An engine that     *)
(* disallows the loss and leaves every surviving lot untouched conserves    *)
(* cost, ties its trial balance and reproduces its digest. The investor is  *)
(* permanently overtaxed. `Ratio.Lots.Wash.disallowing_without_attaching_   *)
(* destroys_the_loss` is the arithmetic; this is the obligation that the    *)
(* write actually happens, to a lot the sale did not take.                  *)
(*                                                                          *)
(* ⚠ THE REPLACEMENT IS A NEW LOT. It is opened by the repurchase, not      *)
(* pulled from what the sale consumed. An implementation that "attached"    *)
(* by adjusting a Taken lot would write a husk — or worse, a lot that is    *)
(* already gone — and the next relief would never see it.                   *)
(*                                                                          *)
(* ⚠ THE WINDOW IS A CONSTANT, a jurisdiction's number, the same way        *)
(* `long_term_days` is. Nothing in Next assigns it.                         *)
(*                                                                          *)
(* `Ratio.Lots.Relief` and `//tla:relief_engine_check` are the pattern:     *)
(* Lean proves what one application computes; this proves what a sequence   *)
(* of sell / repurchase / attach / later-relieve is allowed to do to the    *)
(* replacement's basis.                                                     *)
(***************************************************************************)
EXTENDS Naturals

CONSTANTS
    Sales,               \* sales that realized a loss
    Window,              \* the disallowance window, in days. A jurisdiction's number.
    MaxDay,              \* bound, not a property
    AcquisitionCost,     \* the replacement's cost before any wash. Never 0:
                         \* 0 is unset, and a silent zero here would make
                         \* "not yet opened" indistinguishable from a free lot.
    Loss,                \* the sale's realized loss, as a POSITIVE magnitude.
    AttachToReplacement  \* the fix. FALSE does the first half only.

VARIABLES
    day,
    soldOn,              \* [Sales -> Nat] the day the loss was realized, 0 = not yet
    boughtOn,            \* [Sales -> Nat] the day the replacement opened, 0 = none
    replacementBasis,    \* [Sales -> Nat] 0 = unset (not yet opened)
    disallowed,          \* [Sales -> Nat] the loss deferred this period
    attached,            \* [Sales -> Nat] what was written onto the replacement
    laterTook,           \* [Sales -> Nat] cost a later sale of the replacement took, 0 = not yet
    lastOp

vars == <<day, soldOn, boughtOn, replacementBasis, disallowed, attached,
          laterTook, lastOp>>

Unsold == 0
Unset  == 0

\* A repurchase washes a sale when it lands within the window either side.
\* ⛔ BOTH SIDES. `Ratio.Lots.Wash.the_window_reaches_backwards_too`.
\* Written as two non-negative gaps so Nat subtraction never asks for a
\* negative — `soldOn - Window` when the sale is early is how a neighbouring
\* probe died on the wrong reason.
InWindow(s) ==
    \/ (boughtOn[s] >= soldOn[s] /\ boughtOn[s] - soldOn[s] <= Window)
    \/ (soldOn[s] >= boughtOn[s] /\ soldOn[s] - boughtOn[s] <= Window)

TypeOK ==
    /\ day \in 0..MaxDay
    /\ soldOn \in [Sales -> 0..MaxDay]
    /\ boughtOn \in [Sales -> 0..MaxDay]
    /\ replacementBasis \in [Sales -> 0..(AcquisitionCost + Loss)]
    /\ disallowed \in [Sales -> 0..Loss]
    /\ attached \in [Sales -> 0..Loss]
    /\ laterTook \in [Sales -> 0..(AcquisitionCost + Loss)]

Init ==
    /\ day = 1
    /\ soldOn = [s \in Sales |-> Unsold]
    /\ boughtOn = [s \in Sales |-> Unsold]
    /\ replacementBasis = [s \in Sales |-> Unset]
    /\ disallowed = [s \in Sales |-> 0]
    /\ attached = [s \in Sales |-> 0]
    /\ laterTook = [s \in Sales |-> Unsold]
    /\ lastOp = "init"

Tick ==
    /\ day < MaxDay
    /\ day' = day + 1
    /\ UNCHANGED <<soldOn, boughtOn, replacementBasis, disallowed, attached, laterTook>>
    /\ lastOp' = "tick"

Sell(s) ==
    /\ soldOn[s] = Unsold
    /\ soldOn' = [soldOn EXCEPT ![s] = day]
    /\ UNCHANGED <<day, boughtOn, replacementBasis, disallowed, attached, laterTook>>
    /\ lastOp' = "sell"

\* The replacement opens. It can land before the sale as easily as after —
\* which is why `soldOn[s] = Unsold` is not a precondition.
\* ⛔ THIS IS A NEW LOT. Nothing here writes a lot the sale took; the
\* replacement is created, not selected from the remainder.
Repurchase(s) ==
    /\ boughtOn[s] = Unsold
    /\ boughtOn' = [boughtOn EXCEPT ![s] = day]
    /\ replacementBasis' = [replacementBasis EXCEPT ![s] = AcquisitionCost]
    /\ UNCHANGED <<day, soldOn, disallowed, attached, laterTook>>
    /\ lastOp' = "repurchase"

\* Apply the wash: disallow the loss, and (when the dial is on) write it
\* onto the replacement that is still open.
\*
\* ⛔ THE DIAL IS THE WHOLE THING. FALSE is the engine that implements the
\* first half and calls itself done. Cost is conserved either way.
Wash(s) ==
    /\ soldOn[s] # Unsold
    /\ boughtOn[s] # Unsold
    /\ laterTook[s] = Unsold          \* the replacement is still open
    /\ InWindow(s)
    /\ Loss > 0                       \* a gain is never washed
    /\ disallowed[s] = 0              \* not yet applied
    /\ disallowed' = [disallowed EXCEPT ![s] = Loss]
    /\ IF AttachToReplacement
         THEN /\ attached' = [attached EXCEPT ![s] = Loss]
              /\ replacementBasis' = [replacementBasis EXCEPT ![s] = @ + Loss]
         ELSE UNCHANGED <<attached, replacementBasis>>
    /\ UNCHANGED <<day, soldOn, boughtOn, laterTook>>
    /\ lastOp' = "wash"

\* A later sale of the replacement. It takes the basis the lot carries NOW —
\* acquisition, or acquisition plus the deferral, depending on whether the
\* write landed. It does not look the loss up on the side.
LaterRelieve(s) ==
    /\ boughtOn[s] # Unsold
    /\ laterTook[s] = Unsold
    /\ laterTook' = [laterTook EXCEPT ![s] = replacementBasis[s]]
    /\ UNCHANGED <<day, soldOn, boughtOn, replacementBasis, disallowed, attached>>
    /\ lastOp' = "later-relieve"

Settled ==
    /\ day = MaxDay
    /\ \A s \in Sales :
         soldOn[s] # Unsold /\ boughtOn[s] # Unsold /\ laterTook[s] # Unsold

Next ==
    \/ Tick
    \/ \E s \in Sales : Sell(s) \/ Repurchase(s) \/ Wash(s) \/ LaterRelieve(s)
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(Tick)

(***************************************************************************)
(* ⭐ THE OBLIGATION. A loss that was disallowed is sitting on the           *)
(* replacement, as basis. The two numbers are the same number. Drop the     *)
(* write and they come apart: the period recognizes less loss, and the lot  *)
(* book never heard about it.                                               *)
(*                                                                          *)
(* ⚠ THIS IS NOT VACUOUS. `disallowed` and `attached` are written by        *)
(* different arms of Wash. An engine that "just sets both" in one           *)
(* assignment does not have a dial to flip; this one does, and              *)
(* `//tla:unattached_wash_check` flips it.                                  *)
(***************************************************************************)
ADisallowedLossIsOnTheReplacement ==
    \A s \in Sales : attached[s] = disallowed[s]

(***************************************************************************)
(* The write is to a lot the sale did not relieve: the replacement had      *)
(* been opened (a new lot) and was still open when the write landed.        *)
(* After a later relief the lot is gone and `attached` stays put — the      *)
(* write already happened.                                                  *)
(***************************************************************************)
TheWriteIsToAnOpenReplacement ==
    \A s \in Sales :
        attached[s] > 0 => boughtOn[s] # Unsold

(***************************************************************************)
(* A later relief takes whatever basis the replacement carries, which is    *)
(* acquisition plus whatever was attached. The loss comes back here or it   *)
(* does not come back at all.                                               *)
(*                                                                          *)
(* `Ratio.Lots.Wash.a_later_sale_of_the_replacement_takes_the_adjusted_     *)
(* basis` is the arithmetic; this is the same claim over every interleaving.*)
(***************************************************************************)
LaterReliefTakesTheReplacementBasis ==
    \A s \in Sales :
        laterTook[s] # Unsold => laterTook[s] = AcquisitionCost + attached[s]

(***************************************************************************)
(* A wash fires only inside the configured window. The number is a          *)
(* CONSTANT — change the cfg and the same two dates disagree, which is      *)
(* `Ratio.Lots.Wash.the_window_is_a_jurisdiction_number`.                   *)
(***************************************************************************)
AWashFiresOnlyInsideTheWindow ==
    \A s \in Sales : disallowed[s] > 0 => InWindow(s)

=============================================================================
