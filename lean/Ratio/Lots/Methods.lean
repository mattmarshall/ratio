import Ratio.Lots

set_option warningAsError true
/-! `Ratio.Lots.Methods` — which lots a sale gives up, and why it is not a
detail.

`Ratio.Lots.relieveFifo` takes the head of the list. FIFO is one method among
many, and the choice is a term of an administration agreement because it decides
the REALIZED GAIN — the same holding and the same trade produce different
taxable income under different methods, with no figure on the balance sheet
moving.

⛔ AND THE SPACE IS NOT ALL ORDERINGS. Three shapes, and only the first is a
sort:

  ORDERING          FIFO, LIFO, HIFO, LOFO, long-term-first, and the
                    tax-minimising combinations. Pick a key, sort, walk.
  SPECIFIC ID       the client names the lots. Not an ordering — a selection,
                    and one that may take from the middle of the holding.
  AVERAGE COST      no lot is chosen at all. The holding is pooled at a
                    weighted basis, and "which lot" is not a question the
                    method can answer. `average_cost_is_not_a_lot_walk`.

⭐ CONSERVATION IS ALREADY METHOD-INDEPENDENT, and that is worth noticing rather
than proving again: `Ratio.Lots.cost_is_conserved` is stated over an ARBITRARY
list, so every ordering method inherits it by handing it a different list. What
varies is the gain, and only the gain.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/-- Cost per unit, compared without dividing.

⛔ CROSS-MULTIPLIED, NOT `cost / units`. Integer division would report two lots
as equal whenever their per-unit costs differ by less than one minor unit — a lot at
100/3 and one at 101/3 both floor to 33 — and a method that ties lots which are
not tied picks by whatever the sort does next. -/
def dearerPerUnit (a b : Lot) : Bool := decide (b.units * a.cost > a.units * b.cost)

/-- **⛔ HIFO IS PER UNIT, NOT PER LOT**, and the two disagree on ordinary
holdings.

A lot of 100 units costing 1,000 (10 each) against a lot of 1 unit costing 50.
By TOTAL cost the first is dearer and HIFO takes it; by cost PER UNIT the second
is dearer by five times, and it is the one that shelters the most gain.

Getting this wrong does not fail — it quietly picks the cheaper basis and
overstates the taxable gain, which is the opposite of what the method was chosen
for. -/
theorem hifo_is_per_unit_not_per_lot :
    dearerPerUnit ⟨2, 1, 50⟩ ⟨1, 100, 1000⟩ = true
    ∧ (⟨1, 100, 1000⟩ : Lot).cost > (⟨2, 1, 50⟩ : Lot).cost := by
  constructor
  · decide
  · decide

/-- A method that works by ordering the holding. -/
inductive Order where
  /-- Oldest acquisition first. -/
  | fifo
  /-- Newest first. -/
  | lifo
  /-- Dearest per unit first — the one chosen to reduce a gain. -/
  | hifo
  /-- Cheapest per unit first — chosen to REALIZE a gain, which is a thing funds
  do deliberately against a capital-loss carryforward. -/
  | lofo
deriving DecidableEq, Repr

/-- Insertion sort, defined structurally.

⚠ NOT `List.mergeSort`. That is well-founded recursion, which `decide` cannot
reduce — so every example below would have needed `native_decide`, which
`//lean:audit_proofs_test` forbids and which would have traded a checked
theorem for a trusted compiler. A method's behaviour on a concrete holding is
exactly what wants checking. -/
def insertBy (le : Lot → Lot → Bool) (x : Lot) : List Lot → List Lot
  | [] => [x]
  | y :: ys => if le x y then x :: y :: ys else y :: insertBy le x ys

def sortBy (le : Lot → Lot → Bool) : List Lot → List Lot
  | [] => []
  | x :: xs => insertBy le x (sortBy le xs)

/-- Break a tie by acquisition order, and ONLY a tie.

⛔ `dearer a b || a.seq ≤ b.seq` IS NOT THIS, and it is what I wrote first. That
comparator is true whenever the sequence ascends — whatever the costs — so the
tiebreak overrode the method and HIFO quietly performed FIFO. `decide` reported
the theorem FALSE, which is the only reason it was caught; a test asserting
"HIFO differs from LOFO" would have passed, because LOFO was broken the same way
in the other direction.

⚠ This is the comparator-transitivity trap in its usual costume: a secondary key
applied unconditionally rather than on equality. -/
def thenBySeq (dearer : Lot → Lot → Bool) (a b : Lot) : Bool :=
  dearer a b || (!dearer b a && decide (a.seq ≤ b.seq))

/-- Order a holding for a method.

⚠ `fifo` and `lifo` order by acquisition; `hifo` and `lofo` by cost per unit.
The list handed to `relieveFifo` is what decides the answer — the function
itself only ever takes the head. -/
def arrange : Order → List Lot → List Lot
  | .fifo, ls => sortBy (fun a b => decide (a.seq ≤ b.seq)) ls
  | .lifo, ls => sortBy (fun a b => decide (b.seq ≤ a.seq)) ls
  | .hifo, ls => sortBy (thenBySeq dearerPerUnit) ls
  | .lofo, ls => sortBy (thenBySeq (fun a b => dearerPerUnit b a)) ls

/-- Relieve under a method. -/
def relieveBy (m : Order) (ls : List Lot) (want : Int) : Option (List Taken × List Lot) :=
  relieveFifo (arrange m ls) want

/-- **⭐ EVERY ORDERING METHOD CONSERVES, and none of them needed a new proof.**

`Ratio.Lots.cost_is_conserved` is stated over an ARBITRARY list, so a method
that reorders the holding inherits it — the theorem never cared which order it
was handed. That is the invariant a new method has to preserve, and it holds by
construction rather than by inspection.

What a new method CAN break is the gain, which is what it is for. -/
theorem every_ordering_method_conserves (m : Order) (ls : List Lot) (want : Int)
    (ts : List Taken) (left : List Lot) (h : relieveBy m ls want = some (ts, left)) :
    takenCost ts + totalCost left = totalCost (arrange m ls) := by
  unfold relieveBy at h
  exact cost_is_conserved _ want ts left h

/-- Two one-unit lots — one bought first at 10, one later at 40 — and a sale of
one unit.

**HIFO gives up the dear lot and shelters the gain; LOFO gives up the cheap one
and realizes it.** Same holding, same trade, four times the taxable income
between them, and nothing on the balance sheet moves. -/
theorem the_method_decides_the_taxable_gain :
    (relieveBy .hifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => takenCost r.1) = some 40
    ∧ (relieveBy .lofo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => takenCost r.1) = some 10 := by
  constructor <;> decide

/-- **And FIFO and LIFO are a different pair again**, so the four are genuinely
four rather than two wearing different names on this holding. -/
theorem fifo_and_lifo_differ_here :
    (relieveBy .fifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => takenCost r.1) = some 10
    ∧ (relieveBy .lifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => takenCost r.1) = some 40 := by
  constructor <;> decide

/-- **⚠ AND FIFO IS NOT HIFO EVEN WHEN THE HOLDING IS SORTED BY PRICE.**

The oldest lot is the dearest here, so FIFO and HIFO agree — which is exactly
why a test that uses one holding proves nothing about the method. Stated so the
agreement is on the record as a coincidence rather than a property. -/
theorem two_methods_can_agree_by_accident :
    (relieveBy .fifo [⟨1, 1, 40⟩, ⟨2, 1, 10⟩] 1).map (fun r => takenCost r.1)
      = (relieveBy .hifo [⟨1, 1, 40⟩, ⟨2, 1, 10⟩] 1).map (fun r => takenCost r.1) := by
  decide

/- ── Specific identification ──────────────────────────────────────────── -/

/-- **Specific identification is a SELECTION, not an ordering.**

The client names the lots. That may take from the middle of a holding, may take
partially from several, and cannot be expressed as "sort and walk" — which is
why it is a separate shape rather than another `Order`.

Modelled as the ordering that puts the named lots first, which is faithful for
relief and NOT faithful in general: a selection naming more units than the sale
is a client instruction that contradicts itself, and an ordering cannot say so.
-/
def selectFirst (named : List Nat) (ls : List Lot) : List Lot :=
  ls.filter (fun l => named.contains l.seq) ++ ls.filter (fun l => !named.contains l.seq)

/-- **A named selection takes what it names**, even from the middle. -/
theorem specific_identification_takes_from_the_middle :
    (relieveFifo (selectFirst [2] [⟨1, 1, 10⟩, ⟨2, 1, 40⟩, ⟨3, 1, 70⟩]) 1).map
        (fun r => takenCost r.1)
      = some 40 := by
  decide

/- ── Average cost ─────────────────────────────────────────────────────── -/

/-- The pooled basis of a holding: total cost over total units.

⛔ THIS IS NOT A LOT WALK AND CANNOT BE MADE INTO ONE. Under average cost there
is no lot to give up — every unit has the same basis, so "which lot" is not a
question the method answers. It is modelled here so the difference is explicit
rather than discovered by someone trying to add it as another `Order`.

⚠ AND IT DIVIDES. Total cost over total units rarely lands on a whole minor
unit, so average cost carries a rounding decision that no ordering method has —
a term of an agreement, like every other division in this system. -/
def pooled (ls : List Lot) : Option Int :=
  if totalUnits ls = 0 ∨ totalCost ls % totalUnits ls ≠ 0 then none
  else some (totalCost ls / totalUnits ls)

/-- **Average cost is not a lot walk**: it gives an answer no ordering can
produce.

Three one-unit lots at 10, 40 and 70. Every ordering method relieves one of
those three basis figures on a one-unit sale. Average cost relieves 40 — which
happens to equal the middle lot here, and would not in general. -/
theorem average_cost_is_not_a_lot_walk :
    pooled [⟨1, 1, 10⟩, ⟨2, 1, 40⟩, ⟨3, 1, 70⟩] = some 40 := by
  decide

/-- **And it refuses rather than rounding**, like every other division here.
Two units costing 25 pool to 12.5, and which way to round is a term of an
agreement. -/
theorem an_average_that_does_not_divide_is_refused :
    pooled [⟨1, 1, 12⟩, ⟨2, 1, 13⟩] = none := by
  decide

end Ratio.Lots
