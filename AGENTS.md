# Working on Ratio with an LLM

Replaces `CLINE.md` and `custom-instructions-for-ratio.md`, both of which
described a different program: a Rust/Python personal-finance TUI over
PostgreSQL, with `sqlx`, `tui-rs`, and a `specs/` directory to implement
against. That architecture is obsolete. The journal remains the system of
record; optional Postgres projections and Python Connect apps now exist,
with different roles. Do not revive the old Cargo/TUI program.

Read [HANDOFF.md](HANDOFF.md) first, then its linked
[current-state summary](docs/current-state.md). The summary records what is
current; dated amendments preserve why it changed.

This file is rules for working on the repository with an LLM: the kernel
rules a coding agent must not break, and how **Grok Bot track agents** plus
**Cursor cloud agents** fan out backlog work without thrashing `main`. For
where *production* agents sit relative to the proofs — the fence, the
proposal plane, and what stays human — read [ORCHESTRATION.md](ORCHESTRATION.md).
Those agents touch a fund's *book*. This section is about agents that open
a *pull request*. Do not confuse the two.

## Dispatch — Grok Bot tracks and Cursor cloud agents

How the backlog fans out. This section is a contract, not a product
feature, and it does not close any product issue.

### Owner map

| Agent | Owns | Does not own |
|---|---|---|
| **Ratio** | milestone **Platform & Connect**, and the merge gate for every PR into this tree | Fund / Personal / Project milestone work |
| **Ratio Fund** | milestone **Fund accounting** only | Platform, Personal, Project |
| **Ratio Personal** | milestone **Personal finance** only | Platform, Fund, Project |
| **Ratio Project** | milestone **Project finance** only | Platform, Fund, Personal |

A track agent that picks an issue outside its milestone is the claim that
fails: the labels already named the owner, and crossing them puts two
authors on `PLAN.md` for no reason.

### Dispatch unit

**One GitHub issue → one cloud agent → one PR.**

⛔ Never assign a whole milestone to one agent. A milestone is a queue,
not a prompt. The failure mode is one agent touching every file the
other two need, then a merge that rewrites someone else's Built
honesty.

### Labels

Pick only issues labelled `status:ready`. That label means the
dependency is met and a cloud agent is safe to dispatch.

| Label | Use |
|---|---|
| `status:ready` | deps met — the only issues a track agent may start |
| `status:in-flight` | a cloud agent is assigned, or a PR is open |
| `status:blocked` | waiting on a dependency or a decision — do not dispatch |
| `track:*` | `fund` · `personal` · `project` · `platform` · `ecosystem` |
| `layer:core` · `layer:console` · `layer:connect` | where the work lives |
| `kind:*` | `investment` · `personal` · `project` · `operating` — BookKind, not a product fork |

Flip `status:ready` → `status:in-flight` when the cloud agent starts.
Leave `status:blocked` alone until the named dependency lands; then a
person (or the track agent that owns the dependency) marks `ready`.

### Concurrency

**At most two or three concurrent cloud agents on this repository.**
`PLAN.md`, `HANDOFF.md`, and `site/roadmap.src.html` are on almost every
engine PR, so a fourth agent is a conflict factory, not throughput.

Connect apps that live in a **separate tree or repo** can run wider —
they do not share those three files. Work that stays in *this* tree
still counts against the two-to-three cap, including a Connect-layer
issue that edits PLAN.

### PR rules

- **Batch PLAN / roadmap Built honesty into the engine PR.** The
  amendment that records what landed belongs in the same commit as the
  landing. A follow-up "docs only" PR is how the site stays ahead of
  the product — the risk PLAN.md already named.
- **State leftovers explicitly.** What a walk-through still cannot show
  stays named on the issue it belongs to.
- **Use `Related: #N` and `Remaining work: #N — <gap>` for partial work.**
  A negation before a closing keyword does not prevent GitHub auto-closure.
  Reserve closing keywords for a standalone completion line only after the
  entire issue meets acceptance. Follow the [merge check](docs/issue-completion.md)
  for the PR body, commit messages, and final merge message.
- ⛔ **Never invent a `Method` / `Order` / `lot_method` variant** for
  MinTax, SpecID, average cost, or wash. Each is an election with its
  own shape (`min_tax_short_weight`, `identified_lots`, `average_cost`,
  `wash_window_days` / `wash_keep_holding_period`). The TLA probes that
  treat them as a Method exist so that mistake goes red. HANDOFF.md
  records why.

### Core vs Connect

[#150](https://github.com/mattmarshall/ratio/issues/150) is the scope
catalog and the first-party extension contract. Ratio's core stays a
thin, correct book of record. Breadth is a Connect app unless a PLAN
amendment says otherwise.

**Connect unless PLAN says core:** client / LP / vendor portals, bank
OAuth, AIA G702/G703 pay-app packs, tax packs, EAC / forecast. The
kernel cites the figures those apps read; it does not grow the product
UI. `rules:approve` / `config:promote` are not scopes — the fence is
absence, and #150 does not open it.

### BookKind

A Book is a journal plus content-addressed configuration. It is
independent of Funds and WorkOS organizations: absence is independence,
not an error. `CreateBook` writes no fund and no organization.

Kind selects the chart `chart_for` writes and the chrome `screensFor`
offers — **one list, not a binary fork of the kernel.**
`UNSPECIFIED` is the proto default, not a domain and not a hidden fifth
kind. Do not mint a chrome list per issue, and do not file a household
or a job under a Fund so the old URLs keep working.

### Current queue

The [GitHub project](https://github.com/users/mattmarshall/projects/1) and
[roadmap index #258](https://github.com/mattmarshall/ratio/issues/258) own the
queue. Filter by the owning milestone and `status:ready`; phase and wave
labels express sequencing, not permission to start blocked work. Do not
copy a dated candidate list into an agent prompt.

The September 4 A/B waves are historical: partner cuts, unit movements,
write actors, and the multi-kind seed have landed. The Connect catalog's
remaining tasks are separate issues. Read-only app hosting depends on that
app's registration; write-capable apps additionally depend on the API
allowlist. The LP portal's grant is already proven. Check the current issue
before treating issue 22 or issue 150 as a blanket blocker.

---

## What this actually is

A fund accounting kernel. An append-only journal of conserved postings, with the
core properties machine-checked: Lean 4 for the arithmetic and the lot engine,
TLA+ for staleness and concurrency, Rust for the running system. Some of the
Rust is **emitted from the Lean**, so the theorem and the code are the same
decision rather than two descriptions of one.

The journal and content-addressed configuration are files. Postgres is an
optional read projection selected by `RATIO_PG_URL`; it is not the book of
record. First-party Connect apps in `connect/` use Python. The authenticated
operations console in `console/` is Next.js. The old TUI is only a stub.

## Rules that are not negotiable

**Bazel only. Never `cargo`.** `cargo build` cannot succeed — the workspace
members are stale and the Lean emission step has no Cargo equivalent.

⚠ **`bazel test //...` is the whole suite for everything except `console/`.** The
operations console is a Next.js application built by Vercel, so there is no
JavaScript toolchain in the Bazel graph; `.github/workflows/console.yml` is a
required check and runs `tsc --noEmit`, the render suite, `next build` and six
source-text checks. ⛔ The one thing Bazel still asserts about the console is
`//proto:mirrors_test`, which holds `console/src/wire/types.ts` to the proto.
`//:plan_refusals_test` also holds every `BookKind` in that file to `PLAN.md`.

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
| Dispatch (this file) | Grok Bot tracks + Cursor cloud agents: one issue → one agent → one PR |
| `lean/Ratio/Core.lean` | conservation over a vector of dimensions — everything rests on this |
| `lean/Ratio/Chart/Dimensions.lean` | what a chart of accounts IS: conserved vs partitioning vs measured |
| `crates/ratio-project/src/lib.rs` | the read model, and `AsOf<T>` — the type that makes its one catastrophic failure unrepresentable |
| `tla/Projection.tla` | a figure must pin the prefix it read |

⚠ `specs/` describes the original personal-finance program and is kept only for
history. Nothing in it is a requirement.
