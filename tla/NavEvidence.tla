--------------------------- MODULE NavEvidence ---------------------------
(***************************************************************************)
(* Issue #300: a signed NAV strike must survive a serving-process failure.  *)
(* The object key is (book, view, valuation point); this bounded model fixes *)
(* one book and has two views. A conditional PUT is the uniqueness claim.   *)
(* A process cache can vanish; recovery reads the durable objects again.    *)
(* Turning off the condition acknowledges an overwrite of another person's *)
(* strike, leaving one perfectly shaped answer where there were two acks.   *)
(***************************************************************************)
EXTENDS FiniteSets

CONSTANTS Writers, Views, Point, WriterC, Accounting, Settlement,
          Empty, ConditionalPut

ASSUME Empty \notin Writers
Keys == Views \X {Point}
Key(w) == <<IF w = WriterC THEN Settlement ELSE Accounting, Point>>

VARIABLES durable, visible, acked, alive
vars == <<durable, visible, acked, alive>>

Init ==
    /\ durable = [k \in Keys |-> Empty]
    /\ visible = [k \in Keys |-> Empty]
    /\ acked = {}
    /\ alive = TRUE

Claim(w) ==
    /\ alive
    /\ w \notin acked
    /\ IF ConditionalPut THEN durable[Key(w)] = Empty ELSE TRUE
    /\ durable' = [durable EXCEPT ![Key(w)] = w]
    /\ visible' = [visible EXCEPT ![Key(w)] = w]
    /\ acked' = acked \cup {w}
    /\ UNCHANGED alive

Crash ==
    /\ alive
    /\ visible' = [k \in Keys |-> Empty]
    /\ alive' = FALSE
    /\ UNCHANGED <<durable, acked>>

Recover ==
    /\ ~alive
    /\ visible' = durable
    /\ alive' = TRUE
    /\ UNCHANGED <<durable, acked>>

Next == (\E w \in Writers : Claim(w)) \/ Crash \/ Recover
Spec == Init /\ [][Next]_vars

NoAcknowledgedStrikeIsLost ==
    \A w \in acked : durable[Key(w)] = w

RecoveredReadsTheDurableAnswer ==
    alive => visible = durable

TypeOK ==
    /\ durable \in [Keys -> Writers \cup {Empty}]
    /\ visible \in [Keys -> Writers \cup {Empty}]
    /\ acked \subseteq Writers
    /\ alive \in BOOLEAN

=============================================================================
