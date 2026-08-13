# Handoff — demo readiness, UI buildout, AWS hosting, login

⚠ **HISTORY, NOT INSTRUCTIONS.** This is the prompt that opened the auth,
hosting and console round (#22–#27); it is kept for the record. Parts of it are
now wrong — in particular everything about `//web:console_html`, `//crates/
ratio:console_rs` and rebuilding `//crates/ratio` for a console change. The
console is a Next.js application in `console/`, deployed to Vercel, and Bazel
does not build it. `DEVELOPING.md` and `HANDOFF.md` are the current documents.

Paste the block below as the opening prompt of the new session.

---

ultracode

You are picking up `mattmarshall/ratio` at `main` @ `3b37d75`, tree clean, 57
tests green. Read `HANDOFF.md` and `AGENTS.md` first — they carry the build
rules and the failure table, and both are current.

**The work:** issues #22–#27, which I want built in roughly that order.
Authentication (#22) and tenancy (#23) gate most of the rest; hosting durability
(#24) and the domain (#25) can proceed in parallel; console buildout (#26) and
demo readiness (#27) close it out. #5, #8 and #9 are engine work and are **not**
in scope for this round unless a blocker forces it.

Start by reading all six issues. They were written against the tree on
2026-08-11 with the facts verified, not guessed — the greps and file
references in them are real. Confirm anything you intend to rely on, because
this repository's whole discipline is that a claim nobody re-checked is a claim
that has already drifted.

**The single most important fact:** there is no authentication anywhere in this
system, and four write routes — `applyEvent`, `ingest`, `admit`, `mark` — are
open on a public API Gateway URL with no `AuthorizationType`. That URL is in
CI logs. Treat #22 as the first thing.

**Before you plan, decide with me:** `PLAN.md` was reconciled with reality
yesterday and now records that four "explicitly not building" entries were built
in the 48 hours after it was written. It names the decision and does not make
it — whether the refusal list was wrong, or the last stretch was off-plan. The
wedge's one remaining open gap is *a real customer's period*, and none of
#22–#27 moves it. Say so plainly in your plan rather than routing around it.
`//:plan_refusals_test` will fail if you build something the plan still refuses;
edit `PLAN.md` in the same commit, do not delete the check.

## How this repository works

- **Bazel only.** `bazel test //...` is the gate. `cargo build` cannot succeed —
  the root `Cargo.toml` is incomplete and this is deliberate.
- **Proofs first, then code.** Lean for arithmetic, TLA+ for anything that
  spans time or two writers. `//tla:probes_test` and `tla/probes.sh` enforce
  that every `manual`-tagged probe still goes RED for the invariant it names;
  16 probes today, and all 16 must stay red.
- **DTOs are protos.** Anything crossing a process, language or file boundary
  gets a `.proto` message. `//proto:ratio_aip_lint` enforces AIP conventions.
- **American English** in code, comments, docs and commit messages.
- **No Claude attribution** in commits or PR descriptions.

## Traps that have already cost time here

- ⛔ **Rebuild `//crates/ratio` for console changes.** There is no `//web:console`
  target; the chain is `//web:console_html` → genrule `//crates/ratio:console_rs`
  → `src/console_html.rs` → the binary. Building under `//web:` alone refreshes
  nothing that is served.
- ⛔ **A field can be declared, transcoded, served, typechecked and mirrored
  while no component reads it.** `//web:rendered_test` greps the built bundle
  for exactly this; extend its list when you add a screen.
- ⛔ **Negative-test every test you write.** Three separate checks written in the
  last two days passed against the defect they were written for — a differential
  NAV test whose book netted to zero in every currency, a plan check that could
  not fail because Bazel sandboxes runfiles, and a seed assertion comparing
  multi-line strings. Break what the test protects and watch it go red, or you
  have not written a test.
- ⛔ **Commit before mutating.** `git checkout <file>` to undo a bad edit
  discards uncommitted work in that file. This has happened three times across
  two repositories, each time after writing the warning down. Use `git stash` or
  a `cp` backup taken *before* the risky edit.
- ⛔ **A `python3` heredoc eats Rust `\` line continuations** and leaves runs of
  spaces inside string literals. Happened three times in one session, and one of
  them reached served JSON. Sweep `git diff -U0 | grep -nE '^\+.*"[^"]*[a-z,:] {6,}[a-z]'`
  before committing; column headers are the only false hits. Prefer the Edit tool
  for anything containing a continuation.
- ⚠ **zsh does not word-split `$var`**, and `:a` in `${B}:applyEvent` is a
  modifier — brace it.
- ⚠ **macOS `maximum resident set size` excludes compressed pages.** Quote
  `peak memory footprint` from `/usr/bin/time -l`; at 20M lots the two differ
  by 19×.
- ⚠ **`gh run watch` exits 0 on a cancelled run**, and `statusCheckRollup` can
  report FAILURE for a green PR. Match the run's `head_sha` to `headRefOid`.

## What "done" looks like for this round

A person can reach the demo at a Ratio-owned hostname, sign in, see only the
funds they are entitled to, post an entry that is still there tomorrow, and read
a NAV strike signed with their own identity rather than a string they typed.
Every one of those is currently false.
