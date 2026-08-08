-------------------------- MODULE ControlPlane --------------------------
(***************************************************************************)
(* Ratio's control plane and data plane, as a state machine.               *)
(*                                                                         *)
(* WHY THIS EXISTS, AND WHY THE LEAN PROOFS WERE NOT ENOUGH.               *)
(*                                                                         *)
(* `Ratio.Ingest` proves things about FUNCTIONS: given a master and a rung,*)
(* what resolution produces; given a decimal, what parsing produces. Those *)
(* proofs were correct and STAYED correct while the system was broken —    *)
(* approving a rule deleted the templates beside it, so there were no      *)
(* facts left for the theorems to be about, and every one of them held     *)
(* vacuously.                                                              *)
(*                                                                         *)
(* A proof constrains the function it is about. It says nothing about      *)
(* whether that function is reached, or with what. What was missing is a   *)
(* model of the OPERATIONS and what a sequence of them does to shared      *)
(* state. That is what TLC checks and what Lean, as used here, does not.   *)
(*                                                                         *)
(* Abstracted to what the invariants turn on: a configuration is a set of  *)
(* section names, an entity is an id and a claim, a fact is a reference    *)
(* and a claim. Every further detail is state space bought with nothing.   *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS
    Sections,                   \* {"rule", "template"} — what a config holds
    EntityIds,                  \* the master's ids
    Claims,                     \* an ISIN, a ticker, a broker code
    Refs,                       \* a delivered row's own reference
    NoClaim,                    \* the absence of one (a model value)
    ApproveKeepsOtherSections,  \* the fix. FALSE reproduces the bug.
    MasterSupersedesById        \* the fix. FALSE appends duplicates instead.

VARIABLES
    config,        \* SUBSET Sections — the sections the active config holds
    master,        \* [EntityIds -> Claims \cup {NoClaim}] — current record
    stale,         \* [EntityIds -> Claims \cup {NoClaim}] — a superseded one
    facts,         \* [Refs -> Claims \cup {NoClaim}] — read off delivered files
    posted,        \* SUBSET Refs — facts that reached the journal
    lastOp         \* which verb just ran, so properties can name it

vars == <<config, master, stale, facts, posted, lastOp>>

----------------------------------------------------------------------------
(* Resolution, mirroring `Ratio.Ingest.resolveRung`: exactly one match      *)
(* resolves, none is absent, MORE THAN ONE IS AMBIGUOUS AND BLOCKS.         *)
(*                                                                         *)
(* `stale` is what an append-only master looks like WITHOUT superseding: a  *)
(* corrected entity leaves its old record behind, and both still match.     *)

RecordCount(c) ==
    Cardinality({i \in EntityIds : master[i] = c})
      + (IF MasterSupersedesById
           THEN 0
           ELSE Cardinality({i \in EntityIds : stale[i] = c}))

Resolves(r) ==
    /\ facts[r] # NoClaim
    /\ RecordCount(facts[r]) = 1

Known == {r \in Refs : facts[r] # NoClaim}

(* What the console shows as blocking.
   ⛔ A FACT ALREADY IN THE JOURNAL IS NOT PENDING. Its entry was written
   under the resolution in force at the time and is immutable; re-resolving it
   afterwards asks a question that has already been answered. Leaving posted
   facts in this set is the bug TLC found — see PendingExcludesPosted. *)
Pending == {r \in Known : r \notin posted /\ ~Resolves(r)}

----------------------------------------------------------------------------
(* The operations. Each is one CLI verb.                                   *)

Init ==
    /\ config = {}
    /\ master = [i \in EntityIds |-> NoClaim]
    /\ stale  = [i \in EntityIds |-> NoClaim]
    /\ facts  = [r \in Refs |-> NoClaim]
    /\ posted = {}
    /\ lastOp = "init"

(* `ratio config set` — replaces the whole configuration, on purpose. *)
ConfigSet(s) ==
    /\ config' = s
    /\ lastOp' = "configSet"
    /\ UNCHANGED <<master, stale, facts, posted>>

(* `ratio approve` — merges one rule into the configuration in force.
   ⛔ THE BUG: it used to write back a document derived only from the rules,
   so every other section went with it. *)
Approve ==
    /\ config' = IF ApproveKeepsOtherSections
                   THEN config \cup {"rule"}
                   ELSE {"rule"}
    /\ lastOp' = "approve"
    /\ UNCHANGED <<master, stale, facts, posted>>

(* `ratio ingest` — reads a file into facts. Needs a template IN FORCE, so a
   configuration that lost its templates produces nothing. *)
Ingest(r, c) ==
    /\ "template" \in config
    /\ facts[r] = NoClaim
    /\ facts' = [facts EXCEPT ![r] = c]
    /\ lastOp' = "ingest"
    /\ UNCHANGED <<config, master, stale, posted>>

(* `ratio entity add` — puts something in the master, or corrects it.
   With superseding, the latest record for an id is the current one. Without,
   the old record is still there and still matches. *)
EntityAdd(i, c) ==
    /\ master' = [master EXCEPT ![i] = c]
    /\ stale' = [stale EXCEPT ![i] = master[i]]
    /\ lastOp' = "entityAdd"
    /\ UNCHANGED <<config, facts, posted>>

(* `ratio admit` — posts every fact that fully resolves. *)
Admit(r) ==
    /\ r \in Known
    /\ Resolves(r)
    /\ r \notin posted
    /\ posted' = posted \cup {r}
    /\ lastOp' = "admit"
    /\ UNCHANGED <<config, master, stale, facts>>

Next ==
    \/ \E s \in SUBSET Sections : ConfigSet(s)
    \/ Approve
    \/ \E r \in Refs, c \in Claims : Ingest(r, c)
    \/ \E i \in EntityIds, c \in Claims : EntityAdd(i, c)
    \/ \E r \in Refs : Admit(r)

Spec == Init /\ [][Next]_vars

----------------------------------------------------------------------------
(* THE PROPERTIES.                                                         *)

TypeOK ==
    /\ config \subseteq Sections
    /\ master \in [EntityIds -> Claims \cup {NoClaim}]
    /\ stale \in [EntityIds -> Claims \cup {NoClaim}]
    /\ facts \in [Refs -> Claims \cup {NoClaim}]
    /\ posted \subseteq Refs

(***************************************************************************)
(* 1. APPROVING A RULE DOES NOT DISTURB THE SECTIONS BESIDE IT.            *)
(*                                                                         *)
(*    The bug that shipped, as an action property. It is about a SEQUENCE  *)
(*    — set a configuration holding templates, then approve — which is why *)
(*    no theorem about resolution could have caught it.                    *)
(***************************************************************************)
ApprovePreservesSections ==
    [][ (lastOp' = "approve") => (\A s \in config : s \in config') ]_vars

(***************************************************************************)
(* 2. A FACT IN THE JOURNAL IS NEVER REPORTED PENDING.                     *)
(*                                                                         *)
(*    ⛔ TLC FOUND A REAL BUG HERE, AND FIRST IT REJECTED MY INVARIANT.     *)
(*                                                                         *)
(*    I wrote `PostedStaysResolvable` — every posted fact still resolves —  *)
(*    and TLC produced this, in six states:                                *)
(*                                                                         *)
(*        ingest r1 with claim c1                                          *)
(*        entityAdd i1 -> c1        (now exactly one match)                *)
(*        admit r1                  (posted, entry written)                *)
(*        entityAdd i2 -> c1        (a DIFFERENT instrument, same           *)
(*                                   identifier: two matches, ambiguous)   *)
(*                                                                         *)
(*    The invariant was wrong, and being wrong was the useful part. A       *)
(*    posted fact does not need to keep resolving: its journal entry was    *)
(*    written under the resolution in force at the time and is immutable.   *)
(*    What must not happen is the console reporting it as PENDING again —   *)
(*    which is exactly what the code I had just written would have done,    *)
(*    because it resolved every fact and filtered on admissibility with no  *)
(*    notion of "already in the journal". A duplicate identifier arriving   *)
(*    weeks later would have re-blocked a NAV that was already struck.      *)
(*                                                                         *)
(*    `resolved_never_becomes_absent` in Lean is still true and still the   *)
(*    right theorem about the function. It allows resolved -> AMBIGUOUS by  *)
(*    design. This is the system-level consequence of that allowance, and   *)
(*    no theorem about resolution could have shown it.                      *)
(***************************************************************************)
PendingExcludesPosted ==
    \A r \in posted : r \notin Pending

(***************************************************************************)
(* 2b. Admission is monotone: the journal is append-only, so a fact that    *)
(*     posted stays posted no matter what the master does afterwards.       *)
(***************************************************************************)
PostedNeverRetracted ==
    [][ \A r \in posted : r \in posted' ]_vars

(***************************************************************************)
(* 2c. RESOLUTION GOES THROUGH THE CURRENT MASTER, NOT A CORRECTED-AWAY    *)
(*     RECORD.                                                             *)
(*                                                                         *)
(*     An append-only master keeps every record ever written. If matching   *)
(*     looks at all of them, then correcting an instrument's ISIN — which   *)
(*     someone does BECAUSE IT WAS WRONG — leaves the wrong one matching    *)
(*     forever, and a correction that does not take effect is worse than no *)
(*     correction, because it looks like one.                               *)
(*                                                                         *)
(*     So the master is append-only on disk and superseding in meaning: the *)
(*     latest record for an id is the record. The `MasterDuplicates` probe  *)
(*     turns that off and this goes red.                                    *)
(***************************************************************************)
ResolutionUsesCurrentRecords ==
    \A r \in Known :
        Resolves(r) => (\E i \in EntityIds : master[i] = facts[r])

(***************************************************************************)
(* 3. Nothing reaches the journal that was never read off a file.          *)
(***************************************************************************)
PostedWasRead ==
    posted \subseteq Known

(***************************************************************************)
(* 4. A fact outlives the configuration that read it.                      *)
(*                                                                         *)
(*    Facts are append-only and carry their own provenance, so changing the *)
(*    configuration afterwards must not erase them — otherwise a period     *)
(*    could not span a rule change.                                         *)
(***************************************************************************)
FactsOutliveConfig ==
    [][ (lastOp' \in {"approve", "configSet"}) => (Known \subseteq Known') ]_vars

=============================================================================
