import Ratio.Lots
import Ratio.Lots.Methods

set_option warningAsError true
/-! `Ratio.Lots.AverageCost` — a POOL of the holding, not a sort of the lots.

`Ratio.Lots.Methods` proves the space of lot methods is not all orderings, and
names three shapes that are not: specific identification, average cost, and
tax-minimising relief. MinTax is `Ratio.Lots.MinTax`. SpecID is
`Ratio.Lots.SpecId`. This file is the third as an engine: the holding is
pooled at a weighted basis, and "which lot" is not a question it answers.

⛔ AVERAGE COST IS NOT A LOT WALK AND CANNOT BE MADE INTO ONE. Every `Order`
picks a lot and walks. Average cost cannot: every unit has the same basis, so
there is no lot to give up. Same three lots, one unit sold:

  lot 1   basis 10
  lot 2   basis 20
  lot 3   basis 60

  FIFO / LOFO give up 10. LIFO / HIFO give up 60. Average cost gives up 30
  — a figure no lot carries. `Ratio.Lots.Methods.average_cost_is_not_a_lot_
  walk` used 10 / 40 / 70, where the pool equals the middle lot by
  coincidence. This holding is the case that coincidence hid.

⚠ AND IT DIVIDES. Total cost over total units rarely lands on a whole minor
unit. Which way to round is a term of an agreement, so a figure that will
not divide is refused — the same decision as
`Ratio.Lots.partial_relief_is_exactly_pro_rata` and
`Ratio.Lots.Methods.an_average_that_does_not_divide_is_refused`.

⚠ THE REMAINDER IS A POOL, NOT THE OTHER LOTS. SpecID naming the middle lot
of 10 / 40 / 70 also gives up 40, but leaves lots 1 and 3 intact. Average
cost of that holding leaves one lot of two units at 80. Same taken cost,
different remainder — which is why a test that only checked the basis given
up would not tell the two shapes apart.

⚠ SO THIS IS NOT AN `Order`, AND ADDING IT AS ONE IS THE MISTAKE
`Ratio.Lots.Methods` exists to prevent. `//tla:average_cost_engine_check` is
the sequence obligation: a sort-and-walk that never pools cannot give up 30.
`//tla:sort_and_walk_average_cost_check` is the probe.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The holding whose pooled basis no Order produces ─────────────────── -/

/-- Three one-unit lots at 10, 20 and 60. The pool is 30, and no lot
carries 30. `Ratio.Lots.Methods.average_cost_is_not_a_lot_walk` used
10 / 40 / 70, where the pool equals the middle lot; this is the case
that coincidence hid. -/
def poolHolding : List Lot := [⟨1, 1, 10⟩, ⟨2, 1, 20⟩, ⟨3, 1, 60⟩]

/- ── The decision surface ─────────────────────────────────────────────── -/

/-- The pool's identity is the holding, not a surviving lot.

A remaining lot that kept sequence 1 would look like lot 1 restated,
which is a lie — pooling destroyed the lots. Zero is not an acquisition
ordinal anyone opened. -/
def poolSeq : Nat := 0

/-- Relieve by pooling: every unit has the same basis.

`pooled` is the per-unit figure, or nothing if it will not divide.
Taken is one slice of `want` units at that basis. Remaining is one
pooled lot — or nothing, when the holding is sold out.

Conservation is arithmetic: taken cost plus leftover cost is the
holding's cost, because both sides are the unit basis times their
units. What average cost can break is the gain, which is what pooling
is for.

⛔ NOT `relieveFifo` OVER ANY ORDERING. Walking the lots would give up
one of their bases. The pool's basis is not among them. -/
def relieveAverageCost (ls : List Lot) (want : Int) :
    Option (List Taken × List Lot) :=
  if want < 0 then none
  else if want = 0 then some ([], ls)
  else
    match pooled ls with
    | none => none
    | some unit =>
      if decide (want > totalUnits ls) then none
      else
        let takenC := unit * want
        let leftU := totalUnits ls - want
        if leftU = 0 then
          some ([⟨poolSeq, want, takenC⟩], [])
        else
          some ([⟨poolSeq, want, takenC⟩],
                [⟨poolSeq, leftU, totalCost ls - takenC⟩])

/- ── The pooled basis no Order produces ───────────────────────────────── -/

/-- **⭐ THE POOL IS 30, AND NO LOT CARRIES 30.**

Three units costing 90. Every ordering relieves 10 or 60 on a one-unit
sale. Average cost relieves 30. -/
theorem the_pooled_basis_is_not_any_lots_basis :
    pooled poolHolding = some 30
    ∧ (relieveAverageCost poolHolding 1).map (fun r => takenCost r.1)
        = some 30 := by
  constructor <;> decide

/-- **And the remainder is a pool**, not the other lots. Two units at
60 — the same 30 each — as one lot. FIFO would have left 20 and 60 as
two lots. -/
theorem the_remainder_is_a_pool_not_the_other_lots :
    (relieveAverageCost poolHolding 1).map
        (fun r => r.2.map (fun l => (l.seq, l.units, l.cost)))
      = some [(0, 2, 60)] := by
  decide

/-- **An ordering leaves the other lots intact.** FIFO of the same
holding takes 10 and leaves 20 and 60. Stated so a test that only
checked the taken cost cannot hide a walk behind a pool. -/
theorem an_ordering_leaves_the_other_lots :
    (relieveBy .fifo poolHolding 1).map
        (fun r => (takenCost r.1, r.2.map (·.cost)))
      = some (10, [20, 60]) := by
  decide

/-- **⭐ NO ORDERING GIVES UP THE POOLED BASIS.** FIFO and LOFO give up
10; LIFO and HIFO give up 60. None give up 30, because none of them
pool. -/
theorem no_ordering_gives_up_the_pooled_basis (m : Order) :
    (relieveBy m poolHolding 1).map (fun r => takenCost r.1) ≠ some 30 := by
  cases m <;> decide

/- ── The coincidence Methods already named ────────────────────────────── -/

/-- **Same taken cost as naming the middle lot of 10 / 40 / 70, different
remainder.** `average_cost_is_not_a_lot_walk` equals 40, and SpecID
naming lot 2 also gives up 40. The pool leaves two units at 80; the
named walk leaves 10 and 70. A test that only checked the basis given
up would not tell the shapes apart. -/
theorem the_pooled_remainder_is_not_the_unnamed_lots :
    (relieveAverageCost [⟨1, 1, 10⟩, ⟨2, 1, 40⟩, ⟨3, 1, 70⟩] 1).map
        (fun r => (takenCost r.1, r.2.map (fun l => (l.units, l.cost))))
      = some (40, [(2, 80)])
    ∧ (relieveFifo
          (selectFirst [2] [⟨1, 1, 10⟩, ⟨2, 1, 40⟩, ⟨3, 1, 70⟩]) 1).map
        (fun r => (takenCost r.1, r.2.map (·.cost)))
      = some (40, [10, 70]) := by
  constructor <;> decide

/-- **The Methods theorem is this file's per-unit figure.** Cited, not
re-proved: the pool of 10 / 40 / 70 is 40. -/
theorem methods_already_said_the_pool_is_not_a_walk :
    pooled [⟨1, 1, 10⟩, ⟨2, 1, 40⟩, ⟨3, 1, 70⟩] = some 40 :=
  average_cost_is_not_a_lot_walk

/- ── Refusals ─────────────────────────────────────────────────────────── -/

/-- **⛔ A FIGURE THAT WILL NOT DIVIDE IS REFUSED.** Two units costing 25
pool to 12.5. Which way to round is a term of an agreement.
`Ratio.Lots.Methods.an_average_that_does_not_divide_is_refused`. -/
theorem an_average_that_does_not_divide_is_refused_here :
    relieveAverageCost [⟨1, 1, 12⟩, ⟨2, 1, 13⟩] 1 = none := by
  decide

/-- **A sale bigger than the pool is refused**, not walked into a
shortfall that an ordering would report lot by lot. -/
theorem a_sale_bigger_than_the_pool_is_refused :
    relieveAverageCost poolHolding 4 = none := by
  decide

/-- **A holding of nothing has no pooled basis.** `pooled` already
refuses a zero-unit total. -/
theorem a_zero_unit_holding_has_no_pooled_basis :
    pooled ([] : List Lot) = none
    ∧ relieveAverageCost [] 1 = none := by
  constructor <;> decide

/- ── Conservation, partial, husk ──────────────────────────────────────── -/

/-- **⭐ AVERAGE COST CONSERVES.** Taken plus leftover is the holding.
Stated on the sale whose basis no `Order` produces, so a walk that
gave up 10 cannot hide behind a tying total. -/
theorem average_cost_preserves_the_holding :
    (relieveAverageCost poolHolding 1).map
        (fun r => takenCost r.1 + totalCost r.2)
      = some (totalCost poolHolding) := by
  decide

/-- **A partial pool is still the unit basis.** Two of three units at
30 each: taken 60, leftover one unit at 30. No rounding privilege. -/
theorem a_partial_pool_is_still_the_unit_basis :
    (relieveAverageCost poolHolding 2).map
        (fun r => (takenCost r.1, totalCost r.2, totalUnits r.2))
      = some (60, 30, 1) := by
  decide

/-- **Selling the whole pool leaves nothing.** Three units at 30:
taken 90, leftover empty. -/
theorem selling_the_whole_pool_leaves_nothing :
    (relieveAverageCost poolHolding 3).map
        (fun r => (takenCost r.1, r.2))
      = some (90, []) := by
  decide

/-- **⚠ A HUSK IS ABSORBED INTO THE POOL.** Ranking and naming do not
make `relieveFifo` refuse zero units; pooling does not either. The
husk's cost sits in the total and the unit basis carries it —
`Ratio.Lots.Edges.a_husk_gives_away_its_cost` wearing a pool.
Refusal belongs where the lot is offered, not here.

One unit wanted; the husk (seq 1, 0 units, cost 40) plus a real lot
at 10 pool to 50. Taken cost 50 for one unit. -/
theorem average_cost_absorbs_the_husk :
    (relieveAverageCost [⟨1, 0, 40⟩, ⟨2, 1, 10⟩] 1).map
        (fun r => takenCost r.1)
      = some 50 := by
  decide

end Ratio.Lots
