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
nothing to run as a service.

## What runs in CI

`bazel build //...` then `bazel test //...`, and nothing more. That covers:

| | |
|---|---|
| `//lean:*_proof_test` | the proofs — type-checking IS the test |
| `//lean:audit_proofs_test` | no `sorry`, `admit`, `axiom` or `native_decide`; every file sets `warningAsError` |
| `//tla:*_check` | seven model checks |
| `//tla:probes_test` | every failure-path probe can still say what it claims |
| `//crates/*:*_test` | the Rust |
| `//proto:ratio_aip_lint`, `//proto:mirrors_test` | the wire contract, and its two hand-written mirrors |
| `//crates/ratio-console:transcode_test` | the route table against the proto |
| `//demo:rehearse_test`, `//demo:shadow_run_test` | the demo and the shadow run, end to end |
| `//marketing:language_test` | the licensing language sweep |

`paths-ignore` covers `site/**` and `**/*.md` only. `marketing/`, `paper/` and
`competitive/` are deliberately NOT ignored.

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

## Layout

```
lean/Ratio/       the proofs, and the Emit modules that author Rust from them
tla/              the specs, their MC configs, and the failure-path probes
crates/           the Rust. `ratio` is the binary; the rest are libraries
proto/            the wire types, AIP-linted
web/              the operations console (React + esbuild, one inlined bundle)
demo/             the five-minute demo and the shadow run, as shell tests
deploy/           the Lambda that serves the console and the API
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

⛔ **Rebuild `//crates/ratio`, not `//web:...`.** The chain is
`//web:console_html` → genrule `//crates/ratio:console_rs` →
`src/console_html.rs` → the binary, which embeds the page as a `&str` at compile
time. Building anything under `//web:` alone refreshes nothing that is served.

```bash
bazel build //crates/ratio
bazel run //crates/ratio -- watch --book <dir>   # then open /app
```

⚠ `bazel cquery --output=files` can hand back a stale binary. Build explicitly
first, then query.

Adding a field to the API touches seven files in order: `console.proto`,
`transcode.rs` (route, dispatch arm, `JsonView`), `ratio-console/src/lib.rs`,
`web/src/types.ts`, `web/src/api.ts`, `web/src/App.tsx`, `web/console.css`.
`//proto:mirrors_test` and `//crates/ratio-console:transcode_test` turn any
omission into a build failure rather than a 404.

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
