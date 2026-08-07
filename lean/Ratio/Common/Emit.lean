import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `ratio-common`'s Rust (the Money value layer) as one `syn::File` JSON,
via the `Polyglot.Rust` builders. Captured by `lean_emit` → rendered by
`json_to_rust`; the committed `crates/ratio-common/src/generated.rs` is gated
against this so Lean stays authoritative.

Money is integer minor-units (`i64`), never `f64` — the correct accounting
representation. The algebraic properties of `money_add` / `money_zero` are
proven in `Ratio.Core` (`//lean:money_proof_test`). -/

open Lean (Json)
open Polyglot.Rust.Builders

private def derives : List String := ["Clone", "Copy", "Debug", "PartialEq", "Eq"]

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def fieldOf (v f : String) : Json := mkField (mkPath v) f
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- The items of the emitted `ratio_common` crate: the value types + the Money
operations (as `pub` free fns). -/
def ratioCommonItems : List Json :=
  [ mkEnumItem "Currency" ["Usd", "Eur", "Gbp", "Jpy"] derives,
    mkEnumItem "RoundingMethod"
      ["RoundHalfUp", "RoundHalfDown", "RoundDown", "RoundUp", "Bankers"] derives,
    mkStructItemD "Money" [("minor", ty "i64"), ("currency", ty "Currency")] derives,

    -- pub fn money_zero(currency: Currency) -> Money { Money { minor: 0, currency: currency } }
    mkFnItemD "money_zero" [param "currency" "Currency"] (ty "Money")
      [tail (mkStructLit "Money"
        [("minor", mkLitInt "0"), ("currency", mkPath "currency")])],

    -- pub fn money_add(a: Money, b: Money) -> Money
    --   { Money { minor: a.minor + b.minor, currency: a.currency } }
    mkFnItemD "money_add" [param "a" "Money", param "b" "Money"] (ty "Money")
      [tail (mkStructLit "Money"
        [("minor", mkBin "+" (fieldOf "a" "minor") (fieldOf "b" "minor")),
         ("currency", fieldOf "a" "currency")])],

    -- pub fn money_neg(m: Money) -> Money
    --   { Money { minor: 0 - m.minor, currency: m.currency } }
    mkFnItemD "money_neg" [param "m" "Money"] (ty "Money")
      [tail (mkStructLit "Money"
        [("minor", mkBin "-" (mkLitInt "0") (fieldOf "m" "minor")),
         ("currency", fieldOf "m" "currency")])],

    -- pub fn money_is_zero(m: Money) -> bool { m.minor == 0 }
    mkFnItemD "money_is_zero" [param "m" "Money"] (ty "bool")
      [tail (mkBin "==" (fieldOf "m" "minor") (mkLitInt "0"))] ]

def main : IO Unit :=
  IO.println (Json.compress (mkFile ratioCommonItems))
