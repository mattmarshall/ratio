------------------------- MODULE BookPublication -------------------------
(***************************************************************************)
(* Issue #297: the publication boundary for one newly created book ID.      *)
(* Two creators stage different immutable bootstrap generations. A reader  *)
(* discovers only the conditional publication record, never staging.        *)
(*                                                                        *)
(* The six content addresses below stand for exact, validated bytes. SHA-256*)
(* collision resistance and the byte/proto codec are implementation         *)
(* obligations, not arithmetic proved by this finite state machine.         *)
(* ACTIVE points to the opening config and HISTORY contains that one        *)
(* promotion. A creator grant is a separate authorization record, not a     *)
(* grant inferred from identity, directory existence, or an organization.   *)
(*                                                                        *)
(* A process crash erases its local progress, not durable content or the    *)
(* publication. A failed PUT may leave staged objects; a committed PUT with *)
(* a lost response leaves a complete discoverable book. Neither failure     *)
(* permits a second initial generation to replace the first.                *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS
    Creators,
    NoBook,
    ConditionalPublication,
    RequireComplete,
    KeepGeneration,
    RefuseIncomplete,
    GrantOnlyPublished

ASSUME /\ Cardinality(Creators) = 2
       /\ NoBook \notin Creators

Parts == <<"identity", "chart", "config", "active", "history", "grant">>
Fields == {Parts[i] : i \in 1..Len(Parts)}

(* A content address commits to the exact bytes of one part, including any
   references it contains. In particular, active/history of c cite config c.
   Different creators have different bootstrap bytes in this model. *)
Address(c, part) == <<c, part>>
Addresses == {Address(c, part) : c \in Creators, part \in Fields}
References(c) == [part \in Fields |-> Address(c, part)]
Other(c) == CHOOSE other \in Creators : other # c

(* These are the contents whose exact bytes each address commits to.
   Money and chart evaluation are outside this state machine. *)
Identity(c) == [bookId |-> "same-book-id", kind |-> <<c, "kind">>,
                displayName |-> <<c, "name">>, fund |-> NoBook,
                organization |-> NoBook, createdBy |-> c]
Chart(c) == <<c, "exact-chart-bytes">>
Config(c) == <<c, "exact-opening-config-bytes">>
Active(c) == Address(c, "config")
History(c) == <<Address(c, "config")>>
CreatorGrant(c) == [bookId |-> "same-book-id", subject |-> c]

(* The generation-mixing probe borrows somebody else's ACTIVE. Both objects
   can exist and parse while ACTIVE cites a config outside this bootstrap. *)
Candidate(c) ==
    [creator |-> c,
     refs |-> IF KeepGeneration THEN References(c)
              ELSE [References(c) EXCEPT !["active"] = Address(Other(c), "active")]]

Referenced(snapshot) == {snapshot.refs[part] : part \in Fields}
Coherent(snapshot) ==
    /\ snapshot.refs = References(snapshot.creator)
    /\ Active(snapshot.refs["active"][1]) = snapshot.refs["config"]
    /\ History(snapshot.refs["history"][1]) = <<snapshot.refs["config"]>>
    /\ CreatorGrant(snapshot.refs["grant"][1]).subject = snapshot.creator

VARIABLES
    objects,        \* Durable immutable objects, each at its content address.
    publication,    \* One conditional visibility boundary for the book ID.
    cursor,         \* Local staging progress; a crash may erase it.
    phase,          \* working / won / acknowledged / lost / failed / crashed.
    winners,        \* Ghost history: every successful publication claim.
    acknowledged,   \* Ghost history: replies received by successful creators.
    fault,          \* One missing/corrupt object response, not a default blob.
    lastRead        \* One reader's most recent verified snapshot or refusal.

vars == <<objects, publication, cursor, phase, winners, acknowledged, fault, lastRead>>

EmptyRead == [status |-> "absent", snapshot |-> NoBook,
              verified |-> {}, grants |-> {}]

Init ==
    /\ objects = {}
    /\ publication = NoBook
    /\ cursor = [c \in Creators |-> 0]
    /\ phase = [c \in Creators |-> "working"]
    /\ winners = {}
    /\ acknowledged = {}
    /\ fault = NoBook
    /\ lastRead = EmptyRead

(* A separate durable PUT per part, in a fixed staging order. Interleavings
   and crashes can leave any prefix of either bootstrap behind. The order is
   not the correctness argument: publication checks all referenced objects. *)
Stage(c) ==
    /\ phase[c] = "working"
    /\ cursor[c] < Len(Parts)
    /\ objects' = objects \cup {Address(c, Parts[cursor[c] + 1])}
    /\ cursor' = [cursor EXCEPT ![c] = @ + 1]
    /\ UNCHANGED <<publication, phase, winners, acknowledged, fault, lastRead>>

Publish(c) ==
    /\ phase[c] = "working"
    /\ IF RequireComplete THEN Referenced(Candidate(c)) \subseteq objects ELSE TRUE
    /\ IF ConditionalPublication THEN publication = NoBook ELSE TRUE
    /\ publication' = Candidate(c)
    /\ phase' = [phase EXCEPT ![c] = "won"]
    /\ winners' = winners \cup {c}
    /\ UNCHANGED <<objects, cursor, acknowledged, fault, lastRead>>

(* A lost conditional claim cannot publish or obtain a grant. A real caller
   gets a conflict; no retry under a different generation happens here. *)
Lose(c) ==
    /\ phase[c] = "working"
    /\ publication # NoBook
    /\ publication.creator # c
    /\ Referenced(Candidate(c)) \subseteq objects
    /\ phase' = [phase EXCEPT ![c] = "lost"]
    /\ UNCHANGED <<objects, publication, cursor, winners, acknowledged, fault, lastRead>>

Acknowledge(c) ==
    /\ phase[c] = "won"
    /\ phase' = [phase EXCEPT ![c] = "acknowledged"]
    /\ acknowledged' = acknowledged \cup {c}
    /\ UNCHANGED <<objects, publication, cursor, winners, fault, lastRead>>

(* A failed pre-publication call leaves no claim. A crash after a committed
   publication but before Acknowledge models an uncertain/lost response. *)
Fail(c) ==
    /\ phase[c] = "working"
    /\ phase' = [phase EXCEPT ![c] = "failed"]
    /\ UNCHANGED <<objects, publication, cursor, winners, acknowledged, fault, lastRead>>

Crash(c) ==
    /\ phase[c] # "crashed"
    /\ cursor' = [cursor EXCEPT ![c] = 0]
    /\ phase' = [phase EXCEPT ![c] = "crashed"]
    /\ UNCHANGED <<objects, publication, winners, acknowledged, fault, lastRead>>

(* Missing/corrupt reads are indistinguishable for acceptance: neither is
   a verified object. They do not erase the immutable historical model; the
   reader must refuse while storage cannot supply the committed bytes. *)
FaultRead(address, reason) ==
    /\ publication # NoBook
    /\ address \in Referenced(publication)
    /\ reason \in {"missing", "corrupt"}
    /\ fault' = [address |-> address, reason |-> reason]
    /\ UNCHANGED <<objects, publication, cursor, phase, winners, acknowledged, lastRead>>

RepairRead ==
    /\ fault # NoBook
    /\ fault' = NoBook
    /\ UNCHANGED <<objects, publication, cursor, phase, winners, acknowledged, lastRead>>

Verified(snapshot) ==
    {address \in Referenced(snapshot) \cap objects :
        IF fault = NoBook THEN TRUE ELSE address # fault.address}

Read ==
    /\ lastRead' =
        IF publication = NoBook THEN EmptyRead
        ELSE LET verified == Verified(publication)
                 complete == Referenced(publication) \subseteq verified
                 accept == IF RefuseIncomplete THEN complete /\ Coherent(publication) ELSE TRUE
                 grants == IF GrantOnlyPublished
                           THEN {CreatorGrant(publication.refs["grant"][1]).subject}
                           ELSE {c \in Creators : Address(c, "grant") \in objects}
             IN [status |-> IF accept THEN "open" ELSE "refused",
                 snapshot |-> publication,
                 verified |-> verified,
                 grants |-> IF accept THEN grants ELSE {}]
    /\ UNCHANGED <<objects, publication, cursor, phase, winners, acknowledged, fault>>

(* A fresh reader has no memory to rescue an incomplete durable book. *)
RestartReader ==
    /\ lastRead' = EmptyRead
    /\ UNCHANGED <<objects, publication, cursor, phase, winners, acknowledged, fault>>

Next ==
    \/ \E c \in Creators : Stage(c) \/ Publish(c) \/ Lose(c) \/ Acknowledge(c) \/ Fail(c) \/ Crash(c)
    \/ \E address \in Addresses, reason \in {"missing", "corrupt"} : FaultRead(address, reason)
    \/ RepairRead
    \/ Read
    \/ RestartReader

Spec == Init /\ [][Next]_vars

SnapshotType(snapshot) ==
    IF snapshot = NoBook THEN TRUE
    ELSE /\ snapshot.creator \in Creators
         /\ snapshot.refs \in [Fields -> Addresses]

TypeOK ==
    /\ objects \subseteq Addresses
    /\ SnapshotType(publication)
    /\ cursor \in [Creators -> 0..Len(Parts)]
    /\ phase \in [Creators -> {"working", "won", "acknowledged", "lost", "failed", "crashed"}]
    /\ winners \subseteq Creators
    /\ acknowledged \subseteq Creators
    /\ IF fault = NoBook THEN TRUE
       ELSE /\ fault.address \in Addresses
            /\ fault.reason \in {"missing", "corrupt"}
    /\ lastRead.status \in {"absent", "refused", "open"}
    /\ SnapshotType(lastRead.snapshot)
    /\ lastRead.verified \subseteq Addresses
    /\ lastRead.grants \subseteq Creators

PublishedBookComplete ==
    IF publication = NoBook THEN TRUE
    ELSE Referenced(publication) \subseteq objects

PublishedGenerationAgrees ==
    IF publication = NoBook THEN TRUE ELSE Coherent(publication)

OneWinningGeneration == Cardinality(winners) =< 1

AcknowledgedPublicationSurvives ==
    \A c \in acknowledged :
        /\ publication # NoBook
        /\ publication = [creator |-> c, refs |-> References(c)]
        /\ Referenced(publication) \subseteq objects

ReaderHasVerifiedSnapshot ==
    IF lastRead.status # "open" THEN TRUE
    ELSE /\ lastRead.snapshot # NoBook
         /\ Coherent(lastRead.snapshot)
         /\ Referenced(lastRead.snapshot) \subseteq lastRead.verified

ReaderGrantMatchesSnapshot ==
    IF lastRead.status # "open" THEN lastRead.grants = {}
    ELSE lastRead.grants = {lastRead.snapshot.creator}

LosingCreatorHasNoMembership ==
    \A c \in Creators : phase[c] = "lost" => c \notin lastRead.grants

=============================================================================
