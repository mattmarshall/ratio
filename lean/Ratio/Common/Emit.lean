import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

/-! Emit `ratio-common`'s Rust as one `syn::File` JSON on stdout, via the
`Polyglot.Rust` builders. Captured by `lean_emit` and rendered to formatted
Rust by `json_to_rust` (syn-serde + prettyplease).

Phase 0b smoke: just the `Money` struct, to prove the vendored Lean→Rust
pipeline end-to-end. Phase 1 grows this into the full proven kernel (Money
arithmetic, Currency/RoundingMethod, Account/Split/Transaction) with the
invariant `lean_test`s alongside. -/

open Lean (Json)
open Polyglot.Rust.Builders

/-- The items of the emitted `ratio_common` crate. -/
def ratioCommonItems : List Json :=
  let derives := ["Clone", "Copy", "Debug", "PartialEq", "Eq"]
  [ -- `pub enum Currency { Usd, Eur, Gbp, Jpy }`
    mkEnumItem "Currency" ["Usd", "Eur", "Gbp", "Jpy"] derives,
    -- `pub struct Money { pub minor: i64, pub currency: Currency }` — money as
    -- integer minor-units (never f64), tagged with its currency.
    mkStructItemD "Money"
      [("minor", mkTypeNamed "i64"), ("currency", mkTypeNamed "Currency")]
      derives ]

/-- Print the `syn::File` JSON for `json_to_rust` to render. -/
def main : IO Unit :=
  IO.println (Json.compress (mkFile ratioCommonItems))
