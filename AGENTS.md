# Working on Ratio with an LLM

Replaces `CLINE.md` and `custom-instructions-for-ratio.md`, both of which
described a different program: a Rust/Python personal-finance TUI over
PostgreSQL, with `sqlx`, `tui-rs`, and a `specs/` directory to implement
against. None of that is true, and an agent bootstrapped on it will confidently
build the wrong thing — and reach for a database, a Cargo build and a Python
extension system that are not there.

Read [HANDOFF.md](HANDOFF.md) first. It is the accurate document.

## What this actually is

A fund accounting kernel. An append-only journal of conserved postings, with the
core properties machine-checked: Lean 4 for the arithmetic and the lot engine,
TLA+ for staleness and concurrency, Rust for the running system. Some of the
Rust is **emitted from the Lean**, so the theorem and the code are the same
decision rather than two descriptions of one.

There is no database. There is no Python. There is no TUI beyond a stub.

## Rules that are not negotiable

**Bazel only. Never `cargo`.** `cargo build` cannot succeed — the workspace
members are stale and the Lean emission step has no Cargo equivalent.

⚠ **`bazel test //...` is the whole suite for everything except `console/`.** The
operations console is a Next.js application built by Vercel, so there is no
JavaScript toolchain in the Bazel graph; `.github/workflows/console.yml` is a
required check and runs `tsc --noEmit`, the render suite and `next build`. Bazel
still runs five source-text checks over it (`//console:*`) that need no node.

**Proofs before code, for anything that decides a figure.** The order in this
repository is Lean or TLA+ first, then Rust against it. `//tla:relief_engine_
check` and `Ratio.Lots.Relief` were written before the relief engine ran, and
the engine has been wrong in ways only `decide` reported.

**Never `sorry`, `admit`, `axiom`, or `native_decide`** — `//lean:audit_proofs_
test` fails the build on all four, and every Lean file must set
`warningAsError true`. Before that option was set, a file proving `(1 : Int) = 2`
by `sorry` passed.

**Every `tla_check` tagged `manual` must FAIL.** It is a probe: one dial flipped,
one invariant that must break. Run `tla/probes.sh` after touching a spec — it
checks each went red *for the reason its config names*, because a probe that
fails on a missing constant looks exactly like one doing its job.

**Money is never a float, and a figure that will not divide is refused rather
than rounded.** `Ratio.Lots.partial_relief_is_exactly_pro_rata` — the remainder
would be a misstatement of taxable income, not a rounding error. The one
exception is FX translation, which has no exact answer at any finite precision
and says so where it happens.

**Any emitted function that multiplies or negates needs a bounds check before
the emitted code is asked anything.** Every theorem here is over Lean's `Int`,
which is unbounded; every emitted function runs on `i64`, which is not.
`ratio_common::checked` is the Rust side of `Ratio.Bounded`. Asking a wrapped
number a question gets an answer about a product that never happened.

## How things fail here, which is what to look for

**The books tie and the number is wrong.** Nearly every defect found in this
repository has that shape: conservation holds, the trial balance ties, the digest
reproduces, and one figure is somebody else's. A test that only checks the books
balance will not see any of it. HANDOFF.md has the table.

**The realized gain is the figure with no counterparty.** A wrong NAV is caught
by a reconciliation. A wrong gain is caught by nobody until a tax authority asks.
Most guards in the lot engine are about it.

**Green suites that test nothing.** Mutation testing found three. Break the thing
a test protects and confirm the test fails — writing it is not the same as it
working. Several tests in this repository were added only after a sabotage run
showed the existing ones stayed green.

## Working style that fits this repository

- **Say what is wrong with a claim rather than working around it.** Several
  issues here contain premises that turned out to be false. Reporting that is
  worth more than delivering against them.
- **Comments carry the reason, not the mechanism.** `⛔` for a thing that will
  break, `⚠` for a thing that will surprise, `⭐` for the load-bearing claim.
  Most valuable are the ones recording what was tried and was wrong.
- **Test names are sentences describing a property** —
  `a_husk_gives_away_its_cost`, not `test_relief_2`.
- **Measure before claiming a performance property**, and report both curves when
  there are two. `ratio bench` is shaped to make the overclaim hard.

## Where to start reading

| | |
|---|---|
| `HANDOFF.md` | what is load-bearing and what will bite |
| `lean/Ratio/Core.lean` | conservation over a vector of dimensions — everything rests on this |
| `lean/Ratio/Chart/Dimensions.lean` | what a chart of accounts IS: conserved vs partitioning vs measured |
| `crates/ratio-project/src/lib.rs` | the read model, and `AsOf<T>` — the type that makes its one catastrophic failure unrepresentable |
| `tla/Projection.tla` | a figure must pin the prefix it read |

⚠ `specs/` describes the original personal-finance program and is kept only for
history. Nothing in it is a requirement.
