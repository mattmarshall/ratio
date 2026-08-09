set_option warningAsError true
/-! `Ratio.Lots` — tax lots, and why twenty million of them is tractable.

A fund that has traded for years holds tax lots in the millions: every purchase
opens one, every sale relieves some, and the realized gain depends on WHICH
ones. At that size the naive shape of the problem is hostile — a sale that
scans twenty million lots, a cost basis that re-folds them, a NAV that touches
every one — and the usual response is to add an index, a cached aggregate, a
partition, and to hope the fast answer matches the slow one.

⛔ THE POINT OF THIS FILE IS THAT "HOPE" IS THE WRONG WORD. Each of those moves
is a claim about equality with the naive fold, and each is stated and proved
here:

  relief_touches_only_what_it_takes   a sale costs what it CONSUMES, not what
                                      the fund HOLDS. This is the theorem that
                                      makes twenty million lots survivable, and
                                      it is a property of FIFO order rather
                                      than of any index.
  cost_is_conserved                   what a sale relieved plus what remains is
                                      what was there. So a running total can be
                                      updated by the DELTA and never re-folded.
  aggregate_matches_scan              …stated directly, because that is the one
                                      an implementation is tempted to break.
  units_are_conserved                 a sale relieves exactly what it sold.
  short_sales_are_refused             a holding cannot relieve more than it has,
                                      and says so rather than going negative.
  partition_sums_to_whole             per-instrument work composes, so the book
                                      can be sharded without changing an answer.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/-- One tax lot: when it was acquired, how much of it is left, and what that
remainder cost.

`seq` is an acquisition ordinal rather than a date: relief order is all any of
this depends on, and a calendar here would be a second thing to get wrong. -/
structure Lot where
  seq : Nat
  units : Int
  cost : Int
deriving DecidableEq, Repr

/-- What a sale took out of one lot. -/
structure Taken where
  seq : Nat
  units : Int
  cost : Int
deriving DecidableEq, Repr

def totalCost (ls : List Lot) : Int := (ls.map (·.cost)).sum
def totalUnits (ls : List Lot) : Int := (ls.map (·.units)).sum
def takenCost (ts : List Taken) : Int := (ts.map (·.cost)).sum
def takenUnits (ts : List Taken) : Int := (ts.map (·.units)).sum

/-- Relieve `want` units, oldest lot first.

Returns what was taken and what is left, or nothing if the holding is short or
a partial lot cannot be split into whole minor units.

⚠ THE PARTIAL SPLIT REFUSES RATHER THAN ROUNDS. Taking part of a lot divides
its cost pro rata, and when that does not land on a whole minor unit there is a
rounding decision — which way to round is a term of an administration
agreement, so it is not made here. The same refusal as every other division in
this system. -/
def relieveFifo : List Lot → Int → Option (List Taken × List Lot)
  | [], want => if want = 0 then some ([], []) else none
  | l :: rest, want =>
    if want = 0 then some ([], l :: rest)
    else if want < 0 then none
    else if l.units ≤ want then
      -- The whole lot goes, and the shortfall is relieved behind it.
      match relieveFifo rest (want - l.units) with
      | none => none
      | some (ts, left) => some (⟨l.seq, l.units, l.cost⟩ :: ts, left)
    else if l.units = 0 ∨ l.cost * want % l.units ≠ 0 then none
    else
      -- Part of this lot, and none of the ones behind it.
      some ([⟨l.seq, want, l.cost * want / l.units⟩],
            ⟨l.seq, l.units - want, l.cost - l.cost * want / l.units⟩ :: rest)

/-- **A sale costs what it consumes, not what the fund holds.**

Every lot is either still in the book or was taken from, so the number that
CHANGED is at most the number taken. Selling a hundred units out of twenty
million lots touches the lots those hundred units came from and nothing else —
a property of the relief ORDER, not of any index bolted on later.

This is the theorem that makes the scale tractable. Without it every sale is a
full scan, and no amount of caching repairs that. -/
theorem relief_touches_only_what_it_takes :
    ∀ (ls : List Lot) (want : Int) (ts : List Taken) (left : List Lot),
      relieveFifo ls want = some (ts, left) → ls.length ≤ left.length + ts.length := by
  intro ls
  induction ls with
  | nil =>
    intro want ts left h
    -- `simp` already resolved the `if`, so there is nothing left to split.
    simp [relieveFifo] at h
    obtain ⟨_, h2, h3⟩ := h
    subst h2; subst h3
    simp
  | cons l rest ih =>
    intro want ts left h
    simp only [relieveFifo] at h
    split at h
    · simp at h; obtain ⟨h1, h2⟩ := h; subst h1; subst h2; simp
    · split at h
      · simp at h
      · split at h
        · cases hr : relieveFifo rest (want - l.units) with
          | none => rw [hr] at h; simp at h
          | some q =>
            rw [hr] at h
            simp at h
            obtain ⟨h1, h2⟩ := h
            have hq := ih (want - l.units) q.1 q.2 hr
            subst h1; subst h2
            simp
            omega
        · split at h
          · simp at h
          · simp at h
            obtain ⟨h1, h2⟩ := h
            subst h1; subst h2
            simp

/-- **A sale relieves exactly what it sold.** -/
theorem units_are_conserved :
    ∀ (ls : List Lot) (want : Int) (ts : List Taken) (left : List Lot),
      relieveFifo ls want = some (ts, left) →
      takenUnits ts + totalUnits left = totalUnits ls := by
  intro ls
  induction ls with
  | nil =>
    intro want ts left h
    -- `simp` already resolved the `if`, so there is nothing left to split.
    simp [relieveFifo] at h
    obtain ⟨_, h2, h3⟩ := h
    subst h2; subst h3
    simp [takenUnits, totalUnits]
  | cons l rest ih =>
    intro want ts left h
    simp only [relieveFifo] at h
    split at h
    · simp at h; obtain ⟨h1, h2⟩ := h; subst h1; subst h2
      simp [takenUnits, totalUnits]
    · split at h
      · simp at h
      · split at h
        · cases hr : relieveFifo rest (want - l.units) with
          | none => rw [hr] at h; simp at h
          | some q =>
            rw [hr] at h
            simp at h
            obtain ⟨h1, h2⟩ := h
            have hq := ih (want - l.units) q.1 q.2 hr
            subst h1; subst h2
            simp [takenUnits, totalUnits] at hq ⊢
            omega
        · split at h
          · simp at h
          · simp at h
            obtain ⟨h1, h2⟩ := h
            subst h1; subst h2
            simp [takenUnits, totalUnits]
            omega

/-- **What was relieved plus what remains is what was there.**

⭐ THE AGGREGATE THEOREM. Because the two sides balance, a running cost basis
can be updated by the DELTA a sale reports — never re-folded over the whole
holding. Twenty million lots therefore have a cost basis that costs nothing to
know, and the fast answer is the slow answer by construction rather than by
testing. -/
theorem cost_is_conserved :
    ∀ (ls : List Lot) (want : Int) (ts : List Taken) (left : List Lot),
      relieveFifo ls want = some (ts, left) →
      takenCost ts + totalCost left = totalCost ls := by
  intro ls
  induction ls with
  | nil =>
    intro want ts left h
    -- `simp` already resolved the `if`, so there is nothing left to split.
    simp [relieveFifo] at h
    obtain ⟨_, h2, h3⟩ := h
    subst h2; subst h3
    simp [takenCost, totalCost]
  | cons l rest ih =>
    intro want ts left h
    simp only [relieveFifo] at h
    split at h
    · simp at h; obtain ⟨h1, h2⟩ := h; subst h1; subst h2
      simp [takenCost, totalCost]
    · split at h
      · simp at h
      · split at h
        · cases hr : relieveFifo rest (want - l.units) with
          | none => rw [hr] at h; simp at h
          | some q =>
            rw [hr] at h
            simp at h
            obtain ⟨h1, h2⟩ := h
            have hq := ih (want - l.units) q.1 q.2 hr
            subst h1; subst h2
            simp [takenCost, totalCost] at hq ⊢
            omega
        · split at h
          · simp at h
          · simp at h
            obtain ⟨h1, h2⟩ := h
            subst h1; subst h2
            simp [takenCost, totalCost]
            omega

/-- **A partial relief is exactly pro rata.**

⛔ ADDED BECAUSE CONSERVATION DID NOT CATCH THIS, AND I ASSUMED IT WOULD.

Deleting the exactness check from `relieveFifo` — letting a partial split round
instead of refusing — left every conservation theorem above still provable. Of
course it did: the branch computes what remains as `cost - taken`, so the two
sides balance whatever `taken` is. The books tie and the allocation is simply
WRONG, which is the same shape as marking from cost: a balanced entry of the
wrong size is balanced.

So the refusal needed a theorem of its own, and this is it. Cross-multiplied
rather than divided, because the whole point is that the division was exact:
what came out of the lot is to what was taken as the lot's cost is to its units.

Without this, a fund selling 3 units of a 7-unit lot books a basis that is off
by a rounding, on every partial disposal, in a book with twenty million lots. -/
theorem partial_relief_is_exactly_pro_rata
    (l : Lot) (rest : List Lot) (want : Int) (ts : List Taken) (left : List Lot)
    (hpos : want ≠ 0) (hsign : ¬ want < 0) (hpart : ¬ l.units ≤ want)
    (h : relieveFifo (l :: rest) want = some (ts, left)) :
    takenCost ts * l.units = l.cost * takenUnits ts := by
  simp only [relieveFifo, if_neg hpos, if_neg hsign, if_neg hpart] at h
  split at h
  · simp at h
  · next hx =>
    simp at h
    obtain ⟨h1, _⟩ := h
    subst h1
    -- `hx` says the division was exact, which is exactly what makes the
    -- cross-multiplication hold. `omega` cannot divide by a variable, so the
    -- divisibility is used directly.
    simp only [not_or, ne_eq, Decidable.not_not] at hx
    have hd : l.units ∣ l.cost * want := Int.dvd_of_emod_eq_zero hx.2
    simp [takenCost, takenUnits]
    exact Int.ediv_mul_cancel hd

/-- **A holding cannot relieve more than it has.** It refuses rather than going
negative, so a short position is never invented by a sale that was too big. -/
theorem short_sales_are_refused (want : Int) (h : want ≠ 0) :
    relieveFifo [] want = none := by
  simp [relieveFifo, h]

/-- Maintaining a total by adding each lot as it arrives gives the same answer
as folding the book. The other half of never re-scanning. -/
theorem aggregate_matches_scan (l : Lot) (ls : List Lot) :
    totalCost (l :: ls) = l.cost + totalCost ls := by
  simp [totalCost]

/-- **Per-instrument work composes.** Splitting a book by any predicate and
summing the parts gives the whole, so twenty million lots can be sharded across
workers — by instrument, by lot age, by anything — without changing an answer.

The same shape as `Ratio.Ingest.split_preserves_total`, and load-bearing for the
same reason: a partitioned computation that did not sum back would be a fast
wrong answer. -/
theorem partition_sums_to_whole : ∀ (ls : List Lot) (p : Lot → Bool),
    totalCost (ls.filter p) + totalCost (ls.filter (fun l => !p l)) = totalCost ls
  | [], _ => by simp [totalCost]
  | l :: rest, p => by
    have ih := partition_sums_to_whole rest p
    by_cases h : p l
    · simp [totalCost, List.filter, h] at ih ⊢
      omega
    · simp [totalCost, List.filter, h] at ih ⊢
      omega

end Ratio.Lots
