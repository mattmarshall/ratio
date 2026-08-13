----------------------------- MODULE Views -----------------------------
(***************************************************************************)
(* More than one view over one journal, and the two ways that goes silently *)
(* wrong.                                                                   *)
(*                                                                          *)
(* A fund is asked for two figures about the same day. The accounting book   *)
(* of record recognises a trade when it is STRUCK; a settlement-basis view    *)
(* recognises it when cash and stock actually MOVE, some business days       *)
(* later. `Ratio.Views` proves what one view IS — a recognition predicate,   *)
(* so every view's books tie and two views differ by exactly the entries in  *)
(* flight between them. That is a statement about FUNCTIONS.                 *)
(*                                                                          *)
(* ⛔ WHAT IT CANNOT SAY IS WHERE THE VIEWS ARE WHEN SOMEBODY ASKS. Two      *)
(* folds maintained over one growing journal are two pieces of state, and    *)
(* the failure is not in either fold — it is in comparing them. Both views   *)
(* conserve, both trial balances tie, both digests reproduce, and the        *)
(* difference between them is partly a settlement convention and partly one  *)
(* of them being three entries behind. Nothing downstream can tell those     *)
(* apart, and the whole product argument for having both views in one system *)
(* is that the difference is EXPLAINABLE.                                    *)
(*                                                                          *)
(* Two dials, one per mistake:                                               *)
(*                                                                          *)
(*   OneStreamingPass  the fix. Every view is folded by ONE pass over the    *)
(*                     journal, so they share a prefix by construction.      *)
(*                     FALSE gives each view its own projection, advanced    *)
(*                     when that view is asked — which is the obvious,       *)
(*                     tempting implementation and is                        *)
(*                     `SqlProjection.AFigureIsFoldedFromOnePrefix` with     *)
(*                     views where that spec has tables.                     *)
(*                                                                          *)
(*   CalendarInConfig  the fix. The settlement calendar lives inside the     *)
(*                     content-addressed configuration every journal entry   *)
(*                     already pins, so a strike's prefix determines its     *)
(*                     figure. FALSE puts it in a side file, and a replay    *)
(*                     run later rolls the same trade over a calendar        *)
(*                     amended since — the shape                             *)
(*                     `Projection.ReplayReproduces` already has for         *)
(*                     corporate actions in a side log.                      *)
(*                                                                          *)
(* ⚠ A NEW SPEC RATHER THAN A CONSTANT ADDED TO `Projection` OR              *)
(* `SqlProjection`. Adding a CONSTANT to a spec leaves every neighbouring    *)
(* probe's cfg unassigning it, and TLC dying in configuration exits exactly  *)
(* like a violated invariant — still red, still looking like a probe doing   *)
(* its job, no longer reaching the model. That has happened here once.       *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Views,             \* the views one book declares
    MaxJournal,        \* bound, not a property
    MaxStrikes,        \* bound, not a property
    MaxCalendars,      \* bound, not a property
    OneStreamingPass,  \* the fix. FALSE gives each view its own projection.
    CalendarInConfig   \* the fix. FALSE puts the calendar in a side file.

VARIABLES
    journal,   \* entries appended. ONE journal, and it is the record.
    sideCal,   \* calendar amendments pinned by nothing, when the dial says so
    viewAt,    \* [Views -> Nat] — the prefix each view has folded
    strikes,   \* figures taken: which view, what it pinned, what it read
    recons,    \* comparisons of two views: the prefix each side was read at
    lastOp     \* which verb just ran, so properties can name it

vars == <<journal, sideCal, viewAt, strikes, recons, lastOp>>

----------------------------------------------------------------------------

Init ==
    /\ journal = 0
    /\ sideCal = 0
    /\ viewAt = [v \in Views |-> 0]
    /\ strikes = {}
    /\ recons = {}
    /\ lastOp = "init"

(* An ordinary entry: a trade, a mark, a capital movement. *)
Append ==
    /\ journal < MaxJournal
    /\ journal' = journal + 1
    /\ lastOp' = "append"
    /\ UNCHANGED <<sideCal, viewAt, strikes, recons>>

(* The settlement calendar is amended — a holiday declared, an exchange
   closure added.

   ⛔ WHERE IT LANDS IS THE WHOLE QUESTION, and it is the same question
   `Projection.Announce` asks about a corporate action. Inside the pinned
   configuration, amending it promotes a new digest that later entries name, so
   it sits inside every later strike's prefix and a replay re-derives the same
   day. In a side file it is inside nobody's prefix, and the same trade rolls
   to a different settlement date the next time anybody asks. *)
Amend ==
    /\ IF CalendarInConfig
         THEN /\ journal < MaxJournal
              /\ journal' = journal + 1
              /\ UNCHANGED sideCal
         ELSE /\ sideCal < MaxCalendars
              /\ sideCal' = sideCal + 1
              /\ UNCHANGED journal
    /\ lastOp' = "amend"
    /\ UNCHANGED <<viewAt, strikes, recons>>

(* Fold what has been appended.

   ⛔ ONE PASS, EVERY VIEW. The journal is parsed once and each entry is handed
   to every view's fold, so the views cannot be at different prefixes — there
   is no sequence of operations that leaves them there. The alternative is a
   projection per view advanced when that view is asked, which is cheaper to
   write, indistinguishable in every test that reads one view, and wrong
   exactly when somebody puts two of them side by side. *)
Advance ==
    /\ \E v \in Views : viewAt[v] < journal
    /\ IF OneStreamingPass
         THEN viewAt' = [v \in Views |-> journal]
         ELSE \E v \in Views :
                /\ viewAt[v] < journal
                /\ viewAt' = [viewAt EXCEPT ![v] = journal]
    /\ lastOp' = "advance"
    /\ UNCHANGED <<journal, sideCal, strikes, recons>>

(* Take a figure, in one named view. *)
Strike(v) ==
    /\ Cardinality(strikes) < MaxStrikes
    /\ strikes' = strikes \cup
         {[ view |-> v,
            pin  |-> viewAt[v],
            read |-> viewAt[v],
            cal  |-> sideCal ]}
    /\ lastOp' = "strike"
    /\ UNCHANGED <<journal, sideCal, viewAt, recons>>

(* Compare two views — the reconciliation the console shows.

   ⚠ THIS IS THE ACTION THE WHOLE SPEC IS ABOUT. Reading one view can never
   expose a prefix disagreement; reading two is the only thing that can. *)
Reconcile(a, b) ==
    /\ a # b
    /\ Cardinality(recons) < MaxStrikes
    /\ recons' = recons \cup
         {[ left    |-> a,
            right   |-> b,
            at      |-> viewAt[a],
            against |-> viewAt[b] ]}
    /\ lastOp' = "reconcile"
    /\ UNCHANGED <<journal, sideCal, viewAt, strikes>>

Quiet ==
    /\ journal = MaxJournal
    /\ (sideCal = MaxCalendars \/ CalendarInConfig)
    /\ \A v \in Views : viewAt[v] = journal
    /\ Cardinality(strikes) >= MaxStrikes
    /\ Cardinality(recons) >= MaxStrikes

Next ==
    \/ Append
    \/ Amend
    \/ Advance
    \/ \E v \in Views : Strike(v)
    \/ \E a, b \in Views : Reconcile(a, b)
    \/ (Quiet /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(Advance)

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. ⭐ EVERY VIEW FOLDS THE SAME PREFIX.                                 *)
(*                                                                         *)
(*    One journal, one pass, one position. This is the structural claim     *)
(*    that makes a multi-view book safe, and it is stated over the STATE    *)
(*    rather than over a figure because it has to hold at every instant a   *)
(*    reader could ask — not merely at the instants somebody remembered to  *)
(*    synchronize.                                                         *)
(***************************************************************************)
EveryViewFoldsTheSamePrefix == \A v, w \in Views : viewAt[v] = viewAt[w]

(***************************************************************************)
(* 2. ⛔ A RECONCILIATION COMPARES ONE PREFIX.                             *)
(*                                                                         *)
(*    The consequence, and the one a person sees. A difference between two  *)
(*    views is supposed to be a LIST OF ENTRIES — the trades one has        *)
(*    recognised and the other has not — which is what makes it a           *)
(*    derivation rather than a discrepancy somebody has to hunt.            *)
(*                                                                         *)
(*    Read at two prefixes, that same difference is partly a settlement     *)
(*    convention and partly one view being three entries behind, mixed, in  *)
(*    one number, with nothing saying which part is which. Both folds are   *)
(*    correct. Both books tie. The screen is a fiction.                     *)
(***************************************************************************)
AReconciliationComparesOnePrefix == \A r \in recons : r.at = r.against

(***************************************************************************)
(* 3. A STRIKE FOLDS THE PREFIX IT PINS, IN THE VIEW IT NAMES.             *)
(*                                                                         *)
(*    `Projection.StrikeFoldsItsOwnPrefix` with a view attached. The view   *)
(*    is on the strike because a figure that does not say which view it     *)
(*    came from is the failure HANDOFF.md already records: the console and  *)
(*    the CLI reported different NAVs for one book, neither saying which.   *)
(***************************************************************************)
AStrikeFoldsTheViewItNames ==
    \A s \in strikes : s.pin = s.read /\ s.view \in Views

(***************************************************************************)
(* 4. ⛔ REPLAY REPRODUCES, UNDER EVERY VIEW.                              *)
(*                                                                         *)
(*    A settlement figure depends on its pinned prefix AND on the calendar  *)
(*    the trade was rolled over. If that calendar lives outside the pinned  *)
(*    configuration, the prefix does not determine the figure: a trade      *)
(*    struck on the 30th settles on the 2nd today and on the 3rd after      *)
(*    somebody declares a holiday, and re-deriving answers differently as   *)
(*    the world tells us more.                                             *)
(*                                                                         *)
(*    That is worse than a restatement, because a restatement announces     *)
(*    itself. Same argument as an announcement in a side log, one input     *)
(*    over.                                                                *)
(***************************************************************************)
ReplayFigure(s)   == << s.view, s.pin, IF CalendarInConfig THEN 0 ELSE sideCal >>
OriginalFigure(s) == << s.view, s.pin, IF CalendarInConfig THEN 0 ELSE s.cal >>

ReplayReproducesUnderEveryView ==
    \A s \in strikes : ReplayFigure(s) = OriginalFigure(s)

(***************************************************************************)
(* 5. And every view catches up, so lagging is a delay rather than a       *)
(*    permanent divergence. Liveness, under weak fairness on Advance.      *)
(***************************************************************************)
EveryViewCatchesUp == []<>(\A v \in Views : viewAt[v] = journal)

TypeOK ==
    /\ journal \in 0..MaxJournal
    /\ sideCal \in 0..MaxCalendars
    /\ viewAt \in [Views -> 0..MaxJournal]
    /\ \A s \in strikes : s.view \in Views /\ s.pin \in 0..MaxJournal
                          /\ s.read \in 0..MaxJournal
    /\ \A r \in recons : r.left \in Views /\ r.right \in Views
                         /\ r.at \in 0..MaxJournal
                         /\ r.against \in 0..MaxJournal

=============================================================================
