--------------------------- MODULE ReliefEngine ---------------------------
(***************************************************************************)
(* Sales relieved against lots, while corporate actions keep arriving.      *)
(*                                                                          *)
(* `Ratio.Lots.Relief` proves what ONE relief computes: the gain, the        *)
(* conservation, and the conversion from today's units into the stored ones. *)
(* `//tla:lot_engine_check` proves no lot is relieved twice. Neither says    *)
(* anything about a relief that spans time, and a relief does:               *)
(*                                                                          *)
(*   1. read the position and the splits in force                            *)
(*   2. convert the sale into stored units                                   *)
(*   3. choose lots and write the relief                                     *)
(*                                                                          *)
(* ⛔ AND AN ANNOUNCEMENT CAN LAND BETWEEN 1 AND 3. Under the factor         *)
(* representation nothing is rewritten, so a split changes what the stored   *)
(* units MEAN without touching a single lot. A relief that converts against  *)
(* the factor it read and writes against the factor that is current has      *)
(* taken the wrong number of stored units — and the books tie, because the   *)
(* cost it relieved is the cost of the lots it took.                         *)
(*                                                                          *)
(* That is the same shape as `//tla:projection_check`: a figure computed     *)
(* against one state and recorded against another. It is worth a separate    *)
(* spec because the RESOLUTION is different. A strike pins a prefix and      *)
(* lives with being slightly behind. A relief cannot — it is a WRITE, and    *)
(* writing a relief computed against a stale factor puts the wrong number of *)
(* units into the book permanently.                                          *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Sales,
    MaxSplits,
    PinTheFactor,   \* the fix. FALSE converts on read and writes on current.
    RefuseOnChange  \* the fix. FALSE writes anyway when the factor moved.

VARIABLES
    splits,     \* how many splits have been announced. The factor, abstracted.
    readAt,     \* [Sales -> Nat] the factor each sale converted against
    stage,      \* [Sales -> {"new","converted","written"}]
    wroteAt,    \* [Sales -> Nat] the factor in force when the relief landed
    refused,    \* SUBSET Sales — sent back to be recomputed
    lastOp

vars == <<splits, readAt, stage, wroteAt, refused, lastOp>>

NotRead == 99   \* a marker, distinguishable from any split count

----------------------------------------------------------------------------

Init ==
    /\ splits = 0
    /\ readAt = [s \in Sales |-> NotRead]
    /\ stage = [s \in Sales |-> "new"]
    /\ wroteAt = [s \in Sales |-> NotRead]
    /\ refused = {}
    /\ lastOp = "init"

(* A corporate action is announced. Nothing is rewritten — that is the whole
   design — so this changes what every stored unit MEANS and touches no lot. *)
Announce ==
    /\ splits < MaxSplits
    /\ splits' = splits + 1
    /\ lastOp' = "announce"
    /\ UNCHANGED <<readAt, stage, wroteAt, refused>>

(* Read the position and convert the sale into stored units. *)
Convert(s) ==
    /\ stage[s] = "new"
    /\ readAt' = [readAt EXCEPT ![s] = splits]
    /\ stage' = [stage EXCEPT ![s] = "converted"]
    /\ lastOp' = "convert"
    /\ UNCHANGED <<splits, wroteAt, refused>>

(* Write the relief.
   ⛔ THE CHECK IS THE WHOLE THING. If the factor moved between converting and
   writing, the stored-unit count this sale computed is no longer the right one
   — and nothing downstream can tell, because the relief is internally
   consistent with the lots it took. *)
Write(s) ==
    /\ stage[s] = "converted"
    /\ IF RefuseOnChange /\ readAt[s] # splits
         THEN \* Send it back. The recomputation is cheap; the wrong answer is
              \* permanent.
              /\ stage' = [stage EXCEPT ![s] = "new"]
              /\ readAt' = [readAt EXCEPT ![s] = NotRead]
              /\ refused' = refused \cup {s}
              /\ UNCHANGED wroteAt
         ELSE /\ stage' = [stage EXCEPT ![s] = "written"]
              /\ wroteAt' = [wroteAt EXCEPT ![s] =
                   IF PinTheFactor THEN readAt[s] ELSE splits]
              /\ UNCHANGED <<readAt, refused>>
    /\ lastOp' = "write"
    /\ UNCHANGED splits

Settled == \A s \in Sales : stage[s] = "written"

Next ==
    \/ Announce
    \/ \E s \in Sales : Convert(s)
    \/ \E s \in Sales : Write(s)
    \/ (Settled /\ UNCHANGED vars)

Spec == Init /\ [][Next]_vars /\ WF_vars(\E s \in Sales : Convert(s))
                              /\ WF_vars(\E s \in Sales : Write(s))

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(***************************************************************************)
(* 1. ⭐ A RELIEF IS WRITTEN AGAINST THE FACTOR IT CONVERTED WITH.          *)
(*                                                                         *)
(*    The one that matters, and the one nothing downstream can check. A     *)
(*    relief computed against three splits and written against four takes   *)
(*    half the stored units it should — and the entry BALANCES, because the *)
(*    cost it relieved is the cost of the lots it actually took.            *)
(*    `Ratio.Lots.cost_is_conserved` holds of the wrong answer.             *)
(*                                                                         *)
(*    ⚠ THIS IS A WRITE, WHICH IS WHY IT CANNOT BE HANDLED THE WAY A STRIKE *)
(*    IS. `//tla:projection_check` lets a NAV be slightly behind and cite   *)
(*    the prefix it read. A relief that is slightly behind puts the wrong   *)
(*    number of units into the book, permanently.                          *)
(***************************************************************************)
ReliefIsWrittenAgainstWhatItRead ==
    \A s \in Sales : stage[s] = "written" => wroteAt[s] = readAt[s]

(***************************************************************************)
(* 2. NOTHING IS WRITTEN THAT WAS NEVER CONVERTED.                         *)
(*                                                                         *)
(*    The direction that would otherwise be assumed rather than checked.    *)
(***************************************************************************)
WrittenMeansConverted ==
    \A s \in Sales : stage[s] = "written" => readAt[s] # NotRead

(***************************************************************************)
(* 3. AND A REFUSAL IS NOT A LOSS.                                         *)
(*                                                                         *)
(*    Sending a relief back to be recomputed must not drop it. Recomputing  *)
(*    is cheap — the conversion is O(splits) on one instrument, which is    *)
(*    the entire point of the factor representation. Refusing to write is   *)
(*    only safe if the sale comes back.                                    *)
(***************************************************************************)
EverySaleIsEventuallyWritten == \A s \in Sales : <>(stage[s] = "written")

TypeOK ==
    /\ splits \in 0..MaxSplits
    /\ \A s \in Sales : stage[s] \in {"new", "converted", "written"}

=============================================================================
