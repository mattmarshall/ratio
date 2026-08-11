-------------------------- MODULE WashRestatement --------------------------
(***************************************************************************)
(* A loss disallowed by a repurchase that had not happened yet.             *)
(*                                                                          *)
(* `Ratio.Lots.Wash` proves the arithmetic: how much of a loss a repurchase  *)
(* defers, that a gain is never washed, and that the deferral nets out over  *)
(* the life of the position. Every one of those theorems is about a sale and *)
(* a repurchase considered TOGETHER.                                        *)
(*                                                                          *)
(* ⛔ AND THEY ARE NOT AVAILABLE TOGETHER. The repurchase can arrive up to a  *)
(* window AFTER the sale, and a NAV is struck in between. At the moment of   *)
(* the strike the loss is real, allowable, and reported. Days later a        *)
(* repurchase lands inside the window and the same sale's realized gain is a *)
(* different number — retroactively, for a period that is closed.            *)
(*                                                                          *)
(* This is not the staleness `//tla:relief_engine_check` models. There, a    *)
(* relief read one state and wrote against another, and the fix was to pin   *)
(* what it read. Here NOTHING WAS STALE. The strike read the whole journal,  *)
(* pinned its prefix, and computed the only correct answer available at the  *)
(* time. The figure changed because a FUTURE event reached backwards, which  *)
(* is a property of the tax rule and not a defect in the engine.             *)
(*                                                                          *)
(* ⚠ SO THE OBLIGATION IS NOT "DO NOT LET IT CHANGE". It is: a figure        *)
(* somebody was paid on must not change SILENTLY. Either the strike carries  *)
(* a qualification saying its wash window is still open, or a later          *)
(* repurchase that moves it produces a restatement. An engine that simply    *)
(* recomputes leaves two different answers to "what was the realized gain in *)
(* March", both reproducible, both digested, and no record that they differ. *)
(*                                                                          *)
(* ⛔ AND THE TEMPTING FIX IS WRONG. Applying the adjustment only to periods  *)
(* still open — prospectively — keeps every struck figure stable and reports *)
(* a realized gain the tax rule does not agree with. It trades a visible     *)
(* restatement for an invisible error, which is the trade this whole system  *)
(* exists to refuse.                                                        *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Sales,           \* sales that realized a loss
    Window,          \* the disallowance window, in days. A jurisdiction's number.
    MaxDay,          \* bound, not a property
    QualifyOpenWindow, \* the fix. FALSE strikes a figure that can still move,
                       \* with nothing on it saying so.
    RestateOnLateWash  \* the fix. FALSE recomputes silently.

VARIABLES
    day,          \* the clock
    soldOn,       \* [Sales -> Nat] the day the loss was realized, 0 = not yet
    boughtOn,     \* [Sales -> Nat] the day the replacement landed, 0 = none
    struck,       \* [Sales -> Nat] the day a NAV reported this sale's gain, 0 = never
    qualified,    \* SUBSET Sales — struck while the window was still open, and said so
    restated,     \* SUBSET Sales — a later repurchase moved a struck figure, and said so
    lastOp

vars == <<day, soldOn, boughtOn, struck, qualified, restated, lastOp>>

Unsold == 0

\* A repurchase washes a sale when it lands within the window either side of it.
\* ⛔ BOTH SIDES. `Ratio.Lots.Wash.the_window_reaches_backwards_too` is the
\* arithmetic; here it decides which sales a repurchase can still reach.
Washes(s) ==
    /\ soldOn[s] # Unsold
    /\ boughtOn[s] # Unsold
    /\ boughtOn[s] >= soldOn[s] - Window
    /\ boughtOn[s] <= soldOn[s] + Window

\* The window is still open on a sale: a repurchase could yet arrive and move
\* the figure. This is what a strike cannot see and must say.
WindowOpen(s) == soldOn[s] # Unsold /\ day <= soldOn[s] + Window

TypeOK ==
    /\ day \in 0..MaxDay
    /\ soldOn \in [Sales -> 0..MaxDay]
    /\ boughtOn \in [Sales -> 0..MaxDay]
    /\ struck \in [Sales -> 0..MaxDay]
    /\ qualified \subseteq Sales
    /\ restated \subseteq Sales

Init ==
    /\ day = 1
    /\ soldOn = [s \in Sales |-> Unsold]
    /\ boughtOn = [s \in Sales |-> Unsold]
    /\ struck = [s \in Sales |-> Unsold]
    /\ qualified = {}
    /\ restated = {}
    /\ lastOp = "init"

Tick ==
    /\ day < MaxDay
    /\ day' = day + 1
    /\ UNCHANGED <<soldOn, boughtOn, struck, qualified, restated>>
    /\ lastOp' = "tick"

Sell(s) ==
    /\ soldOn[s] = Unsold
    /\ soldOn' = [soldOn EXCEPT ![s] = day]
    /\ UNCHANGED <<day, boughtOn, struck, qualified, restated>>
    /\ lastOp' = "sell"

\* The replacement purchase. It can land before the sale as easily as after,
\* which is why `soldOn[s] = Unsold` is not a precondition.
Repurchase(s) ==
    /\ boughtOn[s] = Unsold
    /\ boughtOn' = [boughtOn EXCEPT ![s] = day]
    \* ⛔ THE RETROACTIVE MOVE. If this repurchase washes a sale whose gain has
    \* already been reported, the reported figure is now wrong. Saying so is a
    \* restatement; not saying so is two answers to one question.
    /\ IF RestateOnLateWash
         /\ struck[s] # Unsold
         /\ soldOn[s] # Unsold
         /\ boughtOn' [s] >= soldOn[s] - Window
         /\ boughtOn' [s] <= soldOn[s] + Window
       THEN restated' = restated \cup {s}
       ELSE UNCHANGED restated
    /\ UNCHANGED <<day, soldOn, struck, qualified>>
    /\ lastOp' = "repurchase"

\* Strike a NAV that reports this sale's realized gain.
Strike(s) ==
    /\ soldOn[s] # Unsold
    /\ struck[s] = Unsold
    /\ struck' = [struck EXCEPT ![s] = day]
    \* ⚠ THE QUALIFICATION IS WRITTEN AT STRIKE TIME, because that is the only
    \* moment the engine knows the window is open. Adding it later, once a
    \* repurchase has arrived, is a restatement — a different thing, and one
    \* that has already been read.
    /\ IF QualifyOpenWindow /\ WindowOpen(s)
       THEN qualified' = qualified \cup {s}
       ELSE UNCHANGED qualified
    /\ UNCHANGED <<day, soldOn, boughtOn, restated>>
    /\ lastOp' = "strike"

\* Everything that can happen, has. The explicit self-loop keeps a finished
\* behavior from reading as a deadlock — the model has run out of things to
\* do, which is not the same as being stuck.
Settled ==
    /\ day = MaxDay
    /\ \A s \in Sales : soldOn[s] # Unsold /\ boughtOn[s] # Unsold /\ struck[s] # Unsold

Next ==
    \/ Tick
    \/ \E s \in Sales : Sell(s) \/ Repurchase(s) \/ Strike(s)
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(Tick)

(***************************************************************************)
(* ⭐ THE OBLIGATION. A reported realized gain that a later repurchase moved  *)
(* was either flagged as provisional when it was struck, or restated when it *)
(* moved. What is forbidden is the third case: struck clean, changed         *)
(* quietly.                                                                 *)
(*                                                                          *)
(* ⚠ NOTE WHAT THIS DOES *NOT* SAY. It does not require the figure to be     *)
(* stable, and it must not: the tax rule genuinely reaches backwards, so an  *)
(* engine that held the March figure fixed would be reporting a number the   *)
(* rule disagrees with. The property is about the RECORD, not the number.    *)
(***************************************************************************)
AStruckGainThatMovedSaysSo ==
    \A s \in Sales :
        (struck[s] # Unsold /\ Washes(s) /\ boughtOn[s] > struck[s])
            => (s \in qualified \/ s \in restated)

\* A strike taken after the window has closed needs no qualification: nothing
\* can move it. Flagging those too would train a reader to ignore the flag.
AClosedWindowIsNotQualified ==
    \A s \in Sales :
        (s \in qualified) => (struck[s] # Unsold /\ struck[s] <= soldOn[s] + Window)

=============================================================================
