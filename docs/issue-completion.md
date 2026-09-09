# Issue completion and the merge check

GitHub recognizes closing keywords in PR descriptions and commit messages.
A word such as “not” before the keyword does not make it a safe partial-work
reference. See [GitHub's linking rules](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue).

For partial work, use neutral references:

```text
Related: #22
Remaining work: #22 — app registration and the live walkthrough.
```

For a completed issue, put the closing directive on its own line after the
acceptance and validation evidence, separated from prose by a blank line.
Use one issue per line. Keep the title
free of closing directives. A PR may complete a child while leaving its
parent open; reference the parent with `Remaining work:`. Do not list the
same issue for both completion and related/remaining work, including in a
commit message. Avoid quoting historical PR closing text in new PR bodies.

The `issue closures / wording` workflow checks the PR title, body, and every
commit since the PR base on opening, edits, new commits, reopening, and
readiness changes. It runs for documentation-only PRs too. Its parser
allows explicit standalone closing directives and rejects closing keywords
embedded in prose, negations, wrapped lines, quoted examples, or conflicts
with `Related:` / `Remaining work:` references. It does not infer whether
acceptance criteria are actually met. The parser's positive and negative
cases also run as `//:issue_closures_test`.

Check a proposed PR body or merge message locally:

```bash
python3 .github/scripts/check_issue_closures.py --text-file /tmp/pr-body.md
bazel test //:issue_closures_test
```

Before merging:

1. Read the issue's entire acceptance list and verify the evidence. Name
   residual work explicitly; a passing test suite is not full acceptance.
2. Require the wording check to pass on the current PR description and
   commits. Use the repository ruleset to require that check when enforcing
   merge protection; adding a workflow alone does not change rulesets.
3. Inspect the Development links: manually linking an issue can also close
   it, independently of closing text. Leave partial parent issues unlinked.
4. Inspect the final merge or squash message. GitHub can generate it from
   PR/commit text, and an edited message is outside the workflow's snapshot.
   Use the same local wording check if the message changes.
5. Record completion evidence and move the corresponding project item to
   Done only when the whole issue is complete. Otherwise preserve its
   remaining work and appropriate Ready or Blocked state.

Keep historical amendments in PLAN and HANDOFF as evidence, not as text to
copy into a new commit. The [project](https://github.com/users/mattmarshall/projects/1)
and [roadmap index](https://github.com/mattmarshall/ratio/issues/258) are the
current queue; [Current state](current-state.md) is the concise implementation
summary. This process cannot prevent direct pushes or a ruleset bypass;
those remain the merge owner's responsibility.
