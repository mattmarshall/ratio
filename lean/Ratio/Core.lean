/-! `Ratio.Core` — the proven kernel, after `research.tex`
("A Formally Grounded Accounting Kernel for High-Performance Financial Systems").

The paper's thesis: economic computation is **conservation over integer-valued
vector spaces**. A transaction is an integer vector over a basis of conserved
dimensions (currencies, positions, tax lots, P&L components); a *valid*
transaction nets to zero across every dimension (generalizing classical
double-entry); the ledger is a deterministic **monoidal fold** of the
append-only transaction log; and balance is preserved by aggregation, so the
ledger conserves value *independent of business semantics*. Business rules
(posting rules, normal-balance classification) are externalized to a control
plane — they are NOT kernel invariants.

This file is the kernel's formal core. Fixed-precision integers only (never
`f64`) — exact conservation + deterministic replay. Lean core + `omega`, no
Mathlib. The Rust the app runs (`ratio_common::money_*`) is emitted from
`Ratio.Common.Emit` mirroring the §"Money value layer" definitions below. -/

namespace Ratio

/- ── Money value layer: the laws the emitted `money_add` / `money_zero` /
   `money_neg` (ratio-common) must satisfy. (`m*` names avoid shadowing the
   `Zero`/`Neg`/`HAdd` typeclass methods.) ── -/

def mzero : Int := 0
def madd (a b : Int) : Int := a + b
def mneg (a : Int) : Int := -a

theorem madd_comm (a b : Int) : madd a b = madd b a := by
  show a + b = b + a; omega
theorem madd_assoc (a b c : Int) : madd (madd a b) c = madd a (madd b c) := by
  show a + b + c = a + (b + c); omega
theorem madd_zero (a : Int) : madd a mzero = a := by
  show a + 0 = a; omega
theorem mzero_madd (a : Int) : madd mzero a = a := by
  show 0 + a = a; omega
/-- A debit and its equal-and-opposite credit cancel — the atom of conservation. -/
theorem madd_mneg (a : Int) : madd a (mneg a) = mzero := by
  show a + -a = 0; omega

/- ── The kernel: accounting as conservation over integer vectors.
   (research.tex §"Accounting as Conservation Laws", §"A Minimal, Deterministic
   Kernel".) `Dim` is the conserved-quantity basis. ── -/

/-- A transaction's net effect / accounting state: an integer-valued vector over
the dimension basis. Fixed-precision integers → exact conservation, deterministic
replay across architectures. -/
abbrev Vec (Dim : Type) : Type := Dim → Int

namespace Vec
variable {Dim : Type}

def zero : Vec Dim := fun _ => 0
def add (a b : Vec Dim) : Vec Dim := fun d => a d + b d
def neg (a : Vec Dim) : Vec Dim := fun d => -a d

/-- Integer vectors form a commutative group under pointwise addition — the
monoidal substrate the ledger fold aggregates over. -/
theorem add_comm (a b : Vec Dim) : add a b = add b a := by
  funext d; show a d + b d = b d + a d; omega
theorem add_assoc (a b c : Vec Dim) : add (add a b) c = add a (add b c) := by
  funext d; show a d + b d + c d = a d + (b d + c d); omega
theorem add_zero (a : Vec Dim) : add a zero = a := by
  funext d; show a d + 0 = a d; omega
theorem zero_add (a : Vec Dim) : add zero a = a := by
  funext d; show 0 + a d = a d; omega
theorem add_neg (a : Vec Dim) : add a (neg a) = zero := by
  funext d; show a d + -a d = 0; omega

end Vec

/-- A transaction **conserves value** (is balanced) iff its net vector is zero
across every dimension — the kernel's sole, business-agnostic invariant. This
generalizes double-entry's "debits = credits" to arbitrary conserved
dimensions (research.tex §"Accounting as Conservation Laws"). -/
def Conserves {Dim : Type} (v : Vec Dim) : Prop := v = Vec.zero

theorem conserves_zero {Dim : Type} : Conserves (Vec.zero : Vec Dim) := rfl

/-- Conservation is closed under aggregation, with identity + inverse — i.e. the
conserving vectors form a subgroup. This is *why* folding the ledger preserves
balance. -/
theorem conserves_add {Dim : Type} {a b : Vec Dim}
    (ha : Conserves a) (hb : Conserves b) : Conserves (Vec.add a b) := by
  unfold Conserves at ha hb ⊢; rw [ha, hb]; exact Vec.add_zero Vec.zero

theorem conserves_neg {Dim : Type} {a : Vec Dim}
    (ha : Conserves a) : Conserves (Vec.neg a) := by
  unfold Conserves at ha ⊢; rw [ha]; funext d; show -(0 : Int) = 0; omega

/-- The ledger: a deterministic monoidal fold of the append-only transaction log
(research.tex §"Accounting as Conservation Laws" — associative aggregation). -/
def ledger {Dim : Type} (log : List (Vec Dim)) : Vec Dim :=
  log.foldl Vec.add Vec.zero

/-- A folded accumulator stays conserving when seeded conserving and fed only
conserving transactions (the inductive heart of the conservation law). -/
theorem foldl_conserves {Dim : Type} (log : List (Vec Dim)) :
    ∀ acc : Vec Dim, Conserves acc → (∀ t ∈ log, Conserves t) →
      Conserves (log.foldl Vec.add acc) := by
  induction log with
  | nil => intro acc hacc _; exact hacc
  | cons t ts ih =>
      intro acc hacc hall
      have ht : Conserves t := hall t (by simp)
      exact ih (Vec.add acc t) (conserves_add hacc ht)
        (fun s hs => hall s (by simp [hs]))

/-- **Conservation law.** If every transaction in the log conserves value, the
folded ledger conserves value — intrinsic internal consistency + replayable
balance, independent of any business logic (research.tex §"Accounting as
Conservation Laws"). Double-entry balance is the 1-dimensional instance. -/
theorem ledger_conserves {Dim : Type} (log : List (Vec Dim))
    (h : ∀ t ∈ log, Conserves t) : Conserves (ledger log) :=
  foldl_conserves log Vec.zero conserves_zero h

/- ── Double entry as the 1-D instance: a transaction's split amounts net to
   zero (the classical "debits = credits", recovered from conservation). ── -/

def total : List Int → Int
  | [] => mzero
  | x :: xs => madd x (total xs)

def Balanced (splits : List Int) : Prop := total splits = mzero

theorem balanced_nil : Balanced [] := rfl
/-- The canonical two-leg posting `[a, -a]` is balanced. -/
theorem balanced_pair (a : Int) : Balanced [a, mneg a] := by
  show a + (-a + 0) = 0; omega

/- ── Business rules are externalized to the control plane, NOT kernel
   invariants (research.tex §"Externalized Semantics via a Control Plane").
   Normal-balance classification is one such rule, kept here as a checked
   example. ── -/

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
