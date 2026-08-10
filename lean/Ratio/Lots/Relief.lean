import Ratio.Lots
import Ratio.Actions.Factor

set_option warningAsError true
/-! `Ratio.Lots.Relief` — what a sale produces, not just what it consumes.

`Ratio.Lots` proves relief conserves: what was taken plus what remains is what
was there. That is the whole of what it says, and an engine needs three things
it does not cover.

⛔ THE REALIZED GAIN. A sale fetches proceeds and gives up cost, and the
difference is a posting — the one figure in this system that is CREATED by a
transaction rather than moved by it. `Ratio.Lots.cost_is_conserved` holds no
matter what the sale fetched, so it cannot say anything about whether the gain
is right; it is true of a book that posted no gain at all.

⛔ THE METHOD. FIFO is one choice. HIFO, LIFO and specific identification are
others, and they produce DIFFERENT gains from the same lots and the same sale —
which is the entire reason the choice is a term of an agreement rather than an
implementation detail. What must not differ is the conservation, and stating
that as a theorem is what stops a new method being added by someone who checks
the gain and not the invariant.

⛔ AND THE SPLITS. A sale is quoted in TODAY's units; the lots are stored in the
units they were bought in. `Ratio.Actions.Factor.reliefInStoredUnits` converts
one quantity; relieving a whole sale through a factor is the composition, and
nobody had proved the composition conserves anything.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

open Ratio.Actions (Ratio)

/-- What a sale realized: proceeds less the cost of what it gave up. -/
def gain (proceeds : Int) (ts : List Taken) : Int := proceeds - takenCost ts

/-- **The gain and the cost it relieved are exactly the proceeds.**

Nothing is created and nothing is lost at the point of sale: the money that came
in is accounted for as return of cost plus profit, and those two are the only
places it can go. A book where these did not sum would be one where the gain was
computed against something other than the lots it actually relieved. -/
theorem the_gain_and_the_relieved_cost_are_the_proceeds (proceeds : Int) (ts : List Taken) :
    gain proceeds ts + takenCost ts = proceeds := by
  simp [gain]

/-- **⛔ A GAIN IS NOT A REVALUATION.**

What remains is worth what it cost, whatever the sale fetched. The proceeds do
not appear in this statement at all — that is the content: a book that touched
the surviving lots when it recorded a gain would be marking the rest of the
holding to the trade price, which is a valuation nobody asked for and one the
market did not give.

⚠ `cost_is_conserved` CANNOT SAY THIS. It relates taken and remaining to the
original and never mentions proceeds, so it is equally true of an implementation
that wrote the sale price over every surviving lot and adjusted the taken cost to
match. The two theorems together are what pin it. -/
theorem a_gain_is_not_a_revaluation (ls : List Lot) (want : Int)
    (ts : List Taken) (left : List Lot) (h : relieveFifo ls want = some (ts, left)) :
    totalCost left = totalCost ls - takenCost ts := by
  have := cost_is_conserved ls want ts left h
  omega

/-- Relieve by taking the NEWEST lot first — one alternative method, defined by
reversing the order FIFO reads. Enough to show the choice has consequences. -/
def relieveLifo (ls : List Lot) (want : Int) : Option (List Taken × List Lot) :=
  relieveFifo ls.reverse want

/-- **⛔ THE METHOD CHANGES THE GAIN.**

Two lots of one unit — one bought at 10, one at 40 — and a sale of one unit for
50. FIFO gives up the cheap lot and realizes 40. LIFO gives up the dear one and
realizes 10.

Four times the taxable gain from the same holding and the same trade, which is
why the method is a term of an administration agreement and not something an
engine picks. An implementation that changed it would restate every investor's
tax position without changing a single figure on the balance sheet. -/
theorem the_method_changes_the_gain :
    (relieveFifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => gain 50 r.1)
      ≠ (relieveLifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => gain 50 r.1) := by
  simp [relieveFifo, relieveLifo, gain, takenCost]

/-- **And it changes nothing that is conserved.**

The same lots, the same sale, a different method — and what was taken plus what
remains is what was there, both ways. This is the invariant a new method has to
preserve, and it is deliberately stated over the two concrete methods rather
than assumed: someone adding HIFO will check that the gain looks right, because
the gain is what the method is FOR, and this is what they will not think to
check. -/
theorem the_method_changes_nothing_that_is_conserved :
    (relieveFifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => takenCost r.1 + totalCost r.2)
      = (relieveLifo [⟨1, 1, 10⟩, ⟨2, 1, 40⟩] 1).map (fun r => takenCost r.1 + totalCost r.2) := by
  simp [relieveFifo, relieveLifo, takenCost, totalCost]

/- ── Selling through open splits ───────────────────────────────────────── -/

/-- Relieve a sale quoted in TODAY's units against lots stored in their own.

⛔ THE CONVERSION HAPPENS ONCE, ON THE QUANTITY, and never on the lots. Scaling
the lots would be the REWRITE this whole redesign exists to avoid — forty
thousand of them for one split — and it would also lose the acquisition cost,
because a scaled lot is a different lot. -/
def relieveThrough (steps : List Actions.Ratio) (ls : List Lot) (wantedToday : Int) :
    Option (List Taken × List Lot) :=
  match Ratio.Actions.reliefInStoredUnits (Ratio.Actions.compose steps) wantedToday with
  | none => none
  | some stored => relieveFifo ls stored

/-- **Relieving through a factor conserves cost, exactly as relieving without
one does.**

The composition nobody had checked. `Ratio.Lots.cost_is_conserved` is about raw
lots and `Ratio.Actions.Factor` is about one quantity; this is the two of them
in the same statement, which is where a system usually turns out to have been
proved in halves. -/
theorem relief_through_a_factor_conserves_cost
    (steps : List Actions.Ratio) (ls : List Lot) (want : Int)
    (ts : List Taken) (left : List Lot) (h : relieveThrough steps ls want = some (ts, left)) :
    takenCost ts + totalCost left = totalCost ls := by
  unfold relieveThrough at h
  split at h
  · simp at h
  · exact cost_is_conserved ls _ ts left h

/-- **A hundred units after a two-for-one relieve fifty stored ones**, and the
cost that comes with them is the cost of fifty — not of a hundred.

The number that would be wrong if the conversion were skipped, stated concretely
because "conserves cost" is true of the wrong answer too: relieving a hundred
stored units would also conserve, and would give up twice the basis. -/
theorem selling_through_a_split_relieves_the_stored_units :
    relieveThrough [⟨2, 1⟩] [⟨1, 100, 1000⟩] 100
      = some ([⟨1, 50, 500⟩], [⟨1, 50, 500⟩]) := by
  simp [relieveThrough, Ratio.Actions.reliefInStoredUnits, Ratio.Actions.compose, relieveFifo]

/-- **⛔ AND A SALE THAT STRADDLES A STORED LOT IS REFUSED HERE.**

101 units after a two-for-one is fifty and a half stored units, and
`reliefInStoredUnits` reports the fraction rather than rounding it away.

⚠ THIS REFUSAL IS NOT THE CASH-IN-LIEU REFUSAL, and treating it as one would
reject a perfectly ordinary instruction. The client owns 101 shares and is
selling all of them; the fraction is an artifact of storing pre-split units. The
engine's answer is to SUBDIVIDE the straddled lot —
`Ratio.Lots.partition_sums_to_whole` says that changes nothing held and
`partial_relief_is_exactly_pro_rata` says the cost follows exactly — which is
one write, against the forty thousand a rewrite would have cost. -/
theorem a_straddling_sale_is_reported_rather_than_rounded :
    relieveThrough [⟨2, 1⟩] [⟨1, 100, 1000⟩] 101 = none := by
  simp [relieveThrough, Ratio.Actions.reliefInStoredUnits, Ratio.Actions.compose]

/-- An instrument with no corporate history relieves exactly as it always did,
so the factor path costs nothing on the common case. -/
theorem no_history_relieves_the_way_it_always_did (ls : List Lot) (want : Int) :
    relieveThrough [] ls want = relieveFifo ls want := by
  simp [relieveThrough, Ratio.Actions.reliefInStoredUnits, Ratio.Actions.compose]

end Ratio.Lots
