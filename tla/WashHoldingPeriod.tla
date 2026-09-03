------------------------- MODULE WashHoldingPeriod -------------------------
(***************************************************************************)
(* A jurisdiction that does not transfer the holding period on a wash       *)
(* replacement.                                                             *)
(*                                                                          *)
(* `Ratio.Lots.Wash.replacementAcquired` is the US rule: the replacement    *)
(* takes the ORIGINAL lot's acquisition date.                               *)
(* `Ratio.Lots.WashHolding` is the election that is not that rule: the      *)
(* replacement keeps its own date. Choosing the wrong one flips a later     *)
(* disposal between long-term and short-term. Conservation holds, the       *)
(* trial balance ties, the deferred loss still attaches. The figure that    *)
(* goes wrong is the RATE.                                                  *)
(*                                                                          *)
(* ⛔ THIS SPEC IS THE NON-US ELECTION. The US path is already named.        *)
(* Hardcoding `replacementAcquired` here — AssumeUsTransfer — is the        *)
(* defect of treating the US transfer as universal.                         *)
(* `//tla:universal_us_transfer_check` flips the dial and                   *)
(* TheReplacementKeepsItsOwnDate goes red.                                  *)
(*                                                                          *)
(* ⚠ THE DATES ARE THE LEAN EXAMPLE ON PURPOSE. Day 0, repurchase 300,      *)
(* dispose 400, threshold 365. Transfer is long; keep is short. A model     *)
(* that used dates that do not flip would stay green on an engine that      *)
(* never asked the election.                                                *)
(* `Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate`.         *)
(*                                                                          *)
(* ⚠ NOT AN `Order` / `Method` / `lot_method = "wash"`. The election is     *)
(* which DATE a later classification reads. A sort-and-walk has no place    *)
(* to put it.                                                               *)
(*                                                                          *)
(* The two dates and the threshold are CONSTANTS of the example, not a      *)
(* bound. Nothing in Next assigns any of them.                              *)
(***************************************************************************)
EXTENDS Integers

CONSTANTS
    AssumeUsTransfer     \* the bug. TRUE hardcodes replacementAcquired.

VARIABLES
    classified,          \* the date a later isLongTerm is asked. -1 = unset
    lastOp

vars == <<classified, lastOp>>

Unset == -1

\* The Lean example. ⛔ THESE DATES FLIP. Day 0 / 300 / 400 / 365.
OriginalAcquired == 0
RepurchaseOn     == 300
DisposedOn       == 400
Threshold        == 365

\* This spec IS the non-US election. The replacement keeps the repurchase.
ElectedDate == RepurchaseOn
UsDate      == OriginalAcquired

IsLong(acq) == DisposedOn - acq >= Threshold

TypeOK ==
    /\ classified \in {Unset, OriginalAcquired, RepurchaseOn}
    /\ lastOp \in {"init", "classify"}

Init ==
    /\ classified = Unset
    /\ lastOp = "init"

\* Classify the replacement for a later disposal. The dial is the whole
\* thing: FALSE writes the elected date; TRUE writes the US transfer
\* regardless.
Classify ==
    /\ classified = Unset
    /\ classified' = IF AssumeUsTransfer THEN UsDate ELSE ElectedDate
    /\ lastOp' = "classify"

Next ==
    \/ Classify
    \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* ⭐ THE OBLIGATION. The replacement is classified from its own date.      *)
(* Hardcode the US transfer — AssumeUsTransfer — and it is classified      *)
(* from the original. Same units, same basis, same proceeds. The books     *)
(* still tie.                                                              *)
(*                                                                          *)
(* ⚠ NOT VACUOUS. `classified` is written by Classify from one of two      *)
(* arms. An engine that "just sets the repurchase date" in one assignment  *)
(* has no dial to flip; this one does, and                                  *)
(* `//tla:universal_us_transfer_check` flips it.                           *)
(***************************************************************************)
TheReplacementKeepsItsOwnDate ==
    classified = Unset \/ classified = ElectedDate

(***************************************************************************)
(* The two rules disagree on the rate. Stated so a model that used dates   *)
(* that do not flip — a repurchase already past the threshold — cannot     *)
(* pass by never asking the question.                                      *)
(* `Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate`.        *)
(***************************************************************************)
ChoosingTheWrongRuleFlipsTheRate ==
    IsLong(UsDate) # IsLong(ElectedDate)

=============================================================================
