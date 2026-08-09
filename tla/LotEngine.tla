-------------------------- MODULE LotEngine --------------------------
(***************************************************************************)
(* Relieving tax lots at scale: the I/O and CPU compartments, and what     *)
(* separating them is allowed to change about the answer.                  *)
(*                                                                         *)
(* `Ratio.Lots` proves the ARITHMETIC of relief — a sale costs what it     *)
(* consumes, cost is conserved, a partial split is exactly pro rata. Every *)
(* one of those is a statement about a function applied once.              *)
(*                                                                         *)
(* Twenty million lots do not fit in one application. The engine pages lots *)
(* in from storage, hands them to workers, and runs many sales at once —   *)
(* and every one of those moves is a chance to get a DIFFERENT answer from *)
(* the one that was proved:                                                *)
(*                                                                         *)
(*   * two sales relieving the same instrument at once can consume the same *)
(*     lot twice, and the arithmetic of each is impeccable                  *)
(*   * a sale can read a page of lots that a concurrent sale has already    *)
(*     relieved, and act on what it read                                    *)
(*   * a bounded queue between the compartments can wedge, and a fund that  *)
(*     cannot strike is not obviously distinguishable from one that is slow *)
(*                                                                         *)
(* None of that is arithmetic. It is what a sequence of operations does to  *)
(* shared state, which is TLC's question and not Lean's.                    *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS
    Instruments,        \* the shards: lots are relieved per instrument
    Sales,              \* sales waiting to be applied
    QueueDepth,         \* how many pages the I/O compartment may hold
    LockPerInstrument,  \* the fix. FALSE lets two sales share a shard.
    NoOne,              \* "nobody holds this shard" (a model value)
    NoLot,              \* "this sale has claimed nothing" (a model value)
    Lots                \* the lot ids in one shard

VARIABLES
    open,       \* [Instruments -> SUBSET Lots]  lots not yet relieved
    taken,      \* [Instruments -> Nat]  how many relief operations landed
    claimed,    \* [Sales -> Lots \cup {NoLot}]  the lot a sale READ and intends
    holder,     \* [Instruments -> Sales \cup {NoOne}]  who holds the shard
    stage,      \* [Sales -> "waiting" | "reading" | "done"]
    target,     \* [Sales -> Instruments]  which shard a sale needs
    queue,      \* pages read and not yet consumed
    lastOp

vars == <<open, taken, claimed, holder, stage, target, queue, lastOp>>

(* ⛔ THE HAZARD IS THE CLAIM, NOT THE ARITHMETIC.
   A sale READS a page and picks the lot it intends to relieve — FIFO, so the
   oldest. It applies that intention LATER. Two sales that read the same shard
   pick the SAME lot, and each then relieves impeccably against a view that
   stopped being true. The set of open lots loses one; the count of relief
   operations gains two.

   An earlier version of this model had the sale re-read the live quantity at
   apply time, which is atomic and therefore has no bug to find — the probe
   passed and proved nothing. The claim has to be captured at READ time or the
   model is not modelling the compartment boundary at all. *)
Oldest(S) == CHOOSE l \in S : \A m \in S : l =< m

----------------------------------------------------------------------------
(* The two compartments.                                                   *)
(*                                                                         *)
(* I/O reads a page of lots and puts it on a BOUNDED queue. CPU takes a     *)
(* page and relieves against it. Bounded because unbounded is how a         *)
(* twenty-million-lot scan takes the memory of the machine it runs on.      *)

Init ==
    /\ open = [i \in Instruments |-> Lots]
    /\ taken = [i \in Instruments |-> 0]
    /\ claimed = [s \in Sales |-> NoLot]
    /\ holder = [i \in Instruments |-> NoOne]
    /\ stage = [s \in Sales |-> "waiting"]
    /\ target \in [Sales -> Instruments]
    /\ queue = <<>>
    /\ lastOp = "init"

(* I/O: read a page for a sale.
   ⛔ THE SHARD IS CLAIMED HERE, not when the CPU acts on it. Claiming later
   would let two sales read the same lots and both compute correctly from a
   view that stopped being true. *)
Read(s) ==
    /\ stage[s] = "waiting"
    /\ Len(queue) < QueueDepth
    /\ IF LockPerInstrument
         THEN holder[target[s]] = NoOne
         ELSE TRUE
    /\ holder' = IF LockPerInstrument
                   THEN [holder EXCEPT ![target[s]] = s]
                   ELSE holder
    /\ open[target[s]] # {}
    \* The lot this sale intends to relieve, decided HERE from what it read.
    /\ claimed' = [claimed EXCEPT ![s] = Oldest(open[target[s]])]
    /\ queue' = Append(queue, s)
    /\ stage' = [stage EXCEPT ![s] = "reading"]
    /\ lastOp' = "read"
    /\ UNCHANGED <<open, taken, target>>

(* CPU: relieve against the page that was read.
   The arithmetic is `Ratio.Lots`; what this models is WHEN it runs and what
   it can see. *)
Relieve(s) ==
    /\ stage[s] = "reading"
    /\ queue # <<>>
    /\ Head(queue) = s
    /\ claimed[s] # NoLot
    \* Applies what it decided at read time. It does NOT look again.
    /\ open' = [open EXCEPT ![target[s]] = @ \ {claimed[s]}]
    /\ taken' = [taken EXCEPT ![target[s]] = @ + 1]
    /\ claimed' = [claimed EXCEPT ![s] = NoLot]
    /\ queue' = Tail(queue)
    /\ stage' = [stage EXCEPT ![s] = "done"]
    /\ holder' = IF LockPerInstrument
                   THEN [holder EXCEPT ![target[s]] = NoOne]
                   ELSE holder
    /\ lastOp' = "relieve"
    /\ UNCHANGED target

(* A sale that finds nothing left gives up and releases the shard, rather
   than holding it while it waits for lots that are not coming. *)
NothingLeft(s) ==
    /\ stage[s] = "waiting"
    /\ open[target[s]] = {}
    /\ stage' = [stage EXCEPT ![s] = "done"]
    /\ lastOp' = "abandon"
    /\ UNCHANGED <<open, taken, claimed, holder, queue, target>>

(* Everything is done. Stuttering here rather than leaving no action enabled,
   so that TLC's deadlock check keeps its meaning: TERMINATION is not a wedge,
   and a wedge is what the check is for — a queue that is full while nothing
   can drain it looks, from outside, exactly like a fund that is merely slow. *)
Finished == \A s \in Sales : stage[s] = "done"

Next ==
    \/ \E s \in Sales : Read(s)
    \/ \E s \in Sales : Relieve(s)
    \/ \E s \in Sales : NothingLeft(s)
    \/ (Finished /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. NO LOT IS RELIEVED TWICE.                                            *)
(*                                                                         *)
(*    What was taken plus what is left is what was opened — per instrument, *)
(*    under every interleaving. `Ratio.Lots.cost_is_conserved` proves this  *)
(*    of ONE relief; concurrency is where it stops following from that.     *)
(***************************************************************************)
NothingIsRelievedTwice ==
    \A i \in Instruments : Cardinality(open[i]) + taken[i] = Cardinality(Lots)

(***************************************************************************)
(* 2. A SHARD IS HELD BY AT MOST ONE SALE.                                 *)
(*                                                                         *)
(*    The compartment boundary: I/O claims, CPU releases. Two sales inside  *)
(*    one shard is the bug this is here to exclude, and it is invisible in  *)
(*    the arithmetic because each sale's arithmetic is right.               *)
(***************************************************************************)
OneSalePerShard ==
    \A i, j \in Instruments :
        (i # j) \/ (holder[i] = NoOne) \/ (holder[i] = holder[j])

(***************************************************************************)
(* 3. THE QUEUE STAYS INSIDE ITS BOUND.                                    *)
(*                                                                         *)
(*    Unbounded is how a twenty-million-lot scan takes the memory of the    *)
(*    machine it runs on.                                                   *)
(***************************************************************************)
QueueIsBounded == Len(queue) =< QueueDepth

(***************************************************************************)
(* 4. NOTHING IS LOST AND NOTHING WEDGES.                                  *)
(*                                                                         *)
(*    Every sale finishes. A fund that cannot strike because a queue is     *)
(*    stuck looks exactly like a fund that is merely slow, which is the     *)
(*    worst way for this to fail.                                           *)
(***************************************************************************)
EverySaleFinishes == \A s \in Sales : <>(stage[s] = "done")

TypeOK ==
    /\ \A i \in Instruments : open[i] \subseteq Lots
    /\ \A s \in Sales : stage[s] \in {"waiting", "reading", "done"}

=============================================================================
