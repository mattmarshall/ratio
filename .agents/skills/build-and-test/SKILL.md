---
name: build-and-test
description: Build Ratio and run its tests with Bazel — the full CI gate, just the tests your change affects, one test target, or one test function — and read the results. Use before every commit and whenever a change needs verifying. Covers what `bazel test //...` does not run (the console, the TypeScript SDK, the site, manual probes, live Postgres).
---

# Build and test

⛔ **Bazel only. Never `cargo`.** `cargo build` cannot succeed: the root
`Cargo.toml` workspace is stale and the Lean → Rust emission has no Cargo
equivalent. `Cargo.lock` exists only because `rules_rust`'s crate universe
reads it.

If the first Bazel command dies with `GET returned 403 Forbidden` on a
`github.com/.../archive/...` URL, use the **restricted-network-bazel** skill
first, then come back.

## The loop

```bash
# 1. While iterating: only the tests that depend on what you changed
.agents/skills/build-and-test/scripts/affected-tests.sh            # vs origin/main
.agents/skills/build-and-test/scripts/affected-tests.sh --list     # just print them

# 2. One target, or one test function inside it (a substring filter)
bazel test //crates/ratio:ratio_test --test_output=errors
bazel test //crates/ratio:ratio_test --test_arg=as_of_naming

# 3. Before a PR: exactly what CI runs (build.yml)
bazel test --lockfile_mode=error --test_output=errors -- \
  //... -//marketing:positioning -//paper:ratio -//competitive/specs:all
```

`affected-tests.sh` resolves your changed files (commits since the merge base,
staged, unstaged, untracked) to Bazel labels, finds the tests that depend on
them, drops `manual` targets, and runs the rest. Files outside the Bazel graph
are listed at the end — those need the checks below, not Bazel.

The three excluded targets are the tectonic PDFs: tectonic downloads its TeX
bundle mid-build, so CI cannot build them. Their **text** is still checked by
`//marketing:language_test`.

## Reading results

- A failing test's log: `bazel-testlogs/<package>/<target>/test.log`, e.g.
  `bazel-testlogs/crates/ratio/ratio_test/test.log`. Rust tests print
  `test tests::<name> ... ok|FAILED`; grep for the name.
- `(cached) PASSED` means the inputs did not change, not that it ran now.
- The CLI binary after a build: `bazel-bin/crates/ratio/ratio`.
- ⚠ `bazel cquery --output=files` can hand back a stale binary. Build first.
- List everything CI considers a test: `bazel query 'tests(//...)'`.

## Lockfiles

CI runs `--lockfile_mode=error`, so a dependency change must commit
`MODULE.bazel.lock` and `Cargo.lock`. After adding a crate:

```bash
CARGO_BAZEL_REPIN=1 bazel build //...
```

⛔ If you are using the restricted-network overrides, your working
`MODULE.bazel.lock` is rewritten by them. Restore it before committing
(`git checkout -- MODULE.bazel.lock`) unless you meant to change dependencies,
and in that case repin without the overrides.

## What `bazel test //...` does not cover

| Change under | Run instead | Where CI runs it |
|---|---|---|
| `console/**` or `console.proto` | the **console** skill | `console.yml` |
| `sdk/typescript/**` or `ledger.proto` / `chart.proto` | `cd sdk/typescript && pnpm install --frozen-lockfile && pnpm test` | `sdk-typescript.yml` |
| `site/**` | `python3 site/build.py --check-fonts --standalone -d /tmp/_site && python3 site/verify.py /tmp/_site` | `site.yml` |
| a TLA+ spec | `tla/probes.sh` — see the **proofs** skill | not in CI |
| the SQL projection | `//crates/ratio-sql-project:pg_engine_test` with `RATIO_PG_URL` (needs Postgres 16 and `psql`) | `build.yml`, by name |

`manual`-tagged targets never run under `//...`: the TLA+ probes (they are
meant to FAIL) and the live-Postgres tests. Name them explicitly.

## Checks that read prose

These are ordinary tests, but they parse Markdown and fail on wording. Run them
by name when you edit the file:

| Edit | Test |
|---|---|
| `PLAN.md` (refusals, BookKinds) | `//:plan_refusals_test` |
| `HANDOFF.md`, `PLAN.md`, `docs/connect-scopes.md` (Connect grants, measured shapes) | `//:connect_scopes_test`, `//:scale_shapes_test` |
| any `.tex` under `marketing/`, `paper/`, `competitive/` | `//marketing:language_test` |
| a PR body, commit message or merge message | see the **ship-a-change** skill |

## Before you claim a test works

Break the thing it protects and watch it fail, then restore it. Mutation
testing found three suites here that were green, covered the code, and tested
nothing. A test you have not seen fail is a test you have not checked.
