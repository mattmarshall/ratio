import Ratio.Closure

set_option warningAsError true
/-! `Ratio.Closure.Factored` — a correction to the cost model, and the reason
the factor redesign is not an optimization.

⛔ `Ratio.Closure.actionCost` IS WRONG, AND WRONG IN THE DIRECTION THAT
FLATTERS. It says an open corporate action costs `openActions * securities` —
"an action reaches into every security it might touch". A split does not reach
across the chart. It reaches into ONE instrument and touches every LOT of it.
The cost is `openActions * lotsPer`, and for the S&P tracker that is forty
thousand rather than five hundred: EIGHTY TIMES the entire quiet period end, not
one times it.

⭐ AND THE CORRECTION BREAKS THE HEADLINE THEOREM.
`Ratio.Closure.nav_never_reads_the_lots` — hold everything fixed, vary
fragmentation, the cost does not move — is the whole scale argument, the thing
that makes twenty million tax lots free. Under a REWRITE it is false the moment
an action is open, because applying one is exactly a walk over the lots. The
theorem is true today only because the model understated the term.

So the factor representation is not a performance idea that happened to come up.
It is what makes the central claim TRUE. `rewriting_makes_the_nav_read_the_lots`
and `factoring_keeps_the_nav_off_the_lots` are the same dial in both positions.

⚠ AND IT DOES NOT MAKE A QUIET NAV FASTER. On a day with nothing open the two
models agree exactly (`with_nothing_open_the_models_agree`). Anyone reading this
as "the redesign speeds up the NAV" has it backwards: it removes a cliff, and a
cliff is not a slope.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Closure

/-- Applying open actions by REWRITING the affected lots.

⛔ `lotsPer`, not `securities`. A split touches every lot of one instrument. -/
def rewriteActionCost (d : Dials) : Nat := d.openActions * d.lotsPer

/-- Applying open actions by COMPOSING A FACTOR read at the point of use.

Nothing is written, so bringing the book up to date costs nothing at all. What
the factor does cost is paid by whoever READS a lot — and
`Ratio.Closure.nav_never_reads_the_lots` is the statement that a NAV does not.

⚠ It is not free in general. `Ratio.Actions.Factor.a_factor_can_succeed_where_
the_rewrite_refuses` means each lot read must still check that every step
divided, which is O(splits) on the lots actually touched. A sale relieving five
lots pays for five. A NAV pays for none. -/
def factorActionCost (_d : Dials) : Nat := 0

/-- The period end, costed with the rewrite. -/
def rewriteNavCost (d : Dials) : Nat :=
  markCost d + fxCost d + rewriteActionCost d + capitalCost d

/-- The period end, costed with the factor. -/
def factorNavCost (d : Dials) : Nat :=
  markCost d + fxCost d + factorActionCost d + capitalCost d

/-- **⛔ UNDER A REWRITE, A NAV READS THE LOTS.**

Two funds identical but for fragmentation — one lot a name against forty
thousand — with a single corporate action open. The costs differ by 39,999.

`Ratio.Closure.nav_never_reads_the_lots` says this cannot happen, and it is
proved, and it is proved of a model whose action term does not mention `lotsPer`
because I wrote the term wrong. This is the theorem that model was hiding. -/
theorem rewriting_makes_the_nav_read_the_lots :
    rewriteNavCost ⟨500, 3, 1, 1, 0⟩ ≠ rewriteNavCost ⟨500, 3, 40000, 1, 0⟩ := by
  simp [rewriteNavCost, markCost, fxCost, rewriteActionCost, capitalCost]

/-- **⭐ UNDER A FACTOR, IT DOES NOT — for any number of open actions.**

The general statement, not an example: hold the chart, the currencies, the
actions and the capital, vary fragmentation by any amount, and the cost is
unchanged. This is what `nav_never_reads_the_lots` was always meant to say, now
true of a term that models what applying an action actually does. -/
theorem factoring_keeps_the_nav_off_the_lots (sec ccy a b act cap : Nat) :
    factorNavCost ⟨sec, ccy, a, act, cap⟩ = factorNavCost ⟨sec, ccy, b, act, cap⟩ := by
  simp [factorNavCost, markCost, fxCost, factorActionCost, capitalCost]

/-- **⚠ AND ON A QUIET DAY THE TWO ARE THE SAME NUMBER.**

Nothing open, so nothing to apply and nothing to compose. The redesign buys
exactly zero here, and saying otherwise would be the easiest overclaim in this
file to make — the speed-up people will want to quote is on the day when
something IS open, and on that day it is the difference between 503 and 40,503.
-/
theorem with_nothing_open_the_models_agree (sec ccy l cap : Nat) :
    rewriteNavCost ⟨sec, ccy, l, 0, cap⟩ = factorNavCost ⟨sec, ccy, l, 0, cap⟩ := by
  simp [rewriteNavCost, factorNavCost, rewriteActionCost, factorActionCost]

/-- The S&P tracker with one split outstanding, both ways.

Eighty times the whole period end, against nothing. -/
theorem the_cliff :
    rewriteNavCost ⟨500, 3, 40000, 1, 0⟩ = 40503
    ∧ factorNavCost ⟨500, 3, 40000, 1, 0⟩ = 503 := by
  constructor <;>
    simp [rewriteNavCost, factorNavCost, markCost, fxCost,
          rewriteActionCost, factorActionCost, capitalCost]

/-- **And the original model understated it by eighty.**

`Ratio.Closure.actionCost` charges 500 for that open action; the rewrite it was
describing costs 40,000. Stated as a theorem rather than a comment because a
correction nobody can run is a correction nobody will believe. -/
theorem the_original_model_understated_the_cliff :
    actionCost ⟨500, 3, 40000, 1, 0⟩ * 80 = rewriteActionCost ⟨500, 3, 40000, 1, 0⟩ := by
  simp [actionCost, rewriteActionCost]

end Ratio.Closure
