import Ratio.Lots
import Ratio.Lots.Methods

set_option warningAsError true
/-! `Ratio.Lots.Wash` — a loss the rule DEFERS, and the ways an engine removes it.

A loss is disallowed when substantially identical stock is repurchased within a
window of the sale, and the disallowed amount attaches to the replacement lot's
BASIS. Both halves are the rule. An engine that implements the first half alone
is not partially correct — it permanently overtaxes the investor, and it does so
while every conservation check in this system passes.

⛔ THAT IS WHY THIS FILE EXISTS. `Ratio.Lots.cost_is_conserved` relates what was
taken to what remains and never mentions the gain. `Ratio.Chart` ties the trial
balance. Neither says a word about whether a disallowed loss came back. The
figure that goes wrong is the realized gain — the one with no counterparty,
which no reconciliation reaches, and which is the figure the investor is taxed
on.

⚠ AND IT INTERACTS WITH LOT SELECTION, so it cannot be bolted on afterwards.
Which lots a sale gives up decides which losses exist; a wash sale changes the
basis of a lot a LATER sale may relieve. `Ratio.Lots.Methods` and this are one
problem, and the arithmetic below is written so the two compose.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The window ────────────────────────────────────────────────────────── -/

/-- Whether a repurchase falls inside the disallowance window of a sale.

⚠ `30` IS A JURISDICTION'S NUMBER, exactly like the `365` in `isLongTerm`, and
it is a parameter here for the same reason: a fund administered under different
rules uses a different one, and a constant in the engine would make it wrong
somewhere rather than configurable everywhere.

⛔ AND THE WINDOW REACHES BOTH WAYS. A repurchase BEFORE the sale washes it just
as one after does. An engine that only looks forward is the natural
implementation — a sale arrives, you watch for a repurchase — and it is wrong on
every position that was topped up on the way down, which is the shape a loss
harvest actually has. -/
def inWashWindow (window saleDay buyDay : Int) : Bool :=
  decide (-window ≤ buyDay - saleDay ∧ buyDay - saleDay ≤ window)

/-- **⛔ THE WINDOW REACHES BACKWARDS, AND A FORWARD-ONLY ENGINE IS GREEN.**

A repurchase five days BEFORE a sale is inside a thirty-day window. Stated
concretely because the forward-only version satisfies every other theorem in
this file: it disallows correctly, attaches correctly, and conserves — it just
never fires on half the cases. -/
theorem the_window_reaches_backwards_too :
    inWashWindow 30 100 95 = true ∧ inWashWindow 30 100 105 = true := by
  decide

/-- **And it is a window, not a horizon.** -/
theorem outside_the_window_is_not_a_wash :
    inWashWindow 30 100 69 = false ∧ inWashWindow 30 100 131 = false := by
  decide

/- ── How much of the loss is disallowed ────────────────────────────────── -/

/-- The disallowed portion of a loss, as a POSITIVE magnitude.

`loss` is signed the way a realized gain is: negative when money was lost. The
result is what to add back, so it is non-negative, and it is `0` for a gain.

⛔ A GAIN IS NEVER WASHED. The rule disallows LOSSES; a repurchase after a
profitable sale changes nothing. An engine that tested "was there a repurchase
in the window" without testing the sign would defer gains as well, which
understates tax in the current period and is the error nobody reports.

⚠ PRO RATA BY UNITS, AND IT REFUSES RATHER THAN ROUNDS — the same decision as
`Ratio.Lots.partial_relief_is_exactly_pro_rata` and for the same reason: which
way to round is a term of an administration agreement, so it is not made here.
-/
def disallowed (loss soldUnits boughtUnits : Int) : Option Int :=
  if 0 ≤ loss then some 0
  else if soldUnits ≤ 0 then none
  else
    let matched := if boughtUnits ≤ soldUnits then boughtUnits else soldUnits
    if matched ≤ 0 then some 0
    else if (-loss) * matched % soldUnits ≠ 0 then none
    else some ((-loss) * matched / soldUnits)

/-- **A gain is never washed**, however much was repurchased. -/
theorem a_gain_is_never_washed (soldUnits boughtUnits : Int) :
    disallowed 40 soldUnits boughtUnits = some 0 := by
  simp [disallowed]

/-- **Repurchasing everything defers the whole loss.** -/
theorem repurchasing_everything_defers_the_whole_loss :
    disallowed (-1000) 100 100 = some 1000 := by
  decide

/-- **Repurchasing more than was sold defers the whole loss and no more.**

⚠ THE CAP IS THE POINT. Buying 250 shares back after selling 100 does not
disallow two and a half times the loss; the match is bounded by what was sold.
Unbounded, the arithmetic below still conserves — the engine would just attach a
basis adjustment nobody is entitled to. -/
theorem repurchasing_more_than_was_sold_defers_no_more :
    disallowed (-1000) 100 250 = some 1000 := by
  decide

/-- **Repurchasing part defers exactly that part.**

Forty of a hundred shares bought back: 400 of the 1000 loss is deferred and 600
is recognized now. -/
theorem repurchasing_part_defers_exactly_that_part :
    disallowed (-1000) 100 40 = some 400 := by
  decide

/-- **⛔ AND A PRO-RATA SPLIT THAT DOES NOT DIVIDE IS REFUSED.**

A loss of 1000 over 3 shares, one repurchased. A third of 1000 is not a whole
minor unit, and the engine reports that rather than choosing a direction. -/
theorem a_split_that_does_not_divide_is_refused :
    disallowed (-1000) 3 1 = none := by
  decide

/-- **Repurchasing nothing defers nothing** — the common case costs nothing. -/
theorem repurchasing_nothing_defers_nothing (loss : Int) :
    disallowed loss 100 0 = some 0 := by
  unfold disallowed
  split
  · rfl
  · simp

/- ── Where the deferred loss goes ──────────────────────────────────────── -/

/-- What the sale recognizes once the disallowance is applied.

`loss` is negative and `d` is the positive amount deferred, so this moves the
figure TOWARDS zero: less loss now. -/
def recognizedNow (loss d : Int) : Int := loss + d

/-- The replacement lot's basis, with the deferred loss attached.

⛔ THIS IS A WRITE TO A LOT THE ENGINE DID NOT RELIEVE, and nothing else in this
model does that. Relief consumes lots; this ADJUSTS one that a later sale will
consume. An implementation that keeps lots immutable after acquisition has no
place to put this, and the tempting alternative — a side table of adjustments
consulted at disposal — is a second source of basis, which is the shape of every
figure in this system that turned out to have two answers. -/
def replacementBasis (cost d : Int) : Int := cost + d

/-- **⭐ THE WASH RULE MOVES A LOSS IN TIME. IT DOES NOT REMOVE ONE.**

Recognize the reduced loss now, sell the replacement later against its adjusted
basis, and the total recognized over the two disposals is exactly what would
have been recognized had the rule never applied. The deferral nets out.

This is the whole content of the rule, and it is the property an engine breaks
by implementing the first half. `d` appears twice with opposite signs; drop
either occurrence and the investor is taxed on a loss they really suffered.

⚠ IT HOLDS FOR EVERY `d`, INCLUDING A WRONG ONE. That is deliberate and it is a
limitation worth naming: this theorem says the two halves cancel, not that the
disallowed amount was computed correctly. `repurchasing_part_defers_exactly_
that_part` is what pins the amount. Together they are the rule; either alone is
half of it. -/
theorem the_wash_rule_moves_a_loss_it_does_not_remove_it
    (loss d replacementCost laterProceeds : Int) :
    recognizedNow loss d + (laterProceeds - replacementBasis replacementCost d)
      = loss + (laterProceeds - replacementCost) := by
  unfold recognizedNow replacementBasis
  omega

/-- **⛔ AND DISALLOWING WITHOUT ATTACHING PERMANENTLY DESTROYS THE LOSS.**

The same two disposals with the basis adjustment omitted. A thousand of loss the
investor genuinely suffered is never recognized — not deferred, gone — and the
engine that produced it conserved cost, tied its trial balance and reproduced
its digest the entire time.

⚠ THIS IS THE FAILURE MODE, stated as a theorem so that "we disallow wash sales"
cannot be said of an engine that only does the first half. -/
theorem disallowing_without_attaching_destroys_the_loss :
    recognizedNow (-1000) 1000 + (5000 - 5000)
      ≠ (-1000 : Int) + (5000 - 5000) := by
  decide

/- ── The holding period ────────────────────────────────────────────────── -/

/-- The replacement's acquisition date for holding-period purposes: the ORIGINAL
lot's, not the repurchase's.

⚠ THE PERIOD TRANSFERS WITH THE BASIS. Getting this wrong does not change any
total — it moves a disposal between two tax RATES, which no conservation law and
no reconciliation can see. -/
def replacementAcquired (originalAcquired _repurchasedOn : Int) : Int := originalAcquired

/-- **⛔ AND THE TRANSFER DECIDES THE RATE.**

A lot acquired on day 0, washed by a repurchase on day 300, disposed on day 400.
Counting from the ORIGINAL acquisition the holding is 400 days and long-term;
counting from the repurchase it is 100 and short-term. Same units, same basis,
same proceeds, different rate — and every figure on the balance sheet is
identical either way. -/
theorem the_transferred_period_decides_the_rate :
    isLongTerm 365 (replacementAcquired 0 300) 400 = true
    ∧ isLongTerm 365 300 400 = false := by
  decide

/- ── Composing with lot selection ──────────────────────────────────────── -/

/-- **⛔ THE METHOD DECIDES WHETHER THERE IS A LOSS TO WASH AT ALL.**

Two lots of one unit — 10 and 40 — and a sale of one unit at 20. FIFO gives up
the cheap lot and realizes a GAIN of 10, which no repurchase can wash. LIFO
gives up the dear one and realizes a LOSS of 20, which a repurchase in the
window defers entirely.

The same holding, the same trade, the same repurchase: under one method the wash
rule does not fire and under the other it defers the whole loss. This is why the
two cannot be built in sequence — an engine that fixes the method after the wash
logic has already decided which losses exist. -/
theorem the_method_decides_whether_there_is_a_loss_to_wash :
    (relieveFifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map
        (fun r => disallowed (20 - takenCost r.1) 1 1)
      ≠ (relieveFifo ([⟨1, 1, 10⟩, ⟨2, 1, 40⟩] : List Lot).reverse 1).map
        (fun r => disallowed (20 - takenCost r.1) 1 1) := by
  decide

end Ratio.Lots
