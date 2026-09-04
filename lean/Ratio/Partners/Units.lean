import Ratio.Partners.Cut

set_option warningAsError true
/-! `Ratio.Partners.Units` — subscriptions and redemptions as unit movements.

Period NAV already cites contribution / distribution *money*. Unitization
is the other half: a subscription issues units, a redemption retires
them, and units in issue stay **unset** until a unit event posts.

A silent 0 on a book that has only ever contributed (PE-style, no
units) is the defect. After a full redemption, `some 0` is a real
zero — the same distinction undrawn already makes.

Units are MEASURED. They do not enter conservation. A conservation
check over units would refuse every subscription — the same reason
`Ratio.Chart.Dimensions` keeps quantity off the conserved basis.

Allocating a book-level unit figure across partners uses the named
cut, not a partner count. Inventing 1/N units is the equal-split
NAV PLAN already refused. `Ratio.Partners.no_cut_is_unset`.

Lean core + `omega`, no Mathlib. ⛔ `warningAsError` is load-bearing. -/

namespace Ratio.Partners

/-- A partner-unit movement: cash (conserved with capital) and units
(measured). Both positive means subscription — cash in, units issued.
The operator types a positive count; the rule decides the direction. -/
structure Movement where
  partner : Partner
  cash : Int
  units : Int
  deriving DecidableEq, Repr

/-- A movement names a non-zero unit count of the same sign as the
cash. Zero units is not a movement — that is a contribution. Opposite
signs would issue units while paying cash out. -/
def wellFormedMove (m : Movement) : Bool :=
  decide (m.units ≠ 0) && decide (m.cash ≠ 0) && decide (0 < m.units * m.cash)

theorem a_zero_unit_movement_is_refused (p : Partner) (cash : Int) :
    wellFormedMove ⟨p, cash, 0⟩ = false := by
  simp [wellFormedMove]

/-- Opposite signs are not a subscription or a redemption. -/
example : wellFormedMove ⟨0, 100, -10⟩ = false := by
  decide

/-- Cash in, units issued. -/
example : wellFormedMove ⟨0, 100, 10⟩ = true := by
  decide

/-- Cash out, units retired. -/
example : wellFormedMove ⟨0, -40, -4⟩ = true := by
  decide

def issuedSum : List Movement → Int
  | [] => 0
  | m :: ms => m.units + issuedSum ms

/-- Units in issue. Empty is unset — not a measured zero. -/
def unitsInIssue : List Movement → Option Int
  | [] => none
  | m :: ms => some (m.units + issuedSum ms)

theorem no_movement_is_unset : unitsInIssue [] = none := rfl

theorem a_posted_movement_is_not_unset (m : Movement) (ms : List Movement) :
    unitsInIssue (m :: ms) ≠ none := by
  simp [unitsInIssue]

theorem units_in_issue_are_the_sum (m : Movement) (ms : List Movement) :
    unitsInIssue (m :: ms) = some (m.units + issuedSum ms) := rfl

/-- A subscription of 10, then a redemption of 4, leaves 6 — not unset. -/
example : unitsInIssue [⟨0, 100, 10⟩, ⟨0, -40, -4⟩] = some 6 := by
  decide

/-- Positive units in a window. A redemption does not count as issued. -/
def issuedOf : List Movement → Int
  | [] => 0
  | m :: ms => (if 0 < m.units then m.units else 0) + issuedOf ms

/-- Absolute retired units in a window. A subscription does not count. -/
def redeemedOf : List Movement → Int
  | [] => 0
  | m :: ms => (if m.units < 0 then -m.units else 0) + redeemedOf ms

def hasIssue : List Movement → Bool
  | [] => false
  | m :: ms => decide (0 < m.units) || hasIssue ms

def hasRedeem : List Movement → Bool
  | [] => false
  | m :: ms => decide (m.units < 0) || hasRedeem ms

/-- Period issued. Empty, or a window with only redemptions, is unset —
not a silent zero issue. -/
def periodIssued (ms : List Movement) : Option Int :=
  if hasIssue ms then some (issuedOf ms) else none

/-- Period redeemed. Empty, or a window with only subscriptions, is unset —
not a silent zero redemption. -/
def periodRedeemed (ms : List Movement) : Option Int :=
  if hasRedeem ms then some (redeemedOf ms) else none

theorem no_issue_is_unset : periodIssued [] = none := rfl

theorem no_redeem_is_unset : periodRedeemed [] = none := rfl

/-- A subscription window issues and does not redeem. -/
example : periodIssued [⟨0, 100, 10⟩] = some 10 := by
  decide

example : periodRedeemed [⟨0, 100, 10⟩] = none := by
  decide

/-- A redemption window redeems and does not issue. -/
example : periodIssued [⟨0, -40, -4⟩] = none := by
  decide

example : periodRedeemed [⟨0, -40, -4⟩] = some 4 := by
  decide

/-- Issued and redeemed in one window stay apart — the net is not the plug. -/
example : periodIssued [⟨0, 100, 10⟩, ⟨0, -40, -4⟩] = some 10 := by
  decide

example : periodRedeemed [⟨0, 100, 10⟩, ⟨0, -40, -4⟩] = some 4 := by
  decide

/-- A movement's signed units are issued minus redeemed. -/
theorem signed_units_are_issued_minus_redeemed (u : Int) :
    (if 0 < u then u else 0) - (if u < 0 then -u else 0) = u := by
  by_cases hPos : 0 < u
  · have : ¬ u < 0 := by omega
    simp [hPos, this]
  · by_cases hNeg : u < 0
    · simp [hPos, hNeg]
    · have : u = 0 := by omega
      simp [this]

/-- Issued minus redeemed is the signed net the ending units already sum. -/
theorem issued_minus_redeemed_is_the_net :
    (ms : List Movement) → issuedOf ms - redeemedOf ms = issuedSum ms
  | [] => by simp [issuedOf, redeemedOf, issuedSum]
  | m :: ms => by
    have ih := issued_minus_redeemed_is_the_net ms
    simp [issuedOf, redeemedOf, issuedSum]
    have h := signed_units_are_issued_minus_redeemed m.units
    omega

/-- Money legs: cash in, capital up (or the reverse). Units do not
appear — they are measured. -/
def cashLeg (m : Movement) : Int := m.cash
def capitalLeg (m : Movement) : Int := -m.cash

theorem a_movement_conserves (m : Movement) :
    cashLeg m + capitalLeg m = 0 := by
  simp [cashLeg, capitalLeg]
  omega

/-- Redeem `units` from an outstanding figure.

Unset outstanding cannot redeem — there is nothing to retire, and
treating unset as 0 would make the first redemption look like it
retired units nobody issued. Over-redemption refuses. Zero units
is not a redemption. -/
def redeem (outstanding : Option Int) (units : Int) : Option Int :=
  match outstanding with
  | none => none
  | some n =>
    if 0 < units && units ≤ n then
      some (n - units)
    else
      none

theorem cannot_redeem_when_unset (units : Int) :
    redeem none units = none := rfl

theorem a_zero_redeem_is_refused (n : Int) :
    redeem (some n) 0 = none := by
  simp [redeem]

/-- Over-redemption refuses rather than going negative. -/
example : redeem (some 10) 11 = none := by
  decide

/-- Redeeming every issued unit leaves a real zero, not unset. -/
example : redeem (some 10) 10 = some 0 := by
  decide

example : redeem (some 10) 4 = some 6 := by
  decide

/-- Allocating units across partners is the cut, not a partner count. -/
theorem allocating_units_without_a_cut_is_unset (units : Int) :
    allocate units none = none :=
  no_cut_is_unset units

/-- **A silent 1/N of units is not an allocation.** Two partners and
30 units would print 15 each. Nobody named that cut. -/
example : allocate 30 none = none := rfl

/-- 80/20 of 100 units is 80 and 20, not 50/50. -/
example : allocate 100 (some [⟨0, 80⟩, ⟨1, 20⟩]) = some [(0, 80), (1, 20)] := by
  decide

/-- A unit figure that will not divide is refused, not rounded. -/
example : allocate 101 (some [⟨0, 80⟩, ⟨1, 20⟩]) = none := by
  decide

end Ratio.Partners
