set_option warningAsError true
/-! `Ratio.Relational` — the first increment of a query planner, and the traps
it has to survive.

⚠ THIS BELONGS IN LEANGRES, NOT IN RATIO. It is developed here because
`rules-postgres` is outside this session's working directories; the namespace is
the only thing about it that is ratio's, and it should move.

WHAT LEANGRES HAS, having read it: an AST, a catalog, a type system, a PL/pgSQL
surface, an IR of datums and comparisons, and `Query/Top.lean` — which is
top-level DDL statements (CREATE SCHEMA, CREATE TABLE, CREATE VIEW, ALTER
TABLE), not relational algebra. There is no plan node, no cost, no join, and —
the thing that actually blocks everything — NO DENOTATION. Nothing says what a
query MEANS. Until something does, "this plan equals that plan" is not a
statement that can be made, let alone proved.

So this is denotation first, and it deliberately walks into the three places a
naive model gives FALSE theorems:

  BAGS, NOT SETS      SQL keeps duplicates. A model over sets proves
                      `union_idempotent`, which is true of sets and false of
                      SQL, and every rewrite built on it is wrong.
  THREE-VALUED LOGIC  a predicate is true, false, or UNKNOWN, and `WHERE` keeps
                      only TRUE. This is where the interesting bugs live.
  ⛔ SO PREDICATES DO NOT PARTITION. `filter p` and `filter (not p)` do not
                      reassemble the table when a NULL is present — which
                      contradicts the shape `Ratio.Lots.partition_sums_to_whole`
                      and `Ratio.Ingest.split_preserves_total` rely on, both of
                      which are stated over BOOLEAN predicates and are fine.
                      Carrying that habit into SQL would be a fast wrong answer.

Lean core, no Mathlib. -/

namespace Ratio.Relational

/-- A value. `null` is not a number and is not comparable to one. -/
inductive Val where
  | null
  | num (n : Int)
deriving DecidableEq, Repr

/-- A row is positional; a table is a BAG of rows, carried as a list.

A list rather than a set because SQL keeps duplicates, and rather than a
quotient because the rewrites below preserve order anyway and a quotient would
buy proof obligations without buying truth. -/
abbrev Row := List Val
abbrev Table := List Row

/-- SQL's three-valued logic. -/
inductive Three where
  | yes
  | no
  | unknown
deriving DecidableEq, Repr

/-- A predicate over a row, by column position. -/
inductive Pred where
  | isNull (col : Nat)
  | eqNum (col : Nat) (n : Int)
  | and (a b : Pred)
  | not (a : Pred)
deriving DecidableEq, Repr

def cell (r : Row) (i : Nat) : Val := r.getD i .null

/-- ⛔ COMPARING WITH NULL IS UNKNOWN, NOT FALSE. That single line is what makes
the partition theorem below fail, and it is the rule real planners get wrong. -/
def evalPred : Pred → Row → Three
  | .isNull i, r => match cell r i with
    | .null => .yes
    | .num _ => .no
  | .eqNum i n, r => match cell r i with
    | .null => .unknown
    | .num m => if m = n then .yes else .no
  | .and a b, r => match evalPred a r, evalPred b r with
    | .no, _ => .no
    | _, .no => .no
    | .yes, .yes => .yes
    | _, _ => .unknown
  | .not a, r => match evalPred a r with
    | .yes => .no
    | .no => .yes
    | .unknown => .unknown

/-- Plan nodes. Enough to state the first rewrites; joins are the next
increment and are where outer-join pushdown unsoundness lives. -/
inductive Plan where
  | scan (t : String)
  | filter (p : Pred) (child : Plan)
  | union (a b : Plan)
deriving DecidableEq, Repr

/-- What a plan MEANS, against a database that maps a name to a bag of rows.

⛔ `WHERE` KEEPS ONLY `yes`. Not "not no" — a row whose predicate is unknown is
dropped, and that asymmetry is the whole of the trouble below. -/
def denote (db : String → Table) : Plan → Table
  | .scan t => db t
  | .filter p c => (denote db c).filter (fun r => evalPred p r == Three.yes)
  | .union a b => denote db a ++ denote db b

/-- Two plans are equivalent when they mean the same thing on every database.

This is the relation a planner is CORRECT with respect to. Everything a planner
does — reorder, push down, pick an index — has to be an instance of it. -/
def Equiv (p q : Plan) : Prop := ∀ db, denote db p = denote db q

infix:50 " ≡ " => Equiv

/- ── Rewrites that hold ────────────────────────────────────────────────── -/

/-- Filtering is idempotent-composable: two filters are one `and`… almost.
Stated as the composition rather than assumed. -/
theorem filter_over_union (p : Pred) (a b : Plan) :
    Plan.filter p (.union a b) ≡ .union (.filter p a) (.filter p b) := by
  intro db
  simp [denote, List.filter_append]

/-- Order of independent filters does not matter. -/
theorem filters_commute (p q : Pred) (c : Plan) :
    Plan.filter p (.filter q c) ≡ .filter q (.filter p c) := by
  intro db
  simp [denote]
  congr 1
  funext r
  exact Bool.and_comm _ _

/- ── The trap ──────────────────────────────────────────────────────────── -/

/-- **A predicate and its negation do not reassemble the table.**

⛔ THE THEOREM THAT MUST BE STATED BEFORE ANY REWRITE IS BUILT. In ratio,
`split_preserves_total` and `partition_sums_to_whole` say that filtering by a
predicate and by its complement gives the whole back — and they are true, of
BOOLEAN predicates over data with no nulls.

SQL predicates are not boolean. A row whose predicate is UNKNOWN is kept by
neither side, so the two halves are missing it. A planner that carries the
partition habit across writes an optimisation that silently drops every row
with a null in the compared column — which is the most common shape of
production data there is.

The witness is one row and one null. -/
theorem a_predicate_does_not_partition :
    ∃ (db : String → Table) (p : Pred) (t : String),
      denote db (.union (.filter p (.scan t)) (.filter (.not p) (.scan t)))
        ≠ denote db (.scan t) := by
  refine ⟨fun _ => [[Val.null]], .eqNum 0 1, "t", ?_⟩
  simp [denote, evalPred, cell]

/-- What IS true, and all that is: **filtering never invents a row.**

So a planner may use a predicate to PRUNE and never to REASSEMBLE. The
distinction is the whole practical content of the theorem above. -/
theorem a_filter_never_grows_a_table (p : Pred) (t : String) (db : String → Table) :
    (denote db (.filter p (.scan t))).length ≤ (db t).length := by
  simp only [denote]
  exact List.length_filter_le _ _

/-- **And no row is kept by both halves**, which is why the loss above is
silent: nothing double-counts to hide it. A row is kept by `p` or by `not p` or
by NEITHER, and the third case has no symptom at all. -/
theorem no_row_is_kept_twice (p : Pred) (r : Row) :
    ¬((evalPred p r = .yes) ∧ (evalPred (.not p) r = .yes)) := by
  intro ⟨h1, h2⟩
  simp [evalPred, h1] at h2

end Ratio.Relational
