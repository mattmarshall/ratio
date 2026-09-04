set_option warningAsError true
/-! `Pg.Rel` — what a query MEANS, so that "this plan equals that plan" can be
said at all.

This is the denotation `tomato-bazel/rules_postgres#9` merged as
`Pg.Rel.Semantics`. It is carried in-tree so Stage E plan rewrites elaborate
against the same bags-and-three-valued-logic, not a citation of a fetch that
this repository has already failed to retrieve (HANDOFF: Bazel could not
fetch two `tomato-bazel` modules). The theorems keep their names. A silent
drift that dropped the outer-join counterexample would be a planner that
can no longer say the rewrite is unsound.

WHAT THE UPSTREAM REPO HAD: an AST, a catalog, a type system, a PL/pgSQL
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
                      is the habit every engineer brings from a language whose
                      predicates are BOOLEAN. Carried into SQL it becomes an
                      optimisation that silently drops every row with a null in
                      the compared column.

Lean core, no Mathlib. -/

namespace Pg.Rel

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

/-- Whether two rows join, on equality of their first column.

⛔ NULL DOES NOT JOIN TO NULL. `a.x = b.x` is UNKNOWN when either is null, and a
join keeps only TRUE — so two rows that are both null on the key do NOT match.
This is the same rule as `WHERE`, and it is the reason the outer join below has
anything to pad. -/
def joins (r s : Row) : Bool :=
  match cell r 0, cell s 0 with
  | .num a, .num b => a == b
  | _, _ => false

/-- Plan nodes.

`leftJoin` carries the width of its right side, because a row of the left that
matches nothing is emitted PADDED WITH NULLS to that width — and those nulls are
what make predicate pushdown unsound below. -/
inductive Plan where
  | scan (t : String)
  | filter (p : Pred) (child : Plan)
  | union (a b : Plan)
  | leftJoin (a b : Plan) (rightWidth : Nat)
deriving DecidableEq, Repr

/-- What a plan MEANS, against a database that maps a name to a bag of rows.

⛔ `WHERE` KEEPS ONLY `yes`. Not "not no" — a row whose predicate is unknown is
dropped, and that asymmetry is the whole of the trouble below. -/
def denote (db : String → Table) : Plan → Table
  | .scan t => db t
  | .filter p c => (denote db c).filter (fun r => evalPred p r == Three.yes)
  | .union a b => denote db a ++ denote db b
  | .leftJoin a b w =>
    (denote db a).flatMap (fun r =>
      let m := (denote db b).filter (joins r)
      -- The preserved side keeps every row. One that matched nothing comes
      -- back padded with nulls, which is the whole character of an outer join.
      -- Written as ONE map over either the matches or a single null row, so
      -- that both branches visibly share the left prefix — which is what
      -- `everyRowKeepsItsLeft` needs and what makes the pushdown rule provable
      -- rather than merely believable.
      ((if m.isEmpty then [List.replicate w Val.null] else m)).map (fun s => r ++ s))

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

⛔ THE THEOREM THAT MUST BE STATED BEFORE ANY REWRITE IS BUILT. "Filter by a
predicate and by its complement and you have the whole back" is true of BOOLEAN
predicates and is the intuition everyone arrives with.

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

/-- Every row a left row produces carries that left row as its prefix, so a
predicate that reads only the prefix decides the whole group at once.

This is why pushing into the PRESERVED side is sound and pushing into the other
one is not: one predicate sees the same values before and after the join, and
the other sees nulls that did not exist before it. -/
theorem everyRowKeepsItsLeft (p : Pred) (r : Row) (rows : Table)
    (hp : ∀ x y : Row, evalPred p (x ++ y) = evalPred p x) :
    (rows.map (fun s => r ++ s)).filter (fun x => evalPred p x == Three.yes)
      = if evalPred p r == Three.yes then rows.map (fun s => r ++ s) else [] := by
  induction rows with
  | nil => simp
  | cons s rest ih =>
    by_cases h : evalPred p r == Three.yes
    · simp [hp, h] at ih ⊢
      exact ih
    · simp [hp, h] at ih ⊢
      exact ih

/- ── The classic planner bug ───────────────────────────────────────────── -/

/-- **A predicate on the null-padded side CANNOT be pushed below an outer
join.**

⛔ THE BUG THIS FILE WAS BUILT TO CATCH, and the one every real planner has had.

    SELECT * FROM a LEFT JOIN b ON a.k = b.k WHERE b.v = 5

Pushing `b.v = 5` into the scan of `b` looks obviously sound — it is a
restriction on `b`, and `b` is right there. It is not sound, and the two differ
on the rows of `a` THAT MATCH NOTHING:

  * filtering ABOVE the join sees those rows already padded with nulls, so
    `b.v = 5` is UNKNOWN and they are dropped;
  * filtering BELOW the join shrinks `b` first, so those rows still match
    nothing and come back PADDED and kept.

One keeps them, the other does not. The witness needs only a left row with no
partner. -/
theorem pushdown_below_an_outer_join_is_unsound :
    ∃ (db : String → Table) (p : Pred) (a b : String) (w : Nat),
      denote db (.filter p (.leftJoin (.scan a) (.scan b) w))
        ≠ denote db (.leftJoin (.scan a) (.filter p (.scan b)) w) := by
  refine ⟨fun t => if t = "a" then [[Val.num 1]] else [], .eqNum 1 5, "a", "b", 1, ?_⟩
  simp [denote, evalPred, cell]

/-- **What IS sound: a predicate on the PRESERVED side.**

The mirror of the theorem above, and the rewrite a planner actually wants. A
restriction that reads only the left row commutes with the join, because
dropping that row before or after deciding what it matched gives the same rows
either way.

`hp` is the side condition and it is the whole distinction: the predicate must
see the same values before and after the join. A predicate on the right side
does not — after the join it can see nulls that did not exist before it, which
is exactly `pushdown_below_an_outer_join_is_unsound`.

The proof is one step per left row: `everyRowKeepsItsLeft` decides that row's
whole group at once, and the induction carries the rest. -/
theorem pushdown_into_the_preserved_side_is_sound (p : Pred) (a b : Plan) (w : Nat)
    (hp : ∀ r s : Row, evalPred p (r ++ s) = evalPred p r) :
    Plan.filter p (.leftJoin a b w) ≡ .leftJoin (.filter p a) b w := by
  intro db
  simp only [denote]
  -- `denote db a` is not a variable, so inducting on it leaves an induction
  -- hypothesis that does not apply. Generalise first.
  generalize denote db a = as
  induction as with
  | nil => simp
  | cons r rest ih =>
    rw [List.flatMap_cons, List.filter_append, ih,
        everyRowKeepsItsLeft p r _ hp, List.filter_cons]
    by_cases h : evalPred p r == Three.yes
    · rw [if_pos h, if_pos h, List.flatMap_cons]
    · rw [if_neg h, if_neg h, List.nil_append]

end Pg.Rel
