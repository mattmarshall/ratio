---------------------------- MODULE SeedExtension ----------------------------
(***************************************************************************)
(* Extending an already-published demo seed, once, by appending lines.    *)
(*                                                                         *)
(* A deployment publishes each baked seed book to the durable store: it   *)
(* claims every plane's slots with put_if_absent, then writes an          *)
(* immutable marker naming the seed's digest. A later image whose seed    *)
(* differs is refused — "refusing to alter the existing book" — and the   *)
(* store has no overwrite to change a marker with.                        *)
(*                                                                         *)
(* That refusal blocked a correction. Two seeded funds were struck over   *)
(* 250 VWRL that nothing priced; pricing it adds one fact to their seeds, *)
(* and the published marker forbids it. The `appended-price-facts-v1`     *)
(* migration lets a deploy append lines to a published plane and publish  *)
(* a SECOND, immutable extension marker naming what it extends.           *)
(*                                                                         *)
(* What can go wrong is a marker describing bytes the store does not      *)
(* hold — and a serving process trusts the marker, not the bytes:         *)
(*                                                                         *)
(*   * the marker written before the appended slots, then a crash;        *)
(*   * an appended slot a demo user had already filled at runtime,        *)
(*     skipped instead of compared;                                       *)
(*   * an image that rewrites a published line passed off as an          *)
(*     extension, because only lengths were compared.                     *)
(*                                                                         *)
(* One plane stands for each: facts and deliveries are handled            *)
(* identically, and every other plane must not change at all.             *)
(***************************************************************************)
EXTENDS Naturals, Sequences

CONSTANTS
    CheckPrefix,        \* the fix: the image's seed must begin with the published one
    ValidateOccupied,   \* the fix: an occupied appended slot must hold the baked line
    SlotsBeforeMarker   \* the fix: claim every appended slot before the marker

\* The published seed, the seeds a deploying image may carry, and a line a demo
\* user may append after publication. `<<"a">>` is the old image (a rerun of
\* the old deploy), `<<"a", "x">>` and `<<"a", "x", "y">>` extend it, and
\* `<<"b", "x">>` rewrites a published line and must never be accepted.
Base == <<"a">>
Images == {<<"a">>, <<"a", "x">>, <<"a", "x", "y">>, <<"b", "x">>}
RuntimeLine == "u"
Lines == {"a", "b", "x", "y", "u"}

\* Two deploys may run at once: a rerun while the first is still claiming, or
\* the old image's deploy racing the new one.
Publishers == {"p1", "p2"}

VARIABLES
    durable,    \* the durable plane; a slot, once claimed, never changes
    hasExt,     \* whether the extension marker exists
    ext,        \* the seed the extension marker describes
    image,      \* the seed each deploy's image carries
    pc,         \* each deploying publisher's step
    next,       \* the next slot each will claim or compare
    userWrote   \* whether the runtime line has been appended (once is enough)

vars == <<durable, hasExt, ext, image, pc, next, userWrote>>

IsPrefix(p, s) == Len(p) =< Len(s) /\ SubSeq(s, 1, Len(p)) = p

Steps == {"start", "claim", "mark", "markfirst", "done", "refused"}

TypeOK ==
    /\ durable \in Seq(Lines)
    /\ hasExt \in BOOLEAN
    /\ ext \in Seq(Lines)
    /\ image \in [Publishers -> Images]
    /\ pc \in [Publishers -> Steps]
    /\ next \in [Publishers -> Nat]
    /\ userWrote \in BOOLEAN

Init ==
    /\ durable = Base           \* published by an earlier deploy
    /\ hasExt = FALSE
    /\ ext = <<>>
    /\ image \in [Publishers -> Images]
    /\ pc = [p \in Publishers |-> "start"]
    /\ next = [p \in Publishers |-> 0]
    /\ userWrote = FALSE

(* A demo user ingests something into the published book: the next slot. *)
RuntimeAppend ==
    /\ ~userWrote
    /\ durable' = Append(durable, RuntimeLine)
    /\ userWrote' = TRUE
    /\ UNCHANGED <<hasExt, ext, image, pc, next>>

Go(p, step) == pc' = [pc EXCEPT ![p] = step]
At(p, n) == next' = [next EXCEPT ![p] = n]

(* `publish_seed`: a seed that is already published, as the base or as the
   extension, is a no-op; a different extension already published is refused,
   because the migration is one-time. *)
Start(p) ==
    /\ pc[p] = "start"
    /\ IF image[p] = Base \/ (hasExt /\ ext = image[p])
         THEN Go(p, "done") /\ UNCHANGED next
       ELSE IF hasExt
         THEN Go(p, "refused") /\ UNCHANGED next
       ELSE IF CheckPrefix /\ ~IsPrefix(Base, image[p])
         THEN Go(p, "refused") /\ UNCHANGED next
       ELSE IF SlotsBeforeMarker
         THEN Go(p, "claim") /\ At(p, Len(Base) + 1)
       ELSE Go(p, "markfirst") /\ UNCHANGED next
    /\ UNCHANGED <<durable, hasExt, ext, image, userWrote>>

(* Claim the appended slots in order. put_if_absent at the next free slot is
   Append; a slot somebody else already holds — a user, or the other deploy —
   is compared, or (the bug) skipped. *)
Claim(p) ==
    /\ pc[p] = "claim"
    /\ LET n == next[p] IN
       IF n > Len(image[p])
         THEN Go(p, "mark") /\ UNCHANGED <<durable, next>>
       ELSE IF n =< Len(durable)
         THEN IF ValidateOccupied /\ durable[n] # image[p][n]
                THEN Go(p, "refused") /\ UNCHANGED <<durable, next>>
                ELSE At(p, n + 1) /\ UNCHANGED <<durable, pc>>
       ELSE /\ durable' = Append(durable, image[p][n])
            /\ At(p, n + 1)
            /\ UNCHANGED pc
    /\ UNCHANGED <<hasExt, ext, image, userWrote>>

(* Conditionally create the extension marker. Losing the race to an equal
   marker is success; to a different one, refusal. *)
Mark(p) ==
    /\ pc[p] = "mark"
    /\ IF ~hasExt
         THEN hasExt' = TRUE /\ ext' = image[p] /\ Go(p, "done")
       ELSE Go(p, IF ext = image[p] THEN "done" ELSE "refused")
            /\ UNCHANGED <<hasExt, ext>>
    /\ UNCHANGED <<durable, image, next, userWrote>>

(* The bug SlotsBeforeMarker forbids: expose the marker, then claim. *)
MarkFirst(p) ==
    /\ pc[p] = "markfirst"
    /\ IF ~hasExt
         THEN hasExt' = TRUE /\ ext' = image[p]
         ELSE UNCHANGED <<hasExt, ext>>
    /\ Go(p, "claim")
    /\ At(p, Len(Base) + 1)
    /\ UNCHANGED <<durable, image, userWrote>>

(* A deploy dies at any step and is rerun from the top with the same image. *)
Crash(p) ==
    /\ pc[p] \notin {"done", "refused"}
    /\ Go(p, "start")
    /\ At(p, 0)
    /\ UNCHANGED <<durable, hasExt, ext, image, userWrote>>

Finished ==
    /\ \A p \in Publishers : pc[p] \in {"done", "refused"}
    /\ UNCHANGED vars

Next ==
    \/ RuntimeAppend
    \/ \E p \in Publishers :
         Start(p) \/ Claim(p) \/ Mark(p) \/ MarkFirst(p) \/ Crash(p)
    \/ Finished

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

(* What serving startup accepts: an image whose seed is the published base or
   the published extension. *)
Starts(img) == img = Base \/ (hasExt /\ ext = img)

(***************************************************************************)
(* 1. AN IMAGE THAT STARTS FINDS ITS SEED IN THE STORE.                    *)
(*                                                                         *)
(*    `verify_seed_publication` compares markers, never bytes. So whatever *)
(*    a marker it accepts describes must already be durable, line for     *)
(*    line. Violated, a fund serves figures folded from lines that are    *)
(*    not there, or from a user's line where the seed's should be.        *)
(***************************************************************************)
AnImageThatStartsFindsItsSeed ==
    \A img \in Images : Starts(img) => IsPrefix(img, durable)

(***************************************************************************)
(* 2. THE EXTENSION EXTENDS THE PUBLISHED SEED.                            *)
(*                                                                         *)
(*    An extension may only add lines. Violated, a marker says a published *)
(*    line is something it never was.                                     *)
(***************************************************************************)
ExtensionExtendsBase ==
    hasExt => IsPrefix(Base, ext)

=============================================================================
