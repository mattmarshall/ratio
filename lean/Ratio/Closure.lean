set_option warningAsError true
/-! `Ratio.Closure` — everything a NAV needs, and what each of them costs.

`Ratio.Plan` costs the NAV that exists today: positions and prices. A real
period end needs more, and the terms do not have the same shape — some are
deltas, one is superlinear, and one is forced to round. Putting them in one
model is what makes "turn the dial and see how long a NAV takes" a question
with an answer.

The dials are the fund's shape: five hundred securities for an S&P tracker,
three currencies, a fragmentation that grows every year it trades, however many
corporate actions are open, however much capital moved.

⛔ AND THE TERMS ARE NOT ALIKE. That is the point of separating them:

  marking          one price per SECURITY. Independent of lots.
  fx               one rate per CURRENCY. Independent of securities — a fund
                   with five hundred names in three currencies does three
                   translations, not five hundred, because it translates
                   per-currency SUBTOTALS.
  corporate actions  the only superlinear term, and the only RETROACTIVE one.
                   An action reaches back into positions already valued.
  capital activity  one per transaction. Drives units in issue.
  per-share        ⛔ THE ONE DIVISION IN THIS SYSTEM THAT MUST ROUND.

Costs are counts of reads, not seconds. What they buy is knowing which term
moves when a dial does. -/

namespace Ratio.Closure

/-- A fund's shape. Every dial the cost of a period end turns on. -/
structure Dials where
  /-- 500 for an S&P tracker. -/
  securities : Nat
  /-- 1 for a domestic fund; 3 or 4 for a global one. -/
  currencies : Nat
  /-- Open tax lots per security. Grows with every year of trading. -/
  lotsPer : Nat
  /-- Corporate actions not yet applied. -/
  openActions : Nat
  /-- Subscriptions and redemptions in the period. -/
  capitalTxns : Nat
deriving Repr

/-- Total open lots. The number that frightens people, and the one the NAV
below never reads. -/
def lots (d : Dials) : Nat := d.securities * d.lotsPer

/- ── What each term costs ──────────────────────────────────────────────── -/

/-- One price per security. -/
def markCost (d : Dials) : Nat := d.securities

/-- One rate per currency.

⛔ NOT ONE PER POSITION. Translation applies to per-currency SUBTOTALS, and a
subtotal is a partition of the positions — `Ratio.Lots.partition_sums_to_whole`
is what makes summing the parts give the whole. A fund with five hundred names
in three currencies does three translations. -/
def fxCost (d : Dials) : Nat := d.currencies

/-- An action reaches into every security it might touch.

⛔ THE ONLY SUPERLINEAR TERM, and the only RETROACTIVE one: a split effective
last Tuesday changes what positions held before it, so it cannot be applied as
a forward delta the way a price can. This is why the number of OPEN actions is
the dial that matters and not the number of actions ever. -/
def actionCost (d : Dials) : Nat := d.openActions * d.securities

/-- One per subscription or redemption. -/
def capitalCost (d : Dials) : Nat := d.capitalTxns

/-- What a period end reads. -/
def navCost (d : Dials) : Nat :=
  markCost d + fxCost d + actionCost d + capitalCost d

/-- **A NAV never reads the tax lots.**

The whole scale argument in one statement: hold the chart, the currencies, the
actions and the capital fixed, and the cost does not move when fragmentation
does. A fund's twentieth year strikes as fast as its first.

It holds because of `Ratio.Plan.aggregate_agrees_with_scan` — the maintained
totals are what stand in for the lots — and that theorem needs the totals to be
honest, which `Ratio.Lots.cost_is_conserved` is what keeps them. -/
theorem nav_never_reads_the_lots (sec ccy a b act cap : Nat) :
    navCost ⟨sec, ccy, a, act, cap⟩ = navCost ⟨sec, ccy, b, act, cap⟩ := by
  simp [navCost, markCost, fxCost, actionCost, capitalCost]

/-- **Translation is per currency, not per position.** -/
theorem fx_does_not_grow_with_the_chart (a b ccy l act cap : Nat) :
    fxCost ⟨a, ccy, l, act, cap⟩ = fxCost ⟨b, ccy, l, act, cap⟩ := by
  simp [fxCost]

/-- **With nothing outstanding, a period end is the chart plus the currencies.**

The S&P tracker on an ordinary day: five hundred and three reads, whatever the
fund has been doing for twenty years. -/
theorem a_quiet_day_is_the_chart_and_the_currencies (sec ccy l : Nat) :
    navCost ⟨sec, ccy, l, 0, 0⟩ = sec + ccy := by
  simp [navCost, markCost, fxCost, actionCost, capitalCost]

/-- An S&P tracker, ten thousand lots a name, three currencies, quiet day. -/
example : navCost ⟨500, 3, 10000, 0, 0⟩ = 503 := by
  simp [a_quiet_day_is_the_chart_and_the_currencies]

/-- Five million open lots behind it, unread. -/
example : lots ⟨500, 3, 10000, 0, 0⟩ = 5000000 := by simp [lots]

/-- **And one corporate action costs more than the whole quiet day.**

Which is the honest headline: the term to engineer is not the lots, it is the
actions. Twenty million lots are free and one unapplied split is not. -/
theorem an_open_action_costs_a_whole_chart (sec ccy l : Nat) :
    navCost ⟨sec, ccy, l, 1, 0⟩ = navCost ⟨sec, ccy, l, 0, 0⟩ + sec := by
  simp [navCost, markCost, fxCost, actionCost, capitalCost]

/- ── Per share: the one division that must round ───────────────────────── -/

/-- Net asset value per unit in issue, and what the division left over.

⛔ EVERYWHERE ELSE IN THIS SYSTEM AN INEXACT DIVISION IS REFUSED. Here it
cannot be: a fund with 1,000,000.00 of assets and 3 units in issue has a
per-share NAV, and no amount of principle makes 3 divide it. Rounding is
mandatory, so what matters is different — that the rounding is DECLARED, and
that what it left over is ACCOUNTED FOR rather than dropped.

The residual is the fund's. It has to land somewhere, and "nowhere" is how a
book loses money one thousandth at a time. -/
def perShare (nav units : Int) : Int × Int := (nav / units, nav % units)

/-- **The rounding residual is accounted for.**

Per-share times units, plus what was left over, is the net asset value. So a
book that publishes a rounded per-share figure has not quietly lost the
difference — it can still say where it went. -/
theorem residual_is_accounted (nav units : Int) :
    units * (perShare nav units).1 + (perShare nav units).2 = nav := by
  simp [perShare]
  exact Int.mul_ediv_add_emod nav units

/-- And when it does divide exactly, there is nothing left over — so the
general rule degrades to the strict one rather than sitting beside it. -/
theorem an_exact_division_leaves_nothing (nav units : Int)
    (h : nav % units = 0) : (perShare nav units).2 = 0 := by
  simp [perShare, h]

/-- **⛔ THIS DIVISION IS EUCLIDEAN, AND RUST'S IS NOT.**

Checked, because the emitted Rust has to agree with it and the two languages
disagree by default. Lean's `/` and `%` on `Int` are `ediv`/`emod`, which is why
`residual_is_accounted` closes with `Int.mul_ediv_add_emod`; the residual is
always NON-NEGATIVE. Rust's `/` and `%` truncate toward zero, so the residual
carries the sign of the numerator:

    perShare (-7) 3   Lean:  (-3,  2)      Rust `(-7/3, -7%3)`:  (-2, -1)

Both satisfy `units * q + r = nav`, so `residual_is_accounted` would hold of
EITHER and cannot tell them apart — the theorem is not the guard here. The
emitted code uses `div_euclid`/`rem_euclid` for that reason, and this example is
what says which convention it must match.

A negative NAV is not hypothetical: a fund carrying a liability greater than its
assets has one, and units in issue stay positive. -/
example : perShare (-7) 3 = (-3, 2) := by decide

/-- And the ordinary case is the same under either convention, which is exactly
why the disagreement above would not show up in testing. -/
example : perShare 1000 3 = (333, 1) := by decide

end Ratio.Closure
