import Ratio.Lots
import Ratio.Lots.Methods
import Ratio.Lots.Wash

set_option warningAsError true
/-! `Ratio.Lots.WashHolding` — a jurisdiction that does NOT transfer the period.

`Ratio.Lots.Wash.replacementAcquired` is the US rule: the replacement's
acquisition date for holding-period purposes becomes the ORIGINAL lot's,
not the repurchase's. `the_transferred_period_decides_the_rate` already
said getting that wrong moves a disposal between two tax rates.

⛔ AND THAT RULE IS NOT UNIVERSAL. A jurisdiction that does not transfer
the period keeps the replacement's own acquisition date. Assuming
`replacementAcquired` everywhere classifies that disposal at the US rate.
Conservation holds, the trial balance ties, the deferred loss still
attaches. The figure that goes wrong is the RATE — short-term or
long-term — which no reconciliation reaches.

⚠ SO THIS IS NOT AN `Order`, NOT A `Method`, AND NOT `lot_method = "wash"`.
The election is which DATE a later `isLongTerm` is asked. Inventing a
relief variant for it would smuggle a classification rule into lot
selection, which is the mistake `Ratio.Lots.Methods` exists to prevent.

⚠ AND IT IS NOT A JURISDICTION TABLE. The two rules are named. A fund
that wants the non-transfer writes the election. Silence is unset, not
a silent keep and not a silent transfer invented here —
`replacementAcquired` already exists for the US path.

`//tla:wash_holding_check` is the sequence. The probe
`//tla:universal_us_transfer_check` hardcodes the US transfer and
`TheReplacementKeepsItsOwnDate` goes red.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The election ─────────────────────────────────────────────────────── -/

/-- Which date a wash replacement carries for holding-period purposes.

`transfer` is the US rule already named as `replacementAcquired`.
`keep` is the non-US variant: the replacement keeps the repurchase's
own date.

⛔ NOT AN `Order`. An ordering picks a lot. This picks a DATE. Adding
either constructor as a `Method` / `lot_method` variant is the mistake
this file exists to prevent. -/
inductive PeriodRule where
  | transfer
  | keep
deriving DecidableEq, Repr

/-- The replacement's acquisition date under the elected rule.

`transfer` is `replacementAcquired`: the original lot's date.
`keep` is the repurchase's own date. Same two dates, two answers. -/
def acquiredFor (r : PeriodRule) (originalAcquired repurchaseOn : Int) : Int :=
  match r with
  | .transfer => replacementAcquired originalAcquired repurchaseOn
  | .keep => repurchaseOn

/- ── The two rules disagree on the rate, not on the basis ─────────────── -/

/-- **`transfer` is the US rule already named.** This file does not
re-derive it. -/
theorem transfer_is_the_us_rule_already_named (o r : Int) :
    acquiredFor .transfer o r = replacementAcquired o r := by
  rfl

/-- **`keep` is the repurchase's own date**, not a second transfer. -/
theorem keep_is_the_repurchase_date (o r : Int) :
    acquiredFor .keep o r = r := by
  rfl

/-- **⭐ CHOOSING THE WRONG RULE FLIPS THE RATE.**

A lot acquired on day 0, washed by a repurchase on day 300, disposed on
day 400. Transfer: 400 days, long-term. Keep: 100 days, short-term.
Same units, same basis, same proceeds, different rate — and every
figure on the balance sheet is identical either way.

This is `the_transferred_period_decides_the_rate` restated as an
election. The defect is assuming `.transfer` everywhere. -/
theorem choosing_the_wrong_rule_flips_the_rate :
    isLongTerm 365 (acquiredFor .transfer 0 300) 400 = true
    ∧ isLongTerm 365 (acquiredFor .keep 0 300) 400 = false := by
  constructor <;> decide

/-- **⛔ AND ASSUMING THE US TRANSFER WHEN THE ELECTION IS `keep` IS
THAT WRONG RATE.**

`replacementAcquired` on the keep path classifies long; the elected
date classifies short. Same sale. The books still tie. -/
theorem assuming_us_transfer_when_the_election_is_keep_is_the_wrong_rate :
    isLongTerm 365 (replacementAcquired 0 300) 400
      ≠ isLongTerm 365 (acquiredFor .keep 0 300) 400 := by
  decide

/-- **The write is still the write.** Both rules attach the same basis.
The election does not appear in `replacementBasis`. An engine that
also changed the deferral when it changed the date would be a
different rule, and it is not this one.

`Ratio.Lots.Wash.the_wash_rule_moves_a_loss_it_does_not_remove_it`
is unchanged: the two halves still cancel. -/
theorem both_rules_attach_the_same_basis (cost d : Int) :
    replacementBasis cost d = cost + d := by
  unfold replacementBasis
  omega

/-- **A later sale takes the adjusted basis under either rule.** The
date the lot carries does not change what `relieveFifo` gives up.
Cost is conserved either way; the rate is not. -/
theorem a_later_sale_takes_the_same_basis_under_either_rule :
    (attachTo [⟨2, 1, 40⟩] 2 1000).bind
        (fun held => (relieveFifo held 1).map (fun r => takenCost r.1))
      = some 1040 := by
  decide

/-- **And no `Order` is this election.** Every ordering gives up a
lot's cost. None of them rewrite an acquisition date. Stated so
`lot_method = "wash"` cannot be said of a function that only sorts. -/
theorem no_ordering_is_a_period_rule (m : Order) :
    (relieveBy m [⟨1, 1, 10⟩] 1).map (fun r => takenCost r.1) = some 10 := by
  cases m <;> decide

end Ratio.Lots
