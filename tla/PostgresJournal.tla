------------------------ MODULE PostgresJournal ------------------------
EXTENDS Naturals, FiniteSets

(***************************************************************************)
(* #362: transaction-private INSERT, unique (namespace,key), COMMIT, then    *)
(* acknowledge. This is not S3 LIST/PUT: uncommitted rows are invisible and *)
(* abort cannot leave an acknowledged entry. A reader accepts only a        *)
(* contiguous committed prefix. PostgreSQL durability and isolation are     *)
(* assumptions supplied by the database, not arithmetic theorems.           *)
(***************************************************************************)
CONSTANTS
    Writers,
    MaxSeq,
    Empty,
    UniqueClaim,
    AckAfterCommit,
    CommittedReads

ASSUME Empty \notin Writers

VARIABLES log, target, staged, acked, readRows, readAccepted
vars == <<log, target, staged, acked, readRows, readAccepted>>

Occupied(rows) == {s \in 1..MaxSeq : rows[s] # Empty}
HeightOf(rows) == IF Occupied(rows) = {} THEN 0
    ELSE CHOOSE s \in Occupied(rows) : \A t \in Occupied(rows) : t =< s
Height == HeightOf(log)

Init ==
    /\ log = [s \in 1..MaxSeq |-> Empty]
    /\ target = [w \in Writers |-> 0]
    /\ staged = {}
    /\ acked = {}
    /\ readRows = [s \in 1..MaxSeq |-> Empty]
    /\ readAccepted = FALSE

\* Height is an observation; it does not reserve the candidate sequence.
Begin(w) ==
    /\ w \notin acked \cup staged
    /\ Height < MaxSeq
    /\ target' = [target EXCEPT ![w] = Height + 1]
    /\ UNCHANGED <<log, staged, acked, readRows, readAccepted>>

\* A tentative INSERT is private to this transaction. The unique-index
\* conflict is resolved at commit in this abstraction; no loser is acked.
Insert(w) ==
    /\ w \notin acked \cup staged
    /\ target[w] \in 1..MaxSeq
    /\ staged' = staged \cup {w}
    /\ acked' = IF AckAfterCommit THEN acked ELSE acked \cup {w}
    /\ UNCHANGED <<log, target, readRows, readAccepted>>

Commit(w) ==
    /\ w \in staged
    /\ IF UniqueClaim THEN log[target[w]] = Empty ELSE TRUE
    /\ log' = [log EXCEPT ![target[w]] = w]
    /\ staged' = staged \ {w}
    /\ acked' = acked \cup {w}
    /\ UNCHANGED <<target, readRows, readAccepted>>

Abort(w) ==
    /\ w \in staged
    /\ staged' = staged \ {w}
    /\ target' = [target EXCEPT ![w] = 0]
    /\ UNCHANGED <<log, acked, readRows, readAccepted>>

Read ==
    /\ \E rows \in [1..MaxSeq -> Writers \cup {Empty}] :
        /\ IF CommittedReads THEN rows = log
           ELSE \A s \in 1..MaxSeq : rows[s] = log[s] \/
               (\E w \in staged : target[w] = s /\ rows[s] = w)
        /\ readRows' = rows
        /\ readAccepted' = \A s \in 1..HeightOf(rows) : rows[s] # Empty
    /\ UNCHANGED <<log, target, staged, acked>>

Next ==
    \/ \E w \in Writers : Begin(w) \/ Insert(w) \/ Commit(w) \/ Abort(w)
    \/ Read
Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ log \in [1..MaxSeq -> Writers \cup {Empty}]
    /\ target \in [Writers -> 0..MaxSeq]
    /\ staged \subseteq Writers
    /\ acked \subseteq Writers
    /\ readRows \in [1..MaxSeq -> Writers \cup {Empty}]
    /\ readAccepted \in BOOLEAN
NoWriteIsLost == \A w \in acked : \E s \in 1..MaxSeq : log[s] = w
ContiguousNoHoles == \A s \in 1..Height : log[s] # Empty
NoEntryTwice == \A w \in Writers : Cardinality({s \in 1..MaxSeq : log[s] = w}) =< 1
AcceptedReadsAreCommitted == readAccepted =>
    \A s \in Occupied(readRows) : log[s] = readRows[s]
=============================================================================
