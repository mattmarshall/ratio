set_option warningAsError true
/-! `Ratio.Fees.Accrual` — management-fee accrual as a conserved posting.

Stage 1 already has an accrual *rule*: rate, day-count, a balanced
template. This file is the books half: an elected rule posts
receivable / expense, and the receivable figure stays **unset** when
fee terms are absent.

A silent 0 receivable on a book that never elected a fee is the
defect — the books would still tie, and the number would claim the
fund owes nothing when nobody said. After an accrual posts, the
figure is the credit of the receivable; paying it down to zero is a
real zero, not unset.

The amount a day of 75bp produces is Stage 1's (`rate × days ×
basis / (10 000 × denominator)`). This file does not re-decide it.
What it settles before the engine runs:

  1. **Terms are an election.** `None` is unset, not a silent 0 bp.
     A zero rate is not well-formed — omit the rule.
  2. **The posting conserves.** Expense debit and receivable credit
     are opposite signs of one amount. Same-sign legs are not an
     accrual; they would unbalance, or (if both flipped) hide the
     fee while the trial balance still tied.
  3. **A zero amount is not an accrual.** That is a 0-day no-op, and
     posting it would increment a count and print a silent 0.
  4. **The receivable figure is unset without terms**, even if some
     other rule posted to the payable account. Empty postings stay
     unset. A posted then reversed list that sums to 0 is a real
     zero.

Invoice PDF / LP statements / payment collection stay Connect.
`fees:read` / `fees:accrue` are the scopes; this is the journal.

Lean core + `omega`, no Mathlib. ⛔ `warningAsError` is load-bearing. -/

namespace Ratio.Fees

/-- The day-count convention supplies the denominator. How many days
the period contains is the event's, not this file's. -/
inductive DayCount where
  | act365
  | act360
  | thirty360
  deriving DecidableEq, Repr

def denominator : DayCount → Int
  | .act365 => 365
  | .act360 | .thirty360 => 360

/-- Elected fee terms: a positive rate and a convention.

`None` (nobody said) is not this structure. A zero rate is not
well-formed — that would be a silent zero receivable wearing an
election. -/
structure Terms where
  rateBp : Int
  dayCount : DayCount
  deriving DecidableEq, Repr

def wellFormedTerms (t : Terms) : Bool :=
  decide (0 < t.rateBp)

/-- Expense debit and receivable credit of one amount. -/
structure Accrual where
  expense : Int
  receivable : Int
  deriving DecidableEq, Repr

/-- Opposite signs of a non-zero amount. Same-sign is not an
accrual; a zero pair is a no-op, not a posting. -/
def wellFormedAccrual (a : Accrual) : Bool :=
  decide (a.expense ≠ 0) && decide (a.receivable = -a.expense)

def posting (amount : Int) : Accrual :=
  ⟨amount, -amount⟩

theorem a_posting_conserves (amount : Int) :
    (posting amount).expense + (posting amount).receivable = 0 := by
  simp [posting]
  omega

theorem a_well_formed_posting_conserves (a : Accrual)
    (h : wellFormedAccrual a = true) :
    a.expense + a.receivable = 0 := by
  simp [wellFormedAccrual] at h
  omega

theorem a_nonzero_posting_is_well_formed (amount : Int) (h : amount ≠ 0) :
    wellFormedAccrual (posting amount) = true := by
  simp [wellFormedAccrual, posting]
  exact h

/-- Same-sign legs are not an accrual. 100 / 100 would unbalance, or
— if someone treated both as credits — hide the fee. -/
example : wellFormedAccrual ⟨100, 100⟩ = false := by
  decide

/-- A pair that does not conserve is not an accrual. -/
example : wellFormedAccrual ⟨100, -50⟩ = false := by
  decide

example : wellFormedAccrual ⟨100, -100⟩ = true := by
  decide

/-- Accrue from elected terms. `None` is unset, not a silent zero
receivable. A zero amount is not a posting. -/
def accrue (terms : Option Terms) (amount : Int) : Option Accrual :=
  match terms with
  | none => none
  | some t =>
    if wellFormedTerms t && decide (amount ≠ 0) then
      some (posting amount)
    else
      none

theorem no_terms_is_unset (amount : Int) :
    accrue none amount = none := rfl

theorem a_zero_amount_is_not_an_accrual (t : Terms) :
    accrue (some t) 0 = none := by
  simp [accrue]

theorem a_zero_rate_is_not_an_election (amount : Int) :
    accrue (some ⟨0, .act365⟩) amount = none := by
  simp [accrue, wellFormedTerms]

/-- **A silent zero receivable is not an accrual.** Nobody named the
rate. -/
example : accrue none 35959 = none := rfl

/-- 75 bp, a non-zero amount: expense 100, receivable −100. -/
example : accrue (some ⟨75, .act365⟩) 100 = some ⟨100, -100⟩ := by
  decide

def postedSum : List Int → Int
  | [] => 0
  | p :: ps => p + postedSum ps

/-- The citeable receivable.

`None` terms stay unset — even if some other rule posted to the
payable account. An empty journal stays unset, not a measured 0.
A posted then reversed list that sums to 0 is a real zero. -/
def receivable (terms : Option Terms) (posted : List Int) : Option Int :=
  match terms with
  | none => none
  | some t =>
    if wellFormedTerms t then
      match posted with
      | [] => none
      | p :: ps => some (p + postedSum ps)
    else
      none

theorem no_terms_leaves_receivable_unset (posted : List Int) :
    receivable none posted = none := rfl

theorem an_empty_journal_with_terms_is_unset (t : Terms)
    (h : wellFormedTerms t = true) :
    receivable (some t) [] = none := by
  simp [receivable, h]

theorem a_zero_rate_leaves_receivable_unset (posted : List Int) :
    receivable (some ⟨0, .act365⟩) posted = none := by
  simp [receivable, wellFormedTerms]

/-- **Posted under some other rule is not a fee receivable.** Terms
were never elected. -/
example : receivable none [100] = none := rfl

/-- Terms elected, nothing posted: unset, not a silent 0. -/
example : receivable (some ⟨75, .act365⟩) [] = none := by
  decide

/-- One accrual of 35_959 is the receivable. -/
example : receivable (some ⟨75, .act365⟩) [35959] = some 35959 := by
  decide

/-- Accrued then paid in full is a real zero, not unset. -/
example : receivable (some ⟨75, .act365⟩) [35959, -35959] = some 0 := by
  decide

end Ratio.Fees
