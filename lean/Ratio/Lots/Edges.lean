import Ratio.Lots

set_option warningAsError true
/-! `Ratio.Lots.Edges` — the lots that are not ordinary.

Every theorem in `Ratio.Lots` is true, and each was written about a holding that
behaves. This is what the same function does when the holding does not, and two
of the answers are wrong in ways conservation cannot see.

⛔ A LOT HOLDING ZERO UNITS AND CARRYING COST. `relieveFifo` takes it — `units ≤
want` is satisfied by zero — and hands its entire basis to a sale that received
no units for it. Cost is conserved. Units are conserved. The realized gain is
understated by exactly the husk's cost, and every invariant this system had
before today holds of the answer.

⚠ ZERO IS THE CASE THAT KEEPS GETTING MISSED, here and elsewhere. A generator
over "plausible lot sizes" never emits it, a test author writing a holding never
writes it, and it arrives from the outside — a spin-off allocating no units, a
correction, a migration.

⛔ AND `relieveFifo` DOES NOT SORT. It takes the head of the list, so it is FIFO
exactly when the caller handed it acquisition order, and named for a policy it
does not enforce. `sorting_is_the_callers_job` says so out loud, because the
alternative is that everyone assumes the function is doing it.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/-- **⛔ A HUSK GIVES AWAY ITS COST.**

Two lots — one holding nothing but carrying 100 of basis, one holding ten units
at 200 — and a sale of five units. The right answer relieves 100. This relieves
200, because the husk was taken first and contributed its whole basis for no
units at all.

The realized gain is understated by 100 and nothing downstream can tell: the
entry balances, the units are right, and `Ratio.Lots.cost_is_conserved` is
satisfied because the cost really did move. -/
theorem a_husk_gives_away_its_cost :
    relieveFifo [⟨1, 0, 100⟩, ⟨2, 10, 200⟩] 5
      = some ([⟨1, 0, 100⟩, ⟨2, 5, 100⟩], [⟨2, 5, 100⟩]) := by
  decide

/-- And the same sale against the sound holding alone relieves 100 — which is
what makes the previous theorem a defect rather than a convention. -/
theorem the_sound_holding_relieves_half_as_much :
    relieveFifo [⟨2, 10, 200⟩] 5 = some ([⟨2, 5, 100⟩], [⟨2, 5, 100⟩]) := by
  decide

/-- **⛔ AND `relieveFifo` DOES NOT SORT.**

Handed a lot acquired ninth before one acquired first, it takes the ninth. So
the name describes the CALLER'S contract, not the function's behaviour — and a
caller who assumed otherwise would be relieving in whatever order the storage
layer happened to return, which for a projection keyed by instrument is not
acquisition order at all. -/
theorem sorting_is_the_callers_job :
    relieveFifo [⟨9, 1, 90⟩, ⟨1, 1, 10⟩] 1 = some ([⟨9, 1, 90⟩], [⟨1, 1, 10⟩]) := by
  decide

/- ── The guard ─────────────────────────────────────────────────────────── -/

/-- A lot that can be relieved from.

Positive units, or nothing at all: a husk carrying basis is not a holding, and a
NEGATIVE holding is a short, which relief is not the operation for. -/
def sound (l : Lot) : Bool :=
  decide (0 < l.units) || (decide (l.units = 0) && decide (l.cost = 0))

/-- Relieve, refusing a malformed holding rather than consuming it.

⛔ A SEPARATE FUNCTION, LEAVING `relieveFifo` ALONE. Every theorem in
`Ratio.Lots` is about `relieveFifo` and all of them are true; the problem was
never what it computes but what it is handed. Rewriting it would have re-proved
seven theorems to fix an input check. -/
def relieveSound (ls : List Lot) (want : Int) : Option (List Taken × List Lot) :=
  if ls.all sound then relieveFifo ls want else none

/-- **The husk is refused rather than consumed.** -/
theorem a_husk_is_refused :
    relieveSound [⟨1, 0, 100⟩, ⟨2, 10, 200⟩] 5 = none := by
  decide

/-- **And a short lot is refused too.**

Negative units are a short position. Relief takes units OUT of a holding, and
taking units out of a negative one is not the same operation wearing a minus
sign — it is a purchase closing a short, which realizes a gain against a
proceeds figure recorded when the position was opened. -/
theorem a_short_lot_is_refused :
    relieveSound [⟨1, -5, 100⟩] 1 = none := by
  decide

/-- **A fully relieved lot is not a husk** — nothing held and nothing owed — so
it passes, and relieving over it behaves exactly as if it were absent. -/
theorem an_empty_lot_carrying_nothing_is_harmless :
    relieveSound [⟨1, 0, 0⟩, ⟨2, 10, 200⟩] 5
      = some ([⟨1, 0, 0⟩, ⟨2, 5, 100⟩], [⟨2, 5, 100⟩]) := by
  decide

/-- **And an ordinary holding is unchanged by the guard**, so nothing that
worked before now refuses. -/
theorem a_sound_holding_relieves_as_it_always_did (ls : List Lot) (want : Int)
    (h : ls.all sound = true) : relieveSound ls want = relieveFifo ls want := by
  simp [relieveSound, h]

/- ── Boundaries ────────────────────────────────────────────────────────── -/

/-- **Selling exactly what a lot holds takes the lot and leaves nothing of it.**

The boundary between the two branches of `relieveFifo`, which is where an
off-by-one lives: `≤` rather than `<` is what makes an exact sale take the whole
lot instead of splitting it into a zero-unit remainder — which would have
created precisely the husk this file is about. -/
theorem selling_a_whole_lot_leaves_no_husk :
    relieveFifo [⟨1, 10, 200⟩, ⟨2, 5, 50⟩] 10
      = some ([⟨1, 10, 200⟩], [⟨2, 5, 50⟩]) := by
  decide

/-- **Selling the entire holding leaves an empty list**, not a list of husks. -/
theorem selling_everything_leaves_nothing :
    relieveFifo [⟨1, 10, 200⟩, ⟨2, 5, 50⟩] 15 = some ([⟨1, 10, 200⟩, ⟨2, 5, 50⟩], []) := by
  decide

/-- **One unit more than the holding is refused**, rather than relieving what
there is and quietly reporting a smaller sale. -/
theorem one_unit_too_many_is_refused :
    relieveFifo [⟨1, 10, 200⟩, ⟨2, 5, 50⟩] 16 = none := by
  decide

/-- **A sale of nothing takes nothing and disturbs nothing.** -/
theorem a_sale_of_nothing_is_not_a_sale (ls : List Lot) :
    relieveFifo ls 0 = some ([], ls) := by
  cases ls <;> simp [relieveFifo]

end Ratio.Lots
