import Ratio.Core

set_option warningAsError true

/-! `Ratio.Chart` — the two properties the MVP ledger rests on, proved.

`Ratio.Core` proves the kernel's sole invariant: a ledger folded from
conserving transactions conserves value. That is the foundation, but two
further things have to hold before a fund accountant can use the thing, and
both are cheap to prove and expensive to get wrong:

1. **The trial balance ties.** Conservation says the postings net to zero. What
   an accountant checks is different in form: total debits equal total credits.
   `trial_balance_ties` shows these are the same statement — debits minus
   credits *is* the net — so the check they already run is the invariant the
   kernel already holds. Nothing extra has to be enforced at report time.

2. **A balanced rule template stays balanced at every amount.** A posting rule
   is a set of weights over dimensions; applying it at an amount scales each
   weight. `balanced_template_balances` shows that if the weights net to zero
   then every instantiation nets to zero, for any amount and any number of
   applications. This is what makes the control plane safe to be expressive:
   the checker verifies a template **once**, at authoring time, and every
   posting it ever produces is balanced by theorem rather than by inspection.

Both are stated over `Int` — the 1-dimensional instance — matching
`Ratio.Core.total` / `Ratio.Core.Balanced`. Lean core + `omega`, no Mathlib.

⛔ `set_option warningAsError true` above is load-bearing, not hygiene. A
`sorry` in Lean is a *warning*, and `lean` exits 0 on a warning — so before
this option was set, `//lean:*_test` passed with sorries in it. Measured:
a probe file proving `(1 : Int) = 2` by `sorry` went green through
`lean_test`. Every "the proofs ARE the test" claim in `lean/BUILD.bazel`
depended on an assumption that was false. Do not remove it.
-/

namespace Ratio

/- ── Trial balance ─────────────────────────────────────────────────────── -/

/-- The debit side of a signed amount: the amount if positive, else nothing. -/
def posPart (a : Int) : Int := if 0 < a then a else 0

/-- The credit side of a signed amount, as a positive number. -/
def negPart (a : Int) : Int := if a < 0 then -a else 0

/-- Every amount splits into exactly its debit and credit parts. -/
theorem part_split (a : Int) : posPart a - negPart a = a := by
  unfold posPart negPart
  split <;> split <;> omega

/-- Total debits over a list of signed amounts. -/
def debits : List Int → Int
  | [] => 0
  | a :: as => posPart a + debits as

/-- Total credits over a list of signed amounts, as a positive number. -/
def credits : List Int → Int
  | [] => 0
  | a :: as => negPart a + credits as

/-- Debits minus credits is the net — the accountant's view and the kernel's
view are the same arithmetic, not two checks that have to agree. -/
theorem debits_sub_credits (l : List Int) : debits l - credits l = total l := by
  induction l with
  | nil => rfl
  | cons a as ih =>
      have hs := part_split a
      show posPart a + debits as - (negPart a + credits as) = a + total as
      omega

/-- **The trial balance ties.** If the postings conserve value, total debits
equal total credits. This is why a balanced book needs no separate report-time
check: the report is a restatement of the invariant. -/
theorem trial_balance_ties (l : List Int) (h : Balanced l) : debits l = credits l := by
  have hd := debits_sub_credits l
  -- Restate `Balanced l` as an equation on the same atom `total l` that `hd`
  -- mentions. Unfolding `total` here instead would rewrite it to a form omega
  -- reads as a *different* atom, and the two hypotheses would stop connecting.
  have h0 : total l = 0 := h
  omega

/-- A book with nothing in it ties. -/
theorem trial_balance_nil : debits [] = credits [] := rfl

/- ── Posting-rule templates ────────────────────────────────────────────── -/

/-- Instantiate a template of weights at `amount`, scaling each weight. -/
def applyTemplate (amount : Int) : List Int → List Int
  | [] => []
  | w :: ws => (w * amount) :: applyTemplate amount ws

/-- Instantiating a template scales its net by the amount.

`omega` cannot close this: `w * amount` multiplies two variables and omega is
linear-only. `ring` is not available either — this development is deliberately
Mathlib-free (see the header of `Ratio.Core`). Distributivity therefore comes
from the core lemma `Int.add_mul` by name. -/
theorem apply_total (amount : Int) (ws : List Int) :
    total (applyTemplate amount ws) = total ws * amount := by
  induction ws with
  | nil =>
      show (0 : Int) = 0 * amount
      rw [Int.zero_mul]
  | cons w ws ih =>
      show w * amount + total (applyTemplate amount ws) = (w + total ws) * amount
      rw [ih, Int.add_mul]

/-- **A balanced template is balanced at every amount.** The control plane may
be arbitrarily expressive about *which* template applies; if the template's
weights net to zero, no amount it is ever applied at can unbalance the book.
Check the template once, at authoring time. -/
theorem balanced_template_balances (amount : Int) (ws : List Int)
    (h : Balanced ws) : Balanced (applyTemplate amount ws) := by
  unfold Balanced at h ⊢
  rw [apply_total, h]
  show (0 : Int) * amount = 0
  rw [Int.zero_mul]

/-- The canonical two-leg template `[1, -1]` — a debit and its credit — is
balanced, and therefore so is every posting generated from it. -/
theorem two_leg_template_balanced (amount : Int) :
    Balanced (applyTemplate amount [1, -1]) :=
  balanced_template_balances amount [1, -1] (by
    -- `decide` cannot see through `Balanced`, which is a plain def rather than
    -- a reducible abbreviation, so no Decidable instance is found for it.
    show (1 : Int) + (-1 + 0) = 0
    omega)

/-- Instantiating a balanced template never changes the trial balance's tie. -/
theorem template_preserves_tie (amount : Int) (ws : List Int) (h : Balanced ws) :
    debits (applyTemplate amount ws) = credits (applyTemplate amount ws) :=
  trial_balance_ties _ (balanced_template_balances amount ws h)

/- ── Non-vacuity ────────────────────────────────────────────────────────────

A theorem with an unsatisfiable hypothesis is true and worthless, and a theorem
whose conclusion holds for everything constrains nothing. Neither failure is
visible from the statement, and neither is caught by the build. The witnesses
below pin down that the theorems above are about something:

  · the hypotheses are satisfiable — there ARE balanced lists and templates
  · the conclusions are NOT automatic — `debits = credits` is false for some
    list, so `Balanced` is doing real work in `trial_balance_ties`
  · the definitions are not degenerate — `debits` and `credits` are not
    constant, and `Balanced` is not universally true
   ── -/

/-- `debits` is not the zero function. -/
theorem debits_nontrivial : debits [(5 : Int), -3] = 5 := by decide

/-- `credits` is not the zero function, and reports a *positive* total. -/
theorem credits_nontrivial : credits [(5 : Int), -3] = 3 := by decide

/-- The two are genuinely different functions. -/
theorem debits_ne_credits_in_general : debits [(5 : Int), -3] ≠ credits [(5 : Int), -3] := by
  decide

/-- `Balanced` is not universally true, so it is a real hypothesis. -/
theorem not_balanced_singleton : ¬ Balanced [(1 : Int)] := by
  intro h
  have : (1 : Int) + 0 = 0 := h
  omega

/-- `Balanced` is satisfiable by something other than the empty list, so
`trial_balance_ties` is not vacuous. -/
theorem balanced_witness : Balanced [(500 : Int), -500] := by
  show (500 : Int) + (-500 + 0) = 0
  omega

/-- …and on that witness the conclusion says something: both sides are 500,
not zero. A tie between two zeroes would prove nothing. -/
theorem trial_balance_ties_witness :
    debits [(500 : Int), -500] = 500 ∧ credits [(500 : Int), -500] = 500 :=
  ⟨by decide, by decide⟩

/-- `applyTemplate` actually scales — it is not the identity or a constant. -/
theorem apply_template_witness : applyTemplate 7 [(1 : Int), -1] = [7, -7] := by decide

/-- The template theorem is applied to a real, non-degenerate case: a two-leg
template at a non-unit amount, yielding a non-zero balanced posting. -/
theorem balanced_template_witness : Balanced (applyTemplate 7 [(1 : Int), -1]) :=
  two_leg_template_balanced 7

/-- An unbalanced template does NOT become balanced by instantiation — the
hypothesis of `balanced_template_balances` cannot be dropped. -/
theorem unbalanced_template_stays_unbalanced :
    ¬ Balanced (applyTemplate 7 [(1 : Int), -2]) := by
  intro h
  have : (7 : Int) + (-14 + 0) = 0 := h
  omega

end Ratio
