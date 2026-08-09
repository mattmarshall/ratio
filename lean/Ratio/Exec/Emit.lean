import Lean.Data.Json
import Polyglot.Rust.RustAst
import Polyglot.Rust.Builders

set_option warningAsError true

/-! Emit `Ratio.Exec`'s decision as one `syn::File` JSON.

⛔ THE DECISION IS EMITTED AND THE FOLDS ARE NOT, which is the arrangement
`Ratio.Ingest.Emit` already uses: the running code walks partitions with a loop
and asks the emitted function what to do about them. `Ratio.Exec.a_plan_that_
cannot_be_helped_takes_its_io` is what makes that safe — the answer depends on
nothing but the two totals, so a Rust fold and a Lean theorem cannot come to
disagree about a partitioning.

What is decided here is the planner's entire rule, and it is one comparison:
splitting helps only while the slowest partition does more CPU than the plan
does IO. Below that the IO floor binds and another worker has nowhere to take
time from — which is `no_partition_beats_the_io_floor`, and the shape of every
disappointing scaling result there is.

Integers only (`i64`), never `f64`. -/

open Lean (Json)
open Polyglot.Rust.Builders

private def ty (n : String) : Json := mkTypeNamed n
private def param (n t : String) : Json := mkParam (mkPatIdent n) (ty t)
private def tail (e : Json) : Json := mkExprStmt e (withSemi := false)

/-- The items of the emitted executor core. -/
def ratioExecItems : List Json :=
  [ -- pub fn splitting_helps(io: i64, cpu: i64) -> bool { io < cpu }
    -- `Ratio.Exec.splittingHelps`. The planner's whole rule.
    mkFnItemD "splitting_helps" [param "io" "i64", param "cpu" "i64"] (ty "bool")
      [tail (mkBin "<" (mkPath "io") (mkPath "cpu"))],

    -- pub fn elapsed_of(io: i64, cpu: i64) -> i64 { if io < cpu { cpu } else { io } }
    -- `Ratio.Exec.elapsed` on the two totals a fold produced. ⛔ A MAX, NOT A
    -- SUM: IO and CPU overlap, and adding them would say a plan can trade one
    -- for the other, which `Ratio.Exec` exists to deny.
    mkFnItemD "elapsed_of" [param "io" "i64", param "cpu" "i64"] (ty "i64")
      [tail (mkIfThenElse (mkBin "<" (mkPath "io") (mkPath "cpu"))
              [tail (mkPath "cpu")]
              [tail (mkPath "io")])]
  ]

def main : IO Unit :=
  IO.println (mkFile ratioExecItems).compress
