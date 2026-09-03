import Ratio.Lots
import Ratio.Lots.Methods

set_option warningAsError true
/-! `Ratio.Lots.SpecId` — a selection the taxpayer NAMES, not a sort of the holding.

`Ratio.Lots.Methods` proves the space of lot methods is not all orderings, and
names three shapes that are not: specific identification, average cost, and
tax-minimising relief. MinTax is `Ratio.Lots.MinTax`. This file is the first
of those three as an engine: the client names the lots at the sale, and the
holding alone does not pick.

⛔ SPECIFIC IDENTIFICATION IS NOT A FUNCTION OF THE LOTS. Every `Order` looks
at the holding and produces one walk. SpecID cannot: which lots to give up is
an attested choice the taxpayer makes at sale time, and it may take from the
MIDDLE of a holding that FIFO, LIFO, HIFO and LOFO would all walk from an
end. Same three lots, one unit sold:

  lot 1   basis 10
  lot 2   basis 40
  lot 3   basis 70

  FIFO / LOFO give up 10. LIFO / HIFO give up 70. SpecID naming lot 2
  gives up 40. No `Order` produces 40 from this holding.

`Ratio.Lots.Methods.selectFirst` puts the named lots first and walks. That
is faithful for a well-formed selection and NOT faithful in general: a
selection that names more units than the sale, names a lot the holding does
not have, or names nothing at all, is a client instruction that contradicts
itself, and an ordering cannot say so. This file is the surface that can.

⚠ SO THIS IS NOT AN `Order`, AND ADDING IT AS ONE IS THE MISTAKE
`Ratio.Lots.Methods` exists to prevent. `//tla:specid_engine_check` is the
sequence obligation: a sort-and-walk that never sees the name cannot take
the middle lot. `//tla:sort_and_walk_specid_check` is the probe.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The holding the middle-lot example needs ─────────────────────────── -/

/-- Three one-unit lots. SpecID naming the middle one is the case no
`Order` can reproduce. -/
def specHolding : List Lot := [⟨1, 1, 10⟩, ⟨2, 1, 40⟩, ⟨3, 1, 70⟩]

/- ── The decision surface ─────────────────────────────────────────────── -/

/-- Whether a name list repeats a sequence.

Naming the same lot twice is two instructions about one remainder. An
engine that silently took it once would hide the contradiction. -/
def hasDup : List Nat → Bool
  | [] => false
  | x :: xs => xs.contains x || hasDup xs

/-- Look up each named sequence in the holding, in the order named.

⛔ REFUSES AN UNKNOWN NAME. A sequence the holding does not have is not a
lot the taxpayer can identify. Guessing a neighbour, or skipping it, would
relieve somebody else's basis. -/
def lookupNamed : List Nat → List Lot → Option (List Lot)
  | [], _ => some []
  | s :: rest, ls =>
    match ls.find? (fun l => decide (l.seq = s)) with
    | none => none
    | some l =>
      match lookupNamed rest ls with
      | none => none
      | some picked => some (l :: picked)

/-- Resolve a named selection, or nothing.

⛔ EMPTY IS NOT A SELECTION. SpecID elected and lots unnamed is the
instruction that contradicts itself — the taxpayer said "I will name the
lots" and then named none. An engine that fell through to FIFO would
relieve under a method the sale did not elect, and FIFO is a method real
funds elect, so the accident is indistinguishable from the agreement.
`//tla:sort_and_walk_specid_check` is that engine. -/
def picks (named : List Nat) (ls : List Lot) : Option (List Lot) :=
  if named.isEmpty || hasDup named then none
  else lookupNamed named ls

/-- The lots the selection did not name. Untouched; they stay in the book. -/
def unnamed (named : List Nat) (ls : List Lot) : List Lot :=
  ls.filter (fun l => !named.contains l.seq)

/-- Whether a proper prefix of the named lots already covers the sale.

Naming lots the sale will not reach is a client instruction that
contradicts itself: the taxpayer identified more than they sold.
`selectFirst` cannot say so — it puts the extra lots behind the walk and
they simply are not taken. -/
def overspecified : List Lot → Int → Bool
  | [], _ => false
  | [_], _ => false
  | l :: rest, want => decide (want ≤ l.units) || overspecified rest (want - l.units)

/-- Relieve under SpecID: take the named lots, in the order named.

Conservation is inherited — `relieveFifo` never cared which list it was
handed. What SpecID can break is the gain, which is what the name is for.

The leftover is the remainder of the named lots plus every lot the
selection did not name. Unnamed lots are not candidates; falling through
to them is FIFO. -/
def relieveSpecId (named : List Nat) (ls : List Lot) (want : Int) :
    Option (List Taken × List Lot) :=
  match picks named ls with
  | none => none
  | some picked =>
    if overspecified picked want then none
    else
      match relieveFifo picked want with
      | none => none
      | some (ts, leftNamed) => some (ts, leftNamed ++ unnamed named ls)

/- ── Named lots are relieved exactly ──────────────────────────────────── -/

/-- **⭐ A NAMED SELECTION TAKES WHAT IT NAMES**, even from the middle.

Three lots; the taxpayer names lot 2. The basis given up is 40. FIFO
would have given up 10; LIFO/HIFO would have given up 70. -/
theorem specid_takes_from_the_middle :
    (relieveSpecId [2] specHolding 1).map (fun r => takenCost r.1)
      = some 40 := by
  decide

/-- **And the unnamed lots are still there**, in full. Lot 1 and lot 3
were not named; a walk that consumed either of them would have been FIFO
or LIFO wearing SpecID's name. -/
theorem unnamed_lots_are_untouched :
    (relieveSpecId [2] specHolding 1).map (fun r => (r.2).map (·.seq))
      = some [1, 3] := by
  decide

/-- **The taken lot is exactly the one named.** Not "a lot", the lot. -/
theorem named_lots_are_relieved_exactly :
    (relieveSpecId [2] specHolding 1).map (fun r => r.1.map (·.seq))
      = some [2] := by
  decide

/- ── Refusals ─────────────────────────────────────────────────────────── -/

/-- **⛔ AN UNKNOWN LOT IS REFUSED.** Sequence 9 is not in the holding.
Skipping it, or picking a neighbour, would relieve a basis the taxpayer
did not identify. -/
theorem an_unknown_lot_is_refused :
    relieveSpecId [9] specHolding 1 = none := by
  decide

/-- **⛔ AN OVERSPECIFIED SELECTION IS REFUSED.** Naming lots 2 and 3 for
a one-unit sale identifies more than was sold. `selectFirst` would put
both first and walk one; this refuses. -/
theorem an_overspecified_selection_is_refused :
    relieveSpecId [2, 3] specHolding 1 = none := by
  decide

/-- **⛔ AN INSUFFICIENT SELECTION IS REFUSED.** Naming lot 2 (one unit)
for a two-unit sale does not cover what was sold. The remainder is not
FIFO — FIFO is a method real funds elect. -/
theorem an_insufficient_selection_is_refused :
    relieveSpecId [2] specHolding 2 = none := by
  decide

/-- **⛔ AN UNNAMED SELECTION IS REFUSED.** SpecID elected, lots unnamed.
Not FIFO. The empty list is the instruction that said nothing. -/
theorem an_unnamed_selection_is_refused :
    relieveSpecId [] specHolding 1 = none := by
  decide

/-- **A duplicate name is refused.** Two instructions about one lot. -/
theorem a_duplicate_name_is_refused :
    relieveSpecId [2, 2] specHolding 1 = none := by
  decide

/- ── Why this cannot be an Order ──────────────────────────────────────── -/

/-- **⭐ NO ORDERING TAKES THE MIDDLE LOT.** FIFO and LOFO give up 10;
LIFO and HIFO give up 70. None give up 40, because none of them take a
name. -/
theorem no_ordering_takes_the_middle (m : Order) :
    (relieveBy m specHolding 1).map (fun r => takenCost r.1) ≠ some 40 := by
  cases m <;> decide

/-- **And `selectFirst` cannot refuse overspecification.** Same holding,
same overspecified name list: the ordering walk succeeds and this
refuses. That is why SpecID is not "named lots first". -/
theorem selectFirst_cannot_refuse_overspecification :
    (relieveFifo (selectFirst [2, 3] specHolding) 1).isSome = true
    ∧ relieveSpecId [2, 3] specHolding 1 = none := by
  constructor <;> decide

/-- **`selectFirst` of nothing is FIFO**, which is the silent fallback
this surface exists to prevent. An empty name list reorders nothing; the
walk takes lot 1. SpecID refuses. -/
theorem selectFirst_of_nothing_is_fifo :
    (relieveFifo (selectFirst [] specHolding) 1).map (fun r => takenCost r.1)
      = some 10
    ∧ relieveSpecId [] specHolding 1 = none := by
  constructor <;> decide

/- ── Conservation, pro-rata, husk ─────────────────────────────────────── -/

/-- **⭐ SPECID CONSERVES**, and it did not need a new proof. The walk is
`relieveFifo` over the named lots; `cost_is_conserved` is stated over an
arbitrary list. Unnamed lots are appended, not rewritten. -/
theorem specid_conserves (named : List Nat) (ls : List Lot) (want : Int)
    (picked : List Lot) (ts : List Taken) (leftNamed : List Lot)
    (hp : picks named ls = some picked)
    (ho : overspecified picked want = false)
    (h : relieveFifo picked want = some (ts, leftNamed)) :
    takenCost ts + totalCost leftNamed = totalCost picked := by
  -- `hp` / `ho` pin the statement to a well-formed SpecID sale; the walk
  -- itself is `relieveFifo` over those named lots.
  have := hp
  have := ho
  exact cost_is_conserved picked want ts leftNamed h

/-- **The whole holding is what was taken plus what remains.** Named
conservation plus `partition_sums_to_whole` on the complement. Stated
on the middle-lot sale so a walk that consumed an unnamed lot cannot
hide behind the named-only equation. -/
theorem specid_preserves_the_holding :
    (relieveSpecId [2] specHolding 1).map
        (fun r => takenCost r.1 + totalCost r.2)
      = some (totalCost specHolding) := by
  decide

/-- **A partial SpecID relief is still exactly pro rata.** Naming does
not get a rounding privilege the walk refused. Three units of a
seven-unit lot still refuse when the cost will not divide. -/
theorem specid_partial_relief_is_exactly_pro_rata :
    relieveSpecId [1] [⟨1, 7, 100⟩] 3 = none := by
  decide

/-- **⚠ A HUSK IS STILL A HUSK.** Naming does not make `relieveFifo`
refuse zero units. The walk consumes it and hands over the basis —
`Ratio.Lots.Edges.a_husk_gives_away_its_cost`. Refusal belongs where
the lot is offered, not here. Stated so nobody "fixes" SpecID by
adding a check the walk already failed to have.

One unit wanted; the husk (seq 1, 0 units, cost 40) is named first and
taken whole because `0 ≤ want`, then the real lot. Taken cost 50. -/
theorem specid_inherits_the_husk :
    (relieveSpecId [1, 2] [⟨1, 0, 40⟩, ⟨2, 1, 10⟩] 1).map
        (fun r => takenCost r.1)
      = some 50 := by
  decide

/-- **Naming only the sound lot leaves the husk in the book.** The husk
is not a fallback; it is a lot the taxpayer did not identify. -/
theorem a_husk_that_was_not_named_stays :
    (relieveSpecId [2] [⟨1, 0, 40⟩, ⟨2, 1, 10⟩] 1).map
        (fun r => (takenCost r.1, (r.2).map (·.seq)))
      = some (10, [1]) := by
  decide

end Ratio.Lots
