import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

/-! Emit `ratio-chart`'s Rust as one `syn::File` JSON.

This crate carries the pieces of the chart-of-accounts layer whose correctness
is *proved* rather than tested, so they are authored here and emitted rather
than written by hand:

* `Side` / `AccountType` / `normal_side` — the normal-balance classification.
  `Ratio.Core` proves each of the five cases (`asset_debit_normal`, …). Business
  semantics live in the control plane, but *this* rule is a fixed, checked fact
  about the vocabulary, and every posting check depends on it.
* `pos_part` / `neg_part` — the debit and credit sides of a signed amount.
  `Ratio.Chart.part_split` proves `posPart a - negPart a = a`, which is what
  makes `Ratio.Chart.trial_balance_ties` hold: debits minus credits *is* the
  net, so a conserving book ties by construction.

The *folds* over a list of postings (`debits`, `credits`) are hand-written in
`lib.rs`, mirroring the proven `Ratio.Chart` definitions — the same division as
`ratio-kernel`, where the records are emitted and the aggregation is written
against them. Emitting a loop buys nothing here; the per-element arithmetic is
where a mistake would actually hide.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def derives : List String := ["Clone", "Copy", "Debug", "PartialEq", "Eq"]

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- `AccountType::<v>` as a path, for match scrutinee patterns and arm bodies. -/
private def acct (v : String) : Json := mkPathSegments ["AccountType", v]

/-- `Side::<v>`. -/
private def side (v : String) : Json := mkPathSegments ["Side", v]

/-- The items of the emitted `ratio_chart` crate. -/
def ratioChartItems : List Json :=
  [ mkEnumItem "Side" ["Debit", "Credit"] derives,

    mkEnumItem "AccountType"
      ["Asset", "Liability", "Equity", "Income", "Expense"] derives,

    -- pub fn normal_side(t: AccountType) -> Side { match t { … } }
    -- Mirrors `Ratio.Core.normalSide`; each arm is a proved theorem there.
    mkFnItemD "normal_side" [param "t" "AccountType"] (ty "Side")
      [tail (mkMatch (mkPath "t")
        [ mkMatchArm (acct "Asset")     (side "Debit"),
          mkMatchArm (acct "Expense")   (side "Debit"),
          mkMatchArm (acct "Liability") (side "Credit"),
          mkMatchArm (acct "Equity")    (side "Credit"),
          mkMatchArm (acct "Income")    (side "Credit") ])],

    -- pub fn pos_part(a: i64) -> i64 { if 0 < a { a } else { 0 } }
    -- `Ratio.Chart.posPart`.
    mkFnItemD "pos_part" [param "a" "i64"] (ty "i64")
      [tail (mkIfThenElse (mkBin "<" (mkLitInt "0") (mkPath "a"))
        [tail (mkPath "a")]
        [tail (mkLitInt "0")])],

    -- pub fn neg_part(a: i64) -> i64 { if a < 0 { -a } else { 0 } }
    -- `Ratio.Chart.negPart`; returned positive, as a credit total is.
    mkFnItemD "neg_part" [param "a" "i64"] (ty "i64")
      [tail (mkIfThenElse (mkBin "<" (mkPath "a") (mkLitInt "0"))
        [tail (mkUnary "-" (mkPath "a"))]
        [tail (mkLitInt "0")])],

    -- pub struct TrialBalance { pub debits: i64, pub credits: i64 }
    mkStructItemD "TrialBalance" [("debits", ty "i64"), ("credits", ty "i64")]
      ["Clone", "Copy", "Debug", "PartialEq", "Eq"],

    -- pub fn trial_balance_ties(tb: TrialBalance) -> bool { tb.debits == tb.credits }
    -- The accountant's check. `Ratio.Chart.trial_balance_ties` proves this is
    -- implied by conservation, so it can never fail on a book the kernel built.
    mkFnItemD "trial_balance_ties" [param "tb" "TrialBalance"] (ty "bool")
      [tail (mkBin "==" (mkField (mkPath "tb") "debits") (mkField (mkPath "tb") "credits"))] ]

def main : IO Unit :=
  IO.println (Json.compress (mkFile ratioChartItems))
