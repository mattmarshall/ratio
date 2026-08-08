set_option warningAsError true
/-! `Ratio.Period` — the day a fund is valuing, and what that constrains.

`ratio strike --as-of` made the caller supply a valuation date, because
"unpriced" is not a well-formed question until somebody says as of WHEN. That
works and it puts the burden in the wrong place: every caller has to know the
date, and nothing stops two of them disagreeing.

Carrying the date on the FUND is the other half. It is a small piece of state
with large consequences, so the consequences are settled here before any of it
is built.

⚠ TWO PRODUCT DECISIONS ARE ENCODED BELOW, and they are decisions rather than
theorems. They are stated as definitions so that disagreeing with one means
changing a definition and watching a proof fail, instead of discovering it in
behavior:

  1. **A skipped day is allowed, and visible.** Advancing does not require the
     day you are leaving to have been struck. Real funds skip days — a holiday,
     a fund that strikes weekly — and forbidding it would model a stricter
     world than the one that exists. What a gap must not be is INVISIBLE, so
     `struckOn` answers for any day and `advance_preserves_strikes` says
     leaving a day never erases what it held.

  2. **A day is struck at most once.** No restatement. A valuation point has
     one answer; replacing it would remove the first, and the first is what
     somebody was paid on. A restatement, when it is wanted, has to be a new
     kind of thing that cites what it supersedes — not a second value quietly
     occupying the same day.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Period

/-- A day, as an ordinal. Ordering is all any of this depends on. -/
abbrev Day := Int

/-- Where a fund is: the day it is valuing, and what it has struck.

`struck` is a function rather than a list because the question asked of it is
always "what was struck on this day", and a list would invite two answers for
one day — which is exactly what `one_answer_per_day` forbids. -/
structure Fund where
  valuing : Day
  struck : Day → Option Int

/-- A fund that has valued nothing. -/
def opened (d : Day) : Fund := ⟨d, fun _ => none⟩

/-- Move to a later day.

⛔ FORWARD ONLY. If the day could go backwards, a strike taken later could sit
at an earlier valuation point than one taken before it, and "the NAV as of the
30th" would depend on when you asked rather than on what happened. -/
def advance (f : Fund) (d : Day) : Option Fund :=
  if f.valuing < d then some { f with valuing := d } else none

/-- Whether a day has a NAV. -/
def struckOn (f : Fund) (d : Day) : Option Int := f.struck d

/-- Take the NAV for the day the fund is valuing.

Refuses if that day already has one — a valuation point has one answer. -/
def strike (f : Fund) (nav : Int) : Option Fund :=
  match f.struck f.valuing with
  | some _ => none
  | none =>
    some { f with struck := fun d => if d = f.valuing then some nav else f.struck d }

/- ── What the day being on the fund buys ───────────────────────────────── -/

/-- **The day only ever moves forward.** -/
theorem advance_only_forward (f g : Fund) (d : Day) (h : advance f d = some g) :
    f.valuing < g.valuing := by
  unfold advance at h
  by_cases hd : f.valuing < d
  · simp [hd] at h; subst h; simpa using hd
  · simp [hd] at h

/-- **Moving on never erases what a day held.**
The answer to "what happens to a struck NAV when the date advances": nothing.
A strike belongs to the day it was taken, not to the day the fund happens to be
working, so yesterday's figure survives every tomorrow. -/
theorem advance_preserves_strikes (f g : Fund) (d e : Day)
    (h : advance f d = some g) : struckOn g e = struckOn f e := by
  unfold advance at h
  by_cases hd : f.valuing < d
  · simp [hd] at h; subst h; rfl
  · simp [hd] at h

/-- **A day is struck at most once.** A second attempt at a day that already has
a NAV is refused, so the first answer cannot be replaced by a later one. -/
theorem one_answer_per_day (f : Fund) (nav prior : Int)
    (h : f.struck f.valuing = some prior) : strike f nav = none := by
  simp [strike, h]

/-- A strike lands on the day the fund is valuing, and nowhere else. -/
theorem strike_lands_on_the_day_being_valued (f g : Fund) (nav : Int)
    (h : strike f nav = some g) : struckOn g f.valuing = some nav := by
  unfold strike at h
  cases hs : f.struck f.valuing with
  | some p => rw [hs] at h; simp at h
  | none =>
    rw [hs] at h
    simp at h
    subst h
    simp [struckOn]

/-- **And it disturbs no other day.** Striking today does not invent, move or
erase a figure for any other valuation point. -/
theorem strike_disturbs_no_other_day (f g : Fund) (nav : Int) (e : Day)
    (hne : e ≠ f.valuing) (h : strike f nav = some g) :
    struckOn g e = struckOn f e := by
  unfold strike at h
  cases hs : f.struck f.valuing with
  | some p => rw [hs] at h; simp at h
  | none =>
    rw [hs] at h
    simp at h
    subst h
    simp [struckOn, hne]

/-- **A skipped day simply has no NAV**, and says so.

A gap is allowed — real funds skip days — and the point is that it is
answerable rather than silent. Asking a day the fund never valued returns
nothing, which is a different thing from returning zero. -/
theorem a_day_never_valued_has_no_nav (d : Day) (e : Day) :
    struckOn (opened d) e = none := rfl

/-- Advancing past a day without striking leaves it without a NAV, and leaves
every struck day alone. Skipping is a gap, not a loss. -/
theorem skipping_a_day_loses_nothing (f g : Fund) (d : Day)
    (h : advance f d = some g) :
    (∀ e, struckOn g e = struckOn f e) ∧ f.valuing < g.valuing :=
  ⟨fun e => advance_preserves_strikes f g d e h, advance_only_forward f g d h⟩

end Ratio.Period
