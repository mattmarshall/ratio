-------------------------- MODULE DurableControl --------------------------
(***************************************************************************)
(* #303 / #299: bootstrap plus one immutable control stream per book.       *)
(* Prepare checks the predecessor, Publish conditionally claims one slot,  *)
(* and Acknowledge is separate. A conflict never rebases the old request.   *)
(* Exact request bytes survive a server crash at the client; prepared state*)
(* and authorization do not. Digests below are injective abstract content  *)
(* addresses, not a proof of SHA-256 or of a concrete protobuf codec.        *)
(*                                                                        *)
(* Two writers run a finite workload; choices of scheduling include lost   *)
(* responses, exact retries, same-ID/different-payload reuse, and two books.*)
(* A logical deadline bounds both control requests and one ordinary       *)
(* authorized request. Completing that request is NOT a journal append,    *)
(* and the model does not claim an atomic auth-check/journal-write boundary.*)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    NoState,
    ConditionalPublish,
    CheckPredecessor,
    PublishBeforeAck,
    ExactRetry,
    FreshAuthorization,
    ProtectSeed

Books == {"a", "b"}
Writers == {"left", "right"}
Creator == "creator"
Guest == "guest"
Subjects == {Creator, Guest}
None == NoState
OpeningConfig == "config-0"
Configs == {OpeningConfig, "config-1", "config-2"}
MaxRevision == 2
MaxTick == 2
RequestWindow == 1
Workloads == {"race", "independent", "revoke", "grant", "reuse"}

VARIABLES
    observer,          \* Read-only recovery and in-flight observations are checked separately.
    workload,          \* Constant during one execution; union of small workloads.
    streams,           \* Durable, per-book ordered transition records.
    blobs,             \* Durable config objects, already hash verified at PUT.
    requests,          \* Client envelopes retained for exact retries.
    phase, target, reply, authorization, \* Server-local request progress.
    acknowledged,      \* Ghost receipts actually returned to clients.
    acknowledgedHead,  \* Ghost lower bound for an operation starting now.
    recovery,          \* One verified recovered cache (or none after restart).
    inFlight,          \* One bounded request's captured authorization.
    tick

vars == <<observer, workload, streams, blobs, requests, phase, target, reply,
          authorization, acknowledged, acknowledgedHead, recovery, inFlight, tick>>

PlanBook(w) == IF workload = "independent" /\ w = "right" THEN "b" ELSE "a"
PlanId(w) == IF workload \in {"independent", "reuse"} THEN "same-operation"
             ELSE w
PlanPayload(w) ==
    CASE workload = "grant" /\ w = "left" -> [kind |-> "grant", value |-> Guest]
      [] workload = "grant" /\ w = "right" -> [kind |-> "revoke", value |-> Guest]
      [] workload = "revoke" /\ w = "right" -> [kind |-> "revoke", value |-> Creator]
      [] OTHER -> [kind |-> "promote", value |-> IF w = "left" THEN "config-1" ELSE "config-2"]

BootstrapDigest(b) == <<"bootstrap", b>>
TransitionDigest(t) == <<"transition", t>>
HeadDigest(b, s) == IF Len(s) = 0 THEN BootstrapDigest(b) ELSE TransitionDigest(s[Len(s)])

RECURSIVE History(_), Members(_)
History(s) ==
    IF Len(s) = 0 THEN <<OpeningConfig>>
    ELSE LET before == History(SubSeq(s, 1, Len(s) - 1))
             p == s[Len(s)].request.payload
         IN IF p.kind = "promote" THEN <<p.value>> \o before ELSE before
Members(s) ==
    IF Len(s) = 0 THEN {Creator}
    ELSE LET before == Members(SubSeq(s, 1, Len(s) - 1))
             p == s[Len(s)].request.payload
         IN CASE p.kind = "grant" -> before \cup {p.value}
              [] p.kind = "revoke" -> before \ {p.value}
              [] OTHER -> before
ConfigRefs(s) == {History(s)[i] : i \in 1..Len(History(s))}
StateOf(s) == [revision |-> Len(s), active |-> History(s)[1],
               history |-> History(s), members |-> Members(s)]

ClientRequest(w) ==
    LET b == PlanBook(w)
    IN [book |-> b, operationId |-> PlanId(w), actor |-> Creator,
        expectedRevision |-> Len(streams[b]), expectedDigest |-> HeadDigest(b, streams[b]),
        payload |-> PlanPayload(w)]
Transition(w) == [revision |-> target[w], request |-> requests[w]]

(* A fresh authorization reads the verified committed head. Keeping the
   initial cached membership is the stale-authorization probe. The source and
   lower bound are ghost evidence, not client-controlled request fields. *)
Capture(b, subject) ==
    LET source == streams[b]
        snapshot == IF FreshAuthorization THEN source ELSE <<>>
    IN [book |-> b, subject |-> subject, source |-> source, snapshot |-> snapshot,
        observed |-> Len(snapshot), floor |-> acknowledgedHead[b],
        allowed |-> subject \in Members(snapshot), configDigest |-> StateOf(snapshot).active,
        deadline |-> tick + RequestWindow]

OperationAt(b, id) ==
    IF \E i \in 1..Len(streams[b]) : streams[b][i].request.operationId = id
    THEN CHOOSE i \in 1..Len(streams[b]) : streams[b][i].request.operationId = id
    ELSE 0
ExpectedAtHead(r) ==
    /\ r.expectedRevision = Len(streams[r.book])
    /\ r.expectedDigest = HeadDigest(r.book, streams[r.book])
HasConfig(r) == r.payload.kind # "promote" \/ r.payload.value \in blobs
Live(w) == authorization[w] # None /\ tick =< authorization[w].deadline

Init ==
    /\ workload \in Workloads
    /\ observer \in IF workload \in {"revoke", "grant"} THEN {"recovery", "request"} ELSE {"recovery"}
    /\ streams = [b \in Books |-> <<>>]
    /\ blobs = {OpeningConfig}
    /\ requests = [w \in Writers |-> None]
    /\ phase = [w \in Writers |-> "new"]
    /\ target = [w \in Writers |-> 0]
    /\ reply = [w \in Writers |-> None]
    /\ authorization = [w \in Writers |-> None]
    /\ acknowledged = {}
    /\ acknowledgedHead = [b \in Books |-> 0]
    /\ recovery = None
    /\ inFlight = None
    /\ tick = 0

Begin(w) ==
    /\ phase[w] = "new"
    /\ tick < MaxTick
    /\ LET auth == Capture(PlanBook(w), Creator)
       IN /\ requests' = [requests EXCEPT ![w] = ClientRequest(w)]
          /\ authorization' = [authorization EXCEPT ![w] = auth]
          /\ phase' = [phase EXCEPT ![w] = IF auth.allowed THEN "authorized" ELSE "refused"]
    /\ UNCHANGED <<observer, workload, streams, blobs, target, reply, acknowledged,
                    acknowledgedHead, recovery, inFlight, tick>>

StageConfig(w) ==
    /\ phase[w] = "authorized"
    /\ requests[w].payload.kind = "promote"
    /\ requests[w].payload.value \notin blobs
    /\ blobs' = blobs \cup {requests[w].payload.value}
    /\ UNCHANGED <<observer, workload, streams, requests, phase, target, reply, authorization,
                    acknowledged, acknowledgedHead, recovery, inFlight, tick>>

(* The checked head and the conditional slot PUT are separate operations.
   The predecessor probe appends stale input at the new head; it does not
   update the expected predecessor in that input to disguise the conflict. *)
Prepare(w) ==
    /\ phase[w] = "authorized" /\ Live(w)
    /\ LET r == requests[w]
       IN /\ OperationAt(r.book, r.operationId) = 0
          /\ HasConfig(r)
          /\ Len(streams[r.book]) < MaxRevision
          /\ IF CheckPredecessor THEN ExpectedAtHead(r) ELSE TRUE
          /\ target' = [target EXCEPT ![w] = Len(streams[r.book]) + 1]
    /\ phase' = [phase EXCEPT ![w] = "prepared"]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, reply, authorization,
                    acknowledged, acknowledgedHead, recovery, inFlight, tick>>

Publish(w) ==
    /\ phase[w] = "prepared" /\ Live(w)
    /\ LET b == requests[w].book
           n == target[w]
           t == Transition(w)
       IN /\ n \in 1..(Len(streams[b]) + 1)
          /\ IF ConditionalPublish THEN n = Len(streams[b]) + 1 ELSE TRUE
          /\ streams' = [streams EXCEPT ![b] =
                IF n = Len(@) + 1 THEN Append(@, t) ELSE [@ EXCEPT ![n] = t]]
          /\ reply' = [reply EXCEPT ![w] = t]
    /\ phase' = [phase EXCEPT ![w] = "committed"]
    /\ UNCHANGED <<observer, workload, blobs, requests, target, authorization, acknowledged,
                    acknowledgedHead, recovery, inFlight, tick>>

(* Look up the operation before checking its now-old predecessor. Only the
   same complete envelope may receive the original receipt; ID alone is not
   idempotence. The actor is part of that envelope. *)
ResolveRetry(w) ==
    /\ phase[w] = "authorized" /\ Live(w)
    /\ LET r == requests[w]
           i == OperationAt(r.book, r.operationId)
       IN /\ i # 0
          /\ LET existing == streams[r.book][i]
                 same == IF ExactRetry THEN existing.request = r ELSE TRUE
             IN /\ phase' = [phase EXCEPT ![w] = IF same THEN "committed" ELSE "refused"]
                /\ reply' = [reply EXCEPT ![w] = IF same THEN existing ELSE None]
                /\ target' = [target EXCEPT ![w] = IF same THEN i ELSE 0]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, authorization,
                    acknowledged, acknowledgedHead, recovery, inFlight, tick>>

Conflict(w) ==
    /\ Live(w)
    /\ \/ /\ phase[w] = "authorized"
           /\ CheckPredecessor
           /\ OperationAt(requests[w].book, requests[w].operationId) = 0
           /\ ~ExpectedAtHead(requests[w])
       \/ /\ phase[w] = "prepared"
           /\ target[w] =< Len(streams[requests[w].book])
    /\ phase' = [phase EXCEPT ![w] = "conflict"]
    /\ target' = [target EXCEPT ![w] = 0]
    /\ reply' = [reply EXCEPT ![w] = None]
    /\ authorization' = [authorization EXCEPT ![w] = None]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests,
                    acknowledged, acknowledgedHead, recovery, inFlight, tick>>

Acknowledge(w) ==
    /\ Live(w)
    /\ phase[w] = "committed" \/ (~PublishBeforeAck /\ phase[w] = "prepared")
    /\ LET b == requests[w].book
           receipt == IF phase[w] = "committed" THEN reply[w] ELSE Transition(w)
       IN /\ acknowledged' = acknowledged \cup {[input |-> requests[w], receipt |-> receipt]}
          /\ acknowledgedHead' = [acknowledgedHead EXCEPT ![b] =
                  IF receipt.revision > @ THEN receipt.revision ELSE @]
    /\ phase' = [phase EXCEPT ![w] = "acknowledged"]
    /\ target' = [target EXCEPT ![w] = 0]
    /\ reply' = [reply EXCEPT ![w] = None]
    /\ authorization' = [authorization EXCEPT ![w] = None]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, recovery, inFlight, tick>>

(* Server crash/failure cannot erase the stream, the blob, or a receipt that
   was already delivered. Client request bytes remain outside the server. *)
Crash(w) ==
    /\ phase[w] \notin {"new", "crashed"}
    /\ phase' = [phase EXCEPT ![w] = "crashed"]
    /\ target' = [target EXCEPT ![w] = 0]
    /\ reply' = [reply EXCEPT ![w] = None]
    /\ authorization' = [authorization EXCEPT ![w] = None]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, acknowledged,
                    acknowledgedHead, recovery, inFlight, tick>>

(* Includes recovery from a lost publication response. This never rebases
   the original expected predecessor or changes the operation's payload. *)
Retry(w) ==
    /\ phase[w] \in {"crashed", "acknowledged", "conflict", "refused", "expired"}
    /\ tick < MaxTick
    /\ LET auth == Capture(requests[w].book, requests[w].actor)
       IN /\ authorization' = [authorization EXCEPT ![w] = auth]
          /\ phase' = [phase EXCEPT ![w] = IF auth.allowed THEN "authorized" ELSE "refused"]
    /\ target' = [target EXCEPT ![w] = 0]
    /\ reply' = [reply EXCEPT ![w] = None]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, acknowledged,
                    acknowledgedHead, recovery, inFlight, tick>>

ExpireWriter(w) ==
    /\ phase[w] \in {"authorized", "prepared", "committed"}
    /\ ~Live(w)
    /\ phase' = [phase EXCEPT ![w] = "expired"]
    /\ target' = [target EXCEPT ![w] = 0]
    /\ reply' = [reply EXCEPT ![w] = None]
    /\ authorization' = [authorization EXCEPT ![w] = None]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests,
                    acknowledged, acknowledgedHead, recovery, inFlight, tick>>

Recover(b) ==
    /\ observer = "recovery"
    /\ recovery' = [book |-> b, source |-> streams[b], value |-> StateOf(streams[b]),
                     verified |-> ConfigRefs(streams[b]), action |-> "recover"]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, inFlight, tick>>

(* The deliberately dangerous case: a baked seed after an acknowledged
   creator revocation. A safe seed leaves the committed view intact. *)
Seed(b) ==
    /\ observer = "recovery"
    /\ Creator \notin Members(streams[b])
    /\ acknowledgedHead[b] = Len(streams[b])
    /\ recovery' = [book |-> b, source |-> streams[b],
                     value |-> IF ProtectSeed THEN StateOf(streams[b]) ELSE StateOf(<<>>),
                     verified |-> ConfigRefs(streams[b]), action |-> "seed"]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, inFlight, tick>>

BeginRequest(b, subject) ==
    /\ observer = "request"
    /\ inFlight = None /\ tick < MaxTick
    /\ LET auth == Capture(b, subject)
       IN inFlight' = [auth |-> auth, status |-> IF auth.allowed THEN "authorized" ELSE "refused",
                       finishedAt |-> 0]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, recovery, tick>>

FinishRequest ==
    /\ inFlight # None /\ inFlight.status = "authorized"
    /\ tick =< inFlight.auth.deadline
    /\ inFlight' = [inFlight EXCEPT !.status = "finished", !.finishedAt = tick]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, recovery, tick>>

ExpireRequest ==
    /\ inFlight # None /\ inFlight.status = "authorized"
    /\ tick > inFlight.auth.deadline
    /\ inFlight' = [inFlight EXCEPT !.status = "expired"]
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, recovery, tick>>

RestartReader ==
    /\ recovery' = None /\ inFlight' = None
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, tick>>

Tick ==
    /\ tick < MaxTick /\ tick' = tick + 1
    /\ UNCHANGED <<observer, workload, streams, blobs, requests, phase, target, reply,
                    authorization, acknowledged, acknowledgedHead, recovery, inFlight>>

Next ==
    \/ \E w \in Writers : Begin(w) \/ StageConfig(w) \/ Prepare(w) \/ Publish(w)
                           \/ ResolveRetry(w) \/ Conflict(w) \/ Acknowledge(w)
                           \/ Crash(w) \/ Retry(w) \/ ExpireWriter(w)
    \/ \E b \in Books : Recover(b) \/ Seed(b)
    \/ \E b \in Books, s \in Subjects : BeginRequest(b, s)
    \/ FinishRequest \/ ExpireRequest \/ RestartReader \/ Tick

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ observer \in {"recovery", "request"}
    /\ workload \in Workloads
    /\ DOMAIN streams = Books
    /\ \A b \in Books : Len(streams[b]) \in 0..MaxRevision
    /\ blobs \subseteq Configs
    /\ phase \in [Writers -> {"new", "authorized", "prepared", "committed", "acknowledged",
                              "refused", "conflict", "crashed", "expired"}]
    /\ target \in [Writers -> 0..MaxRevision]
    /\ acknowledgedHead \in [Books -> 0..MaxRevision]
    /\ tick \in 0..MaxTick

AcknowledgedTransitionsSurvive ==
    \A a \in acknowledged :
        LET b == a.receipt.request.book
            n == a.receipt.revision
        IN IF n \notin 1..Len(streams[b]) THEN FALSE ELSE streams[b][n] = a.receipt

CommittedPredecessorMatches ==
    \A b \in Books : \A n \in 1..Len(streams[b]) :
        /\ streams[b][n].revision = n
        /\ streams[b][n].request.book = b
        /\ streams[b][n].request.expectedRevision = n - 1
        /\ streams[b][n].request.expectedDigest = HeadDigest(b, SubSeq(streams[b], 1, n - 1))

CommittedOperationIdsAreUnique ==
    \A b \in Books : \A i, j \in 1..Len(streams[b]) :
        streams[b][i].request.operationId = streams[b][j].request.operationId => i = j

RetryResponseMatchesPayload ==
    \A a \in acknowledged : a.input = a.receipt.request

ActiveHasCommittedHistoryAndBlob ==
    \A b \in Books :
        /\ ConfigRefs(streams[b]) \subseteq blobs
        /\ StateOf(streams[b]).active = History(streams[b])[1]

FreshAuth(a) ==
    /\ a.observed >= a.floor
    /\ a.snapshot = a.source
    /\ a.allowed = (a.subject \in Members(a.source))
FreshAuthorizationAtRequestStart ==
    /\ \A w \in Writers : IF authorization[w] = None THEN TRUE ELSE FreshAuth(authorization[w])
    /\ IF inFlight = None THEN TRUE ELSE FreshAuth(inFlight.auth)

RecoveryMatchesCommittedSnapshot ==
    IF recovery = None THEN TRUE
    ELSE /\ recovery.value = StateOf(recovery.source)
         /\ ConfigRefs(recovery.source) \subseteq recovery.verified
         /\ recovery.verified \subseteq blobs

SeedCannotResurrectCreator ==
    IF recovery = None THEN TRUE
    ELSE IF recovery.action # "seed" THEN TRUE
         ELSE Creator \notin Members(recovery.source) => Creator \notin recovery.value.members

InFlightAuthorizationIsBounded ==
    IF inFlight = None THEN TRUE
    ELSE IF inFlight.status # "finished" THEN TRUE
         ELSE /\ inFlight.auth.allowed
              /\ inFlight.auth.subject \in Members(inFlight.auth.snapshot)
              /\ inFlight.finishedAt =< inFlight.auth.deadline
              /\ inFlight.auth.configDigest = StateOf(inFlight.auth.snapshot).active

(* No global lock or revision: an unoccupied slot for this book remains
   enabled regardless of another book's head, conflict, or crashed writer.
   This is enabledness, not eventual progress under an unfair scheduler. *)
IndependentBooksCanPublish ==
    \A w \in Writers :
        (phase[w] = "prepared" /\ Live(w) /\
         target[w] = Len(streams[requests[w].book]) + 1) => ENABLED Publish(w)

=============================================================================
