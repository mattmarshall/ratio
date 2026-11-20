---
name: backlog-dispatch
description: The contract for fanning Ratio's GitHub backlog out to track agents and cloud agents — which agent owns which milestone, one issue → one agent → one PR, the status labels, the two-to-three concurrent-agent cap, and how the queue is read. Use when picking an issue to work on, dispatching one to another agent, or relabelling one.
---

# Dispatch — Grok Bot tracks and Cursor cloud agents

⚠ Moved here verbatim from AGENTS.md, where it was the "Dispatch" section.
It governs agents that open a *pull request*. Agents that touch a fund's
*book* are a different thing: see ORCHESTRATION.md.

How the backlog fans out. This section is a contract, not a product
feature, and it does not close any product issue.

## Owner map

| Agent | Owns | Does not own |
|---|---|---|
| **Ratio** | milestone **Platform & Connect**, and the merge gate for every PR into this tree | Fund / Personal / Project milestone work |
| **Ratio Fund** | milestone **Fund accounting** only | Platform, Personal, Project |
| **Ratio Personal** | milestone **Personal finance** only | Platform, Fund, Project |
| **Ratio Project** | milestone **Project finance** only | Platform, Fund, Personal |
| **Ratio iOS** | the [`mattmarshall/ratio-ios`](https://github.com/mattmarshall/ratio-ios) repository — the SwiftUI shell, its bridge, and its build | The kernel, the journal format, any amount-deciding behavior, and every milestone in this tree |

A track agent that picks an issue outside its milestone is the claim that
fails: the labels already named the owner, and crossing them puts two
authors on `PLAN.md` for no reason.

⛔ **Ratio iOS does not change an amount.** The shell consumes a pinned
kernel revision. A change to the lot engine, the projection, the journal
format, or anything emitted from Lean is a PR into *this* tree under
Platform & Connect — never a patch in the shell. A kernel forked into
`ratio-ios` is the exact failure that row exists to prevent, and it would
arrive looking like a local fix to an iOS bug.

## Dispatch unit

**One GitHub issue → one cloud agent → one PR.**

⛔ Never assign a whole milestone to one agent. A milestone is a queue,
not a prompt. The failure mode is one agent touching every file the
other two need, then a merge that rewrites someone else's Built
honesty.

## Labels

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

## Concurrency

**At most two or three concurrent cloud agents on this repository.**
`PLAN.md`, `HANDOFF.md`, and `site/roadmap.src.html` are on almost every
engine PR, so a fourth agent is a conflict factory, not throughput.

Work that lives in a **separate tree or repo** can run wider — Connect
apps, and the iOS shell in `mattmarshall/ratio-ios`. They do not share
those three files, so they are not the conflict this cap is about.

Work that stays in *this* tree still counts against the two-to-three cap,
including a Connect-layer issue that edits PLAN, and including the
kernel-side half of an iOS slice. ⚠ A two-repo slice is still one issue
and one agent; it is not a license to open a PR in each tree and let the
pinned revision drift between them.

## PR rules

- **Batch PLAN / roadmap Built honesty into the engine PR.** The
  amendment that records what landed belongs in the same commit as the
  landing. A follow-up "docs only" PR is how the site stays ahead of
  the product — the risk PLAN.md already named.
- **State leftovers explicitly.** What a walk-through still cannot show
  stays named on the issue it belongs to.
- **Use `Related: #N` and `Remaining work: #N — <gap>` for partial work.**
  A negation before a closing keyword does not prevent GitHub auto-closure.
  Reserve closing keywords for a standalone completion line only after the
  entire issue meets acceptance. Follow the [merge check](../../../docs/issue-completion.md)
  for the PR body, commit messages, and final merge message.
- ⛔ **Never invent a `Method` / `Order` / `lot_method` variant** for
  MinTax, SpecID, average cost, or wash. Each is an election with its
  own shape (`min_tax_short_weight`, `identified_lots`, `average_cost`,
  `wash_window_days` / `wash_keep_holding_period`). The TLA probes that
  treat them as a Method exist so that mistake goes red. HANDOFF.md
  records why.

## Core vs Connect

[#150](https://github.com/mattmarshall/ratio/issues/150) is the scope
catalog and the first-party extension contract. Ratio's core stays a
thin, correct book of record. Breadth is a Connect app unless a PLAN
amendment says otherwise.

**Connect unless PLAN says core:** client / LP / vendor portals, bank
OAuth, AIA G702/G703 pay-app packs, tax packs, EAC / forecast. The
kernel cites the figures those apps read; it does not grow the product
UI. `rules:approve` / `config:promote` are not scopes — the fence is
absence, and #150 does not open it.

## BookKind

A Book is a journal plus content-addressed configuration. It is
independent of Funds and WorkOS organizations: absence is independence,
not an error. `CreateBook` writes no fund and no organization.

Kind selects the chart `chart_for` writes and the chrome `screensFor`
offers — **one list, not a binary fork of the kernel.**
`UNSPECIFIED` is the proto default, not a domain and not a hidden fifth
kind. Do not mint a chrome list per issue, and do not file a household
or a job under a Fund so the old URLs keep working.

## Current queue

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
