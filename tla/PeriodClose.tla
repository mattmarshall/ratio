----------------------------- MODULE PeriodClose -----------------------------
(***************************************************************************)
(* A period close: the journal door, not the NAV gate.                      *)
(*                                                                          *)
(* `Ratio.Close` proves the arithmetic of one close: the posting conserves, *)
(* the equity leg is the surplus, a missing destination refuses, and a      *)
(* dated entry on or before the closed-through day is refused. That is one  *)
(* step. This is what a SEQUENCE of posts and closes does to the boundary.  *)
(*                                                                          *)
(* ⛔ AND APPEND-ONLY STORAGE IS NOT THIS. A journal that never rewrites    *)
(* still accepts a back-dated entry unless a door refuses it. PLAN.md       *)
(* Stage 1 named "no effect on a closed period" as a required check; the    *)
(* rule compiler never asked, and `Journal::append` never asked. This spec  *)
(* is that door.                                                            *)
(*                                                                          *)
(* ⭐ THE INVARIANT is `AClosedPeriodRefusesABackdatedPost`: after a view    *)
(* is closed through D at prefix P, every later journal slot that is        *)
(* occupied is undated or dated after D. The closing entry itself is        *)
(* posted BEFORE the close is recorded, so it sits at or before P and is    *)
(* not a violation.                                                         *)
(*                                                                          *)
(* `CloseGate.tla` is whether a NAV may be struck over an unexplained       *)
(* break. `Ratio.Period` is the valuation day. Neither is a fiscal close.   *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Views,                 \* books of record a close is recorded against
    Days,                  \* calendar days an entry may be dated
    MaxPos,                \* bound, not a property
    RefuseOnClosedPeriod   \* the fix. FALSE lets a back-date land.

VARIABLES
    pos,            \* how long the journal is
    journal,        \* [1..MaxPos -> Days ∪ {0}]  0 = undated
    closedThrough,  \* [Views -> Days ∪ {0}]      0 = not closed
    closedAt,       \* [Views -> 0..MaxPos]       prefix the close cited
    lastOp

vars == <<pos, journal, closedThrough, closedAt, lastOp>>

NoDay == 0
NoClose == 0

Init ==
    /\ pos = 0
    /\ journal = [i \in 1..MaxPos |-> NoDay]
    /\ closedThrough = [v \in Views |-> NoClose]
    /\ closedAt = [v \in Views |-> 0]
    /\ lastOp = "init"

TypeOK ==
    /\ pos \in 0..MaxPos
    /\ journal \in [1..MaxPos -> Days \cup {NoDay}]
    /\ closedThrough \in [Views -> Days \cup {NoClose}]
    /\ closedAt \in [Views -> 0..MaxPos]

\* A dated entry is blocked when ANY view is closed through that day or later.
\* ⚠ CONSERVATIVE ON PURPOSE. The journal is shared; a close is per view.
\* Refusing against the most restrictive close is the door that cannot
\* silently put an entry into a view that already signed the period.
Blocked(d) ==
    /\ RefuseOnClosedPeriod
    /\ d # NoDay
    /\ \E v \in Views :
          closedThrough[v] # NoClose /\ d <= closedThrough[v]

Post(d) ==
    /\ pos < MaxPos
    /\ ~Blocked(d)
    /\ pos' = pos + 1
    /\ journal' = [journal EXCEPT ![pos'] = d]
    /\ UNCHANGED <<closedThrough, closedAt>>
    /\ lastOp' = "post"

\* The door held. A blocked post is a stutter on the books.
RefusePost(d) ==
    /\ pos < MaxPos
    /\ Blocked(d)
    /\ UNCHANGED <<pos, journal, closedThrough, closedAt>>
    /\ lastOp' = "refuse"

\* Record a close of view v through day d at the current prefix.
\* ⛔ FORWARD ONLY. A second close of the same through, or an earlier one,
\* is refused — `Ratio.Close.a_close_only_moves_forward`.
Close(v, d) ==
    /\ d \in Days
    /\ IF closedThrough[v] # NoClose THEN d > closedThrough[v] ELSE TRUE
    /\ closedThrough' = [closedThrough EXCEPT ![v] = d]
    /\ closedAt' = [closedAt EXCEPT ![v] = pos]
    /\ UNCHANGED <<pos, journal>>
    /\ lastOp' = "close"

Next ==
    \/ \E d \in Days \cup {NoDay} : Post(d) \/ RefusePost(d)
    \/ \E v \in Views, d \in Days : Close(v, d)

Spec == Init /\ [][Next]_vars

\* After a close of v through D at prefix P, every later occupied slot
\* is undated or dated after D.
AClosedPeriodRefusesABackdatedPost ==
    \A v \in Views :
        closedThrough[v] # NoClose =>
            \A i \in 1..pos :
                i > closedAt[v] =>
                    journal[i] = NoDay \/ journal[i] > closedThrough[v]

\* A close cites a prefix that exists, and never rewinds the journal.
ACloseOnlyMovesForward ==
    \A v \in Views :
        closedThrough[v] # NoClose => closedAt[v] <= pos

=============================================================================
