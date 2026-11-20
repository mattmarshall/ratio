---
name: ship-a-change
description: Take a change in Ratio from a branch to a merged PR the way this repository expects — commit style, keeping PLAN.md / HANDOFF.md / current-state honest in the same PR, issue-closure wording that will not auto-close the wrong issue, the merge check, and the files you must never commit. Use when committing, opening or describing a PR, writing a merge message, or deciding whether an issue is complete.
---

# Ship a change

⛔ **Owner-only repository.** Outside contributions are prohibited
(`CONTRIBUTING.md`, `LICENSING.md`). This process is for owner-authored and
owner-directed work. Do not import an outside patch.

## Before you commit

1. Run the tests your change affects — the **build-and-test** skill.
2. If you touched a spec, `tla/probes.sh` — the **proofs** skill.
3. If you touched `console/`, its checks — the **console** skill.
4. `git status`: ⛔ never commit a `MODULE.bazel.lock` rewritten by local
   module overrides (`git checkout -- MODULE.bazel.lock`), `bazel-*` symlinks,
   `node_modules/`, or anything from `.env`.

## Commits

- **Not conventional commits.** The subject is a sentence about what changed
  ("Refuse a strike --as-of that names a day the strike would not land on").
  The body says why, what was learned or surprising, and what is now
  load-bearing — including what was tried first and was wrong.
- One commit per coherent step.
- American English.

## Keep the documents honest in the same PR

- **Built honesty goes in the engine PR.** When behavior lands, the
  `PLAN.md` / `site/roadmap.src.html` amendment that records it goes in the
  same commit. A follow-up docs-only PR is how the site gets ahead of the
  product.
- Update `docs/current-state.md` when supported behavior changes.
- Update `HANDOFF.md` when something becomes load-bearing or a trap cost you
  time. Correct an entry that your change makes false.
- ⛔ `//:plan_refusals_test` fails if you build something `PLAN.md` still
  refuses. Edit `PLAN.md` in the same commit; never delete the check.
- ⚠ Do not update `specs/`. It is the original program's history.

## Issue references: the closure wording check

GitHub closes an issue from a keyword anywhere in a PR body or commit message,
and a negation in front of it does not stop it. So:

- Partial work: `Related: #N` and `Remaining work: #N — <what is left>`.
- Completion: a standalone line with the closing keyword, after the acceptance
  evidence, one issue per line — only when the issue's whole acceptance list
  is met.
- Never name one issue as both complete and related/remaining.

Check any PR body, commit message or merge message before using it:

```bash
python3 .github/scripts/check_issue_closures.py --text-file /tmp/pr-body.md
```

The `issue closures / wording` workflow runs the same parser on the PR title,
body and every commit. The full rules are in `docs/issue-completion.md`.

## The PR

- Title: a sentence, no closing keyword.
- Body: what changed and why, the evidence (which tests, how many states a
  model explored, the sabotage run that showed a new test can fail), what was
  **not** run and why, and what remains — named on the issue it belongs to.
- State leftovers explicitly. A reviewer should not have to infer them.
- `bazel test //...` is not the whole gate: `console.yml` and
  `sdk-typescript.yml` are required for changes under their paths.

## Merging (the merge check)

1. Read the issue's entire acceptance list and verify the evidence. A passing
   suite is not acceptance.
2. The wording check passes on the current body and commits.
3. Inspect the PR's Development links: a manually linked issue closes too.
4. The squash message is outside the workflow's snapshot — run the wording
   check on it locally if you edit it. Repository convention: squash merge,
   subject ending `(#N)`.
5. Move the project item to Done only when the whole issue is complete.

Merging and approving are the owner's decisions. An agent prepares a PR to be
mergeable; it does not merge without being asked.
