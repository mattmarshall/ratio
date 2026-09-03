set_option warningAsError true
/-! `Ratio.Close` — a period close: the surplus roll-forward, and the refuse.

The console already shows `surplus = income + expenses` as the residual that
makes a sheet foot **while the books have not closed**. That is a picture of
an open period. A close is a different thing: a citeable boundary, a conserved
posting that carries the residual into equity, and a door that refuses a
later entry dated on or before the day that was closed.

`Ratio.Period` is the NAV valuation day. `CloseGate` is whether a strike may
be taken over an unexplained break. Neither is this. A period close is a
fiscal boundary per book of record, and the two questions that decide a
figure here are settled before any of it is built:

  1. **The closing posting conserves, and the residual it carries is the
     surplus.** Negating every income and expense balance and posting their
     sum to an equity destination is a conserved template
     (`closing_conserves`). The amount that lands on equity *is* income plus
     expenses (`the_equity_leg_is_the_surplus`) — not a second sum somebody
     could get wrong independently. A remainder here would be a
     misstatement of taxable income, the same shape
     `Ratio.Lots.partial_relief_is_exactly_pro_rata` refuses to round.

  2. **Unset stays unset.** A missing equity destination refuses the close
     (`missing_destination_refuses_the_close`). A missing beginning or a
     missing surplus leaves the roll-forward unset, not a measured zero
     (`missing_beginning_is_unset`, `missing_surplus_is_unset`). Defaulting
     either to zero would make an unclosed book indistinguishable from one
     that closed at nothing.

  3. **A dated entry on or before the closed-through day is refused.** An
     undated entry has no period and is not in one (`an_undated_entry_is_
     not_in_a_period`). Append-only storage is not this door.

  4. **A close only moves forward, per view.** A second close of the same
     through-date is refused. Extending to a later day is the one allowed
     step.

Lean core + `omega`, no Mathlib. ⛔ `warningAsError` is load-bearing — see
`Ratio.Chart`. -/

namespace Ratio.Close

/-- A calendar day, as an ordinal. Ordering is all any of this depends on. -/
abbrev Day := Int

/-- A book of record. `Ratio.Views` is what one IS. -/
abbrev View := String

/-- A chart dimension. -/
abbrev Dim := Int

/-- What a close may move. Income and expense are temporary; equity is the
destination; everything else is left alone. -/
inductive Kind where
  | income
  | expense
  | equity
  | other
  deriving DecidableEq, Repr

structure Account where
  dim : Dim
  kind : Kind
  balance : Int

/- ── Surplus, and the closing template ─────────────────────────────────── -/

/-- Temporary-account balances, in raw debit-minus-credit. A profitable
period is negative, the way equity is. -/
def tempSum : List Account → Int
  | [] => 0
  | a :: as =>
    if a.kind = .income ∨ a.kind = .expense then
      a.balance + tempSum as
    else
      tempSum as

/-- The surplus the sheet already shows: income + expenses. -/
def surplus (as : List Account) : Int := tempSum as

def sumLegs : List (Dim × Int) → Int
  | [] => 0
  | p :: ps => p.2 + sumLegs ps

/-- Negate every temporary balance. The other half of the close. -/
def negTemps : List Account → List (Dim × Int)
  | [] => []
  | a :: as =>
    if a.kind = .income ∨ a.kind = .expense then
      (a.dim, -a.balance) :: negTemps as
    else
      negTemps as

/-- Closing postings: zero the temporaries and carry their sum to `dest`. -/
def closingLegs (as : List Account) (dest : Dim) : List (Dim × Int) :=
  negTemps as ++ [(dest, tempSum as)]

theorem sumLegs_append (xs ys : List (Dim × Int)) :
    sumLegs (xs ++ ys) = sumLegs xs + sumLegs ys := by
  induction xs with
  | nil => simp [sumLegs]
  | cons p xs ih =>
    simp [sumLegs, ih]
    omega

theorem neg_temps_sum (as : List Account) :
    sumLegs (negTemps as) = - tempSum as := by
  induction as with
  | nil => rfl
  | cons a as ih =>
    unfold negTemps tempSum
    split
    · simp [sumLegs, ih]; omega
    · exact ih

/-- **The closing posting conserves.** Negating the temporaries and posting
their sum to equity nets to zero at every chart, which is why a close can
be a journal entry rather than a side-plane adjustment. -/
theorem closing_conserves (as : List Account) (dest : Dim) :
    sumLegs (closingLegs as dest) = 0 := by
  unfold closingLegs
  rw [sumLegs_append]
  simp [sumLegs]
  rw [neg_temps_sum]
  omega

/-- **The equity leg is the surplus**, not a second figure. The amount that
lands on retained earnings is income plus expenses in raw
debit-minus-credit — the same residual `sheetTotals.surplus` already
shows. Two sums that were allowed to disagree would be the books tying
on the wrong taxable income. -/
theorem the_equity_leg_is_the_surplus (as : List Account) (dest : Dim) :
    (closingLegs as dest).getLast? = some (dest, surplus as) := by
  unfold closingLegs surplus
  exact List.getLast?_concat (negTemps as) (dest, tempSum as)

/- ── Unset stays unset ─────────────────────────────────────────────────── -/

/-- A close without a named equity destination is refused, not pointed at
Opening equity or Funding by habit. -/
def closeBooks (as : List Account) (dest : Option Dim) : Option (List (Dim × Int)) :=
  match dest with
  | none => none
  | some d =>
    match as.find? (fun a => a.dim = d ∧ a.kind = .equity) with
    | none => none
    | some _ => some (closingLegs as d)

theorem missing_destination_refuses_the_close (as : List Account) :
    closeBooks as none = none := rfl

/-- Beginning + surplus + named adjustments. Any missing cut stays unset
rather than becoming a measured zero. -/
def rollForward (beginning surplus adj : Option Int) : Option Int :=
  match beginning, surplus with
  | some b, some s =>
    match adj with
    | some a => some (b + s + a)
    | none => some (b + s)
  | _, _ => none

theorem missing_beginning_is_unset (s : Int) (a : Option Int) :
    rollForward none (some s) a = none := by
  cases a <;> rfl

theorem missing_surplus_is_unset (b : Int) (a : Option Int) :
    rollForward (some b) none a = none := by
  cases a <;> rfl

/-- **The roll-forward ties when every cut is present.** Beginning retained
earnings plus period surplus plus named adjustments is ending retained
earnings — not a picture that foots by dropping a term. -/
theorem the_roll_forward_ties (b s a : Int) :
    rollForward (some b) (some s) (some a) = some (b + s + a) := rfl

theorem a_roll_forward_without_adjustments_is_beginning_plus_surplus
    (b s : Int) : rollForward (some b) (some s) none = some (b + s) := rfl

/- ── The door ──────────────────────────────────────────────────────────── -/

/-- Whether a posting dated `day` is refused by a close through `through`.

An undated entry has no period: period folds already skip it, and treating
it as "now" or as "epoch" would put it in a window nobody elected. -/
def refused (through : Day) (day : Option Day) : Bool :=
  match day with
  | none => false
  | some d => if d ≤ through then true else false

/-- **A dated entry on or before the closed-through day is refused.**
Append-only storage is not this. -/
theorem a_dated_entry_on_or_before_close_is_refused (through d : Day)
    (h : d ≤ through) : refused through (some d) = true := by
  simp [refused, h]

/-- **An undated entry is not in a period**, so the close door does not
claim it. The period fold already drops it; this matches that refusal
rather than inventing a day. -/
theorem an_undated_entry_is_not_in_a_period (through : Day) :
    refused through none = false := rfl

/-- A later day is the open period, and the door leaves it alone. -/
theorem a_later_day_is_not_refused (through d : Day) (h : through < d) :
    refused through (some d) = false := by
  simp [refused]
  omega

/- ── A close only moves forward ────────────────────────────────────────── -/

/-- Record a close through `d`. Refuses if that view is already closed
through `d` or later. -/
def close (prior : Option Day) (d : Day) : Option Day :=
  match prior with
  | none => some d
  | some t => if d ≤ t then none else some d

theorem an_open_book_accepts_a_first_close (d : Day) :
    close none d = some d := rfl

/-- **A second close of the same through-date is refused.** One answer per
view per period, the same shape as `Ratio.Period.one_answer_per_view_per_day`
and for the same reason: replacing it would hide the first, and the first
is what somebody signed. -/
theorem a_second_close_of_the_same_through_is_refused (d : Day) :
    close (some d) d = none := by
  simp [close]

/-- **A close only moves forward.** Extending through a later day is the
one allowed step; walking it back would reopen a signed period without an
audited verb. -/
theorem a_close_only_moves_forward (t d : Day)
    (h : close (some t) d = some d) : t < d := by
  unfold close at h
  split at h
  · contradiction
  · omega

end Ratio.Close
