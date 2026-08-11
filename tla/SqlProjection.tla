--------------------------- MODULE SqlProjection ---------------------------
(***************************************************************************)
(* The projection as TABLES, and the figure that straddles two of them.     *)
(*                                                                          *)
(* `//tla:projection_check` proves the read model must pin the prefix it    *)
(* READ rather than the journal head, and `AsOf<T>` is how the Rust makes   *)
(* dropping that prefix a type error: the value and the position it was     *)
(* folded to are one object, and there is no way to hold the figure without *)
(* holding the position.                                                    *)
(*                                                                          *)
(* ⛔ A SQL READ PATH HAS NO SUCH TYPE, AND THE PROJECTION IS NOT ONE        *)
(* OBJECT. Lots, positions and aggregates are separate tables maintained by *)
(* a projector that is catching up with the journal, and a figure is a      *)
(* query — or several — across more than one of them. `projAt` was a scalar *)
(* in that spec. Here it is a function, and that is the whole difference.   *)
(*                                                                          *)
(* So a NAV can be folded from positions at prefix 100 and lots at prefix   *)
(* 105. Both tables are internally consistent. Neither is corrupt. Neither  *)
(* query is wrong. The figure is one that never existed at any instant of   *)
(* the journal, and it ties: the trial balance is a sum over rows that all  *)
(* balance, and `ratio replay` re-derives from ONE prefix and gets a        *)
(* different number.                                                        *)
(*                                                                          *)
(* ⚠ AND THERE ARE TWO INDEPENDENT WAYS TO PRODUCE IT, which is the point   *)
(* of modelling this before there is a schema:                              *)
(*                                                                          *)
(*   ApplyAtomically   the projector advances EVERY table for one journal   *)
(*                     entry in ONE transaction. FALSE lets lots reach      *)
(*                     entry 105 while positions is still at 100, so even a *)
(*                     perfect snapshot read sees two prefixes.             *)
(*   SnapshotRead      the reader observes ONE instant across every table.  *)
(*                     FALSE reads each table when it gets to it, so the    *)
(*                     projector can commit between two statements of the   *)
(*                     same figure.                                         *)
(*                                                                          *)
(* ⛔ NEITHER ALONE IS ENOUGH, and that is what a model check says and       *)
(* reasoning does not. Atomic writes with per-statement reads straddle;     *)
(* snapshot reads over independently-advanced tables straddle. Both probes  *)
(* below go red, and they go red for the same invariant by different routes.*)
(*                                                                          *)
(* ⚠ AND THE OBVIOUS REPAIR IS WRONG. "Take the minimum of the prefixes you *)
(* read" cannot work: a table at 105 has ALREADY applied entries 101–105 and *)
(* there is nothing to un-apply them with. The prefix is not a label on the *)
(* answer, it is a property of the state that produced it.                  *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Tables,          \* the projected tables one figure reads
    MaxJournal,      \* bound, not a property
    MaxFigures,      \* bound, not a property
    ApplyAtomically, \* the fix. FALSE advances tables one at a time.
    SnapshotRead     \* the fix. FALSE reads each table when it gets to it.

VARIABLES
    journal,   \* entries appended. The system of record.
    at,        \* [Tables -> Nat] the journal prefix each table reflects
    busy,      \* is a figure being read right now
    pending,   \* SUBSET Tables — tables this figure has not read yet
    snap,      \* [Tables -> Nat] what a snapshot read froze at
    seen,      \* the set of prefixes this figure has actually observed
    figures,   \* the recorded figures, each as the set of prefixes it folded
    lastOp

vars == <<journal, at, busy, pending, snap, seen, figures, lastOp>>

TypeOK ==
    /\ journal \in 0..MaxJournal
    /\ at \in [Tables -> 0..MaxJournal]
    /\ busy \in BOOLEAN
    /\ pending \subseteq Tables
    /\ snap \in [Tables -> 0..MaxJournal]
    /\ seen \subseteq 0..MaxJournal
    /\ figures \subseteq SUBSET (0..MaxJournal)

Init ==
    /\ journal = 0
    /\ at = [t \in Tables |-> 0]
    /\ busy = FALSE
    /\ pending = {}
    /\ snap = [t \in Tables |-> 0]
    /\ seen = {}
    /\ figures = {}
    /\ lastOp = "init"

Append ==
    /\ journal < MaxJournal
    /\ journal' = journal + 1
    /\ UNCHANGED <<at, busy, pending, snap, seen, figures>>
    /\ lastOp' = "append"

\* The projector catches up. ⛔ WHETHER THIS MOVES ONE TABLE OR ALL OF THEM IS
\* THE FIRST DIAL: one transaction per journal entry across every table, or a
\* worker per table making its own progress.
Project ==
    /\ \E t \in Tables : at[t] < journal
    /\ IF ApplyAtomically
       THEN at' = [t \in Tables |-> IF at[t] < journal THEN at[t] + 1 ELSE at[t]]
       ELSE \E t \in Tables :
              /\ at[t] < journal
              /\ at' = [at EXCEPT ![t] = at[t] + 1]
    /\ UNCHANGED <<journal, busy, pending, snap, seen, figures>>
    /\ lastOp' = "project"

StartRead ==
    /\ ~busy
    /\ Cardinality(figures) < MaxFigures
    /\ busy' = TRUE
    /\ pending' = Tables
    /\ snap' = at        \* only consulted when SnapshotRead
    /\ seen' = {}
    /\ UNCHANGED <<journal, at, figures>>
    /\ lastOp' = "start"

\* ⛔ ONE TABLE AT A TIME, WHICH IS WHAT A QUERY PLAN ACTUALLY DOES. The second
\* dial decides whether the row it reads comes from the instant the read began
\* or from whenever the reader got to that table.
ReadTable(t) ==
    /\ busy
    /\ t \in pending
    /\ pending' = pending \ {t}
    /\ seen' = seen \cup {IF SnapshotRead THEN snap[t] ELSE at[t]}
    /\ UNCHANGED <<journal, at, busy, snap, figures>>
    /\ lastOp' = "read"

FinishRead ==
    /\ busy
    /\ pending = {}
    /\ figures' = figures \cup {seen}
    /\ busy' = FALSE
    /\ UNCHANGED <<journal, at, pending, snap, seen>>
    /\ lastOp' = "finish"

\* Everything that can happen, has. An explicit self-loop, so a finished
\* behavior does not read as a deadlock.
Settled ==
    /\ journal = MaxJournal
    /\ \A t \in Tables : at[t] = journal
    /\ ~busy
    /\ Cardinality(figures) = MaxFigures

Next ==
    \/ Append
    \/ Project
    \/ StartRead
    \/ \E t \in Tables : ReadTable(t)
    \/ FinishRead
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(Project) /\ WF_vars(FinishRead)

(***************************************************************************)
(* ⭐ THE OBLIGATION. Every recorded figure was folded from ONE journal      *)
(* prefix. Not a close one, not the minimum of several — one.               *)
(*                                                                          *)
(* ⚠ THIS IS THE PROPERTY `AsOf<T>` GIVES THE RUST FOR FREE, and the reason *)
(* it is free is that the Rust projection is a single object with a single  *)
(* position. Split it into tables and the guarantee is gone, with nothing   *)
(* at the type level to notice.                                             *)
(***************************************************************************)
AFigureIsFoldedFromOnePrefix ==
    \A f \in figures : Cardinality(f) = 1

\* And the projector never runs ahead of the log it is folding.
NoTableOutrunsTheJournal ==
    \A t \in Tables : at[t] <= journal

=============================================================================
