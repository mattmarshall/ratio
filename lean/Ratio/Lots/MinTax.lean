import Ratio.Lots
import Ratio.Lots.Methods

set_option warningAsError true
/-! `Ratio.Lots.MinTax` — a ranking at a PRICE, not a sort of the holding.

`Ratio.Lots.Methods` proves the space of lot methods is not all orderings, and
names three shapes that are not: specific identification, average cost, and
tax-minimising relief. SpecID is `Ratio.Lots.SpecId`. Average cost is
`Ratio.Lots.AverageCost`. This file is the ranking, because the decision
surface is a function of the sale PRICE and the holding period, and an
`Order` has no place to put either.

⛔ A TAX-MINIMISING METHOD IS NOT A FUNCTION OF THE LOTS. Whether a lot yields
a GAIN or a LOSS depends on the sale price. A short-term LOSS is worth more
than a long-term one; a short-term GAIN is worth less. Same two lots, two
prices, two right answers:

  lot A   basis 10, held SHORT
  lot B   basis 12, held LONG

  sold at 50   A costs 80; B costs 38.  ⇒ give up B.
  sold at  5   A costs −10; B costs −7. ⇒ give up A.

`Ratio.Lots.Methods.a_tax_minimising_method_is_not_a_function_of_the_lots` is
the inequality. This file is the ranking that inequality describes, and the
proof that no `Order` reproduces both answers.

⚠ THE BASES MUST BE CLOSE FOR THE PREFERENCE TO FLIP. At a basis of 40 lot B
carries a loss so much larger that it wins at both prices, and the example
proves nothing — `decide` reported that version false. The close-base holding
is the one that is load-bearing; the far-base holding is on the record as the
vacuous case.

⚠ SO THIS IS NOT AN `Order`, AND ADDING IT AS ONE IS THE MISTAKE
`Ratio.Lots.Methods` exists to prevent. `//tla:mintax_engine_check` is the
sequence obligation: a sort-and-walk that never sees the price cannot flip.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The decision surface ─────────────────────────────────────────────── -/

/-- Per-unit proceeds of a sale, or nothing if the figure will not divide.

⛔ REFUSES RATHER THAN ROUNDS, the same decision as
`Ratio.Lots.partial_relief_is_exactly_pro_rata` and
`Ratio.Lots.Methods.an_average_that_does_not_divide_is_refused`. A sale of
three units for 100 does not have a whole-minor-unit price, and which way to
round would move the ranking of every lot — a misstatement of taxable income,
not a rounding error. -/
def unitPrice (proceeds want : Int) : Option Int :=
  if want ≤ 0 ∨ proceeds % want ≠ 0 then none
  else some (proceeds / want)

/-- **A price that will not divide is refused.** Three units for 100 is 33⅓,
and the ranking is not defined until the agreement says which way to round. -/
theorem a_price_that_does_not_divide_is_refused :
    unitPrice 100 3 = none := by
  decide

/-- **And one that does is the per-unit proceeds**, not a guess. -/
theorem a_price_that_divides_is_the_per_unit_proceeds :
    unitPrice 150 3 = some 50 := by
  decide

/-- Tax of giving up a lot at a per-unit sale price.

`gain = price * units − cost`. `taxCost` then weights a short-term result.
Cross-multiplied ranking below compares this figure per unit, so a large cheap
lot does not outrank a small dear one the way a total-cost HIFO would.

⚠ `price` IS PER UNIT. Total proceeds over units is `unitPrice`'s job; this
function is the ranking, and it is already a multiplication. -/
def taxAt (shortWeight price : Int) (l : Lot) (short : Bool) : Int :=
  taxCost shortWeight (price * l.units - l.cost) short

/-- Whether lot `a` is cheaper in tax per unit than lot `b` at this price.

⛔ CROSS-MULTIPLIED, NOT `tax / units`. Integer division would tie lots whose
per-unit tax differs by less than one, and a ranking that ties lots which are
not tied picks by whatever the sort does next — the same trap as
`dearerPerUnit`. -/
def cheaperTax (shortWeight price : Int)
    (a : Lot) (aShort : Bool) (b : Lot) (bShort : Bool) : Bool :=
  decide (taxAt shortWeight price a aShort * b.units
        < taxAt shortWeight price b bShort * a.units)

/- ── The close-base holding the flip needs ────────────────────────────── -/

/-- Lot A: one unit, basis 10. The short-term lot in the flip. -/
def lotA : Lot := ⟨1, 1, 10⟩

/-- Lot B: one unit, basis 12. The long-term lot in the flip.

⚠ TWELVE, NOT FORTY. At 40 the long lot's loss dominates both prices and the
example is vacuous. -/
def lotB : Lot := ⟨2, 1, 12⟩

def closeBases : List Lot := [lotA, lotB]

/-- A is short, B is long — looked up, not stored, for the same reason
`longestHeldFirst` looks up: `Lot` is `seq`/`units`/`cost`. -/
def closeAcq : Nat → Option Int
  | 1 => some 900
  | 2 => some 100
  | _ => none

/-- **⭐ AT A GAIN THE LONG LOT COSTS LESS.** Sold at 50: A realizes 40 short
(tax 80); B realizes 38 long (tax 38). Give up B. -/
theorem at_a_gain_the_long_lot_costs_less :
    cheaperTax 2 50 lotA true lotB false = false
    ∧ cheaperTax 2 50 lotB false lotA true = true := by
  constructor <;> decide

/-- **⭐ AT A LOSS THE SHORT LOT IS WORTH MORE.** Sold at 5: A realizes −5
short (tax −10); B realizes −7 long (tax −7). Give up A — the smaller loss
offsets income taxed at the higher rate. -/
theorem at_a_loss_the_short_lot_is_worth_more :
    cheaperTax 2 5 lotA true lotB false = true
    ∧ cheaperTax 2 5 lotB false lotA true = false := by
  constructor <;> decide

/-- **The same two lots, two prices, the preference flips.** This is
`a_tax_minimising_method_is_not_a_function_of_the_lots` as a ranking, not
just as an inequality on `taxCost`. -/
theorem the_preference_flips_when_the_bases_are_close :
    cheaperTax 2 50 lotA true lotB false = false
    ∧ cheaperTax 2 5 lotA true lotB false = true := by
  constructor <;> decide

/-- **⚠ FAR BASES DO NOT FLIP**, and that is why the close holding is the
example. Lot B at basis 40: at 50 its long gain of 10 costs 10 against A's 80;
at 5 its long loss of −35 costs −35 against A's −10. B wins both times, and
a test that used this holding would stay green on an engine that never saw
the price. -/
theorem far_bases_do_not_flip :
    let dear : Lot := ⟨2, 1, 40⟩
    cheaperTax 2 50 lotA true dear false = false
    ∧ cheaperTax 2 5 lotA true dear false = false := by
  constructor <;> decide

/- ── Ranking the holding at a price ───────────────────────────────────── -/

/-- Rank a holding by tax at a sale price, cheapest-tax first.

⛔ THE DATE IS LOOKED UP, NOT STORED ON THE LOT — same as `longestHeldFirst`.
A lot with no acquisition date cannot be classified short or long, and the
two obvious defaults are wrong in opposite directions. So it refuses.

⛔ AND THE PRICE IS AN ARGUMENT. `arrange` has no such parameter, which is
why this cannot be an `Order`. `//tla:sort_and_walk_mintax_check` is the
engine that pretends it can. -/
def arrangeMinTax (shortWeight threshold asOf price : Int)
    (acq : Nat → Option Int) : List Lot → Option (List Lot)
  | [] => some []
  | ls =>
    if ls.any (fun l => (acq l.seq).isNone) then none
    else some (sortBy (fun a b =>
      thenBySeq (fun x y =>
        cheaperTax shortWeight price
          x (decide (isLongTerm threshold ((acq x.seq).getD asOf) asOf = false))
          y (decide (isLongTerm threshold ((acq y.seq).getD asOf) asOf = false)))
        a b) ls)

/-- Relieve under MinTax: rank at the price, then walk.

Conservation is inherited — `relieveFifo` never cared which order it was
handed, and `every_ordering_method_conserves` already said so. What MinTax
can break is the gain, which is what it is for. -/
def relieveMinTax (shortWeight threshold asOf price : Int)
    (acq : Nat → Option Int) (ls : List Lot) (want : Int) :
    Option (List Taken × List Lot) :=
  match arrangeMinTax shortWeight threshold asOf price acq ls with
  | none => none
  | some ordered => relieveFifo ordered want

/-- **⛔ A LOT WITH NO ACQUISITION DATE REFUSES MINTAX.** Not "assume long",
not "assume short" — refuse. The rate is not a thing to guess from an absence. -/
theorem a_missing_acquisition_date_refuses_mintax :
    arrangeMinTax 2 365 1000 50 (fun _ => none) [lotA] = none := by
  decide

/-- **At 50 the ranking puts the long lot first.** Lot B, then lot A. -/
theorem at_fifty_the_long_lot_is_first :
    (arrangeMinTax 2 365 1000 50 closeAcq closeBases).map
        (fun ls => ls.map (·.seq))
      = some [2, 1] := by
  decide

/-- **At 5 the ranking puts the short lot first.** Lot A, then lot B. -/
theorem at_five_the_short_lot_is_first :
    (arrangeMinTax 2 365 1000 5 closeAcq closeBases).map
        (fun ls => ls.map (·.seq))
      = some [1, 2] := by
  decide

/-- **⭐ THE TWO PRICES RANK THE SAME HOLDING DIFFERENTLY.** That is the
whole content of "not a function of the lots": an `arrange` that does not
take a price cannot produce both of these lists. -/
theorem the_two_prices_rank_the_holding_differently :
    arrangeMinTax 2 365 1000 50 closeAcq closeBases
      ≠ arrangeMinTax 2 365 1000 5 closeAcq closeBases := by
  decide

/-- **And a sale of one unit takes the first of that ranking.** At 50 the
basis given up is 12; at 5 it is 10. Same holding, same want, two taxable
incomes. -/
theorem mintax_takes_different_lots_at_the_two_prices :
    (relieveMinTax 2 365 1000 50 closeAcq closeBases 1).map
        (fun r => takenCost r.1)
      = some 12
    ∧ (relieveMinTax 2 365 1000 5 closeAcq closeBases 1).map
        (fun r => takenCost r.1)
      = some 10 := by
  constructor <;> decide

/-- **⛔ NO ORDERING REPRODUCES BOTH ANSWERS.** An `Order` sees the lots and
not the price, so it produces one basis from this holding. FIFO and LOFO
give up 10; LIFO and HIFO give up 12. None give up 12 at one price and 10
at the other, because none of them take a price. -/
theorem no_ordering_reproduces_both_mintax_answers (m : Order) :
    (relieveBy m closeBases 1).map (fun r => takenCost r.1) ≠ some 12
    ∨ (relieveBy m closeBases 1).map (fun r => takenCost r.1) ≠ some 10 := by
  cases m <;> decide

/-- **Preferring long-term is not minimising tax.** `longestHeldFirst` puts
B first at every price, because it never sees one. MinTax puts B first at
50 and A first at 5. The distinction the deleted duplicate theorem was
reaching for, now stated against two functions rather than one inequality. -/
theorem preferring_long_term_is_not_minimising_tax :
    (longestHeldFirst 365 1000 closeAcq closeBases).map
        (fun ls => ls.map (·.seq))
      = some [2, 1]
    ∧ (arrangeMinTax 2 365 1000 5 closeAcq closeBases).map
        (fun ls => ls.map (·.seq))
      = some [1, 2] := by
  constructor <;> decide

/-- **⭐ MINTAX CONSERVES**, and it did not need a new proof. The walk is
`relieveFifo` over the ranked list; `cost_is_conserved` is stated over an
arbitrary list. -/
theorem mintax_conserves
    (shortWeight threshold asOf price : Int) (acq : Nat → Option Int)
    (ls : List Lot) (want : Int)
    (ts : List Taken) (left : List Lot)
    (h : relieveMinTax shortWeight threshold asOf price acq ls want
          = some (ts, left)) :
    takenCost ts + totalCost left
      = totalCost ((arrangeMinTax shortWeight threshold asOf price acq ls).getD []) := by
  unfold relieveMinTax at h
  cases harr : arrangeMinTax shortWeight threshold asOf price acq ls with
  | none =>
    rw [harr] at h
    simp at h
  | some ordered =>
    rw [harr] at h
    simp at h
    have := cost_is_conserved ordered want ts left h
    simp
    exact this

/-- **A partial MinTax relief is still exactly pro rata.** Ranking does not
get a rounding privilege the walk refused. Three units of a seven-unit lot
still refuse when the cost will not divide. -/
theorem mintax_partial_relief_is_exactly_pro_rata :
    relieveMinTax 2 365 1000 50 (fun _ => some 100) [⟨1, 7, 100⟩] 3 = none := by
  decide

/-- **⚠ A HUSK IS STILL A HUSK.** Ranking at a price does not make
`relieveFifo` refuse zero units. The walk consumes it and hands over the
basis — `Ratio.Lots.Edges.a_husk_gives_away_its_cost`. Refusal belongs
where the lot is offered, not here. Stated so nobody "fixes" MinTax by
adding a check the walk already failed to have.

One unit wanted; the husk (seq 1, 0 units, cost 40) is taken whole because
`0 ≤ want`, then the real lot. Taken cost 50 for one unit. -/
theorem mintax_inherits_the_husk :
    (relieveMinTax 2 365 1000 50 (fun _ => some 100)
        [⟨1, 0, 40⟩, ⟨2, 1, 10⟩] 1).map
        (fun r => takenCost r.1)
      = some 50 := by
  decide

/-- **Equal tax at one price falls back to acquisition order**, not to
whatever the sort happened to do. Two short lots at the same basis, sold
at the same price: sequence, not a coin flip. Two runs of the same fund
must produce the same figure. -/
theorem equal_tax_falls_back_to_acquisition_order :
    (arrangeMinTax 2 365 1000 50 (fun _ => some 900)
        [⟨2, 1, 10⟩, ⟨1, 1, 10⟩]).map
        (fun ls => ls.map (·.seq))
      = some [1, 2] := by
  decide

end Ratio.Lots
