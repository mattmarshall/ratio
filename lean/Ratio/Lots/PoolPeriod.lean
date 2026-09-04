import Ratio.Lots
import Ratio.Lots.Methods

set_option warningAsError true
/-! `Ratio.Lots.PoolPeriod` — the date a pooled holding carries, not a category.

`Ratio.Lots.AverageCost` pools the basis. It does not say what DATE the
remainder and the slice carry for holding-period purposes. US single-
category invents FIFO's oldest date on a mixed pool and classifies the
sale long-term. Double-category invents two pools and splits the sale.
Both invent a short-vs-long answer the lots do not support.

⛔ THE HONEST RULE IS A DATE, NOT A CATEGORY. When every lot agrees on
an acquisition date, that date is carried. Mixed or missing dates stay
unset. No category is invented. Unset is not a silent long and not a
silent short — `classify` leaves the gain unclassified, which is
`Realized::unclassified`, the remainder.

⚠ SO THIS IS NOT AN `Order`, NOT A `Method`, AND NOT A `lot_method`
VARIANT. FIFO on mixed dates would take the oldest date and invent
long-term. Adding a category constructor is the mistake
`Ratio.Lots.Methods` exists to prevent.

`//tla:pool_period_engine_check` is the sequence. The probe
`//tla:sort_and_walk_pool_period_check` treats the rule as FIFO and
`ThePoolDateStaysUnset` goes red.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The date a pool carries ──────────────────────────────────────────── -/

/-- The acquisition date a pool carries, if every lot agrees.

`none` is unset — mixed dates, a missing date, or an empty holding.
`some d` is the date every lot agreed on.

⛔ NOT AN `Order`. FIFO would pick the oldest date and invent long-term
on a mixed holding. That is single-category by stealth. Double-category
would invent two pools. Neither is this function. -/
def poolAcquired : List (Option Int) → Option Int
  | [] => none
  | d :: rest =>
    if rest.all (fun x => decide (x = d)) then d else none

/- ── Shared dates are carried; mixed or missing stay unset ────────────── -/

/-- **⭐ A SHARED DATE IS CARRIED.** Three lots acquired on day 100.
The pool is day 100. Nothing was invented — every lot said so. -/
theorem a_shared_date_is_carried :
    poolAcquired [some 100, some 100, some 100] = some 100 := by
  decide

/-- **A single dated lot is a shared date of one.** The pool is not a
second opinion. -/
theorem a_single_dated_lot_is_carried :
    poolAcquired [some 100] = some 100 := by
  decide

/-- **⭐ MIXED DATES STAY UNSET.** Day 0 and day 400 do not agree.
No category is invented. -/
theorem mixed_dates_stay_unset :
    poolAcquired [some 0, some 400] = none := by
  decide

/-- **A MISSING DATE STAYS UNSET.** One lot dated, one not. Assuming
the dated lot's day would invent a category on records that do not
all support it. -/
theorem a_missing_date_stays_unset :
    poolAcquired [some 100, none] = none := by
  decide

/-- **An empty holding has no date.** There is nothing to agree. -/
theorem an_empty_pool_has_no_date :
    poolAcquired ([] : List (Option Int)) = none := by
  decide

/-- **All-missing is still unset**, not a silent epoch. They agree on
having no date, which is not a date. -/
theorem all_missing_stays_unset :
    poolAcquired [none, none] = none := by
  decide

/- ── Treating mixed dates as an Order invents a category ──────────────── -/

/-- **⭐ TREATING MIXED DATES AS FIFO INVENTS LONG-TERM.**

Day 0 and day 400, disposed on day 400, threshold 365. FIFO takes
day 0: 400 days, long-term. The other lot is 0 days, short-term.
The pool carries neither. Same units, same basis, same proceeds.
The books still tie. The figure that goes wrong is the RATE.

This is US single-category by stealth: the oldest date classifies
the whole sale. Double-category would invent two pools. Both are
refused. `poolAcquired` stays unset. -/
theorem treating_mixed_dates_as_an_order_invents_a_category :
    poolAcquired [some 0, some 400] = none
    ∧ isLongTerm 365 0 400 = true
    ∧ isLongTerm 365 400 400 = false := by
  constructor
  · decide
  · constructor <;> decide

/-- **And a shared date that is long stays long.** Stated so an engine
that "always leaves the date unset" cannot hide behind the mixed
case. Day 0 / day 0 / dispose 400 is long, and the pool says so. -/
theorem a_shared_long_date_classifies_long :
    poolAcquired [some 0, some 0] = some 0
    ∧ isLongTerm 365 0 400 = true := by
  constructor <;> decide

/-- **⛔ AND NO `Order` IS THIS RULE.** Every ordering gives up a
lot's cost. None of them decide a pool date. Stated so
`lot_method = "category"` cannot be said of a function that only
sorts. -/
theorem no_ordering_is_a_category_rule (m : Order) :
    (relieveBy m [⟨1, 1, 10⟩] 1).map (fun r => takenCost r.1) = some 10 := by
  cases m <;> decide

end Ratio.Lots
