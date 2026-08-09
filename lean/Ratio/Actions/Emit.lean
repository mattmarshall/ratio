import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `Ratio.Actions.Factor`'s decisions as one `syn::File` JSON.

⛔ WHAT IS EMITTED IS THE PER-STEP GUARD, NOT A COMPOSED FACTOR, and that is the
whole lesson of `a_factor_can_succeed_where_the_rewrite_refuses`. Composing
3-for-2 and 2-for-1 gives ⟨6,2⟩, which divides five units cleanly and returns
fifteen — while applying them in order REFUSES, because the intermediate 7.5
shares were never held and the half was paid out as cash in lieu.

So the running code folds the announcements one at a time and asks these
functions at each step. That is O(splits) on the lots actually read — a NAV
reads none, a sale reads a handful — rather than O(lots), which is what the
rewrite costs and what `Ratio.Closure.an_open_action_makes_the_nav_read_the_lots`
is about.

⚠ `rem_euclid`/`div_euclid`, NOT `%` and `/`. Lean's `Int` division is
Euclidean and Rust's truncates; they disagree the moment a quantity is negative,
and a short position has negative units. Same trap `Ratio.Closure.Emit` names
for `perShare`, and the reason it is worth naming twice is that the ordinary
case works identically under both.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- `a.rem_euclid(b)` — Lean's `%` on `Int`. -/
private def remEuclid (a b : Json) : Json := mkMethodCall a "rem_euclid" [b]

/-- `a.div_euclid(b)` — Lean's `/` on `Int`. -/
private def divEuclid (a b : Json) : Json := mkMethodCall a "div_euclid" [b]

/-- The items of the emitted corporate-action core. -/
def ratioActionsItems : List Json :=
  [ -- pub fn step_divides(units: i64, num: i64, den: i64) -> bool {
    --   den != 0 && (units * num).rem_euclid(den) == 0 }
    --
    -- ⛔ ONE STEP. `Ratio.Actions.Factor.a_factor_can_succeed_where_the_rewrite
    -- _refuses` — asking this of a COMPOSED ratio instead would let a holding
    -- through that was really paid out as cash in lieu.
    mkFnItemD "step_divides"
      [param "units" "i64", param "num" "i64", param "den" "i64"] (ty "bool")
      [tail (mkBin "&&"
              (mkBin "!=" (mkPath "den") (mkLitInt "0"))
              (mkBin "=="
                (remEuclid (mkParen (mkBin "*" (mkPath "units") (mkPath "num"))) (mkPath "den"))
                (mkLitInt "0")))],

    -- pub fn step_units(units: i64, num: i64, den: i64) -> i64 {
    --   (units * num).div_euclid(den) }
    -- `Ratio.Actions.split` — only meaningful where `step_divides` holds.
    mkFnItemD "step_units"
      [param "units" "i64", param "num" "i64", param "den" "i64"] (ty "i64")
      [tail (divEuclid (mkParen (mkBin "*" (mkPath "units") (mkPath "num"))) (mkPath "den"))],

    -- pub fn relief_divides(wanted: i64, num: i64, den: i64) -> bool {
    --   num != 0 && (wanted * den).rem_euclid(num) == 0 }
    --
    -- The other direction: a sale quoted in TODAY's units against a lot stored
    -- in the units it was bought in.
    -- `Ratio.Actions.Factor.an_odd_sale_after_a_split_does_not_land_on_a_
    -- stored_lot` — 101 shares at 2-for-1 is fifty and a half stored units.
    --
    -- ⛔ FAILING THIS IS NOT THE CASH-IN-LIEU CASE AND MUST NOT BE HANDLED LIKE
    -- IT. Nothing fractional is being paid away; the client owns 101 shares and
    -- is selling all of them. The fraction is an artifact of storing pre-split
    -- units, and the answer is to SUBDIVIDE the lot, not to refuse the sale.
    -- The two failures are the same division and mean opposite things.
    mkFnItemD "relief_divides"
      [param "wanted" "i64", param "num" "i64", param "den" "i64"] (ty "bool")
      [tail (mkBin "&&"
              (mkBin "!=" (mkPath "num") (mkLitInt "0"))
              (mkBin "=="
                (remEuclid (mkParen (mkBin "*" (mkPath "wanted") (mkPath "den"))) (mkPath "num"))
                (mkLitInt "0")))],

    -- pub fn relief_units(wanted: i64, num: i64, den: i64) -> i64 {
    --   (wanted * den).div_euclid(num) }
    mkFnItemD "relief_units"
      [param "wanted" "i64", param "num" "i64", param "den" "i64"] (ty "i64")
      [tail (divEuclid (mkParen (mkBin "*" (mkPath "wanted") (mkPath "den"))) (mkPath "num"))]
  ]

def main : IO Unit :=
  IO.println (mkFile ratioActionsItems).compress
