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

/-- Applying an open action by REWRITING the lots it touches.

⛔ `lotsPer`, NOT `securities`, AND THE DIFFERENCE IS EIGHTY-FOLD. This term
read `openActions * securities` until 2026-08-09, on the reasoning that "an
action reaches into every security it might touch". It does not. A split
reaches into ONE instrument and touches every LOT of it — forty thousand for the
S&P tracker, against five hundred. The model understated its own worst term, and
understated it in the direction that made the system look better.

Still the only RETROACTIVE term: a split effective last Tuesday changes what
positions held before it, so it cannot be applied as a forward delta the way a
price can. Which is why OPEN actions are the dial and actions-ever is not. -/
def actionCost (d : Dials) : Nat := d.openActions * d.lotsPer

/-- Applying an open action by composing a FACTOR read at the point of use.

Nothing is written, so bringing the book up to date costs nothing.
`Ratio.Actions.Factor` is the equivalence, its counterexample, and the
precondition that announcements be journal entries.

⚠ NOT FREE IN GENERAL — each lot READ must still check that every step divided,
which is O(splits) on the lots actually touched. A sale relieving five lots pays
for five. A NAV pays for none, which is the whole point. -/
def factorActionCost (_d : Dials) : Nat := 0

/-- One per subscription or redemption. -/
def capitalCost (d : Dials) : Nat := d.capitalTxns

/-- What a period end reads TODAY — actions applied by rewriting. -/
def navCost (d : Dials) : Nat :=
  markCost d + fxCost d + actionCost d + capitalCost d

/-- What it reads once actions are factors instead. -/
def factorNavCost (d : Dials) : Nat :=
  markCost d + fxCost d + factorActionCost d + capitalCost d

/-- **A NAV with nothing outstanding never reads the tax lots.**

The scale argument: hold the chart, the currencies and the capital fixed, and
the cost does not move when fragmentation does. A fund's twentieth year strikes
as fast as its first.

It holds because of `Ratio.Plan.aggregate_agrees_with_scan` — the maintained
totals stand in for the lots — and that theorem needs the totals honest, which
`Ratio.Lots.cost_is_conserved` is what keeps them.

⛔ NOTE THE HYPOTHESIS `0` IN THE ACTION SLOT. This theorem was stated for ALL
`act` and was true of a model whose action term did not mention `lotsPer`
because that term was written wrong. It is not true of the corrected one, and
the next theorem is why. -/
theorem a_quiet_nav_never_reads_the_lots (sec ccy a b cap : Nat) :
    navCost ⟨sec, ccy, a, 0, cap⟩ = navCost ⟨sec, ccy, b, 0, cap⟩ := by
  simp [navCost, markCost, fxCost, actionCost, capitalCost]

/-- **⛔ AND WITH AN ACTION OPEN, A REWRITE MAKES THE NAV READ THEM.**

Two funds identical but for fragmentation — one lot a name against forty
thousand — with a single split outstanding. 40,503 against 504.

This is the claim the whole product rests on, failing. Applying an action by
rewriting IS a walk over the lots, so on any day something is outstanding the
flat-in-fragmentation promise is simply not true. -/
theorem an_open_action_makes_the_nav_read_the_lots :
    navCost ⟨500, 3, 1, 1, 0⟩ ≠ navCost ⟨500, 3, 40000, 1, 0⟩ := by
  simp [navCost, markCost, fxCost, actionCost, capitalCost]

/-- **⭐ UNDER A FACTOR IT HOLDS FOR ANY NUMBER OF OPEN ACTIONS.**

The unconditional statement, and what `nav_never_reads_the_lots` was always
meant to say. `Ratio.Actions.Factor` is not an optimization beside the claim —
it is what makes the claim true. -/
theorem factored_nav_never_reads_the_lots (sec ccy a b act cap : Nat) :
    factorNavCost ⟨sec, ccy, a, act, cap⟩ = factorNavCost ⟨sec, ccy, b, act, cap⟩ := by
  simp [factorNavCost, markCost, fxCost, factorActionCost, capitalCost]

/-- **⚠ And on a quiet day the two models are the same number.**

The redesign removes a cliff, and a cliff is not a slope. Anyone quoting it as
"the NAV got faster" has it backwards. -/
theorem with_nothing_open_the_models_agree (sec ccy l cap : Nat) :
    navCost ⟨sec, ccy, l, 0, cap⟩ = factorNavCost ⟨sec, ccy, l, 0, cap⟩ := by
  simp [navCost, factorNavCost, actionCost, factorActionCost]

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

/-- **And one outstanding action costs the lots of its instrument** — eighty
times the entire quiet period end, not one time it.

The honest headline, and it is not the one I first wrote. The term to engineer
is the actions; twenty million lots are free until a split makes forty thousand
of them the whole job. -/
theorem an_open_action_costs_the_lots_of_its_instrument (sec ccy l : Nat) :
    navCost ⟨sec, ccy, l, 1, 0⟩ = navCost ⟨sec, ccy, l, 0, 0⟩ + l := by
  simp [navCost, markCost, fxCost, actionCost, capitalCost]

/-- The S&P tracker with one split outstanding, both ways. -/
theorem the_cliff :
    navCost ⟨500, 3, 40000, 1, 0⟩ = 40503
    ∧ factorNavCost ⟨500, 3, 40000, 1, 0⟩ = 503 := by
  constructor <;>
    simp [navCost, factorNavCost, markCost, fxCost, actionCost,
          factorActionCost, capitalCost]

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
