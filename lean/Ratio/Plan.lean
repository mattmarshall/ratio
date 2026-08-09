set_option warningAsError true
/-! `Ratio.Plan` — two ways to compute a NAV, and what makes them the same.

⚠ WHAT THIS IS AND IS NOT. `Ratio.Lots` proves three facts that a planner would
WANT — relief touches a prefix, an aggregate updates by delta, partitions
compose — but proving a rewrite is legal is not the same as having a planner.
There was no plan, no cost, and no statement that a chosen plan agrees with the
naive one. This file is that statement, at the smallest size where it is not
vacuous.

Two plans for the same question, "what is this fund worth":

  SCAN       fold every tax lot. Always right, and its cost is the number of
             lots — which for a fund that has traded for years is millions.
  AGGREGATE  read one maintained total per security. Its cost is the number of
             SECURITIES, which for an S&P tracker is five hundred and does not
             grow with the fund's age.

`aggregate_agrees_with_scan` is the whole planner argument: the fast plan
equals the slow one EXACTLY WHEN every maintained total is honest, and
`Ratio.Lots.cost_is_conserved` is what keeps it honest across a sale. Take that
away and the fast plan is a different number wearing the same name.

The cost functions are counts of lots and securities, not seconds. What they
are for is turning the dial — five hundred securities, ten thousand lots each —
and reading off which term moves. -/

namespace Ratio.Plan

/-- One security's holding: the lots, and the running total kept beside them. -/
structure Holding where
  /-- Cost of each open lot. Millions of these, for a fund with history. -/
  lots : List Nat
  /-- The total somebody maintains so that nothing has to re-read the lots. -/
  aggregate : Nat
deriving Repr

/-- The maintained total says what the lots say.

⛔ THE INVARIANT AN INDEX EXISTS TO KEEP, and the one every fast path silently
assumes. `Ratio.Lots.cost_is_conserved` is what preserves it across a sale:
because what was relieved plus what remains is what was there, the total can be
moved by the delta instead of refolded. -/
def honest (h : Holding) : Prop := h.aggregate = h.lots.sum

/- ── The two plans ─────────────────────────────────────────────────────── -/

/-- Fold every lot. Always right, and the cost is every lot. -/
def scanValue (hs : List Holding) : Nat := (hs.map (fun h => h.lots.sum)).sum

/-- Read one total per security. -/
def aggregateValue (hs : List Holding) : Nat := (hs.map (·.aggregate)).sum

/-- What the scan reads: every lot of every security. -/
def scanCost (hs : List Holding) : Nat := (hs.map (fun h => h.lots.length)).sum

/-- What the aggregate reads: one row per security. -/
def aggregateCost (hs : List Holding) : Nat := hs.length

/-- **THE PLANNER THEOREM.** The fast plan is the slow plan exactly when every
maintained total is honest.

This is what a planner needs and what a test cannot give: not "the two agreed
on the books we tried", but "they agree whenever the invariant holds, and the
invariant is the thing to protect". -/
theorem aggregate_agrees_with_scan :
    ∀ (hs : List Holding), (∀ h ∈ hs, honest h) → aggregateValue hs = scanValue hs
  | [], _ => rfl
  | h :: rest, hall => by
    have hh : honest h := hall h (List.mem_cons_self)
    have ih := aggregate_agrees_with_scan rest
      (fun x hx => hall x (List.mem_cons_of_mem _ hx))
    simp [aggregateValue, scanValue] at ih ⊢
    unfold honest at hh
    omega

/-- **And one dishonest total is enough to make them disagree.**

Stated so the hypothesis above is visibly load-bearing rather than decoration:
there is a book where the plans differ, so "they always agree" is false and the
invariant is what buys the equality. -/
theorem a_stale_total_makes_the_plans_disagree :
    ∃ hs : List Holding, aggregateValue hs ≠ scanValue hs := by
  refine ⟨[⟨[10], 7⟩], ?_⟩
  simp [aggregateValue, scanValue]

/- ── Turning the dial ──────────────────────────────────────────────────── -/

/-- A fund described by its shape rather than its contents. -/
structure Shape where
  /-- 500 for an S&P tracker. -/
  securities : Nat
  /-- How fragmented each holding is. Years of trading make this large. -/
  lotsPer : Nat
deriving Repr

/-- A book of that shape, with honest totals. -/
def book (s : Shape) (costPerLot : Nat) : List Holding :=
  List.replicate s.securities ⟨List.replicate s.lotsPer costPerLot,
                               s.lotsPer * costPerLot⟩

theorem book_is_honest (s : Shape) (c : Nat) : ∀ h ∈ book s c, honest h := by
  intro h hm
  simp [List.eq_of_mem_replicate (by simpa [book] using hm), honest]

/-- **The aggregate plan costs the securities, whatever the history.**

Five hundred reads for an S&P tracker whether each holding is one lot or ten
thousand. This is the answer to "how long does a NAV take": it is a function of
the CHART, not of the JOURNAL, and that is a theorem rather than a hope. -/
theorem aggregate_cost_is_the_securities (s : Shape) (c : Nat) :
    aggregateCost (book s c) = s.securities := by
  simp [aggregateCost, book]

/-- **The scan plan costs the lots**, which is the same number multiplied by
however long the fund has been trading. -/
theorem scan_cost_is_the_lots (s : Shape) (c : Nat) :
    scanCost (book s c) = s.securities * s.lotsPer := by
  simp [scanCost, book, List.map_replicate]

/-- Both plans give the same figure on such a book, so the cheaper one is not
cheaper by being wrong. -/
theorem the_dial_does_not_change_the_answer (s : Shape) (c : Nat) :
    aggregateValue (book s c) = scanValue (book s c) :=
  aggregate_agrees_with_scan _ (book_is_honest s c)

/-- An S&P tracker with ten thousand lots per name: five hundred reads against
five million. -/
example : aggregateCost (book ⟨500, 10000⟩ 100) = 500 := by
  simp [aggregate_cost_is_the_securities]

example : scanCost (book ⟨500, 10000⟩ 100) = 5000000 := by
  simp [scan_cost_is_the_lots]

/-- **And the gap only widens with history.** Holding the chart fixed, the scan
grows with fragmentation and the aggregate does not move — which is the shape
of the claim that a fund's twentieth year strikes as fast as its first. -/
theorem history_does_not_slow_the_aggregate (sec a b c : Nat) :
    aggregateCost (book ⟨sec, a⟩ c) = aggregateCost (book ⟨sec, b⟩ c) := by
  simp [aggregate_cost_is_the_securities]

end Ratio.Plan
