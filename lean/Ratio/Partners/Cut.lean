set_option warningAsError true
/-! `Ratio.Partners.Cut` — a partner allocation cut, and the refuse.

`/capital` cites beginning → contributions → distributions → allocated
income / expense / unrealized → ending. The first four stocks and flows
are journal postings on partner capital. The allocated plugs are not:
the Investment chart has no partner dim on income, expense, or
Unrealized gain, and `allocate_*_lp` closes an exact amount into
capital (already on In / Out).

Without a cut those plugs stay **unset**. A silent 1/N of book NAV, or
a fabricated 0.00 share of a figure that moved, is the defect — the
books would still tie, and the number would be somebody else's.

This file is the cut, settled before the engine runs:

  1. **A cut is named weights, not a partner count.** `None` and an
     empty list are unset, not an equal split
     (`no_cut_is_unset`, `an_empty_cut_is_unset`). Writing equal
     weights is an election; inventing them from `length` is not.

  2. **A figure that will not divide is refused rather than rounded.**
     The same decision as `Ratio.Lots.partial_relief_is_exactly_pro_rata`.
     The remainder would be a misstatement of who owns the income, not
     a rounding error. Cross-multiplied: what a partner takes is to the
     figure as their weight is to the total.

  3. **When every slice divides, the shares sum to the figure.**
     Conservation of the cut, not of the journal — the journal already
     conserves. A cut that handed out more or less than the book figure
     would be a balanced lie.

  4. **A special allocation replaces the default cut for one kind.**
     100% of expense to the GP is a one-share cut of that kind, not a
     silent remainder after 1/N. No specials and no default is unset.

  5. **A journal fact is an exact amount.** `None` means this entry
     carries no special. `Some []` is elected and unnamed and refuses
     — the SpecID shape. Facts that cover the figure are the
     allocation; a remainder needs a cut and must divide; an overshoot
     refuses.

Lean core + `omega`, no Mathlib. ⛔ `warningAsError` is load-bearing. -/

namespace Ratio.Partners

/-- A partner, as an identifier. Rust maps the grain
(`Partner capital — LP` → `LP`) onto this. -/
abbrev Partner := Nat

/-- One named weight in a cut. -/
structure Share where
  partner : Partner
  weight : Int
  deriving DecidableEq, Repr

def weights : List Share → Int
  | [] => 0
  | s :: ss => s.weight + weights ss

def positive : List Share → Bool
  | [] => true
  | s :: ss => decide (s.weight > 0) && positive ss

def names : List Share → List Partner
  | [] => []
  | s :: ss => s.partner :: names ss

def nodup : List Partner → Bool
  | [] => true
  | p :: ps => !(ps.contains p) && nodup ps

/-- A cut is nonempty, every weight is positive, and no partner is
named twice. Two rows for one partner are two answers under one name. -/
def wellFormed (cut : List Share) : Bool :=
  !cut.isEmpty && positive cut && nodup (names cut)

/-- One partner's slice of `figure`, or nothing if it will not divide.

`total` is the sum of the cut's weights, not a partner count.
Dividing by `length` is the silent 1/N this exists to refuse. -/
def slice (figure weight total : Int) : Option Int :=
  if 0 < total then
    if (figure * weight) % total = 0 then
      some ((figure * weight) / total)
    else
      none
  else
    none

def slices (figure total : Int) : List Share → Option (List (Partner × Int))
  | [] => some []
  | s :: ss =>
    match slice figure s.weight total with
    | none => none
    | some a =>
      match slices figure total ss with
      | none => none
      | some rest => some ((s.partner, a) :: rest)

/-- Apply a cut to a figure.

`none` (nobody said) and a cut that is not well-formed stay unset.
There is no fallback to 1/N: an equal split is a method real funds
elect, so a book split under it by accident is indistinguishable from
one split under it by agreement. -/
def allocate (figure : Int) (cut : Option (List Share)) :
    Option (List (Partner × Int)) :=
  match cut with
  | none => none
  | some shares =>
    if wellFormed shares then
      slices figure (weights shares) shares
    else
      none

def allocatedSum : List (Partner × Int) → Int
  | [] => 0
  | p :: ps => p.2 + allocatedSum ps

/- ── Unset stays unset ──────────────────────────────────────────────── -/

theorem no_cut_is_unset (figure : Int) :
    allocate figure none = none := rfl

theorem an_empty_cut_is_unset (figure : Int) :
    allocate figure (some []) = none := by
  simp [allocate, wellFormed]

/-- **A silent 1/N is not an allocation.** Two partners and a 30 figure
would print 15 each. Nobody named that cut. -/
example : allocate 30 none = none := rfl

/-- A zero-weight row is not a cut. It would hand the whole figure to
the other names, or refuse in a way that looks like "the figure would
not divide" when the configuration is what is wrong. -/
example : allocate 100 (some [⟨0, 80⟩, ⟨1, 0⟩]) = none := by
  simp [allocate, wellFormed, positive]

/-- A duplicate partner is two answers under one name. -/
example : allocate 100 (some [⟨0, 50⟩, ⟨0, 50⟩]) = none := by
  simp [allocate, wellFormed, positive, names, nodup]

/- ── Exactness, and conservation of the cut ─────────────────────────── -/

theorem slice_exact (figure weight total a : Int)
    (h : slice figure weight total = some a) :
    a * total = figure * weight := by
  unfold slice at h
  split at h
  · next hpos =>
    split at h
    · next hdiv =>
      simp at h
      subst h
      have hd : total ∣ figure * weight := Int.dvd_of_emod_eq_zero hdiv
      exact Int.ediv_mul_cancel hd
    · simp at h
  · simp at h

theorem slices_sum (figure total : Int) :
    ∀ ss alloc,
      slices figure total ss = some alloc →
      allocatedSum alloc * total = figure * weights ss
  | [], alloc, h => by
    simp [slices] at h
    subst h
    simp [allocatedSum, weights]
  | s :: ss, alloc, h => by
    unfold slices at h
    cases hslice : slice figure s.weight total with
    | none => simp [hslice] at h
    | some a =>
      simp [hslice] at h
      cases hrest : slices figure total ss with
      | none => simp [hrest] at h
      | some rest =>
        simp [hrest] at h
        subst h
        have ih := slices_sum figure total ss rest hrest
        have hs := slice_exact figure s.weight total a hslice
        simp [allocatedSum, weights]
        rw [Int.add_mul, hs, ih]
        omega

theorem positive_weights_nonneg :
    ∀ ss, positive ss = true → 0 ≤ weights ss
  | [], _ => by simp [weights]
  | s :: ss, h => by
    simp [positive] at h
    have ih := positive_weights_nonneg ss h.2
    simp [weights]
    omega

theorem wellFormed_weights_pos (ss : List Share)
    (h : wellFormed ss = true) : 0 < weights ss := by
  simp [wellFormed] at h
  cases ss with
  | nil =>
    simp at h
  | cons s rest =>
    simp [positive, weights] at h ⊢
    have := positive_weights_nonneg rest h.1.2
    omega

/-- **When the cut applies, the shares are the figure.** Not a second
sum somebody could get wrong independently. A remainder here would be
a misstatement of taxable income, the same shape
`Ratio.Lots.partial_relief_is_exactly_pro_rata` refuses to round. -/
theorem allocated_shares_sum_to_the_figure
    (figure : Int) (shares : List Share) (alloc : List (Partner × Int))
    (h : allocate figure (some shares) = some alloc) :
    allocatedSum alloc = figure := by
  simp [allocate] at h
  obtain ⟨hw, hslices⟩ := h
  have hsum := slices_sum figure (weights shares) shares alloc hslices
  have hpos := wellFormed_weights_pos shares hw
  have : weights shares ≠ 0 := by omega
  exact Int.eq_of_mul_eq_mul_right this hsum

/-- **What a partner takes is exactly pro rata.** Cross-multiplied
because the whole point is that the division was exact. -/
theorem a_slice_is_exactly_pro_rata
    (figure weight total a : Int)
    (h : slice figure weight total = some a) :
    a * total = figure * weight :=
  slice_exact figure weight total a h

/-- A figure that will not divide is refused, not rounded. 80/20 of
101 would be 80.8 — flooring would leave 0.20 nowhere, and the books
would still tie. -/
example : allocate 101 (some [⟨0, 80⟩, ⟨1, 20⟩]) = none := by
  decide

/-- 80/20 of 100 is 80 and 20. The load-bearing holding: the shares
are the figure, not an equal split of it. -/
example : allocate 100 (some [⟨0, 80⟩, ⟨1, 20⟩]) = some [(0, 80), (1, 20)] := by
  decide

/-- Equal weights that divide are an election, not a silent 1/N.
Somebody wrote 1, 1. 30 divides. -/
example : allocate 30 (some [⟨0, 1⟩, ⟨1, 1⟩]) = some [(0, 15), (1, 15)] := by
  decide

/-- Equal weights that do not divide refuse. 31 / 2 has a remainder. -/
example : allocate 31 (some [⟨0, 1⟩, ⟨1, 1⟩]) = none := by
  decide

/- ── Special allocations, by kind ───────────────────────────────────── -/

/-- The book figure a special or a default cut applies to. -/
inductive Kind where
  | income
  | expense
  | unrealized
  deriving DecidableEq, Repr

/-- A standing special: this partner's weight of this kind, replacing
the default cut for that kind. -/
structure Special where
  partner : Partner
  kind : Kind
  weight : Int
  deriving DecidableEq, Repr

def specialsFor (k : Kind) : List Special → List Share
  | [] => []
  | s :: ss =>
    if s.kind = k then
      ⟨s.partner, s.weight⟩ :: specialsFor k ss
    else
      specialsFor k ss

/-- The cut that applies to a kind: the specials if any were named,
otherwise the default. Empty specials are silence, not a zero-share
cut of that kind. -/
def cutFor (k : Kind) (default : Option (List Share)) (specials : List Special) :
    Option (List Share) :=
  match specialsFor k specials with
  | [] => default
  | s :: ss => some (s :: ss)

theorem a_kind_without_specials_uses_the_default
    (k : Kind) (c : List Share) :
    cutFor k (some c) [] = some c := by
  simp [cutFor, specialsFor]

theorem no_cut_and_no_specials_is_unset (k : Kind) :
    cutFor k none [] = none := rfl

/-- 100% of expense to partner 1, default 80/20 elsewhere. -/
example :
    cutFor .expense (some [⟨0, 80⟩, ⟨1, 20⟩]) [⟨1, .expense, 1⟩]
      = some [⟨1, 1⟩] := by
  decide

example :
    allocate 50 (cutFor .expense (some [⟨0, 80⟩, ⟨1, 20⟩]) [⟨1, .expense, 1⟩])
      = some [(1, 50)] := by
  decide

example :
    allocate 100 (cutFor .income (some [⟨0, 80⟩, ⟨1, 20⟩]) [⟨1, .expense, 1⟩])
      = some [(0, 80), (1, 20)] := by
  decide

/- ── Journal facts: exact amounts ───────────────────────────────────── -/

/-- An exact amount named on an entry. Not a weight. -/
structure Fact where
  partner : Partner
  amount : Int
  deriving DecidableEq, Repr

def factSum : List Fact → Int
  | [] => 0
  | f :: fs => f.amount + factSum fs

def factsAsAlloc : List Fact → List (Partner × Int)
  | [] => []
  | f :: fs => (f.partner, f.amount) :: factsAsAlloc fs

theorem facts_sum_is_the_amounts :
    ∀ fs, allocatedSum (factsAsAlloc fs) = factSum fs
  | [] => rfl
  | f :: fs => by
    simp [factsAsAlloc, factSum, allocatedSum]
    exact facts_sum_is_the_amounts fs

/-- Apply journal specials, then the remainder cut.

`none` facts fall through to the cut (which may itself be unset).
`some []` is elected and unnamed — refuse, do not invent 1/N.
Facts that cover the figure are the allocation. A remainder needs a
cut that divides. An overshoot refuses. -/
def applyFacts (figure : Int) (facts : Option (List Fact))
    (remainder : Option (List Share)) : Option (List (Partner × Int)) :=
  match facts with
  | none => allocate figure remainder
  | some [] => none
  | some fs =>
    let taken := factSum fs
    if taken = figure then
      some (factsAsAlloc fs)
    else if taken < figure then
      match allocate (figure - taken) remainder with
      | none => none
      | some rest => some (factsAsAlloc fs ++ rest)
    else
      none

theorem unnamed_facts_refuse (figure : Int) (cut : Option (List Share)) :
    applyFacts figure (some []) cut = none := by
  simp [applyFacts]

theorem no_facts_fall_through_to_the_cut (figure : Int) (cut : Option (List Share)) :
    applyFacts figure none cut = allocate figure cut := rfl

theorem no_facts_and_no_cut_is_unset (figure : Int) :
    applyFacts figure none none = none := rfl

theorem facts_that_cover_the_figure_are_the_allocation
    (f : Fact) (fs : List Fact) :
    applyFacts (factSum (f :: fs)) (some (f :: fs)) none
      = some (factsAsAlloc (f :: fs)) := by
  simp [applyFacts]

theorem an_overshoot_refuses (cut : Option (List Share)) :
    applyFacts 10 (some [⟨0, 12⟩]) cut = none := by
  simp [applyFacts, factSum]

/-- Facts plus a remainder that divides conserve. 40 named + 80/20 of
the leftover 60 is 40 + 48 + 12 = 100. -/
example :
    applyFacts 100 (some [⟨2, 40⟩]) (some [⟨0, 80⟩, ⟨1, 20⟩])
      = some [(2, 40), (0, 48), (1, 12)] := by
  decide

/-- A remainder without a cut is unset, not a silent split of what is
left. -/
example : applyFacts 100 (some [⟨2, 40⟩]) none = none := by
  decide

end Ratio.Partners
