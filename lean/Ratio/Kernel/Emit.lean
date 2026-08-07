import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `ratio-kernel`'s Rust conservation records as one `syn::File` JSON.

Per `research.tex`, the kernel is non-expressive: it models a transaction as an
integer vector over a conserved-dimension basis. A `Posting` is one component
(`dim`, `amount`); a `Transaction` is the vector (its postings). Balance =
postings net to zero — the conservation check, hand-written in `lib.rs` mirroring
the proven `Ratio.Core` (`Balanced` / `ledger_conserves`). Business rules
(accounts, normal-balance) are control-plane, not kernel — so they are NOT
emitted here. Integers only (i64), never floats. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def ty (n : String) : Json := mkTypeNamed n

/-- The items of the emitted `ratio_kernel` crate: the conservation records. -/
def ratioKernelItems : List Json :=
  [ -- pub struct Posting { pub dim: i64, pub amount: i64 }
    -- one component of a transaction's integer vector: a conserved dimension + amount.
    mkStructItemD "Posting" [("dim", ty "i64"), ("amount", ty "i64")]
      ["Clone", "Copy", "Debug", "PartialEq", "Eq"],

    -- pub struct Transaction { pub postings: Vec<Posting> }
    -- the integer vector itself; valid iff its postings net to zero (conservation).
    mkStructItemD "Transaction" [("postings", mkTypeApp "Vec" [ty "Posting"])]
      ["Clone", "Debug", "PartialEq", "Eq"] ]

def main : IO Unit :=
  IO.println (Json.compress (mkFile ratioKernelItems))
