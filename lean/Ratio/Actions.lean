set_option warningAsError true
/-! `Ratio.Actions` — corporate actions, which behave like nothing else here.

`Ratio.Closure` showed that one unapplied corporate action costs more than an
entire quiet period end: twenty million tax lots are free and a single split is
not. So this is the term to get right, and it is the term whose behavior
contradicts the habits every other part of this system has taught.

⛔ THREE WAYS AN ACTION IS UNLIKE EVERYTHING ELSE MODELLED SO FAR.

  IT CONSERVES COST AND DELIBERATELY CHANGES UNITS. Relief conserves both. A
  mark conserves value. A split doubles the units and leaves the cost exactly
  where it was — so a check that "units did not move" is a check that the
  action did NOT happen.

  ⛔ IT IS NOT IDEMPOTENT. `Ratio.Valuation.mark_again_posts_nothing` says
  marking twice at one price is safe, and that safety is load-bearing — the
  console re-marks freely. APPLYING A SPLIT TWICE QUADRUPLES THE POSITION.
  `applying_twice_is_not_applying_once` is the theorem, and it exists because
  the opposite habit is already established in this codebase.

  IT IS RETROACTIVE. A split effective last Tuesday changes what a position
  held before it, so it cannot be a forward delta the way a price can.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Actions

/-- A lot, as `Ratio.Lots` has it: an acquisition ordinal, units, and the cost
of those units. -/
structure Lot where
  seq : Nat
  units : Int
  cost : Int
deriving DecidableEq, Repr

/-- A ratio, as an action declares it: `2 for 1`, `3 for 2`, `1 for 10`. -/
structure Ratio where
  /-- Units received. -/
  num : Int
  /-- Units given up. -/
  den : Int
deriving DecidableEq, Repr

/-- Split a lot.

Refuses when the units do not divide — a 3-for-2 split of a 5-unit lot is 7.5
units, and a fractional share is not a share. What happens to the fraction is
CASH IN LIEU, which realizes a gain and is therefore a posting rather than a
rounding, and belongs to the configuration that declares how the fund handles
it. Silently flooring it would lose part of a holding and balance perfectly. -/
def split (r : Ratio) (l : Lot) : Option Lot :=
  if r.den = 0 ∨ l.units * r.num % r.den ≠ 0 then none
  else some { l with units := l.units * r.num / r.den }

/-- **A split does not change what the holding cost.**

The entire economic content: the same money now buys a different number of
units. A book that revalued on a split would be inventing a gain that the
market did not give it. -/
theorem split_conserves_cost (r : Ratio) (l m : Lot) (h : split r l = some m) :
    m.cost = l.cost := by
  unfold split at h
  split at h
  · simp at h
  · simp at h; subst h; rfl

/-- **And it does change the units**, by exactly the declared ratio. -/
theorem split_scales_units (r : Ratio) (l m : Lot) (h : split r l = some m) :
    m.units * r.den = l.units * r.num := by
  unfold split at h
  split at h
  · simp at h
  · next hx =>
    simp at h; subst h
    simp only []
    simp only [not_or, ne_eq, Decidable.not_not] at hx
    exact Int.ediv_mul_cancel (Int.dvd_of_emod_eq_zero hx.2)

/-- **The acquisition ordinal survives.**

A split does not re-acquire the position. Holding period runs from the original
purchase, and FIFO order is unchanged — so `Ratio.Lots.relieve_touches_only_
what_it_takes` still describes what a later sale reads. An implementation that
re-sequenced on a split would silently re-age every lot and change which ones a
sale relieves. -/
theorem split_preserves_acquisition (r : Ratio) (l m : Lot) (h : split r l = some m) :
    m.seq = l.seq := by
  unfold split at h
  split at h
  · simp at h
  · simp at h; subst h; rfl

/-- **A split that does not divide is refused, not floored.**

Five units, three for two. Flooring gives seven and loses half a share; the
cost is unchanged either way, so the books TIE while the holding is short. The
same shape as the partial-lot rounding in `Ratio.Lots` and the mark-from-cost
bug: a balanced entry of the wrong size. -/
theorem an_indivisible_split_is_refused :
    split ⟨3, 2⟩ ⟨1, 5, 100⟩ = none := by
  simp [split]

/-- **⛔ APPLYING A SPLIT TWICE IS NOT APPLYING IT ONCE.**

This is the theorem that had to be written down, because everything else in
this system has taught the opposite reflex. Marking twice at one price posts
nothing (`mark_again_posts_nothing`). Admitting the same fact twice writes
nothing. Ingesting the same file twice reads nothing. Every one of those is
safe to retry, and the console retries them freely.

An action is not. Two-for-one applied twice is four-for-one, the cost is
unchanged so the trial balance still ties, and the position is silently double.
Whatever applies actions must record that it did — idempotence has to be built
around this, because it is not in it. -/
theorem applying_twice_is_not_applying_once :
    (split ⟨2, 1⟩ ⟨1, 100, 5000⟩ >>= split ⟨2, 1⟩) ≠ split ⟨2, 1⟩ ⟨1, 100, 5000⟩ := by
  simp [split]

/- ── Spin-offs: one basis, two instruments ─────────────────────────────── -/

/-- Split a cost basis between the parent and what came out of it.

`toSpun` is the fraction, in basis points, that moves to the new instrument. -/
def spinOff (bp : Int) (l : Lot) : Option (Lot × Lot) :=
  if bp < 0 ∨ 10000 < bp ∨ l.cost * bp % 10000 ≠ 0 then none
  else
    let moved := l.cost * bp / 10000
    some ({ l with cost := l.cost - moved }, { l with cost := moved })

/-- **A spin-off allocates the basis; it does not create any.**

What stays with the parent plus what leaves with the child is what was there.
A spin-off that did not sum would either invent basis — and with it a phantom
loss on the eventual sale — or lose it. -/
theorem spin_off_allocates_the_whole_basis (bp : Int) (l : Lot) (a b : Lot)
    (h : spinOff bp l = some (a, b)) : a.cost + b.cost = l.cost := by
  unfold spinOff at h
  split at h
  · simp at h
  · simp at h
    obtain ⟨h1, h2⟩ := h
    subst h1; subst h2
    simp

/-- And an allocation that does not divide exactly is refused, for the same
reason a partial lot relief is: which way to round is a term of an agreement,
not a property of arithmetic. -/
theorem an_indivisible_allocation_is_refused :
    spinOff 3333 ⟨1, 10, 100⟩ = none := by
  simp [spinOff]

/- ── Order ─────────────────────────────────────────────────────────────── -/

/-- Apply a list of splits in order. -/
def applyAll : List Ratio → Lot → Option Lot
  | [], l => some l
  | r :: rest, l =>
    match split r l with
    | none => none
    | some m => applyAll rest m

/-- **Actions do not commute**, so the order they are applied in is part of the
answer rather than an implementation detail.

A 3-for-2 then a 2-for-1 is not always a 2-for-1 then a 3-for-2: one order
divides and the other does not, so one succeeds and one refuses. Applying by
arrival order rather than by EX-DATE order is therefore not a performance
question — it changes whether the book can be brought up to date at all. -/
theorem actions_do_not_commute :
    applyAll [⟨2, 1⟩, ⟨3, 2⟩] ⟨1, 5, 100⟩ ≠ applyAll [⟨3, 2⟩, ⟨2, 1⟩] ⟨1, 5, 100⟩ := by
  simp [applyAll, split]

/-- Applying nothing changes nothing, so a book with no open actions is already
up to date — the base case `Ratio.Closure.a_quiet_day_is_the_chart_and_the
_currencies` is costing. -/
theorem no_actions_change_nothing (l : Lot) : applyAll [] l = some l := rfl

end Ratio.Actions
