import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `Ratio.Closure`'s cost model as one `syn::File` JSON.

`Ratio.Closure` answers "how long does this fund's period end take" in READS.

⛔ AND IT EMITS TWO ANSWERS, because the interesting claim is only true of one
of them. A NAV never reads the tax lots — twenty million cost nothing — EXCEPT
that applying an open action by rewriting IS a walk over them, forty thousand
per split. `nav_cost` is what the system does today and carries that cliff;
`factor_nav_cost` is what it costs once `Ratio.Actions.Factor` lands, and is the
one for which the flat-in-fragmentation claim holds unconditionally.

Emitting both is deliberate. A single number would have to pick, and picking the
better one would be the same mistake the model already made once.

⛔ EMITTED RATHER THAN REWRITTEN, because the entire value of the answer is that
it is the proved function. A hand-written `nav_cost` in Rust would be a claim
ABOUT `Ratio.Closure`; this one IS it. The thing being sold is that the model
and the running code cannot disagree, and a second implementation is exactly how
they would.

⛔ AND THE ONE PLACE THE TWO LANGUAGES DO NOT AGREE BY DEFAULT: `perShare`.
Lean's `/` and `%` on `Int` are EUCLIDEAN — the residual is never negative.
Rust's truncate toward zero. `Ratio.Closure` pins the case that separates them
(`perShare (-7) 3 = (-3, 2)`, not `(-2, -1)`), and this file emits
`div_euclid`/`rem_euclid` so the emitted code lands on the proved side.

⚠ `residual_is_accounted` CANNOT catch that. `units * q + r = nav` holds under
both conventions, so the theorem is true of the wrong one too. It is the
`example`, not the theorem, that decides this.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def derives : List String :=
  ["Clone", "Copy", "Debug", "PartialEq", "Eq"]

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- `d.<f>` — a dial. -/
private def dial (f : String) : Json := mkField (mkPath "d") f

/-- `<name>(d)` — one of the terms below, called by name so the sum reads the
way `Ratio.Closure.navCost` is written. -/
private def term (name : String) : Json := mkCall (mkPath name) [mkPath "d"]

/-- The items of the emitted cost model. -/
def ratioClosureItems : List Json :=
  [ -- pub struct Dials { securities, currencies, lots_per, open_actions,
    --                    capital_txns: i64 }
    -- `Ratio.Closure.Dials`. Every dial a period end's cost turns on.
    mkStructItemD "Dials"
      [("securities", ty "i64"),
       ("currencies", ty "i64"),
       ("lots_per", ty "i64"),
       ("open_actions", ty "i64"),
       ("capital_txns", ty "i64")]
      derives,

    -- pub fn lots(d: Dials) -> i64 { d.securities * d.lots_per }
    -- `Ratio.Closure.lots` — the number that frightens people, and the one
    -- `nav_cost` below never reads.
    mkFnItemD "lots" [param "d" "Dials"] (ty "i64")
      [tail (mkBin "*" (dial "securities") (dial "lots_per"))],

    -- pub fn mark_cost(d: Dials) -> i64 { d.securities }
    -- One price per security. Independent of lots.
    mkFnItemD "mark_cost" [param "d" "Dials"] (ty "i64")
      [tail (dial "securities")],

    -- pub fn fx_cost(d: Dials) -> i64 { d.currencies }
    -- ⛔ NOT one per position. Translation applies to per-currency SUBTOTALS,
    -- so five hundred names in three currencies is three translations.
    -- `Ratio.Closure.fx_does_not_grow_with_the_chart`.
    mkFnItemD "fx_cost" [param "d" "Dials"] (ty "i64")
      [tail (dial "currencies")],

    -- pub fn action_cost(d: Dials) -> i64 { d.open_actions * d.lots_per }
    -- ⛔ `lots_per`, NOT `securities`. A split reaches into ONE instrument and
    -- touches every LOT of it. This read `securities` until 2026-08-09 and
    -- understated its own worst term by eighty.
    mkFnItemD "action_cost" [param "d" "Dials"] (ty "i64")
      [tail (mkBin "*" (dial "open_actions") (dial "lots_per"))],

    -- pub fn factor_action_cost(_d: Dials) -> i64 { 0 }
    -- Applying by composing a factor read at the point of use: nothing is
    -- written, so bringing the book up to date costs nothing.
    -- `Ratio.Actions.Factor` is the equivalence and its counterexample.
    mkFnItemD "factor_action_cost" [param "_d" "Dials"] (ty "i64")
      [tail (mkLitInt "0")],

    -- pub fn capital_cost(d: Dials) -> i64 { d.capital_txns }
    mkFnItemD "capital_cost" [param "d" "Dials"] (ty "i64")
      [tail (dial "capital_txns")],

    -- pub fn nav_cost(d: Dials) -> i64 {
    --   mark_cost(d) + fx_cost(d) + action_cost(d) + capital_cost(d) }
    -- ⛔ `lots` is absent HERE and present inside `action_cost`, which is
    -- exactly the problem: `Ratio.Closure.an_open_action_makes_the_nav_read_
    -- the_lots`. The flat-in-fragmentation claim holds only on a quiet day.
    mkFnItemD "nav_cost" [param "d" "Dials"] (ty "i64")
      [tail (mkBin "+"
              (mkBin "+"
                (mkBin "+" (term "mark_cost") (term "fx_cost"))
                (term "action_cost"))
              (term "capital_cost"))],

    -- pub fn factor_nav_cost(d: Dials) -> i64 {
    --   mark_cost(d) + fx_cost(d) + factor_action_cost(d) + capital_cost(d) }
    -- ⭐ `Ratio.Closure.factored_nav_never_reads_the_lots` — unconditional,
    -- for any number of open actions. What the other one was meant to say.
    mkFnItemD "factor_nav_cost" [param "d" "Dials"] (ty "i64")
      [tail (mkBin "+"
              (mkBin "+"
                (mkBin "+" (term "mark_cost") (term "fx_cost"))
                (term "factor_action_cost"))
              (term "capital_cost"))],

    -- pub struct PerShare { per_unit: i64, residual: i64 }
    -- A tuple would do in Rust, but `.1` at a call site is how a residual gets
    -- dropped. The residual is the fund's money; it gets a name.
    mkStructItemD "PerShare"
      [("per_unit", ty "i64"), ("residual", ty "i64")]
      derives,

    -- pub fn per_share(nav: i64, units: i64) -> PerShare {
    --   PerShare { per_unit: nav.div_euclid(units),
    --              residual: nav.rem_euclid(units) } }
    --
    -- ⛔ `div_euclid`/`rem_euclid`, NOT `/` and `%`. Lean's `Int` division is
    -- Euclidean; Rust's truncates toward zero, and they disagree whenever the
    -- NAV is negative. `Ratio.Closure` pins `perShare (-7) 3 = (-3, 2)`.
    --
    -- ⚠ `residual_is_accounted` holds under BOTH conventions, so the theorem
    -- cannot tell them apart. This is one of the few places where the guard is
    -- an example rather than a proof.
    mkFnItemD "per_share" [param "nav" "i64", param "units" "i64"] (ty "PerShare")
      [tail (mkStructLit "PerShare"
        [("per_unit", mkMethodCall (mkPath "nav") "div_euclid" [mkPath "units"]),
         ("residual", mkMethodCall (mkPath "nav") "rem_euclid" [mkPath "units"])])]
  ]

def main : IO Unit :=
  IO.println (mkFile ratioClosureItems).compress
