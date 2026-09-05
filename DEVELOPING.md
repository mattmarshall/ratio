# Developing Ratio

## Build

⛔ **Bazel is the only build path. `cargo build` does not work here.** The
workspace members in the root `Cargo.toml` are stale, several crate manifests
omit dependencies that `BUILD.bazel` requires, and the Lean → Rust emission step
has no Cargo equivalent. `Cargo.lock` is committed because `rules_rust`'s crate
universe reads it, not because Cargo drives anything.

```bash
bazel build //...                   # everything, including the Lean emission
bazel test  //...                   # proofs, TLA+ checks, crates, demos, lints
bazel run //crates/ratio -- --help  # the CLI
```

The lockfiles are committed and CI runs `--lockfile_mode=error`, so a build that
needs `CARGO_BAZEL_REPIN=1` is a change somebody forgot to commit. Repin with:

```bash
CARGO_BAZEL_REPIN=1 bazel build //...
```

### Prerequisites

Bazelisk (or Bazel at the version in `.bazelversion`), a JDK for TLC, and
Python 3 for the lint and mirror tests. The Lean toolchain, the Rust toolchain
and every crate are fetched hermetically — there is nothing else to install, and
nothing to run as a service for `bazel test //...`.

The live projection engine (`//crates/ratio-sql-project:pg_engine_test`) is
tagged `manual` so it is not in that set. It needs `psql` and a Postgres
16 server; see [Live projection engine](#live-projection-engine) below.

## What runs in CI

`bazel build //...` then `bazel test //...`, plus `console.yml` for a change
under `console/`, plus the live projection engine against a Postgres 16
service container. ⚠ Bazel alone WAS the whole gate and no longer is — it has no
JavaScript toolchain since the console left the binary. Bazel covers:

| | |
|---|---|
| `//lean:*_proof_test` | the proofs — type-checking IS the test |
| `//lean:audit_proofs_test` | no `sorry`, `admit`, `axiom` or `native_decide`; every file sets `warningAsError` |
| `//tla:*_check` | seven model checks |
| `//tla:probes_test` | every failure-path probe can still say what it claims |
| `//crates/*:*_test` | the Rust (except `pg_engine_test`, below) |
| `//crates/ratio-sql-project:pg_engine_test` | live apply of `schema.sql` — tagged `manual`, run by name in `build.yml` |
| `//crates/ratio-sql-project:fold_scale_test` | measured 20M-lot fold of the HANDOFF geometry (10,000 × 2,000); in `//...` |
| `//proto:ratio_aip_lint`, `//proto:mirrors_test` | the wire contract, and its two hand-written mirrors |
| `//crates/ratio-console:transcode_test` | the route table against the proto |
| `//demo:rehearse_test`, `//demo:shadow_run_test` | the demo and the shadow run, end to end |
| `//:connect_scopes_test` | PLAN, HANDOFF and `docs/connect-scopes.md` still name the same Connect grants and leftovers — the in-process authorizer accepts catalog scopes; API Gateway JWT verifies Connect tokens; first-party Connect apps call ConnectApiUrl; WorkOS dashboard registration stays leftover |
| `//connect/bank-feed:mapper_test` | Personal bank-feed mapper (#165): canonical scopes, empty-allowlist / closed-through / conservation refusals — not that a token is accepted |
| `//connect/tax-pack:pack_test` | Personal tax-pack builder (#166): canonical scopes, 8949-ish CSV, mixed-date category refusal — not that a token is accepted and not an IRS e-file |
| `//connect/goals:goals_test` | Personal net-worth goals / scenarios (#168): canonical scopes, opt-in / empty-allowlist / closed-through refusals — not that a token is accepted and not a cash forecast |
| `//connect/aia-pay-app:pack_test` | Project AIA pay-app packer (#184): canonical scopes, G702-ish / G703-ish CSV, unset billed / retainage / CO refusal — not that a token is accepted and not a licensed AIA form |
| `//connect/vendor-portal:portal_test` | Project vendor / GC portal (#172): canonical scopes, unset billed / retainage / remaining, empty-allowlist / closed-through refusals — not that a token is accepted and not a vendor directory |
| `//connect/eac-forecast:pack_test` | Project EAC / forecast packer (#169): canonical scopes, remaining-to-spend / EAC unset refusals — not that a token is accepted and not a silent EAC of 0 |
| `//connect/program-rollup:rollup_test` | Project program roll-up (#179): canonical scopes, alias / mega-book / unset billed refusals — not that a token is accepted and not a silent program 0.00 |
| `//connect/grant:grant_test` | Shared Connect grant helper: ConnectApiUrl only, never DemoUrl, never `RATIO_DEMO_OPEN`, never `org:{id}` |
| `//connect/audit-export:pack_test` | Audit evidence ZIP (#185): canonical scopes, unset / cited-empty / empty-digest refusals — fetch/deliver call ConnectApiUrl; WorkOS dashboard registration stays leftover |
| `//connect/fund-ops-alerts:alerts_test` | Fund ops alerts (#162): canonical scopes, unset / cited-empty / unpriced-without-as-of refusals — fetch/deliver call ConnectApiUrl; Slack / email / PagerDuty and WorkOS dashboard registration stay leftover |
| `//marketing:language_test` | the licensing language sweep |

`paths-ignore` covers `site/**` and `**/*.md` only. `marketing/`, `paper/` and
`competitive/` are deliberately NOT ignored.

`.github/workflows/console.yml` runs `tsc --noEmit`, the render suite,
`next build` and the six checks in `console/scripts/` on any change under
`console/` — and on a change to `console.proto`, because the wire types mirror
it. `site.yml` re-runs `tokens_test.py`, because `site/**` is ignored above and
a token changed there has to go red where it was changed.

⚠ **Bazel does not run those six.** They were `sh_test`s under `//console:` and
failed twice on Bazel wiring rather than on anything they check — a
package-relative path that does not survive the runfiles root, then a label its
own `glob` already matched. They are green in `console.yml`, which is the one
place they can be run by whoever is editing them.

### ⛔ What CI does not run

**The `manual`-tagged TLA probes.** Each is a spec with one dial flipped and one
invariant that MUST break, and `tags = ["manual"]` keeps them out of `//...` so
the suite stays green. Run them yourself after touching a spec:

```bash
tla/probes.sh
```

It checks each went red **for the reason its config names**. That distinction is
the point: a probe that dies on a missing constant exits exactly like one whose
invariant was violated, and adding a `CONSTANT` to a spec has already turned a
neighbouring probe into a test of nothing.

**The live projection engine is also tagged `manual`, for the opposite
reason.** It must PASS, and it needs a server, so it is not in `//...`.
`build.yml` runs `//crates/ratio-sql-project:pg_engine_test` by name.

## Live projection engine

`crates/ratio-sql-project/schema.sql` is the Postgres contract. The
denotational store (`SqlProjection`) always runs in `//...`. Applying the
same file to a live engine is `PgProjection` (via `psql`). Relief stays
`relieve_by`. The journal stays the system of record.

```bash
# Docker
docker run -d --name ratio-pg \
  -e POSTGRES_USER=ratio -e POSTGRES_PASSWORD=ratio -e POSTGRES_DB=ratio \
  -p 5432:5432 postgres:16
export RATIO_PG_URL=postgres://ratio:ratio@127.0.0.1:5432/ratio
bazel test --test_env=RATIO_PG_URL --test_output=errors \
  //crates/ratio-sql-project:pg_engine_test
```

A local cluster works the same way: create a role and database, point
`RATIO_PG_URL` at them, keep `psql` on `PATH`. `RATIO_PG_URL` unset is a
refuse, not a skip.

`RATIO_PG_URL` (optional `RATIO_PG_SCHEMA`) is also the dial for
console / API reads of lots, positions, and Current aggregates.
Empty or missing keeps the in-memory fold. The journal stays the
system of record. A missing watermark refuses rather than
answering with empty lots.

Planner pushdown vs `Pg.Rel.Semantics` is
`//lean:sql_pushdown_proof_test` and `src/plan.rs`. The measured
20M-lot fold is `//crates/ratio-sql-project:fold_scale_test`
(HANDOFF 10,000 × 2,000). The 140M-entry / 40GB journal fold
stays on Fargate ScaleTask.

## Layout

```
lean/Ratio/       the proofs, and the Emit modules that author Rust from them
tla/              the specs, their MC configs, and the failure-path probes
crates/           the Rust. `ratio` is the binary; the rest are libraries
proto/            the wire types, AIP-linted
console/          the operations console (Next.js, deployed to Vercel — NOT built by Bazel)
connect/          first-party WorkOS Connect apps (sibling trees; not kernel RPCs)
demo/             the five-minute demo and the shadow run, as shell tests
deploy/           the Lambda that serves the API and the three public screens
site/ paper/ marketing/ competitive/   the written material
specs/            ⚠ the ORIGINAL program's specs. History, not requirements.
```

### The Lean → Rust seam

`lean/Ratio/*/Emit.lean` produces a `syn` AST as JSON; a genrule runs it through
`//rust/json_to_rust` into `src/generated*.rs`, which is a direct `src` of the
consuming crate. Nothing is committed — it is one DAG edge from Lean source to
compiled Rust, so a Lean file that fails to elaborate breaks the Rust build.

The division is deliberate: **the walk stays in Rust, the decision is emitted.**
`relieveFifo`'s fold is `crates/ratio-project/src/relief.rs`; what comes from
Lean is `takes_whole_lot`, `partial_divides`, `partial_cost`, `lot_is_sound`.

## Console changes

⛔ **Bazel does not build the console, and there is no JavaScript toolchain in
the Bazel graph any more.** It used to be `//web:console_html` → genrule
`//crates/ratio:console_rs` → `src/console_html.rs` → the binary, which embedded
the whole page as a `&str` at compile time — so a stylesheet change needed an
image build and a CloudFormation deploy, and the entire console was one URL.

`console/` is a Next.js application deployed to Vercel, with a route per
resource. Two commands, not one:

```bash
bazel run //crates/ratio -- watch --book <dir>   # the API on :7373
cd console && pnpm dev                            # the console on :3000
```

⭐ **A local run needs no WorkOS, no secret and no network.** `ratio watch` sets
none of the `RATIO_WORKOS_*` variables, so `/authconfig.json` answers with empty
strings, the console skips its sign-in gate, and the server answers as
`Subject::Local` — unrestricted, and not a tenant. `RATIO_DEMO_OPEN` is
unset by default (including on the deployed demo). Set `RATIO_DEMO_OPEN=1`
only when a local or CI process should grant every authenticated AuthKit
session every fund; Connect tokens never take that dial.

```bash
cd console && pnpm check    # tsc --noEmit, the render suite, next build
```

Adding a field to the API touches six files in order: `console.proto`,
`transcode.rs` (route, dispatch arm, `JsonView`), `ratio-console/src/lib.rs`,
`console/src/wire/types.ts`, `console/src/wire/client.ts`, and the page under
`console/src/app/` that reads it. Four tests turn an omission into a failing
build rather than a 404: `//proto:mirrors_test` (the two hand-written mirrors),
`//crates/ratio-console:transcode_test` (the route table), and
`console/scripts/route_manifest_test.py` — which asserts the console calls
exactly the contract's routes **and** that no RPC goes unread by any screen.
The first two run under Bazel; the third runs in `console.yml`.

⚠ If the field is one a reader looks for, add it to `console/scripts/fields_test.py` and
give it a case in `console/src/app/screens.test.tsx`. Between them they are the
successor to `//web:rendered_test` and to `ratio_test`'s
`the_served_console_carries_the_lot_engine`, both of which existed because a
field has already been "declared, transcoded, served, typechecked and mirrored
while no component reads it".

⚠ `bazel cquery --output=files` can hand back a stale binary. Build explicitly
first, then query.

## Adding a proof

1. `lean/Ratio/Foo/Bar.lean`, with `set_option warningAsError true` and a module
   doc saying what it proves and why.
2. A `lean_test` in `lean/BUILD.bazel` listing the file **and its whole
   transitive import closure** in `srcs`, with `entry` set to the file.
3. `audit_proofs_test` picks it up from a glob — no BUILD edit needed.

⚠ `decide` cannot reduce `List.mergeSort` (well-founded recursion). Use a
structural insertion sort; `native_decide` would trade a checked theorem for a
trusted compiler and the audit rejects it.

## Adding a TLA spec

1. `Foo.tla` with `CONSTANTS` for the bounds and a boolean dial per fix, and
   actions branching `IF Dial THEN <correct> ELSE <bug>`.
2. `MCFoo.tla` (three lines, `EXTENDS Foo`) and `MCFoo.cfg` with every dial
   `TRUE`.
3. A probe: `BadThing.tla` (three lines) and `BadThing.cfg` with one dial
   `FALSE`, a header comment ending `<Invariant> MUST go red.`, and `INVARIANTS`
   narrowed to `TypeOK` plus that one.
4. `tla_library` + `tla_check`, and the probe `tla_check` with
   `tags = ["manual"]`.

## Working with an LLM

See [AGENTS.md](AGENTS.md). Most of this repository was written that way, and
the rules there are the ones that matter — proofs first, Bazel only, and break
what a test protects to find out whether it works.
