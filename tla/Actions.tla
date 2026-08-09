--------------------------- MODULE Actions ---------------------------
(***************************************************************************)
(* Corporate actions arriving, being applied, and NAVs struck around them.  *)
(*                                                                         *)
(* `Ratio.Actions` proves what one action DOES: a split conserves cost,     *)
(* changes units by the declared ratio, keeps the acquisition ordinal, and  *)
(* — the one that matters here — IS NOT IDEMPOTENT. Applying a two-for-one  *)
(* twice quadruples the position and the trial balance still ties.          *)
(*                                                                         *)
(* Non-idempotence is a sequence problem, not an arithmetic one. Every      *)
(* other write in this system is safe to retry: marking twice at one price  *)
(* posts nothing, admitting a fact twice writes nothing, ingesting a file   *)
(* twice reads nothing. The console retries all of them freely. An action   *)
(* breaks that habit, so whatever applies one has to carry the "did I       *)
(* already?" itself.                                                        *)
(*                                                                         *)
(* And they are RETROACTIVE, which the append-only journal does not let us  *)
(* undo. `Ratio.Period.one_answer_per_day` says a valuation point is struck *)
(* once and never restated. So an action arriving after a NAV was struck    *)
(* cannot change that NAV — and the only honest thing left is to KNOW which *)
(* figures were taken without it.                                           *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    ActionIds,          \* corporate actions that will arrive
    Days,               \* valuation days
    RecordApplied,      \* the fix. FALSE forgets, and re-applies.
    ApplyInExDateOrder  \* the fix. FALSE applies in arrival order.

VARIABLES
    exDate,     \* [ActionIds -> Days]  when each action takes effect
    arrived,    \* SUBSET ActionIds — we have been told about it
    applied,    \* SUBSET ActionIds — it has been put through the book
    applyCount, \* [ActionIds -> Nat]  how many times, which is the bug
    struck,     \* SUBSET Days — NAVs taken
    valuing,    \* the day being valued
    lastOp

vars == <<exDate, arrived, applied, applyCount, struck, valuing, lastOp>>

----------------------------------------------------------------------------

Init ==
    /\ exDate \in [ActionIds -> Days]
    /\ arrived = {}
    /\ applied = {}
    /\ applyCount = [a \in ActionIds |-> 0]
    /\ struck = {}
    /\ valuing = CHOOSE d \in Days : \A e \in Days : d =< e
    /\ lastOp = "init"

(* An action is announced. Announcement is not application: the book does not
   move until somebody applies it, which is why OPEN actions are the dial in
   `Ratio.Closure` and not actions ever. *)
Announce(a) ==
    /\ a \notin arrived
    /\ arrived' = arrived \cup {a}
    /\ lastOp' = "announce"
    /\ UNCHANGED <<exDate, applied, applyCount, struck, valuing>>

(* Put it through the book.
   ⛔ THE GUARD IS THE WHOLE THING. `Ratio.Actions.applying_twice_is_not_
   applying_once` — there is no arithmetic here that notices a second
   application, so the record of having applied it IS the idempotence. *)
Apply(a) ==
    /\ a \in arrived
    /\ IF RecordApplied THEN a \notin applied ELSE TRUE
    \* Actions do not commute, so an earlier ex-date must go first.
    /\ IF ApplyInExDateOrder
         THEN \A b \in arrived \ applied : exDate[a] =< exDate[b]
         ELSE TRUE
    /\ applied' = applied \cup {a}
    /\ applyCount' = [applyCount EXCEPT ![a] = @ + 1]
    /\ lastOp' = "apply"
    /\ UNCHANGED <<exDate, arrived, struck, valuing>>

(* Strike the NAV for the day being valued. *)
Strike ==
    /\ valuing \notin struck
    /\ struck' = struck \cup {valuing}
    /\ lastOp' = "strike"
    /\ UNCHANGED <<exDate, arrived, applied, applyCount, valuing>>

Advance(d) ==
    /\ valuing < d
    /\ valuing' = d
    /\ lastOp' = "advance"
    /\ UNCHANGED <<exDate, arrived, applied, applyCount, struck>>

Settled == \A a \in ActionIds : a \in applied

Next ==
    \/ \E a \in ActionIds : Announce(a)
    \/ \E a \in ActionIds : Apply(a)
    \/ Strike
    \/ \E d \in Days : Advance(d)
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. AN ACTION IS APPLIED AT MOST ONCE.                                   *)
(*                                                                         *)
(*    There is no arithmetic that catches a second application — the cost   *)
(*    is unchanged and the books tie — so this is the only thing standing   *)
(*    between a retry and a doubled position.                               *)
(***************************************************************************)
AppliedAtMostOnce == \A a \in ActionIds : applyCount[a] =< 1

(***************************************************************************)
(* 2. THE EARLIEST ACTION WE KNOW OF GOES FIRST.                           *)
(*                                                                         *)
(*    `Ratio.Actions.actions_do_not_commute`: a 3-for-2 then a 2-for-1 is   *)
(*    not a 2-for-1 then a 3-for-2 — one order divides and the other        *)
(*    refuses. So order is not an implementation detail; it decides whether *)
(*    the book can be brought up to date at all.                            *)
(*                                                                         *)
(*    ⚠ STATED OVER WHAT IS KNOWN, and it took a failing check to learn     *)
(*    why. I first wrote this as a state invariant — every applied action    *)
(*    older than every waiting one — and TLC broke it against the FIXED      *)
(*    model in five states. An action can be ANNOUNCED LATE with an ex-date  *)
(*    earlier than one already applied, and no guard can prevent that: the   *)
(*    announcement is the outside world's, not ours.                         *)
(*                                                                          *)
(*    So the enforceable property is about the decision, not the history —   *)
(*    when we apply, we apply the earliest of what we have been told. What   *)
(*    happens to the one that arrives afterwards is property 3.              *)
(***************************************************************************)
EarliestKnownFirst ==
    [][ (lastOp' = "apply") =>
          (\A a \in applied' \ applied :
             \A b \in arrived \ applied : exDate[a] =< exDate[b]) ]_vars

(*  An action announced after something later was already applied. It cannot
    be woven into a book that has moved past it, and `Ratio.Period` refuses to
    restate — so this set is the one an operator has to be shown.  *)
ArrivedTooLate ==
    {a \in arrived \ applied : \E b \in applied : exDate[a] < exDate[b]}

(***************************************************************************)
(* 3. ⭐ A NAV STRUCK WITHOUT AN ACTION IS KNOWN.                           *)
(*                                                                         *)
(*    `Ratio.Period.one_answer_per_day` refuses restatement: a valuation    *)
(*    point has one answer and replacing it would remove the first, which   *)
(*    is what somebody was paid on. An action that arrives late therefore   *)
(*    CANNOT correct the NAVs it should have been in.                       *)
(*                                                                         *)
(*    So the obligation is not to fix them — it is to be able to NAME them. *)
(*    This is the set, derived rather than remembered, and a fund that      *)
(*    cannot compute it is publishing figures it cannot qualify.            *)
(***************************************************************************)
StruckWithoutAction(a) == {d \in struck : exDate[a] =< d /\ a \notin applied}

Stale == UNION {StruckWithoutAction(a) : a \in ActionIds}

(*  The property is that this is ANSWERABLE — every stale day is a day that
    was struck, so the qualification attaches to a figure that exists.  *)
StalenessIsAttributable == Stale \subseteq struck

(***************************************************************************)
(* 4. And once everything has been applied, nothing is stale.              *)
(***************************************************************************)
SettledMeansNothingStale == Settled => (Stale = {})

TypeOK ==
    /\ arrived \subseteq ActionIds
    /\ applied \subseteq arrived
    /\ struck \subseteq Days

=============================================================================
