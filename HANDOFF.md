# Handoff — tax lots, corporate actions, and the dimensional chart

**State**: `main` at `f780f34`. 50 bazel tests green, 18 `lean_test`, 19
`tla_check`. Nothing uncommitted, no branches outstanding.

Open work is in GitHub issues #4–#9. This file is the part that does not fit in
an issue: what was learned, what is load-bearing, and what will bite.

---

## Read this first: how things fail here

Every defect found in this stretch had one shape. **The books tie and the number
is wrong.** Conservation, the trial balance, and the digest are all satisfied by
the wrong answer, so nothing downstream reports anything.

| what happened | what noticed |
|---|---|
| An unbalanced transaction summed to zero by `i64` wraparound and passed the door | a test written for something else |
| `per_share(x, 0)` panicked on the NAV path — a fund with no units in issue | a sweep of the emitted surface |
| A husk (zero units, carrying cost) handed its whole basis to a sale that got no units | a Lean `example` |
| HIFO silently performed FIFO — the tiebreak overrode the method | Lean's `decide` reporting a theorem FALSE |
| A projection's lots and its positions drifted, each internally consistent | a reconciliation test between the two paths |
| `[USD +100, EUR −100]` passed as balanced | `Ratio.Chart.Dimensions` |

⛔ **The realized gain is the figure with no counterparty.** A wrong NAV is caught
by a reconciliation. A wrong gain is caught by nobody until a tax authority asks.
Most guards in the lot engine are about it rather than about conservation.

⚠ **Mutation testing found three green suites that tested nothing** — the ex-date
boundary, the per-instrument filter, and the lot's trade-date propagation. All
passed; all covered the code and none covered the case. Break the thing you
think a test protects, and check the test fails.

---

## The gap that is not in any proof

Every theorem here is over Lean's `Int`, which is unbounded. Every emitted
function runs on `i64`, which is not. `Ratio.Bounded` is where that hypothesis is
finally written down; before it, the failure was not a crash:

```
4_000_000_000_000_000_000 * 3        wraps to  -6446744073709551616
(-6446744073709551616).rem_euclid 2  ==        0
```

A divisibility guard says YES about a product that never happened, and the proof
cannot see it because in `Int` the multiplication simply happened.

⛔ **Any new emitted function that multiplies or negates needs a bounds check
before the emitted code is asked anything** — asking first is asking about a
wrapped number. `ratio_common::checked` is the Rust side.

⚠ Still unguarded, low consequence and named rather than hidden: `money_add` /
`money_neg` in the emitted kernel (barely reached — `Ratio.Core` conserves over
`Vec Dim`, not `Money`), and `neg_part` in reporting.

---

## The architecture decision worth not re-litigating

`Ratio.Core.Conserves` is `v = Vec.zero` — zero in **every** dimension — and
`Dim` is the conserved-quantity basis. The running kernel had collapsed it to one
sum. The correction is a classification, and it is the whole chart design:

| | |
|---|---|
| **conserved** | **currency**. Two currencies are two independent laws, not one law over a sum. |
| **partitioning** | account, instrument, share class, counterparty. They say *where* value sits, do **not** net to zero, and roll up. |
| **measured** | quantity. Buying 100 shares *creates* 100 in the book, so a conservation check over units would refuse every purchase. |

A chart of accounts is the partitioning dimension people argue about. It is not
the conserved one, and the kernel never said it was.

---

## Load-bearing things a change could quietly break

- **`AsOf<T>`** (`ratio-project`). Every read carries the journal prefix it was
  folded from, and there is no other way to get a number out — so a caller
  cannot pin the head while reading a lagging projection. `//tla:projection_
  check`. ⚠ Weaker than it looks: nothing stops taking `.value` and pairing it
  with a position from elsewhere. What the type buys is that doing so requires
  saying so.
- **`advance` uses `max`, not assignment.** A shorter journal must not rewind the
  prefix, or the next advance re-folds and double-counts.
- **`follow` refuses a journal shorter than it read.** An append-only log does
  not shrink, so that is a *different book*; resuming would splice two histories.
- **Announcements are journal entries.** Under the factor representation nothing
  is applied, so an action in a side plane is pinned by no strike and a replay
  answers differently as the world tells us more. `Ratio.Actions.Factor.replay_
  is_determined_by_the_prefix`.
- **Corporate actions are read through per STEP, never as a composed ratio.**
  3-for-2 then 2-for-1 composes to ⟨6,2⟩, divides five units cleanly, and
  silently swallows the half-share the holder was paid cash for.
- **`relieve` sorts by `seq` itself.** `Ratio.Lots.relieveFifo` takes the head, so
  it is FIFO exactly when the caller supplied acquisition order — and a
  projection keyed by instrument does not.
- **The gain leg is derived, never supplied.** A parameter for it would be drift
  written down as an API, and the drift is silent because the gain leg absorbs
  whatever the other two legs leave.
- **`ChartRoles` are checked when the configuration is READ.** A chart that
  cannot express a gain is wrong the moment it is written down; finding out at
  the first disposal means finding out in production.

---

## Traps that cost real time

- ⛔ **`bazel cquery --output=files` can hand back a stale binary.** Twice: an API
  returned an empty list and the code was right. Explicit `bazel build` first,
  then query.
- ⛔ **A console change needs `//crates/ratio` rebuilt, not `//web:...`.** The
  chain is `//web:console_html` → genrule `//crates/ratio:console_rs` →
  `src/console_html.rs` → the binary, which embeds it as a `&str` at compile
  time. Building anything under `//web:` alone refreshes nothing that is served.
  ⚠ This entry used to say "`//web:console` and `//web:console_html` are
  different targets" — there is no `//web:console` target and there never was.
  `//crates/ratio:ratio_test` now greps the served HTML, so the trap fails
  loudly instead of being remembered.
- ⛔ **`append` and `append_all` are two doors with the same law**, with different
  indentation. A fix applied by string replace hits one.
- ⛔ **Python edits collapse Rust `\` line continuations** into runs of spaces
  inside string literals. Three occurrences so far; sweep with
  `grep -rnE '"[^"]*[a-z,] {8,}[a-z]'` and expect column headers as false hits.
- ⚠ **`decide` cannot reduce `List.mergeSort`** (well-founded recursion). Use a
  structural insertion sort; `native_decide` is forbidden by
  `//lean:audit_proofs_test` and would trade a checked theorem for a trusted
  compiler.
- ⚠ **zsh does not word-split `$var`.** `R="bazel run -- "; $R foo` fails.

---

## What the demo shows, and what it does not

```
lots/sec   open lots      entries      MB   COLD BUILD   NAV STRIKE
      20        9684        67792       0     323.9 ms       414 µs
     100       50213       351495       1        1.8 s       435 µs
     500      252843      1769905       9       11.4 s       403 µs
    2000     1022625      7158379      39       91.7 s       395 µs
```

`ratio bench` generates a fund and measures a period end. 100× the lots, NAV
strike unchanged.

⛔ **It reports two curves and both must be quoted.** Folding the journal is
O(entries) and grows — an append-only log does not forget a closed lot. Only the
strike off a maintained projection is flat.

⚠ The generator sold at cost until the gain posting landed, so every disposal
realized nothing and the gain account never moved — six lot methods and the whole
relief layer running against a book where every gain was zero. If you add a
scenario, check it exercises what you think it does.

---

## Where things are

| | |
|---|---|
| `lean/Ratio/` | the proofs. `Bounded`, `Chart/Dimensions`, `Lots/{Relief,Methods,Edges,Posting}`, `Actions/Factor`, `Closure`, `Exec` |
| `tla/` | `Projection`, `Executor`, `ReliefEngine`, `LotEngine`, `Actions`, `Valuation`, `ControlPlane`. Each has `manual`-tagged probes that must go RED |
| `crates/ratio-project` | the read model, the lot book, the relief engine |
| `crates/ratio-gen` + `ratio bench` | the generated fund and the measurement |
| `tomato-bazel/rules_postgres` | `Pg.Rel.Semantics` — merged, PR #9 |

⚠ Every `tla_check` tagged `manual` is a probe that must FAIL. Run them after
changing a spec; a probe that goes green means the invariant stopped checking.
