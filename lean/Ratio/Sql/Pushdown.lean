import Pg.Rel.Semantics

set_option warningAsError true
/-! `Ratio.Sql.Pushdown` — Stage E read plans against `Pg.Rel.Semantics`.

#8 leftover after the live-engine apply and the console/API store door: a
planner rewrite that is a theorem, not a SQL string. The denotation is
`Pg.Rel` (bags, three-valued logic, `≡`). This file instantiates it on the
lots / positions / aggregates / watermark catalog.

⛔ TWO SCANS, NOT ONE JOIN THAT CAN RETURN EMPTY. A missing watermark is a
refuse (`//tla:unpinned_projection_check`). An INNER JOIN of watermark ⋉ lots
that yields `[]` looks like a fund that sold everything. `an_empty_pin_is_
not_an_empty_holding` is why the store keeps the pin check and the lots
scan as two plans.

⛔ RELIEF IS NOT A REWRITE. `ORDER BY seq` is display order on a seq-keyed
table. HIFO takes the dear lot. Treating a seq filter as the HIFO take is
the silent SQL FIFO `//tla:stale_method_relief_check` exists to catch.
`seq_scan_is_not_hifo`.

The sound rewrite the store uses: a pin predicate that reads only the
watermark prefix pushes into the watermark scan of a watermark ⋉ lots
outer join — `pushdown_into_the_preserved_side_is_sound`, with `hp` the
width-3 obligation the schema already has.

The rewrite the store refuses: a filter on `acquired` (NULL-as-unset, the
lots side) pushed below that outer join — Stage E's instance of
`pushdown_below_an_outer_join_is_unsound`.

Lean core + `omega`, no Mathlib. -/

open Pg.Rel

namespace Ratio.Sql.Pushdown

/-- Stage E table names. One watermark, three children. -/
def watermark : String := "projection_watermark"
def lots : String := "lots"
def positions : String := "positions"
def aggregates : String := "aggregates"

/-- `lots` row: book, view, dim, instrument, seq, units, cost, acquired. -/
def lotsWidth : Nat := 8

/-- `projection_watermark` row: book, prefix, digest. -/
def watermarkWidth : Nat := 3

/-- Pin: book = `book` ∧ prefix = `prefix` ∧ digest = `digest` (cols 0,1,2). -/
def pinPred (book prefix digest : Int) : Pred :=
  .and (.eqNum 0 book) (.and (.eqNum 1 prefix) (.eqNum 2 digest))

/-- Filter the watermark scan down to one pin. Empty is a refuse, not lots. -/
def pinPlan (book prefix digest : Int) : Plan :=
  .filter (pinPred book prefix digest) (.scan watermark)

/-- Filter the lots scan to one holding. Keys are Int-encoded in the model;
the Rust door carries the same positions as strings. -/
def lotsPlan (book view dim inst : Int) : Plan :=
  .filter
    (.and (.eqNum 0 book)
      (.and (.eqNum 1 view)
        (.and (.eqNum 2 dim) (.eqNum 3 inst))))
    (.scan lots)

/-- The join a naive planner writes: watermark ⋉ lots. Right width is the
lots row, so an unmatched watermark comes back padded with eight nulls —
including a null `acquired`, which is unset, not a default day. -/
def joined : Plan :=
  .leftJoin (.scan watermark) (.scan lots) lotsWidth

/-- Sound shape: pin pushed into the preserved (watermark) scan. -/
def pinPushed (book prefix digest : Int) : Plan :=
  .leftJoin (pinPlan book prefix digest) (.scan lots) lotsWidth

/-- `acquired = day` on the JOINED row (watermark 3 + lots col 7). -/
def acquiredOnJoin (day : Int) : Pred :=
  .eqNum (watermarkWidth + 7) day

/-- Two lots, cheap then dear. Seq 0 costs 1000; seq 1 costs 10000.
Physical storage is FIFO-shaped. HIFO is not. -/
def cheapThenDear : Table :=
  [
    [Val.num 1, Val.num 0, Val.num 1, Val.num 1,
     Val.num 0, Val.num 10, Val.num 1000, Val.null],
    [Val.num 1, Val.num 0, Val.num 1, Val.num 1,
     Val.num 1, Val.num 10, Val.num 10000, Val.null]
  ]

/- ── Schema obligation: a pin reads only the watermark prefix ────────── -/

/-- `cell` on a prefix does not see the suffix. The schema's width-3
watermark rows are what make a pin predicate left-only. -/
theorem cell_append_left (r s : Row) (i : Nat) (h : i < r.length) :
    cell (r ++ s) i = cell r i := by
  induction r generalizing i with
  | nil => cases h
  | cons x xs ih =>
    cases i with
    | zero => simp [cell]
    | succ j =>
      have hj : j < xs.length := by
        simp at h
        omega
      simpa [cell] using ih j hj

/-- A pin predicate on columns 0,1,2 is independent of the lots suffix
once the watermark row is at least width 3. That is the `hp` of
`pushdown_into_the_preserved_side_is_sound` on a well-typed snapshot. -/
theorem pin_reads_only_the_watermark_prefix
    (book prefix digest : Int) (r s : Row) (hr : 3 ≤ r.length) :
    evalPred (pinPred book prefix digest) (r ++ s)
      = evalPred (pinPred book prefix digest) r := by
  have h0 : 0 < r.length := Nat.lt_of_lt_of_le (by decide : 0 < 3) hr
  have h1 : 1 < r.length := Nat.lt_of_lt_of_le (by decide : 1 < 3) hr
  have h2 : 2 < r.length := Nat.lt_of_lt_of_le (by decide : 2 < 3) hr
  have c0 := cell_append_left r s 0 h0
  have c1 := cell_append_left r s 1 h1
  have c2 := cell_append_left r s 2 h2
  simp [pinPred, evalPred, c0, c1, c2]

/-- If a pin is left-only on every row — the schema obligation above, on
every watermark row — then pushing it into the watermark scan is `≡`.

This is not a new rewrite. It is `pushdown_into_the_preserved_side_is_
sound` at the Stage E catalog. -/
theorem stage_e_pin_pushes_into_the_watermark
    (book prefix digest : Int)
    (hp : ∀ r s : Row,
        evalPred (pinPred book prefix digest) (r ++ s)
          = evalPred (pinPred book prefix digest) r) :
    Plan.filter (pinPred book prefix digest) joined
      ≡ pinPushed book prefix digest :=
  pushdown_into_the_preserved_side_is_sound
    (pinPred book prefix digest) (.scan watermark) (.scan lots) lotsWidth hp

/- ── The Stage E outer-join trap ─────────────────────────────────────── -/

/-- **`acquired` cannot be pushed below watermark ⋉ lots.**

Stage E's instance of `pushdown_below_an_outer_join_is_unsound`. A
watermark row with no lots partner is padded with a null `acquired`.
Filtering `acquired = 5` ABOVE the join sees UNKNOWN and drops the row.
Pushing the same predicate into the lots scan leaves the lots bag empty,
so the watermark row still matches nothing and comes back padded and
kept.

`acquired` NULL is unset, not a default day. A rewrite that keeps the
padded row invents a holding the pin never had. -/
theorem stage_e_acquired_below_join_is_unsound :
    ∃ (db : String → Table) (p : Pred),
      denote db (.filter p joined)
        ≠ denote db (.leftJoin (.scan watermark) (.filter p (.scan lots))
            lotsWidth) := by
  refine ⟨fun t => if t = watermark then [[Val.num 1, Val.num 0, Val.num 0]] else [],
          acquiredOnJoin 5, ?_⟩
  simp [denote, evalPred, cell, joined, lotsWidth, watermark, lots,
        acquiredOnJoin, watermarkWidth]

/- ── Empty pin is not an empty holding ───────────────────────────────── -/

/-- A store that has never been replayed has an empty watermark filter
and may still have lots rows in some other bag — or, more usefully, the
lots scan of a book that *was* folded is non-empty while a *wrong* pin
is empty.

Collapsing the two into one INNER JOIN that returns `[]` is the silent
empty fund `//tla:unpinned_projection_check` exists to refuse. -/
theorem an_empty_pin_is_not_an_empty_holding :
    ∃ (db : String → Table),
      denote db (pinPlan 1 2 3) = []
      ∧ denote db (lotsPlan 1 0 1 1) ≠ [] := by
  refine ⟨fun t => if t = lots then cheapThenDear else [], ?_⟩
  simp [pinPlan, lotsPlan, pinPred, denote, evalPred, cell, watermark, lots,
        cheapThenDear]

/- ── Seq order is not HIFO ───────────────────────────────────────────── -/

/-- **A seq-index take is not the HIFO take.**

Same two lots: cheap at seq 0, dear at seq 1. `eqNum 4 0` is
`ORDER BY seq` taking the head. `eqNum 6 10000` is the dear lot HIFO
must take. They are not `≡`. Relief stays `relieve_by`. -/
theorem seq_scan_is_not_hifo :
    ∃ (db : String → Table),
      denote db (.filter (.eqNum 4 0) (.scan lots))
        ≠ denote db (.filter (.eqNum 6 10000) (.scan lots)) := by
  refine ⟨fun t => if t = lots then cheapThenDear else [], ?_⟩
  simp [denote, evalPred, cell, lots, cheapThenDear]

/-- Stacked Stage E filters commute. Book / view / dim / instrument can
be pushed in any order; the rewrite is `filters_commute`, not a new law. -/
theorem stage_e_holding_filters_commute (book view : Int) (c : Plan) :
    Plan.filter (.eqNum 0 book) (.filter (.eqNum 1 view) c)
      ≡ .filter (.eqNum 1 view) (.filter (.eqNum 0 book) c) :=
  filters_commute _ _ _

end Ratio.Sql.Pushdown
