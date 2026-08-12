--------------------------- MODULE S3Journal ---------------------------
(***************************************************************************)
(* The journal on an object store, one object per entry, and what a race    *)
(* between two writers does to it.                                          *)
(*                                                                          *)
(* Issue #24: the demo writes the journal to a Lambda's `/tmp`, so a posted  *)
(* entry is lost on the next cold start and two concurrent containers build  *)
(* two divergent journals under one URL — and the one thing this design      *)
(* cannot tolerate is a journal that silently FORKS, because each fork ties, *)
(* digests and replays. Moving the journal to a single object store closes   *)
(* the fork (there is one store, not one per container). What it opens        *)
(* instead is a WRITE race, and this is that race.                          *)
(*                                                                          *)
(* The append is object-per-entry: a writer LISTs the journal prefix, reads  *)
(* the highest sequence present, and PUTs its entry at `max + 1`. Two        *)
(* writers that LIST at the same moment both read the same max and both aim  *)
(* at the same slot.                                                         *)
(*                                                                          *)
(* ⛔ THE CONDITIONAL PUT IS THE CLAIM, MADE DURABLE. `//tla:executor_check`  *)
(* proves a partition run twice double-counts unless a worker CLAIMS it       *)
(* before working; issue #24 says in as many words that "a second writer is   *)
(* a second claim". Here the claim is an `If-None-Match:*` PUT — it writes    *)
(* the object only if the key is absent — so the second writer to a slot is   *)
(* refused and retries at the next one. Turn it off and the second PUT        *)
(* OVERWRITES the first: the losing entry received a success and then         *)
(* vanished, and the journal is one object shorter than the writes that       *)
(* believe they are in it. Arithmetically silent, exactly like the double     *)
(* count — the shortened journal ties, digests and replays.                  *)
(*                                                                          *)
(* One dial.                                                                 *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Writers,          \* the concurrent appenders; each has one entry to write
    MaxSeq,           \* bound, not a property — the store is unbounded
    Empty,            \* a model value: a sequence key with no object at it
    ConditionalPut    \* the fix. FALSE lets a PUT overwrite an occupied key.

\* ⚠ A MODEL VALUE, not `CHOOSE`. `Empty` marks an absent key, and it must be
\* distinct from every writer so "the slot holds writer w" and "the slot is
\* absent" are never confused.
ASSUME Empty \notin Writers

VARIABLES
    log,        \* [1..MaxSeq -> Writers \cup {Empty}] — the objects present
    target,     \* [Writers -> 0..MaxSeq] — the slot a writer's last LIST chose
    acked       \* SUBSET Writers — writers whose PUT returned success

vars == <<log, target, acked>>

----------------------------------------------------------------------------

\* The highest sequence key that holds an object — what a strongly-consistent
\* S3 LIST over `journal/` returns. Zero on an empty journal.
NonEmpty == {s \in 1..MaxSeq : log[s] # Empty}
Height == IF NonEmpty = {} THEN 0
          ELSE CHOOSE s \in NonEmpty : \A t \in NonEmpty : t =< s

Init ==
    /\ log = [s \in 1..MaxSeq |-> Empty]
    /\ target = [w \in Writers |-> 0]
    /\ acked = {}

(* LIST the journal and aim at the slot after the last one.
   ⛔ THE READ AND THE WRITE ARE SEPARATE STEPS, which is the whole point:
   two writers can LIST the same max before either PUTs, and then they aim
   at one slot. A writer may re-LIST at any time — that is the retry. *)
Reserve(w) ==
    /\ w \notin acked
    /\ Height + 1 =< MaxSeq
    /\ target' = [target EXCEPT ![w] = Height + 1]
    /\ UNCHANGED <<log, acked>>

(* PUT the entry at the reserved slot.
   ⛔ CONDITIONAL: with the dial on, the PUT lands only if the key is absent
   (`If-None-Match:*`); a writer that finds its slot taken has NOT acked and
   must Reserve again. With the dial off, the PUT overwrites whatever is
   there and acks anyway — the losing writer's entry is gone. *)
Put(w) ==
    /\ w \notin acked
    /\ target[w] \in 1..MaxSeq
    /\ IF ConditionalPut THEN log[target[w]] = Empty ELSE TRUE
    /\ log' = [log EXCEPT ![target[w]] = w]
    /\ acked' = acked \cup {w}
    /\ UNCHANGED target

Done == acked = Writers

Next ==
    \/ \E w \in Writers : Reserve(w)
    \/ \E w \in Writers : Put(w)
    \/ (Done /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. ⭐ NO ACKNOWLEDGED WRITE IS LOST.                                     *)
(*                                                                         *)
(*    A writer that received a success for its PUT must still be readable   *)
(*    in the journal. The conditional PUT guarantees it: a slot, once       *)
(*    claimed, is never overwritten, so an ack means the entry is there for *)
(*    good. Without the condition a later blind PUT to the same slot        *)
(*    silently replaces it — the entry is acked and absent, and the journal *)
(*    is shorter than the writes it promised, which is the fork of #24      *)
(*    reappearing as a truncation.                                         *)
(***************************************************************************)
NoWriteIsLost == \A w \in acked : \E s \in 1..MaxSeq : log[s] = w

(***************************************************************************)
(* 2. THE OBJECTS PRESENT FORM A GAPLESS PREFIX.                           *)
(*                                                                         *)
(*    Each claim targets `max + 1`, so the keys that exist are exactly      *)
(*    `1..k` — no holes. This is what lets the journal stay ONE contiguous  *)
(*    range that a fold reads as a single prefix, the obligation            *)
(*    `//tla:sql_projection_check` carries: a figure is folded from one     *)
(*    prefix, not spliced across a gap.                                    *)
(***************************************************************************)
ContiguousNoHoles == NonEmpty = 1..Cardinality(NonEmpty)

(***************************************************************************)
(* 3. AND NO ENTRY IS WRITTEN TWICE.                                       *)
(*                                                                         *)
(*    The dual of 1, and the durable form of `NoPartitionRunsTwice`: a      *)
(*    fold over the one prefix reads each write once, never the same        *)
(*    writer's entry from two keys. Stated apart because the failure that    *)
(*    breaks 1 (an overwrite) and the one that would break this (a          *)
(*    duplicate) are different halves of "read every row once".            *)
(***************************************************************************)
NoEntryTwice ==
    \A s1, s2 \in NonEmpty : (log[s1] = log[s2]) => (s1 = s2)

TypeOK ==
    /\ log \in [1..MaxSeq -> Writers \cup {Empty}]
    /\ target \in [Writers -> 0..MaxSeq]
    /\ acked \subseteq Writers

=============================================================================
