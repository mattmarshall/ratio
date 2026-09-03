# Contributing to Ratio

Read [HANDOFF.md](HANDOFF.md) first, then [AGENTS.md](AGENTS.md) — most of this
repository is written with LLM assistance and the rules there apply to everyone.
[DEVELOPING.md](DEVELOPING.md) has the build.

## The process

```bash
git checkout -b <a-branch>
bazel test //...        # ⛔ never `cargo test` — see DEVELOPING.md
tla/probes.sh           # if you touched a spec
cd console && pnpm check   # only if you touched console/
```

Then a pull request. CI runs `bazel build //...` and `bazel test //...`, and —
for a change under `console/` — `.github/workflows/console.yml`.

⚠ **`bazel test //...` is no longer the whole gate, and it used to be.** The
operations console is a Next.js application built by Vercel, so Bazel has no
JavaScript toolchain any more (see the ⛔ in `MODULE.bazel`) and cannot see a
type error in `console/`. `console.yml` is a required check for that reason.
`site/` is the older precedent for a non-Bazel path.

`console.yml` runs `tsc --noEmit`, the render suite, `next build`, and six
source-text checks — the contract reaches every screen, the screens still read
the fields whose absence has shipped before, the fixtures match the proto, the
design tokens still agree with `site/style.css`, nothing that belongs in the
environment is committed, and every `BookKind` is named in `PLAN.md`.

⛔ **The one thing Bazel still asserts about the console is
`//proto:mirrors_test`**, which reads `console/src/wire/types.ts` by label and
holds it to `console.proto` field-for-field. Do not delete
`console/BUILD.bazel`; it exists for that export.

## What a change looks like here

**Proofs before code, for anything that decides a figure.** Lean or TLA+ first,
then the Rust against it. This is not ceremony: the relief engine has been wrong
in ways only `decide` reported, and the tiebreak that made HIFO silently perform
FIFO would have passed a test asserting "HIFO differs from LOFO", because both
were broken the same way in opposite directions.

**Break the thing your test protects, and check the test fails.** Mutation
testing found three suites here that were green and covered the code and tested
nothing. Writing a test is not the same as it working, and this is the only way
to tell.

**One commit per coherent step.** Look at `git log`: the subject is a sentence
about what changed, and the body says what was learned, what was surprising, and
what is now load-bearing.

⚠ **Not conventional commits.** This file used to mandate
`feat(scope): subject`, and the history has never used it. What matters is that
somebody reading the log in a year can tell why.

**Record what was wrong, not just what is right.** The most valuable comments and
messages here are the ones saying "the first version of this did X and `decide`
reported it false". A defect that is fixed and unexplained gets reintroduced.

## Coding standards

- **Money is never a float**, and a figure that will not divide exactly is
  refused rather than rounded.
- **Comments carry the reason, not the mechanism.** `⛔` for a thing that will
  break, `⚠` for a thing that will surprise, `⭐` for the load-bearing claim.
- **Test names are sentences describing a property** —
  `a_husk_gives_away_its_cost`, not `test_relief_2`.
- **Types over checks where it is possible to make the failure
  unrepresentable.** `AsOf<T>` exists so a caller cannot pin the journal head
  while reading a lagging projection: it never has the head to hand.
- No `sorry`, `admit`, `axiom` or `native_decide` in Lean — `//lean:audit_proofs_
  test` fails on all four.
- No Python or Rust dependency added without a reason that survives the question
  "what happens when this version changes?" There is no RNG crate in
  `ratio-gen` for exactly that reason.

## Reporting an issue

Say what happened and what you expected. If it concerns a figure, include the
figure, the config digest it was produced under, and the journal prefix — a
number without those cannot be reproduced, and reproducibility is the product.

⚠ **An issue's premise can be wrong.** Several here have been. If you find that
while working on one, say so in the issue rather than building against it.

## Documentation

Update `HANDOFF.md` when something becomes load-bearing or when you hit a trap
that cost you time. It is the document a new reader is pointed at, and it is
worth more than the rest combined.

⚠ Do **not** update `specs/`. It describes the personal-finance program this
repository began as and is kept only for history.
