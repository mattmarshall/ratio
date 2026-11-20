---
name: proofs
description: Write and check Ratio's machine-checked properties — Lean 4 proofs (arithmetic, the lot engine, anything emitted to Rust) and TLA+ specs (staleness, concurrency, sequences of operations) — including the failure-path probes that must go red. Use before changing anything that decides a figure, and whenever you touch lean/ or tla/.
---

# Proofs and specs

**Proofs before code, for anything that decides a figure.** Lean or TLA+
first, then the Rust against it. The relief engine has been wrong in ways only
`decide` reported, and a HIFO bug passed a test asserting "HIFO differs from
LOFO" because both sides were broken in opposite directions.

Which tool:

- **Lean 4** (`lean/Ratio/`) — what one function computes: conservation,
  exact pro-rata relief, valuation, the bounds every emitted function assumes.
  Some Rust is emitted from it; see "The Lean → Rust seam".
- **TLA+** (`tla/`) — what a *sequence* or two writers do: a projection read
  at the wrong prefix, a strike checked on one day and landed on another, a
  mark computed from cost instead of the carrying value.

## Lean

1. `lean/Ratio/Foo/Bar.lean`, with `set_option warningAsError true` and a
   module doc saying what it proves and why it matters.
2. A `lean_test` in `lean/BUILD.bazel` listing the file **and its whole
   transitive import closure** in `srcs`, with `entry` set to the file.
3. `bazel test //lean:<name>_proof_test`. Type-checking *is* the test.

⛔ Never `sorry`, `admit`, `axiom` or `native_decide`. `//lean:audit_proofs_test`
fails on all four, and on any file without `warningAsError true`. It finds new
files by glob, so no BUILD edit is needed for the audit.

⚠ `decide` cannot reduce `List.mergeSort` (well-founded recursion). Use a
structural insertion sort.

⚠ Every theorem is over Lean's unbounded `Int`; every emitted function runs on
`i64`. A function that multiplies or negates needs a bounds check before the
emitted code is asked anything — `ratio_common::checked` is the Rust side of
`Ratio.Bounded`.

### The Lean → Rust seam

`lean/Ratio/*/Emit.lean` produces a `syn` AST as JSON; a genrule runs it
through `//rust/json_to_rust` into `generated.rs`, a direct `src` of the
consuming crate. Nothing is committed, so a Lean file that fails to elaborate
breaks the Rust build. The walk stays in Rust; the decision is emitted.

## TLA+

1. `Foo.tla`: `CONSTANTS` for the bounds and one boolean **dial** per fix,
   with actions branching `IF Dial THEN <correct> ELSE <the bug>`.
2. `MCFoo.tla` (`EXTENDS Foo`, nothing else) and `MCFoo.cfg` with every fix on.
3. For each fix, a **probe**: `BadThing.tla` (`EXTENDS Foo`) and `BadThing.cfg`
   with that dial flipped, a first-line header `\* <Invariant> MUST go red.`,
   and `INVARIANTS` narrowed to `TypeOK` plus that one invariant.
4. In `tla/BUILD.bazel`: a `tla_library`, a `tla_check` for `MCFoo`, and a
   `tla_check` per probe with `tags = ["manual"]`.

```bash
bazel test //tla:foo_check //tla:probes_test        # the model, and every probe's cfg
.agents/skills/proofs/scripts/tlc.sh tla/MCFoo.cfg  # state counts, outside Bazel
.agents/skills/proofs/scripts/tlc.sh tla/BadThing.cfg   # must print "<Invariant> is violated"
tla/probes.sh                                       # every probe, red for the reason it names
```

⛔ **Every `manual` probe must FAIL, and for the reason its cfg names.** A
probe that dies on a missing constant exits exactly like one whose invariant
broke. `//tla:probes_test` checks the configuration statically in CI;
`tla/probes.sh` runs TLC on every probe and checks *which* invariant went red.
Run it after touching any spec — adding a `CONSTANT` has already turned a
neighboring probe into a test of nothing.

⚠ `rules_tla` runs TLC inside a build action and shows its log only on
failure, so a passing `tla_check` says nothing about how much it explored.
`tlc.sh` prints the distinct-state count. A model that explored ten states
proves little; say the number in the PR.

## A worked example

`tla/StrikeDay.tla` models the day a strike's price check asks about against
the day the strike lands on. `MCStrikeDay.cfg`, with both fixes on, passes over
1,741 distinct states. Each fix has a probe that flips it back and goes red:
`StrikeAsOfElsewhere` and `StrikeCheckedElsewhere` for `--as-of` (#393), and
`BareStrikeIsUnchecked` for the bare strike, which first shipped as a probe
kept red on purpose so the gap could not be forgotten, then became the fix's
probe once the gap closed. Copy that shape.

`tla/SeedExtension.tla` is the concurrency case: two deploys, a demo user
writing at runtime, and a crash at any step, against an append-only store whose
markers cannot be overwritten. Its three probes each remove one ordering or
comparison the Rust performs, and each has a Rust test that a sabotage run
showed goes red too.
