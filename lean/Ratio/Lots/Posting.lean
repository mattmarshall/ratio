import Ratio.Lots
import Ratio.Chart.Dimensions

set_option warningAsError true
/-! `Ratio.Lots.Posting` — where a realized gain goes.

The engine computes a gain: proceeds less the cost of the lots given up. Nothing
posts it. Six methods, holding-period classification, and the whole of
`Ratio.Lots.Relief` decide a number that is then discarded — the machine that
determines taxable income, with its answer thrown away.

⛔ AND A GAIN IS THE ONE FIGURE A TRANSACTION CREATES. Everything else in this
system MOVES value: cash out, investments in, the total unchanged. A sale moves
cost out and proceeds in, and they are not equal — the difference did not come
from another account, it came into existence. That is why the posting has three
legs rather than two, and why getting it wrong leaves the books tying.

⛔ THE FAILURE TO PREVENT IS THE GAIN COMPUTED TWICE.

  proceeds − relieved cost      by the relief engine, walking the lots
  proceeds − posted basis       by whoever writes the entry

If those disagree the entry still balances — the gain leg absorbs the
difference — and the realized gain is wrong by exactly the drift. This is the
same shape as the posted-vs-computed basis break the projection already reports,
one level closer to the money. `the_gain_is_the_relief_and_not_a_second_sum` is
the statement that there is one computation.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

open Ratio.Chart (Posting Key netIn netOf)

/-- The accounts a sale touches. Numbers stand for a chart; what matters is that
they are three DIFFERENT partitions of one conserved currency. -/
structure Accounts where
  investments : Nat
  cash : Nat
  /-- Where the difference lands. ⛔ A partition like any other — the gain is not
  a fourth kind of thing, it is value sitting in a different account. -/
  realizedGain : Nat
deriving Repr

/-- The three accounts are DIFFERENT accounts.

⛔ NOT AN OBVIOUS HYPOTHESIS — it is the one the proofs below turned out to need,
and its absence is a real way a chart goes wrong. Map the realized gain to the
same dimension as investments and the gain nets against the disposal: the income
account reports what is left over rather than what was earned, and the entry
still balances, because the legs are all still there.

⚠ A chart is configuration. Nothing stops two roles pointing at one dimension,
and `a_collided_chart_hides_the_gain` is what that produces. -/
def Accounts.distinct (a : Accounts) : Prop :=
  a.investments ≠ a.cash ∧ a.investments ≠ a.realizedGain ∧ a.cash ≠ a.realizedGain

/-- The legs of a sale.

⛔ THE GAIN LEG IS DERIVED FROM THE OTHER TWO AND NEVER SUPPLIED. It is
`proceeds − relieved`, computed here from the figures already on the entry, so
there is no second opinion for the first to disagree with. A signature taking a
gain as a parameter would be the drift, written down as an API. -/
def salePostings (a : Accounts) (currency : String) (instrument : String)
    (units relieved proceeds : Int) : List Posting :=
  [ ⟨⟨a.investments, currency, some instrument⟩, -relieved, -units⟩,
    ⟨⟨a.cash, currency, none⟩, proceeds, 0⟩,
    -- Credit-normal: a GAIN is negative here, which is why the sign is not
    -- `proceeds - relieved`. An income account holding a positive number would
    -- be a loss wearing a gain's name.
    ⟨⟨a.realizedGain, currency, none⟩, relieved - proceeds, 0⟩ ]

/-- **⭐ THE SALE CONSERVES, WHATEVER THE GAIN.**

Three legs, one currency, netting to zero for any proceeds and any basis. The
gain leg is what makes it conserve — remove it and the entry is out by exactly
the gain, which is the shape of a book that records a disposal at cost. -/
theorem a_sale_conserves_every_currency (a : Accounts) (c i : String)
    (units relieved proceeds : Int) :
    netIn c (salePostings a c i units relieved proceeds) = 0 := by
  simp [salePostings, netIn]
  omega

/-- **⛔ AND WITHOUT THE GAIN LEG IT IS OUT BY THE GAIN.**

Selling a lot that cost 100 for 150: two legs leave the entry short by 50. The
gain is not bookkeeping decoration — it is what closes the transaction. -/
theorem two_legs_are_out_by_the_gain :
    netIn "USD"
      [ ⟨⟨1, "USD", some "VTI"⟩, -100, -10⟩,
        ⟨⟨2, "USD", none⟩, 150, 0⟩ ] = 50 := by
  decide

/-- **⭐ THE GAIN IS THE RELIEF, NOT A SECOND SUM.**

What lands in the income account is exactly `proceeds − relieved`, where
`relieved` is the figure the lot walk produced. There is no parameter for a
separately-computed gain, so there is nothing for the two to disagree about.

⚠ The failure this prevents is silent: an entry carrying its own gain figure
still BALANCES, because the gain leg absorbs whatever the other two legs leave.
The books tie and the taxable income is wrong by the drift. -/
theorem the_gain_is_the_relief_and_not_a_second_sum (a : Accounts) (c i : String)
    (units relieved proceeds : Int) (h : a.distinct) :
    netOf a.realizedGain (salePostings a c i units relieved proceeds) = relieved - proceeds := by
  obtain ⟨_, hi, hc⟩ := h
  simp [salePostings, netOf, hi, hc]

/-- **A sale at cost realizes nothing**, and still posts three legs.

The degenerate case, pinned: an implementation that omitted the gain leg when it
was zero would be right here and wrong the moment it was not, and the omission
would be invisible because a zero leg changes no total. -/
theorem a_sale_at_cost_realizes_nothing (a : Accounts) (c i : String) (units v : Int)
    (h : a.distinct) : netOf a.realizedGain (salePostings a c i units v v) = 0 := by
  obtain ⟨_, hi, hc⟩ := h
  simp [salePostings, netOf, hi, hc]

/-- **A loss is a gain with the other sign**, and needs no separate path.

Selling a lot that cost 150 for 100 puts +50 in the income account — a debit,
which is what a loss is. One leg, one formula, both directions: a system with a
`realizedLoss` account would have to decide which to use, and would decide it
from the sign of a number it had just computed. -/
theorem a_loss_needs_no_separate_account :
    netOf 3 (salePostings ⟨1, 2, 3⟩ "USD" "VTI" 10 150 100) = 50 := by
  decide

/-- **⛔ A COLLIDED CHART HIDES THE GAIN.**

Investments and the realized-gain account pointing at the same dimension. Sell a
lot that cost 100 for 150: the disposal posts −100 and the gain posts −50 to the
SAME place, so that account reports −150 and the income account — which is the
same account — reports no gain at all. The entry conserves. The trial balance
ties. The taxable income is nowhere.

⚠ A chart is CONFIGURATION, so this is a thing somebody can write down, and it
is why `Accounts.distinct` is a hypothesis rather than an assumption. -/
theorem a_collided_chart_hides_the_gain :
    netOf 1 (salePostings ⟨1, 2, 1⟩ "USD" "VTI" 10 100 150) = -150
    ∧ ¬ (Accounts.distinct ⟨1, 2, 1⟩) := by
  constructor
  · decide
  · simp [Accounts.distinct]

/-- **⛔ AND THE UNITS LEAVE WITH THE VALUE.**

The instrument leg carries `-units`, so the position falls by what was sold.
`Ratio.Chart.Dimensions.units_are_not_conserved_by_a_purchase` — quantity is
measured rather than conserved, so nothing checks this for us and a sale that
moved cost without moving units would leave a position with basis and no
holding. That is the husk `Ratio.Lots.Edges` is about, created by the posting
rather than found in the data. -/
theorem the_units_leave_with_the_value :
    (salePostings ⟨1, 2, 3⟩ "USD" "VTI" 10 100 150).head?.map (·.quantity) = some (-10) := by
  decide

/-- **And the gain is denominated in the currency of the sale**, not a base.

A gain on a EUR-denominated lot is a EUR gain. Translating it at posting time
would mix a conversion into a realization and make the figure depend on when
somebody looked — `Ratio.Chart.Dimensions` conserves per currency precisely so
this stays separable. -/
theorem the_gain_is_in_the_currency_of_the_sale (a : Accounts) (i : String)
    (units relieved proceeds : Int) :
    netIn "EUR" (salePostings a "EUR" i units relieved proceeds) = 0
    ∧ netIn "USD" (salePostings a "EUR" i units relieved proceeds) = 0 := by
  constructor
  · simp [salePostings, netIn]; omega
  · simp [salePostings, netIn]

end Ratio.Lots
