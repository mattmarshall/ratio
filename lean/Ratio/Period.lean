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

  2. **A day is struck at most once, PER VIEW.** No restatement. A valuation
     point has one answer; replacing it would remove the first, and the first
     is what somebody was paid on. A restatement, when it is wanted, has to be
     a new kind of thing that cites what it supersedes — not a second value
     quietly occupying the same day.

     ⚠ **THE INDEX BY VIEW IS A WIDENING, NOT A WEAKENING, AND THE DIFFERENCE
     IS WORTH BEING EXACT ABOUT.** A fund keeps more than one book of record:
     the accounting book recognises a trade when it is struck, a settlement
     book when cash and stock move some days later (`Ratio.Views`). Those are
     two answers to two DIFFERENT questions about one day, and neither
     supersedes the other. What is still refused is what was always refused: a
     second answer to the SAME question. `struck` was `Day → Option Int` and is
     now `View → Day → Option Int`, so the refusal is per view and nothing that
     used to be forbidden has become allowed.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Period

/-- A day, as an ordinal. Ordering is all any of this depends on. -/
abbrev Day := Int

/-- A book of record, as an identifier. `Ratio.Views` is what one IS. -/
abbrev View := String

/-- Where a fund is: the day it is valuing, and what each of its books has struck.

`struck` is a function rather than a list because the question asked of it is
always "what did THIS VIEW strike on this day", and a list would invite two
answers to one question — which is exactly what `one_answer_per_view_per_day`
forbids.

⚠ THE DAY IS NOT INDEXED BY VIEW AND MUST NOT BECOME SO. Every book of record a
fund keeps is valuing the same day; they differ in which entries they recognise
by then, not in what the date is. Two views at two valuation points would be two
funds. -/
structure Fund where
  valuing : Day
  struck : View → Day → Option Int

/-- A fund that has valued nothing. -/
def opened (d : Day) : Fund := ⟨d, fun _ _ => none⟩

/-- Move to a later day.

⛔ FORWARD ONLY. If the day could go backwards, a strike taken later could sit
at an earlier valuation point than one taken before it, and "the NAV as of the
30th" would depend on when you asked rather than on what happened. -/
def advance (f : Fund) (d : Day) : Option Fund :=
  if f.valuing < d then some { f with valuing := d } else none

/-- Whether a day has a NAV, in one view. -/
def struckOn (f : Fund) (v : View) (d : Day) : Option Int := f.struck v d

/-- Take the NAV for the day the fund is valuing.

Refuses if that view already has one for that day — a valuation point has one
answer PER BOOK OF RECORD. -/
def strike (f : Fund) (v : View) (nav : Int) : Option Fund :=
  match f.struck v f.valuing with
  | some _ => none
  | none =>
    some { f with
      struck := fun w d => if w = v ∧ d = f.valuing then some nav else f.struck w d }

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
theorem advance_preserves_strikes (f g : Fund) (v : View) (d e : Day)
    (h : advance f d = some g) : struckOn g v e = struckOn f v e := by
  unfold advance at h
  by_cases hd : f.valuing < d
  · simp [hd] at h; subst h; rfl
  · simp [hd] at h

/-- **A day is struck at most once, in each book of record.** A second attempt
at a day that view already has a NAV for is refused, so the first answer cannot
be replaced by a later one.

⚠ RENAMED FROM `one_answer_per_day`, AND FOUR RUST FILES CITE IT BY NAME. The
statement is the same refusal it always was; what changed is that the question
it is about now names a view. -/
theorem one_answer_per_view_per_day (f : Fund) (v : View) (nav prior : Int)
    (h : f.struck v f.valuing = some prior) : strike f v nav = none := by
  simp [strike, h]

/-- A strike lands on the day the fund is valuing, and nowhere else. -/
theorem strike_lands_on_the_day_being_valued (f g : Fund) (v : View) (nav : Int)
    (h : strike f v nav = some g) : struckOn g v f.valuing = some nav := by
  unfold strike at h
  cases hs : f.struck v f.valuing with
  | some p => rw [hs] at h; simp at h
  | none =>
    rw [hs] at h
    simp at h
    subst h
    simp [struckOn]

/-- **And it disturbs no other day.** Striking today does not invent, move or
erase a figure for any other valuation point. -/
theorem strike_disturbs_no_other_day (f g : Fund) (v : View) (nav : Int) (e : Day)
    (hne : e ≠ f.valuing) (h : strike f v nav = some g) :
    struckOn g v e = struckOn f v e := by
  unfold strike at h
  cases hs : f.struck v f.valuing with
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
theorem a_day_never_valued_has_no_nav (v : View) (d : Day) (e : Day) :
    struckOn (opened d) v e = none := rfl

/-- Advancing past a day without striking leaves it without a NAV, and leaves
every struck day alone. Skipping is a gap, not a loss. -/
theorem skipping_a_day_loses_nothing (f g : Fund) (d : Day)
    (h : advance f d = some g) :
    (∀ v e, struckOn g v e = struckOn f v e) ∧ f.valuing < g.valuing :=
  ⟨fun v e => advance_preserves_strikes f g v d e h, advance_only_forward f g d h⟩

/-- **A view that never struck a day says so, rather than borrowing another's
figure.**

⛔ THE SCREEN THIS IS ABOUT. A fund's accounting book may be struck for the 30th
while its settlement book is not, and a console falling back to the sibling
would show a figure produced under a convention nobody asked about — labelled
with the one they did. An em dash is the honest answer, and this is what makes
it available. -/
theorem a_view_that_never_struck_a_day_does_not_borrow_another
    (f g : Fund) (v w : View) (hvw : v ≠ w) (nav : Int)
    (h : strike f v nav = some g) (hw : f.struck w f.valuing = none) :
    struckOn g w f.valuing = none := by
  unfold strike at h
  cases hs : f.struck v f.valuing with
  | some p => rw [hs] at h; simp at h
  | none =>
      rw [hs] at h
      simp at h
      subst h
      simpa [struckOn, Ne.symm hvw] using hw

/-- Striking does not move the day the fund is valuing. -/
theorem strike_keeps_the_valuation_day (f g : Fund) (v : View) (nav : Int)
    (h : strike f v nav = some g) : g.valuing = f.valuing := by
  unfold strike at h
  cases hs : f.struck v f.valuing with
  | some p => rw [hs] at h; simp at h
  | none => rw [hs] at h; simp at h; subst h; rfl

/-- **Two views are two answers, and neither restates the other.**

⭐ THE FACT A READER LOOKS FOR IMMEDIATELY AFTER `one_answer_per_view_per_day`,
and the one that makes multi-view books possible at all. Having struck the
accounting book for a day, the settlement book can still be struck for the SAME
day — and the first figure is untouched.

That is not a restatement. A restatement replaces the answer to ONE question;
these are answers to two, the same journal read under two recognition
conventions (`Ratio.Views.the_view_decides_the_figure`). The refusal that
remains is the one that always mattered: a second answer to the same question,
which `one_answer_per_view_per_day` still forbids. -/
theorem two_views_are_two_answers_and_neither_restates_the_other
    (f g : Fund) (v w : View) (hvw : v ≠ w) (nav second : Int)
    (h : strike f v nav = some g) (hw : f.struck w f.valuing = none) :
    (strike g w second).isSome ∧ struckOn g v f.valuing = some nav := by
  have hday : g.valuing = f.valuing := strike_keeps_the_valuation_day f g v nav h
  have hnone : struckOn g w f.valuing = none :=
    a_view_that_never_struck_a_day_does_not_borrow_another f g v w hvw nav h hw
  refine ⟨?_, strike_lands_on_the_day_being_valued f g v nav h⟩
  unfold strike
  rw [hday]
  simp only [struckOn] at hnone
  rw [hnone]
  simp

end Ratio.Period
