------------------------------ MODULE CloseGate ------------------------------
(***************************************************************************)
(* A NAV struck over a difference nobody has explained.                     *)
(*                                                                          *)
(* `Ratio.Tolerance` proves which differences block: the bands partition    *)
(* every figure, the boundary is inclusive, and an inverted pair leaves a   *)
(* grade nothing can be. Every one of those theorems is about ONE           *)
(* difference, considered on its own.                                       *)
(*                                                                          *)
(* ⛔ AND THE GATE IS NOT ABOUT ONE DIFFERENCE. It is about the order of    *)
(* four things that happen on a NAV morning: a reconciliation reports a     *)
(* figure, a person explains it, the journal grows, and somebody strikes.   *)
(* The console reported `STATE_BLOCKED` for months and nothing consulted it *)
(* before striking, which is not a defect any theorem here could see —      *)
(* a screen and a command disagreeing is a sequence problem.                *)
(*                                                                          *)
(* ⭐ THE HARD QUESTION IS WHAT MAKES AN EXPLANATION STALE, and both        *)
(* obvious answers are wrong in opposite directions.                       *)
(*                                                                          *)
(* Pin it to the journal prefix the accepter read — the obvious reading of  *)
(* "a proposal is as-of" — and every explanation is retired by the next     *)
(* posting. A NAV morning posts constantly, so the gate becomes one nobody  *)
(* can ever clear, while looking exactly like the software being careful.   *)
(* `ExplanationPinnedToThePrefix.cfg` is that design, and it is why         *)
(* `PostingDoesNotUnexplainABreak` is stated rather than assumed.           *)
(*                                                                          *)
(* Ignore the figure instead and explanations are eternal: a fund is struck *)
(* on a difference somebody explained in February, which has since moved,   *)
(* and nobody has looked at what it is now. `StaleExplanationUnblocks.cfg`  *)
(* is that one.                                                            *)
(*                                                                          *)
(* ⚠ SO THE RULE IS NEITHER: an explanation is current when the break it    *)
(* names still shows the same figure under the same terms. The journal is   *)
(* free to grow; a new figure retires the note. Nothing about staleness is  *)
(* stored — it is two recorded values compared, the same arrangement        *)
(* `Plane::Actions` uses to identify a strike taken before an action.       *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Breaks,      \* the breaks a reconciliation can report on this fund
    Figures,     \* the values a difference can take. 0 means "not a break"
    Facts,       \* facts that can pend
    MaxPos,      \* bound, not a property
    Threshold,   \* `Ratio.Tolerance.blocksNav`, in the same minor units
    RefuseOnUnexplainedHigh,   \* the fix. FALSE strikes anyway.
    RetireOnANewFigure,        \* the fix. FALSE lets a stale note clear the gate.
    PinExplanationToThePrefix  \* the design NOT shipped. TRUE deadlocks.

VARIABLES
    pos,           \* how long the journal is
    figure,        \* [Breaks -> Figures] what the newest report shows
    explainedFor,  \* [Breaks -> Figures] the figure a note was written about, 0 = none
    explainedAt,   \* [Breaks -> Nat] the prefix the accepter had read
    pending,       \* SUBSET Facts
    struck,        \* SUBSET Nat — the positions a NAV was taken at
    lastOp

vars == <<pos, figure, explainedFor, explainedAt, pending, struck, lastOp>>

NoNote == 0

Init ==
    /\ pos = 0
    /\ figure = [b \in Breaks |-> 0]
    /\ explainedFor = [b \in Breaks |-> NoNote]
    /\ explainedAt = [b \in Breaks |-> 0]
    /\ pending = {}
    /\ struck = {}
    /\ lastOp = "init"

TypeOK ==
    /\ pos \in 0..MaxPos
    /\ figure \in [Breaks -> Figures]
    /\ explainedFor \in [Breaks -> Figures]
    /\ explainedAt \in [Breaks -> 0..MaxPos]
    /\ pending \subseteq Facts
    /\ struck \subseteq 0..MaxPos

\* A difference at or above the threshold stops the NAV.
\* ⚠ INCLUSIVE, matching `Ratio.Tolerance.a_difference_at_the_threshold_blocks_
\* the_nav`. One minor unit either side of this line is the difference between
\* a close that stops and one that does not.
IsHigh(b) == figure[b] >= Threshold

\* ⭐ THE WHOLE STALENESS DESIGN, IN ONE OPERATOR.
Explained(b) ==
    /\ explainedFor[b] # NoNote
    /\ IF RetireOnANewFigure THEN explainedFor[b] = figure[b] ELSE TRUE
    \* ⛔ The design that was NOT shipped. Under it a note holds only while the
    \* journal is exactly where it was when somebody wrote it.
    /\ IF PinExplanationToThePrefix THEN explainedAt[b] = pos ELSE TRUE

Blocking == {b \in Breaks : IsHigh(b) /\ ~Explained(b)}
Blocked == Blocking # {} \/ pending # {}

\* An entry lands. Nothing about any figure changes.
Post ==
    /\ pos < MaxPos
    /\ pos' = pos + 1
    /\ UNCHANGED <<figure, explainedFor, explainedAt, pending, struck>>
    /\ lastOp' = "post"

\* A reconciliation runs and reports a figure for one break.
Reconcile(b, v) ==
    /\ figure[b] # v
    /\ figure' = [figure EXCEPT ![b] = v]
    /\ UNCHANGED <<pos, explainedFor, explainedAt, pending, struck>>
    /\ lastOp' = "reconcile"

\* A person records why the difference is acceptable.
\* ⭐ THE PIN. The note is written against the figure in front of them, and
\* against the prefix they had read — the second recorded for the audit trail
\* and, in the shipped design, read by nothing.
Accept(b) ==
    /\ figure[b] # 0
    /\ explainedFor' = [explainedFor EXCEPT ![b] = figure[b]]
    /\ explainedAt' = [explainedAt EXCEPT ![b] = pos]
    /\ UNCHANGED <<pos, figure, pending, struck>>
    /\ lastOp' = "accept"

Arrive(f) ==
    /\ f \notin pending
    /\ pending' = pending \cup {f}
    /\ UNCHANGED <<pos, figure, explainedFor, explainedAt, struck>>
    /\ lastOp' = "arrive"

Admit(f) ==
    /\ f \in pending
    /\ pending' = pending \ {f}
    /\ UNCHANGED <<pos, figure, explainedFor, explainedAt, struck>>
    /\ lastOp' = "admit"

\* ⛔ THE GATE. Below the fence: a refusal in the engine, not a judgment
\* somebody exercises on top of a state nothing enforces.
Strike ==
    /\ IF RefuseOnUnexplainedHigh THEN ~Blocked ELSE TRUE
    /\ struck' = struck \cup {pos}
    /\ UNCHANGED <<pos, figure, explainedFor, explainedAt, pending>>
    /\ lastOp' = "strike"

\* Everything that can happen, has. The explicit self-loop keeps a finished
\* behavior from reading as a deadlock.
Settled ==
    /\ pos = MaxPos
    /\ pending = {}
    /\ \A b \in Breaks : figure[b] # 0 => Explained(b)

Next ==
    \/ Post
    \/ Strike
    \/ \E b \in Breaks, v \in Figures : Reconcile(b, v)
    \/ \E b \in Breaks : Accept(b)
    \/ \E f \in Facts : Arrive(f) \/ Admit(f)
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* ⛔ THE OBLIGATION. A NAV is not struck while a difference the fund's own  *)
(* tolerance grades as blocking has nobody's name against it.               *)
(*                                                                          *)
(* Stated on `lastOp` because `Strike` leaves everything else unchanged, so  *)
(* the state after one still describes the moment it was taken — the same    *)
(* idiom `Actions.tla` uses.                                                *)
(***************************************************************************)
AStrikeIsNotTakenOverABlockingBreak ==
    lastOp = "strike" => Blocking = {}

(***************************************************************************)
(* ⚠ TWO INVARIANTS, NOT ONE, and deliberately. A break wants somebody's     *)
(* judgment; a pending fact wants a record in the master. Collapsing them    *)
(* into "not blocked" would still be true and would stop saying WHICH — the  *)
(* same reason `replay` reports history-intact and reproduced separately.    *)
(***************************************************************************)
AStrikeIsNotTakenWithAFactPending ==
    lastOp = "strike" => pending = {}

(***************************************************************************)
(* ⭐ THE ANTI-DEADLOCK PROPERTY, AND THE REASON THE SHIPPED RULE READS THE  *)
(* FIGURE RATHER THAN THE PREFIX. Posting an entry changes no figure, so it  *)
(* cannot retire anybody's note. Under                                      *)
(* `PinExplanationToThePrefix` it retires every one of them, and a fund      *)
(* that posts on the morning of a close can never be struck at all.          *)
(***************************************************************************)
PostingDoesNotUnexplainABreak ==
    [][ (lastOp' = "post") => \A b \in Breaks : Explained(b) => Explained(b)' ]_vars

(***************************************************************************)
(* ⭐ THE COMPLEMENT. A note clears a break only for the figure it was       *)
(* written about. Without it an explanation outlives what it described and a *)
(* fund is struck over a difference that has moved since anybody looked.     *)
(*                                                                          *)
(* ⚠ STATED AS "THE NOTE NAMES THE CURRENT FIGURE" RATHER THAN "A CHANGE     *)
(* RETIRES IT", and the model checker is why. The first version said a       *)
(* reconciliation that moved a figure must leave the break unexplained,      *)
(* which TLC refuted in five states: move a figure away from the explained   *)
(* value and back, and the note is about the figure again — correctly, since *)
(* the claim was about a difference of that size and the difference is that  *)
(* size. The obligation was never "changes retire notes"; it is that a note  *)
(* and the figure it clears are the same figure.                            *)
(***************************************************************************)
ANewFigureRetiresItsExplanation ==
    \A b \in Breaks : Explained(b) => explainedFor[b] = figure[b]

(***************************************************************************)
(* ⚠ THE ANTI-VACUITY GUARD, and it is not decoration: without it a gate     *)
(* that refuses EVERYTHING satisfies every invariant above. A fund whose     *)
(* blocking breaks are all explained and whose facts are all admitted is not *)
(* blocked — the refusal has to let a worked fund through.                   *)
(***************************************************************************)
WorkedMeansNothingBlocks ==
    ((\A b \in Breaks : IsHigh(b) => Explained(b)) /\ pending = {}) => ~Blocked

=============================================================================
