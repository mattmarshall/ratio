import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `ratio-ingest`'s decided core as one `syn::File` JSON.

The data plane is mostly folds — over rows in a file, over rungs in an identity
ladder, over entities in the master. Those are hand-written in `lib.rs`. What is
emitted here is the part of the plane that *decides*, because that is where a
mistake would be invisible:

* `resolution_of_count` — nothing matched, exactly one matched, or more than one
  did. `Ratio.Ingest.kind_is_determined_by_count` proves the outcome depends on
  nothing but the count, which is what lets the Rust count with a loop and
  decide with this without the two being able to disagree.
* `rung_satisfied` — the empty-rung guard. `List.all` on an empty list is
  `true`, so a ladder that read only columns the file does not have would match
  EVERY instrument in the master and book against an arbitrary one.
  `Ratio.Ingest.empty_rung_matches_nothing` is the theorem, and removing the
  guard makes it unprovable — checked, not assumed.
* `claim_holds` — silence is not assent. An entity that records nothing for an
  attribute does not match a claim about it.
* `scale_for` / `minor_units` — a decimal to minor units. A multiplication and
  an addition, with `0` standing for "not representable" so a third decimal
  place is refused rather than truncated.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def derives : List String := ["Clone", "Copy", "Debug", "PartialEq", "Eq"]

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- `ResolutionKind::<v>`. -/
private def kind (v : String) : Json := mkPathSegments ["ResolutionKind", v]

/-- The items of the emitted `ratio_ingest` crate. -/
def ratioIngestItems : List Json :=
  [ mkEnumItem "ResolutionKind" ["Resolved", "Absent", "Ambiguous"] derives,

    -- pub fn resolution_of_count(n: i64) -> ResolutionKind {
    --   match n { 0 => Absent, 1 => Resolved, _ => Ambiguous } }
    -- Mirrors `Ratio.Ingest.resolutionOfCount`; the bridge to the fold is
    -- `kind_is_determined_by_count`.
    mkFnItemD "resolution_of_count" [param "n" "i64"] (ty "ResolutionKind")
      [tail (mkMatch (mkPath "n")
        [mkMatchArm (mkPatLit (mkLitInt "0")) (kind "Absent"),
         mkMatchArm (mkPatLit (mkLitInt "1")) (kind "Resolved"),
         mkMatchArm mkPatWild (kind "Ambiguous")])],

    -- pub fn rung_satisfied(is_empty: bool, all_hold: bool) -> bool
    --   { !is_empty && all_hold }
    mkFnItemD "rung_satisfied"
      [param "is_empty" "bool", param "all_hold" "bool"] (ty "bool")
      [tail (mkBin "&&" (mkUnary "!" (mkPath "is_empty")) (mkPath "all_hold"))],

    -- pub fn claim_holds(recorded: bool, values_equal: bool) -> bool
    --   { recorded && values_equal }
    mkFnItemD "claim_holds"
      [param "recorded" "bool", param "values_equal" "bool"] (ty "bool")
      [tail (mkBin "&&" (mkPath "recorded") (mkPath "values_equal"))],

    -- pub fn scale_for(frac_len: i64) -> i64 {
    --   match frac_len { 0 => 100, 1 => 10, 2 => 1, _ => 0 } }
    -- 0 means NOT REPRESENTABLE. `scale_int_zero_iff_refused` proves that is
    -- exactly the case the proved `scale` refuses.
    mkFnItemD "scale_for" [param "frac_len" "i64"] (ty "i64")
      [tail (mkMatch (mkPath "frac_len")
        [mkMatchArm (mkPatLit (mkLitInt "0")) (mkLitInt "100"),
         mkMatchArm (mkPatLit (mkLitInt "1")) (mkLitInt "10"),
         mkMatchArm (mkPatLit (mkLitInt "2")) (mkLitInt "1"),
         mkMatchArm mkPatWild (mkLitInt "0")])],

    -- pub fn minor_units(whole: i64, frac: i64, scale: i64) -> i64
    --   { whole * 100 + frac * scale }
    -- No division. There is nowhere for a rounding decision to hide.
    mkFnItemD "minor_units"
      [param "whole" "i64", param "frac" "i64", param "scale" "i64"] (ty "i64")
      [tail (mkBin "+"
        (mkBin "*" (mkPath "whole") (mkLitInt "100"))
        (mkBin "*" (mkPath "frac") (mkPath "scale")))] ]

def main : IO Unit :=
  IO.println (Json.compress (mkFile ratioIngestItems))
