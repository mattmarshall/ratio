/-! `Ratio.Core` — the proven model of ratio's accounting invariants.

This is the formal-verification half of the Lean-first kernel: a Lean model of
Money (integer minor-units) and double-entry, with the algebraic + accounting
properties proven. The Rust the app runs is emitted from `Ratio.Common.Emit`
(money_add, money_zero, …) mirroring these definitions; the proofs here pin the
laws that mirror is expected to satisfy. (Lean core only — `omega` decides the
integer arithmetic, no Mathlib needed.)

Defs are named `m*` (mzero/madd/mneg) to avoid clashing with the `Zero`/`Neg`/
`HAdd` typeclass methods (which would shadow `unfold`). -/

namespace Ratio

-- Money as integer minor-units (cents, …) — `Int`. Currency is orthogonal to
-- the arithmetic proven here; mixing currencies is a kernel-level guard.

def mzero : Int := 0
def madd (a b : Int) : Int := a + b
def mneg (a : Int) : Int := -a

/- ── Money forms a commutative group under `madd` (the laws the emitted
   `money_add` / `money_zero` / `money_neg` must satisfy). ── -/

theorem madd_comm (a b : Int) : madd a b = madd b a := by
  show a + b = b + a; omega

theorem madd_assoc (a b c : Int) : madd (madd a b) c = madd a (madd b c) := by
  show a + b + c = a + (b + c); omega

theorem madd_zero (a : Int) : madd a mzero = a := by
  show a + 0 = a; omega

theorem mzero_madd (a : Int) : madd mzero a = a := by
  show 0 + a = a; omega

/-- A debit and its equal-and-opposite credit cancel — the atom of double entry. -/
theorem madd_mneg (a : Int) : madd a (mneg a) = mzero := by
  show a + -a = 0; omega

/- ── Double-entry balance: a transaction is balanced iff its splits sum to 0. ── -/

/-- Sum of a transaction's split amounts. -/
def total : List Int → Int
  | [] => mzero
  | x :: xs => madd x (total xs)

/-- A transaction is balanced when its splits net to zero. -/
def Balanced (splits : List Int) : Prop := total splits = mzero

/-- The empty transaction is (vacuously) balanced. -/
theorem balanced_nil : Balanced [] := rfl

/-- The canonical two-leg posting `[a, -a]` is balanced. -/
theorem balanced_pair (a : Int) : Balanced [a, mneg a] := by
  show a + (-a + 0) = 0; omega

/- ── Normal balance: each account type posts on a fixed side. ── -/

inductive Side
  | debit
  | credit
deriving DecidableEq, Repr

inductive AccountType
  | asset
  | liability
  | equity
  | income
  | expense
deriving DecidableEq, Repr

/-- The five normal-balance rules: assets + expenses are debit-normal;
liabilities, equity, and income are credit-normal. -/
def normalSide : AccountType → Side
  | .asset   => .debit
  | .expense => .debit
  | .liability => .credit
  | .equity    => .credit
  | .income    => .credit

theorem asset_debit_normal      : normalSide .asset     = .debit  := rfl
theorem expense_debit_normal    : normalSide .expense   = .debit  := rfl
theorem liability_credit_normal : normalSide .liability = .credit := rfl
theorem equity_credit_normal    : normalSide .equity    = .credit := rfl
theorem income_credit_normal    : normalSide .income    = .credit := rfl

end Ratio
