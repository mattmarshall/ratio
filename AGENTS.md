# AGENTS.md — working on Ratio

Instructions for coding agents in this repository, whatever the harness. This
file and [`.agents/skills/`](.agents/skills/) are the only agent instructions:
there are no vendor-specific rule files. The old `.clinerules/`, `.clineignore`,
`CLINE.md` and `custom-instructions-for-ratio.md` described a different
program — a Rust/Python personal-finance TUI over PostgreSQL with `sqlx`,
`tui-rs`, PyO3 and a `specs/` directory to implement against. That
architecture is obsolete and those files are gone. Do not revive the old
Cargo/TUI program.

## What Ratio is

A double-entry accounting kernel with its core properties machine-checked. The
book is an append-only journal of conserved postings plus content-addressed
configuration. **Lean 4** proves the arithmetic and the lot engine, **TLA+**
checks staleness and concurrency, and **Rust** runs it — with some of the Rust
**emitted from the Lean**, so the theorem and the code are the same decision
rather than two descriptions of one.

One kernel serves investment funds, personal finance, project finance and
operating businesses. A book's kind selects its chart and its screens; it is
not a fork. The journal and configuration are files. Postgres
(`RATIO_PG_URL`) is an optional read projection, never the book of record. The
operations console in `console/` is Next.js on Vercel. First-party Connect
apps in `connect/` are Python. The TUI is a stub.

## Start here

1. **This file** — the rules that are not negotiable, and the failure shapes.
2. **The skill for your task** (below). Each one has commands that have been
   run, and scripts that check what they claim.
3. **[docs/current-state.md](docs/current-state.md)** — what is true now.
4. **[HANDOFF.md](HANDOFF.md)** — what is load-bearing and what will bite.
   Search it for your area rather than reading all 130 KB; dated amendments
   sit at the top.

Not working in this tree? The iOS shell is
[`mattmarshall/ratio-ios`](https://github.com/mattmarshall/ratio-ios), and
agents that act on a fund's *book* (not on this repository) are
[ORCHESTRATION.md](ORCHESTRATION.md). Do not confuse the two.

## Skills

In [`.agents/skills/`](.agents/skills/), in the Agent Skills format (a
`SKILL.md` with `name` and `description` frontmatter, plus `scripts/`).

| Skill | Use it to |
|---|---|
| [`build-and-test`](.agents/skills/build-and-test/SKILL.md) | build with Bazel, run only the tests your change affects (`scripts/affected-tests.sh`), match CI, read results, and find what `bazel test //...` does not cover |
| [`restricted-network-bazel`](.agents/skills/restricted-network-bazel/SKILL.md) | fix `403 Forbidden` on GitHub archive downloads in a sandbox (`scripts/override_github_archives.py`) — usually the first thing a cloud container needs |
| [`run-ratio`](.agents/skills/run-ratio/SKILL.md) | drive the CLI and the gRPC/REST server on a real book, smoke-test the binary (`scripts/smoke.sh`), seed the nine demo books |
| [`proofs`](.agents/skills/proofs/SKILL.md) | add or change a Lean proof or a TLA+ spec and its probes; count states with `scripts/tlc.sh` |
| [`console`](.agents/skills/console/SKILL.md) | work on the Next.js console, which Bazel does not build; run its contract checks (`scripts/contract-checks.sh`) |
| [`ship-a-change`](.agents/skills/ship-a-change/SKILL.md) | commit, write a PR or merge message that passes the issue-closure check, keep PLAN/HANDOFF honest, merge |
| [`backlog-dispatch`](.agents/skills/backlog-dispatch/SKILL.md) | pick, dispatch or relabel a backlog issue: the owner map, one issue → one agent → one PR, labels, concurrency |

Quick reference, if nothing else:

```bash
bazel test //...                                              # the whole Bazel gate
.agents/skills/build-and-test/scripts/affected-tests.sh       # just what your change touches
.agents/skills/run-ratio/scripts/smoke.sh                     # the binary, end to end
```

## Repository map

```
lean/Ratio/      the proofs, and the Emit modules that author Rust from them
tla/             the specs, their MC configs, and the failure-path probes
crates/          the Rust. `ratio` is the binary (CLI, `server`, `watch`, `mcp`);
                 `ratio-api` is the kernel API; `ratio-client` the Rust SDK
proto/           the wire contract, AIP-linted
sdk/             Python and TypeScript SDKs (TypeScript is not built by Bazel)
console/         the operations console — Next.js, Vercel, NOT built by Bazel
connect/         first-party Connect apps (Python), outside the kernel
demo/            the five-minute demo and the shadow run, as shell tests
deploy/          the Lambda image, its stacks, the demo seed scripts
site/ paper/ marketing/ competitive/    written material, with checks on the wording
docs/            current state, decisions, evidence; docs/history/ is history
specs/           ⚠ the ORIGINAL program's specs. History, not requirements
```

## Rules that are not negotiable

**Bazel only. Never `cargo`.** `cargo build` cannot succeed — the workspace
members are stale and the Lean emission step has no Cargo equivalent.

⚠ **`bazel test //...` is the whole suite for everything except `console/`**
(and `sdk/typescript/`, and `site/`). The console has no JavaScript toolchain in
the Bazel graph; `.github/workflows/console.yml` is a required check. ⛔ The one
thing Bazel still asserts about the console is `//proto:mirrors_test`, which
holds `console/src/wire/types.ts` to the proto. `//:plan_refusals_test` also
holds every `BookKind` in that file to `PLAN.md`.

**Proofs before code, for anything that decides a figure.** Lean or TLA+ first,
then Rust against it. `//tla:relief_engine_check` and `Ratio.Lots.Relief` were
written before the relief engine ran, and the engine has been wrong in ways
only `decide` reported.

**Never `sorry`, `admit`, `axiom`, or `native_decide`** — `//lean:audit_proofs_
test` fails the build on all four, and every Lean file must set
`warningAsError true`. Before that option was set, a file proving `(1 : Int) = 2`
by `sorry` passed.

**Every `tla_check` tagged `manual` must FAIL.** It is a probe: one dial
flipped, one invariant that must break. Run `tla/probes.sh` after touching a
spec — it checks each went red *for the reason its config names*, because a
probe that fails on a missing constant looks exactly like one doing its job.

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

⛔ **Never invent a `Method` / `Order` / `lot_method` variant** for MinTax,
SpecID, average cost, or wash. Each is an election with its own shape
(`min_tax_short_weight`, `identified_lots`, `average_cost`,
`wash_window_days` / `wash_keep_holding_period`). The TLA probes that treat
them as a Method exist so that mistake goes red. HANDOFF.md records why.

**Core stays a thin, correct book of record; breadth is a Connect app** unless
a PLAN amendment says otherwise — client/LP/vendor portals, bank OAuth, AIA pay
apps, tax packs, EAC/forecast
([#150](https://github.com/mattmarshall/ratio/issues/150)). The kernel cites
the figures those apps read. `rules:approve` and `config:promote` are not
scopes: the fence is absence.

**`BookKind` is one list, not a fork.** A Book is a journal plus
content-addressed configuration, independent of Funds and WorkOS organizations.
Kind selects the chart and the screens. `UNSPECIFIED` is the proto default, not
a hidden fifth kind. Do not mint a screen list per issue, and do not file a
household or a job under a Fund so old URLs keep working.

⛔ **The iOS shell never changes an amount.** It consumes a pinned kernel
revision. A change to the lot engine, the projection, the journal format or
anything emitted from Lean is a PR into *this* tree.

**Humans approve.** `ratio approve`, `ratio accept` and `ratio close` record a
person's decision. An agent proposes, and does not merge or approve its own
work without being asked.

## How things fail here, which is what to look for

**The books tie and the number is wrong.** Nearly every defect found in this
repository has that shape: conservation holds, the trial balance ties, the
digest reproduces, and one figure is somebody else's. A test that only checks
the books balance will not see any of it. HANDOFF.md has the table.

**The realized gain is the figure with no counterparty.** A wrong NAV is caught
by a reconciliation. A wrong gain is caught by nobody until a tax authority
asks. Most guards in the lot engine are about it.

**Green suites that test nothing.** Mutation testing found three. Break the
thing a test protects and confirm the test fails — writing it is not the same
as it working. Several tests here were added only after a sabotage run showed
the existing ones stayed green. A comment nothing tests will drift.

## Working style that fits this repository

- **Say what is wrong with a claim rather than working around it.** Several
  issues here contain premises that turned out to be false. Reporting that is
  worth more than delivering against them.
- **Comments carry the reason, not the mechanism.** `⛔` for a thing that will
  break, `⚠` for a thing that will surprise, `⭐` for the load-bearing claim.
  Most valuable are the ones recording what was tried and was wrong.
- **Test names are sentences describing a property** —
  `a_husk_gives_away_its_cost`, not `test_relief_2`.
- **Measure before claiming a performance property**, and report both curves
  when there are two. `ratio bench` is shaped to make the overclaim hard.
- **American English** in code, comments, docs and commit messages.

## Context budget — what not to read whole

| Path | Instead |
|---|---|
| `MODULE.bazel.lock` (1.2 MB), `Cargo.lock`, `console/pnpm-lock.yaml` | never read; regenerate (see `build-and-test`) |
| `PLAN.md` (310 KB), `HANDOFF.md` (130 KB) | search for the section, issue number or term you need |
| `crates/ratio-console/src/lib.rs` (680 KB), `crates/ratio-project/src/lib.rs`, `crates/ratio/src/{main,watch}.rs`, `crates/ratio-store/src/lib.rs`, `crates/ratio-rules/src/lib.rs`, `console/src/app/screens.test.tsx`, `proto/ratio/console/v1/console.proto` | search for the function, RPC or test name and read around it |
| `bazel-bin/`, `bazel-out/`, `bazel-testlogs/`, `bazel-ratio/`, `node_modules/` | build output; read a specific test log, never the tree |
| `images/`, `site/fonts/`, `docs/evidence/` | binary assets and recorded evidence |
| `specs/`, `docs/history/` | history of an earlier program and earlier sessions; not instructions |
| `generated.rs` | not in the tree: emitted from Lean at build time |

## Where to read the code from

| | |
|---|---|
| `lean/Ratio/Core.lean` | conservation over a vector of dimensions — everything rests on this |
| `lean/Ratio/Chart/Dimensions.lean` | what a chart of accounts IS: conserved vs partitioning vs measured |
| `crates/ratio-project/src/lib.rs` | the read model, and `AsOf<T>` — the type that makes its one catastrophic failure unrepresentable |
| `tla/Projection.tla` | a figure must pin the prefix it read |
| `crates/ratio/src/main.rs` | every CLI verb; `ratio --help` lists them |
| `DEVELOPING.md` | the human-oriented build guide, CI coverage table, and layout notes |
