------------------------- MODULE FileBookState -------------------------
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************)
(* #363: immutable full-state revisions for an unpublished FileBook.         *)
(* A writer prepares from one revision, stores configuration bytes first,   *)
(* and conditionally claims that exact successor. Retrying a stale full      *)
(* snapshot at a later slot would discard another acknowledged promotion.   *)
(* Cache bytes are checked against their pinned historical revision; reads  *)
(* then select the current durable state, never a cache-selected figure.     *)
(* Payload identity abstracts verified content-addressed chart/rules bytes.  *)
(***************************************************************************)
CONSTANTS
    Writers,
    MaxRevision,
    Empty,
    ConditionalRevision,
    StoredBeforePromotion,
    VerifyCache

ASSUME Empty \notin Writers

VARIABLES revision, snapshots, stored, prepared, base, acked,
    cacheRevision, cacheValue, acceptedRevision, acceptedValue
vars == <<revision, snapshots, stored, prepared, base, acked,
    cacheRevision, cacheValue, acceptedRevision, acceptedValue>>

Values(history) == {history[i] : i \in 1..Len(history)}
Active(history) == IF Len(history) = 0 THEN Empty ELSE history[Len(history)]

Init ==
    /\ revision = 0
    /\ snapshots = [r \in 0..MaxRevision |-> <<>>]
    /\ stored = {}
    /\ prepared = {}
    /\ base = [w \in Writers |-> 0]
    /\ acked = {}
    /\ cacheRevision = 0
    /\ cacheValue = Empty
    /\ acceptedRevision = 0
    /\ acceptedValue = Empty

Stage(w) ==
    /\ stored' = stored \cup {w}
    /\ UNCHANGED <<revision, snapshots, prepared, base, acked,
        cacheRevision, cacheValue, acceptedRevision, acceptedValue>>
Prepare(w) ==
    /\ w \notin acked
    /\ prepared' = prepared \cup {w}
    /\ base' = [base EXCEPT ![w] = revision]
    /\ UNCHANGED <<revision, snapshots, stored, acked,
        cacheRevision, cacheValue, acceptedRevision, acceptedValue>>
Commit(w) ==
    /\ w \in prepared \ acked
    /\ revision < MaxRevision
    /\ IF ConditionalRevision THEN base[w] = revision ELSE TRUE
    /\ IF StoredBeforePromotion THEN w \in stored ELSE TRUE
    /\ snapshots' = [snapshots EXCEPT ![revision + 1] = Append(snapshots[base[w]], w)]
    /\ revision' = revision + 1
    /\ acked' = acked \cup {w}
    /\ UNCHANGED <<stored, prepared, base, cacheRevision, cacheValue,
        acceptedRevision, acceptedValue>>
LoadCache(r) ==
    /\ r \in 0..revision
    /\ cacheRevision' = r
    /\ cacheValue' = Active(snapshots[r])
    /\ UNCHANGED <<revision, snapshots, stored, prepared, base, acked,
        acceptedRevision, acceptedValue>>
TamperCache(value) ==
    /\ cacheValue' = value
    /\ UNCHANGED <<revision, snapshots, stored, prepared, base, acked,
        cacheRevision, acceptedRevision, acceptedValue>>
Read ==
    /\ IF VerifyCache THEN cacheValue = Active(snapshots[cacheRevision]) ELSE TRUE
    /\ acceptedRevision' = revision
    /\ acceptedValue' = IF VerifyCache THEN Active(snapshots[revision]) ELSE cacheValue
    /\ UNCHANGED <<revision, snapshots, stored, prepared, base, acked,
        cacheRevision, cacheValue>>
Next ==
    \/ \E w \in Writers : Stage(w) \/ Prepare(w) \/ Commit(w)
    \/ \E r \in 0..revision : LoadCache(r)
    \/ \E value \in Writers \cup {Empty} : TamperCache(value)
    \/ Read
Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ revision \in 0..MaxRevision
    /\ snapshots \in [0..MaxRevision -> Seq(Writers)]
    /\ stored \subseteq Writers
    /\ prepared \subseteq Writers
    /\ base \in [Writers -> 0..MaxRevision]
    /\ acked \subseteq Writers
    /\ cacheRevision \in 0..revision
    /\ cacheValue \in Writers \cup {Empty}
    /\ acceptedRevision \in 0..revision
    /\ acceptedValue \in Writers \cup {Empty}
AcknowledgedPromotionsRemainInHistory == acked \subseteq Values(snapshots[revision])
PromotionsNameStoredBytes == \A r \in 0..revision : Values(snapshots[r]) \subseteq stored
CacheCannotSelectAFigure == acceptedValue = Active(snapshots[acceptedRevision])
=============================================================================
