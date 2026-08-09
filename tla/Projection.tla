--------------------------- MODULE Projection ---------------------------
(***************************************************************************)
(* A derived read model, and the one way it can silently produce a wrong    *)
(* figure.                                                                  *)
(*                                                                          *)
(* Twenty million lots do not scan through a JSONL journal quickly — 89     *)
(* seconds at the rate `ratio_nav::closure::measure` reports. So reads move *)
(* to a PROJECTION built by folding a journal prefix, and the journal stays *)
(* the system of record because replay and content-addressed digests are    *)
(* the product.                                                             *)
(*                                                                          *)
(* ⛔ THAT INTRODUCES EXACTLY ONE CATASTROPHIC FAILURE, and it is silent: a *)
(* strike PINS a journal position and READS a projection built from a       *)
(* different one. Every downstream check still passes. The trial balance    *)
(* ties, the digest is well-formed, `ratio replay` recomputes from the      *)
(* pinned prefix and gets a different number — and by then the first number *)
(* is what somebody was paid on, and `Ratio.Period.one_answer_per_day`      *)
(* refuses to restate it.                                                   *)
(*                                                                          *)
(* Three separate mistakes produce it, so all three are dials here:         *)
(*                                                                          *)
(*   PinWhatWasRead         pin the projection's position, not the journal  *)
(*                          head. Pinning HEAD while reading a lagging      *)
(*                          projection is the obvious, tempting bug.        *)
(*   IncrementalProject     fold forward from where the projection is, not  *)
(*                          from zero onto existing state.                  *)
(*   AnnouncementsInJournal `Ratio.Actions.Factor.replay_is_determined_by_  *)
(*                          the_prefix` — under the factor representation   *)
(*                          nothing is applied, so a corporate action in a  *)
(*                          SIDE LOG is not pinned by any strike, and a     *)
(*                          replay reads whatever arrived since.            *)
(*                                                                          *)
(* The Lean counterexample `an_unpinned_announcement_changes_the_answer`    *)
(* is one step of the third dial. This is what a run of them does.          *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    MaxJournal,             \* bound, not a property
    MaxStrikes,             \* bound, not a property
    PinWhatWasRead,         \* the fix. FALSE pins the head and reads stale.
    IncrementalProject,     \* the fix. FALSE re-folds from zero onto state.
    AnnouncementsInJournal  \* the fix. FALSE puts them in a side plane.

VARIABLES
    journal,    \* entries appended. The system of record.
    sideLog,    \* announcements NOT in the journal, when the dial says so.
    projAt,     \* the journal prefix the projection claims to reflect
    projValue,  \* what the projection actually accumulated
    strikes,    \* figures taken: what each pinned, read, and could see
    lastOp

vars == <<journal, sideLog, projAt, projValue, strikes, lastOp>>

----------------------------------------------------------------------------

Init ==
    /\ journal = 0
    /\ sideLog = 0
    /\ projAt = 0
    /\ projValue = 0
    /\ strikes = {}
    /\ lastOp = "init"

(* An ordinary entry: a trade, a mark, a capital movement. *)
Append ==
    /\ journal < MaxJournal
    /\ journal' = journal + 1
    /\ lastOp' = "append"
    /\ UNCHANGED <<sideLog, projAt, projValue, strikes>>

(* A corporate action is announced.
   ⛔ WHERE IT LANDS IS THE WHOLE QUESTION. In the journal it is inside every
   later strike's pinned prefix and replay is determined. In a side plane it
   is inside nobody's prefix. *)
Announce ==
    /\ IF AnnouncementsInJournal
         THEN /\ journal < MaxJournal
              /\ journal' = journal + 1
              /\ UNCHANGED sideLog
         ELSE /\ sideLog < MaxJournal
              /\ sideLog' = sideLog + 1
              /\ UNCHANGED journal
    /\ lastOp' = "announce"
    /\ UNCHANGED <<projAt, projValue, strikes>>

(* Catch the projection up.
   ⚠ `projValue` is the fold the projection ACTUALLY holds; `projAt` is the
   prefix it CLAIMS. A rebuild that re-reads from zero onto existing state
   leaves the two disagreeing, and nothing about the resulting number looks
   wrong — it is simply too big. *)
Project ==
    /\ projAt < journal
    /\ projAt' = journal
    /\ projValue' = IF IncrementalProject
                      THEN projValue + (journal - projAt)
                      ELSE projValue + journal
    /\ lastOp' = "project"
    /\ UNCHANGED <<journal, sideLog, strikes>>

(* Take a figure off the projection. *)
Strike ==
    /\ Cardinality(strikes) < MaxStrikes
    /\ strikes' = strikes \cup
         {[ pin  |-> IF PinWhatWasRead THEN projAt ELSE journal,
            read |-> projAt,
            seen |-> sideLog ]}
    /\ lastOp' = "strike"
    /\ UNCHANGED <<journal, sideLog, projAt, projValue>>

Quiet ==
    /\ journal = MaxJournal
    /\ sideLog = MaxJournal \/ AnnouncementsInJournal
    /\ projAt = journal
    /\ Cardinality(strikes) >= MaxStrikes

Next ==
    \/ Append
    \/ Announce
    \/ Project
    \/ Strike
    \/ (Quiet /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(Project)

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. ⭐ A STRIKE FOLDS THE PREFIX IT PINS.                                *)
(*                                                                         *)
(*    The whole safety argument for a derived read model in one line. A    *)
(*    figure's citation must name the data it was computed from, or the    *)
(*    citation is decoration — and `ratio replay` would disagree with the  *)
(*    figure it was asked to confirm, which is the accusation this system  *)
(*    exists to be able to make about ITSELF and nothing else.             *)
(***************************************************************************)
StrikeFoldsItsOwnPrefix == \A s \in strikes : s.pin = s.read

(***************************************************************************)
(* 2. THE PROJECTION HOLDS THE FOLD OF THE PREFIX IT CLAIMS.               *)
(*                                                                         *)
(*    A rebuild is supposed to be idempotent. One that re-reads from zero  *)
(*    onto existing state double-counts, and the result is a projection    *)
(*    whose claimed position is honest and whose contents are not.         *)
(***************************************************************************)
ProjectionIsTheFold == projValue = projAt

(***************************************************************************)
(* 3. THE PROJECTION NEVER LEADS THE JOURNAL.                              *)
(*                                                                         *)
(*    Lagging is fine and expected — it is what `StrikeFoldsItsOwnPrefix`  *)
(*    makes safe. Leading would mean the read model holds something the    *)
(*    system of record does not, which no replay could ever justify.       *)
(***************************************************************************)
ProjectionNeverLeadsTheJournal == projAt =< journal

(***************************************************************************)
(* 4. ⛔ REPLAY REPRODUCES.                                                *)
(*                                                                         *)
(*    A strike's figure depends on its pinned prefix AND on the corporate  *)
(*    actions in force. Under `Ratio.Actions.Factor` nothing is applied,   *)
(*    so the actions in force are read from the announced set — and if     *)
(*    that set lives outside the journal, the prefix does not determine    *)
(*    the figure.                                                          *)
(*                                                                         *)
(*    Re-deriving would then answer differently as the world tells us more,*)
(*    which is worse than a restatement: a restatement announces itself.   *)
(***************************************************************************)
ReplayFigure(s)   == << s.pin, IF AnnouncementsInJournal THEN 0 ELSE sideLog >>
OriginalFigure(s) == << s.pin, IF AnnouncementsInJournal THEN 0 ELSE s.seen >>

ReplayReproduces == \A s \in strikes : ReplayFigure(s) = OriginalFigure(s)

(***************************************************************************)
(* 5. And the projection catches up, so lagging is a delay rather than a   *)
(*    permanent divergence. Liveness, under weak fairness on Project.      *)
(***************************************************************************)
ProjectionConverges == []<>(projAt = journal)

TypeOK ==
    /\ journal \in 0..MaxJournal
    /\ sideLog \in 0..MaxJournal
    /\ projAt \in 0..MaxJournal
    /\ \A s \in strikes : s.pin \in 0..MaxJournal /\ s.read \in 0..MaxJournal

=============================================================================
