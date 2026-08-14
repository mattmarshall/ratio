import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `Ratio.Tolerance`'s two decisions as one `syn::File` JSON.

⛔ THE DECISION IS EMITTED AND THE WALK IS NOT, the arrangement
`Ratio.Exec.Emit` and `Ratio.Ingest.Emit` already use. The running code loops
over a reconciliation report's lines; what it asks about each one comes from
here. `Ratio.Tolerance.the_three_bands_partition_every_difference` is the
bridge: the grade depends on nothing but the magnitude and the two bounds, so a
Rust loop and a Lean theorem cannot come to disagree about a severity.

⚠ THE MAGNITUDE IS A PARAMETER, NOT TAKEN HERE, and that is deliberate rather
than tidy. `Ratio.Tolerance.magnitude` negates, and negation on `i64` is the
one arithmetic operation in this file that can fail: `i64::MIN` has no positive
counterpart, so `(-d)` wraps back to itself and a grading asked about the
wrapped number is answering about a difference that never happened. The theorem
is over `Int`, which is unbounded and where the negation simply exists — the
gap `Ratio.Bounded` is about. So the Rust takes the magnitude with a checked
absolute value BEFORE it asks anything, and grades an unrepresentable one as
blocking. Asking first would be asking about a wrapped number.

⛔ THE RETURNED CODES ARE THE WIRE'S, NOT AN INVENTION: 1 low, 2 medium, 3 high,
matching `ratio.console.v1.Severity`, whose 0 is UNSPECIFIED and is therefore
not a grade this can return. A separate mapping table on the Rust side would be
a second place for the order to be wrong.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- The items of the emitted tolerance core. -/
def ratioToleranceItems : List Json :=
  [ -- pub fn severity_of(magnitude: i64, below_notice: i64, blocks_nav: i64) -> i64
    -- `Ratio.Tolerance.severity`, over a magnitude the caller has already taken.
    -- ⚠ Both comparisons are `<=`: a difference of exactly a bound is IN the
    -- band it names. `a_difference_at_the_threshold_blocks_the_nav`.
    mkFnItemD "severity_of"
      [param "magnitude" "i64", param "below_notice" "i64", param "blocks_nav" "i64"]
      (ty "i64")
      [tail (mkIfThenElse (mkBin "<=" (mkPath "blocks_nav") (mkPath "magnitude"))
              [tail (mkLitInt "3")]
              [tail (mkIfThenElse (mkBin "<=" (mkPath "below_notice") (mkPath "magnitude"))
                      [tail (mkLitInt "2")]
                      [tail (mkLitInt "1")])])],

    -- pub fn tolerance_is_well_formed(below_notice: i64, blocks_nav: i64) -> bool
    -- `Ratio.Tolerance.wellFormed`. ⛔ Read when the CONFIGURATION is read, not
    -- when a break is graded: bounds the wrong way round leave a grade nothing
    -- can ever be — `an_inverted_tolerance_makes_the_middle_band_unreachable`.
    mkFnItemD "tolerance_is_well_formed"
      [param "below_notice" "i64", param "blocks_nav" "i64"]
      (ty "bool")
      [tail (mkBin "&&"
              (mkBin "<=" (mkLitInt "0") (mkPath "below_notice"))
              (mkBin "<=" (mkPath "below_notice") (mkPath "blocks_nav")))]
  ]

def main : IO Unit :=
  IO.println (mkFile ratioToleranceItems).compress
