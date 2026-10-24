--------------------------- MODULE OPFSJournal ---------------------------
(* One dedicated worker holds ownership through prepare, write, flush, ack.
   OPFS create:true is not a conditional claim. Exclusion supplies the claim.
   Reload releases a dead worker; it never steals from a live/frozen owner.
   Complete unacknowledged records may survive a crash. Retrying an unknown
   outcome is not an exactly-once promise; reconcile before retrying it. *)
EXTENDS Naturals, FiniteSets

CONSTANTS Writers, MaxSeq, Empty, ExclusiveOwner

ASSUME Empty \notin Writers

VARIABLES owners, phase, target, volatile, durable, acked
vars == <<owners, phase, target, volatile, durable, acked>>

Present(log) == {s \in 1..MaxSeq : log[s] # Empty}
Height(log) == IF Present(log) = {} THEN 0
              ELSE CHOOSE s \in Present(log) : \A t \in Present(log) : t =< s

Init ==
    /\ owners = {}
    /\ phase = [w \in Writers |-> 0]
    /\ target = [w \in Writers |-> 0]
    /\ volatile = [s \in 1..MaxSeq |-> Empty]
    /\ durable = volatile
    /\ acked = {}

Acquire(w) ==
    /\ w \notin owners \cup acked
    /\ IF ExclusiveOwner THEN owners = {} ELSE TRUE
    /\ owners' = owners \cup {w}
    /\ UNCHANGED <<phase, target, volatile, durable, acked>>

Prepare(w) ==
    /\ w \in owners
    /\ phase[w] = 0
    /\ Height(volatile) < MaxSeq
    /\ target' = [target EXCEPT ![w] = Height(volatile) + 1]
    /\ phase' = [phase EXCEPT ![w] = 1]
    /\ UNCHANGED <<owners, volatile, durable, acked>>

Write(w) ==
    /\ w \in owners
    /\ phase[w] = 1
    \* No filesystem CAS: a second prepared writer could overwrite this slot.
    /\ volatile' = [volatile EXCEPT ![target[w]] = w]
    /\ phase' = [phase EXCEPT ![w] = 2]
    /\ UNCHANGED <<owners, target, durable, acked>>

FlushAndAck(w) ==
    /\ w \in owners
    /\ phase[w] = 2
    /\ durable' = volatile
    /\ acked' = acked \cup {w}
    /\ phase' = [phase EXCEPT ![w] = 3]
    /\ UNCHANGED <<owners, target, volatile>>

Release(w) ==
    /\ w \in owners
    /\ phase[w] = 3
    /\ owners' = owners \ {w}
    /\ UNCHANGED <<phase, target, volatile, durable, acked>>

Crash(w, keepComplete) ==
    /\ w \in owners
    /\ owners' = owners \ {w}
    /\ phase' = [phase EXCEPT ![w] = 0]
    /\ target' = [target EXCEPT ![w] = 0]
    \* Recover a tail only after the last owner is gone. Killing one worker
    \* cannot roll back a different live worker's in-flight write.
    /\ IF owners' = {} THEN
           /\ durable' = IF keepComplete /\ phase[w] = 2 THEN volatile ELSE durable
           /\ volatile' = durable'
       ELSE UNCHANGED <<volatile, durable>>
    /\ UNCHANGED acked

Next ==
    \/ \E w \in Writers : Acquire(w) \/ Prepare(w) \/ Write(w)
                           \/ FlushAndAck(w) \/ Release(w)
                           \/ Crash(w, TRUE) \/ Crash(w, FALSE)
    \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

NoWriteIsLost == \A w \in acked : \E s \in 1..MaxSeq : durable[s] = w
SingleWriter == Cardinality(owners) =< 1
ContiguousNoHoles == Present(durable) = 1..Cardinality(Present(durable))
TypeOK ==
    /\ owners \subseteq Writers
    /\ phase \in [Writers -> 0..3]
    /\ target \in [Writers -> 0..MaxSeq]
    /\ volatile \in [1..MaxSeq -> Writers \cup {Empty}]
    /\ durable \in [1..MaxSeq -> Writers \cup {Empty}]
    /\ acked \subseteq Writers
=============================================================================
