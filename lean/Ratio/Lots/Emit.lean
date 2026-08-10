import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `Ratio.Lots`'s per-lot decisions as one `syn::File` JSON.

`relieveFifo` walks a list, and the walk belongs in Rust — it is a fold, and a
fold over lots paged out of a store is not something to express in Lean. What is
emitted is what the walk DECIDES at each lot, which is where a mistake would be
invisible:

* `takes_whole_lot` — this lot is consumed entirely, or split. The boundary is
  `≤` rather than `<`, and `Ratio.Lots.Edges.selling_a_whole_lot_leaves_no_husk`
  is why: `<` would leave a zero-unit remainder carrying no cost, and a husk is
  precisely what `a_husk_gives_away_its_cost` shows being consumed by the NEXT
  sale for basis it does not represent.

* `partial_divides` / `partial_cost` — taking part of a lot divides its cost pro
  rata, and `Ratio.Lots.partial_relief_is_exactly_pro_rata` is the theorem that
  the split is exact. ⛔ IT REFUSES RATHER THAN ROUNDING: which way to round is
  a term of an administration agreement, and conservation holds of a rounded
  answer, so nothing downstream would catch it.

* `lot_is_sound` — `Ratio.Lots.Edges.sound`. Positive units, or nothing at all.

⚠ `rem_euclid`/`div_euclid`, not `%` and `/`. Lean's `Int` division is Euclidean
and Rust's truncates; a lot can carry negative cost after an allocation, and the
two disagree there. Third place this trap has come up.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- The items of the emitted relief core. -/
def ratioLotsItems : List Json :=
  [ -- pub fn takes_whole_lot(lot_units: i64, want: i64) -> bool { lot_units <= want }
    -- ⛔ `<=`, not `<`. See the module note: `<` leaves a husk.
    mkFnItemD "takes_whole_lot" [param "lot_units" "i64", param "want" "i64"] (ty "bool")
      [tail (mkBin "<=" (mkPath "lot_units") (mkPath "want"))],

    -- pub fn partial_divides(cost: i64, want: i64, lot_units: i64) -> bool {
    --   lot_units != 0 && (cost * want).rem_euclid(lot_units) == 0 }
    mkFnItemD "partial_divides"
      [param "cost" "i64", param "want" "i64", param "lot_units" "i64"] (ty "bool")
      [tail (mkBin "&&"
              (mkBin "!=" (mkPath "lot_units") (mkLitInt "0"))
              (mkBin "=="
                (mkMethodCall
                  (mkParen (mkBin "*" (mkPath "cost") (mkPath "want")))
                  "rem_euclid" [mkPath "lot_units"])
                (mkLitInt "0")))],

    -- pub fn partial_cost(cost: i64, want: i64, lot_units: i64) -> i64 {
    --   (cost * want).div_euclid(lot_units) }
    -- `Ratio.Lots.partial_relief_is_exactly_pro_rata`.
    mkFnItemD "partial_cost"
      [param "cost" "i64", param "want" "i64", param "lot_units" "i64"] (ty "i64")
      [tail (mkMethodCall
              (mkParen (mkBin "*" (mkPath "cost") (mkPath "want")))
              "div_euclid" [mkPath "lot_units"])],

    -- pub fn lot_is_sound(units: i64, cost: i64) -> bool {
    --   0 < units || (units == 0 && cost == 0) }
    -- `Ratio.Lots.Edges.sound`.
    mkFnItemD "lot_is_sound" [param "units" "i64", param "cost" "i64"] (ty "bool")
      [tail (mkBin "||"
              (mkBin "<" (mkLitInt "0") (mkPath "units"))
              (mkParen (mkBin "&&"
                (mkBin "==" (mkPath "units") (mkLitInt "0"))
                (mkBin "==" (mkPath "cost") (mkLitInt "0")))))]
  ]

def main : IO Unit :=
  IO.println (mkFile ratioLotsItems).compress
