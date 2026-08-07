# Ratio — path to revenue

**Written 2026-08-07.** Supersedes `specs/iterations/iteration-1-mvp.md`, which
is an eight-week plan for a product nobody was buying.

Three constraints set everything below:

| | |
|---|---|
| **Revenue path** | Shadow-run wedge, sold as independent verification |
| **Capacity** | One engineer, part-time — nights and weekends |
| **Demo** | Expert talks, rules appear, books balance |

The capacity constraint dominates. At perhaps ten hours a week, the difference
between a good plan and a bad one is not which features get built — it is how
much gets **refused**. Everything here is sequenced so that each stage is
independently useful and nothing is built twice.

---

## The shape of the bet

The wedge is not a smaller version of the platform. It is a different product
that happens to share a core:

> Run Ratio beside whatever the customer already has, on their own feed, and
> report every discrepancy — itemized, reproducible, and attributable to a
> named configuration.

Nobody replaces anything. The buying decision is small, the proof is their own
book, and it is the only path where the first thing built is the first thing
sold. Everything on the roadmap page is downstream of it.

**What makes it hard is not building it.** It is that a shadow run producing
*false* breaks is worse than no shadow run at all: it burns the one thing being
sold, which is that a discrepancy from Ratio means something. That single fact
drives the scoping below more than anything else.

---

## What the demo and the wedge share

Nearly everything. Building them as one thing is what makes this tractable
part-time.

```
                  ┌──────────────────────────────────┐
   MCP demo  ───▶  │  config → rules → postings       │  ◀─── shadow wedge
   (drafting,      │  → kernel → balances             │       (ingest,
    checks,        │                                  │        replay,
    approval)      │  the part that is built ONCE     │        compare)
                  └──────────────────────────────────┘
```

The demo exercises the left edge (authoring). The wedge exercises the right
edge (ingest and compare). The middle is the same code, and it is the middle
that does not exist yet.

---

## Build order

Each stage ends with something demonstrable. Nothing starts before the stage
above it works.

### Stage 0 — a ledger you can query *(the gap that blocks everything)*

The kernel proves conservation over vectors in memory. There is no persistence,
no chart of accounts, no way to ask what the balances are. Until that exists,
neither the demo nor the wedge can be built at all.

- ~~SQLite, not Postgres.~~ **Built with no database at all** — see below.
- CLI: load a config, post events, print a trial balance.
- **Done when:** `ratio post events.json && ratio balance` prints a trial
  balance that ties.

✅ **DONE 2026-08-07.** `crates/ratio-store` + the `ratio` CLI. Measured on a
five-entry fund book: total 36,503,867.45 on both sides, difference 0.00. Two
deliberately unbalanced entries were refused with their exact shortfall, zero
of them reached the journal, and the book still tied afterwards.

**Amended: no SQLite.** The plan said SQLite and the build does not use it. The
reason is the one this document already gives for the control plane: the
smallest thing that is not a lie. The log *is* the database — entries are
immutable and appended, and the trial balance is a fold of them, which is the
definition rather than an optimisation. A relational engine at this size buys
indexed lookup that nothing needs yet, in exchange for a C dependency
(`libsqlite3-sys`) inside a hermetic Bazel build maintained part-time.

The seam is a trait either way, so this is an implementation swap rather than a
rewrite. **Trigger to adopt a real engine:** the wedge needing per-account
lookups over a period without a full scan, or concurrent writers. Neither has
arrived.

Crate-universe repinning was verified working in the process
(`CARGO_BAZEL_REPIN=1`), so adding SQLite later is a dependency edit, not a
research project.

### Stage 1 — rules that compile and get checked

The heart of both products.

- ⛔ **Do not build a language.** The `rule … { }` syntax on the site is a
  *rendering*. Rules are TOML with a schema; a pretty-printer produces the
  syntax for display. A parser is weeks of work that buys nothing a customer
  can see, and it is the single most likely way this stalls.
- Compiler: `(rule, event) → postings`.
- Three rule types only, chosen because they cover a real simple fund:
  trade posting, dividend accrual, management-fee accrual.
- The checks, which are the product's actual differentiator:
  balances on every dimension · accounts exist with the assumed normal balance ·
  quantities are typed, never floats · no effect on a closed period ·
  every referenced fact has provenance.
- **Done when:** a bad rule is rejected with a reason a fund accountant would
  recognize, not a stack trace.

✅ **DONE 2026-08-07.** `crates/ratio-rules` + `ratio rules check` / `rules
show` / `apply`. A rule set with three faults reports:

```
x management_fee_accrual: does not balance: the posting weights net to -1,
  not 0. Every debit needs a matching credit — check the signs.
x management_fee_accrual: posts to account 999 which is not in the chart of
  accounts. Add it, or point the rule at an existing account.
? management_fee_accrual: is an accrual with no day-count convention.
  Which applies — act/365, act/360 or 30/360?
```

Rules are TOML; `render` produces the `rule … { }` form for humans and nothing
parses it. `rate_bp` is an integer and `day_count` an enum, so a float is
**inexpressible** rather than rejected by a check somebody might forget to run.

**A false positive was found and removed.** The first version flagged any leg
posting against its account's normal side — which fires on every purchase,
because purchases credit cash. Normal side is a property of a *balance*, not a
posting, so the indicator stays on the trial balance and the rule check says
nothing about it. Same principle as false breaks in a shadow run: a check that
cries wolf trains people to ignore the checks. A correct rule set now produces
**no output at all**, which is what makes a finding worth reading.

### Stage 2 — the demo — ✅ DONE (`crates/ratio-mcp`, `ratio mcp`, `ratio approve`)

- MCP server exposing exactly: `list_accounts`, `propose_rule`, `check_rule`,
  `post_events`, `trial_balance`, `explain_figure`. Line-delimited JSON-RPC 2.0
  on stdio; `ratio mcp` serves it.
- **The model can call `propose_rule` and nothing else that writes.** Approval
  is `ratio approve <id>`, a CLI command, run by a person.
- A live trial-balance page that updates as events post — the visual payoff.
  `ratio watch` serves it on loopback: one page, one JSON endpoint, no
  dependencies, same palette as the website. Verified in a browser — 250
  accruals posted from a second terminal moved the table with no reload,
  difference held at `0.00`, console clean.
- **Done when:** the five-minute script below runs end to end without a
  rehearsal. *Steps 1–3 and 5–7 verified end to end over real stdio; step 4's
  answer-a-question exchange is the deviation below.*

#### Two deviations from this plan, both deliberate

**`approve_rule` is not a tool, and neither is `answer_question`.** This plan
listed both, *and* required that the model "can call `propose_rule` and nothing
else that writes… true in the code, not just in the narration." Those two
demands are in direct conflict: a write tool that a model can call is a write
tool a model can call, whatever the permission check around it says. The fence
won. `approve_rule` is not exposed, not dispatched, and not reachable — the
dispatcher answers the name with a sentence explaining that a person runs
`ratio approve` at a terminal. It is enforced by **absence**, not by a
permission check somebody could later relax. `answer_question` went with it:
the questions `check_rule` returns are answered by revising the rule and
proposing again, which leaves the whole exchange in the proposal history rather
than in a side channel nothing content-addresses.

**Approval re-checks and merges.** `approve` re-runs `check` at approval time
(the chart may have moved since the proposal was made — approving something
that no longer passes would put a bad template into production with a human's
name on it), and *merges* into the active rule set rather than replacing it, so
approving a fee rule does not silently retire the trade rules. Both are tested,
and the merge test was negative-tested by sabotaging the merge.

### Stage 3 — the wedge

- Ingest **one** file format. Pick the one the first prospect actually has;
  do not build a framework for formats you have not seen.
- Replay against the customer's reported positions and produce a break report:
  each difference, its cause, and the config hash that produced Ratio's figure.
- **Scope it to a fund type where coverage can be complete** — single currency,
  long-only equities, plain trades, cash dividends, one management fee. A fund
  with FX, corporate actions or tax lots will generate false breaks and the
  engagement dies on the first call.
- **Done when:** a real period reconciles to zero differences, or every
  difference has an explanation the customer agrees with.

---

## The five-minute demo

Written as a script because the demo is a *performance*, and the thing being
performed is that the model is fenced.

1. **"Here's our chart of accounts and an empty book."** Trial balance on
   screen, zeros.
2. **Say the rule out loud, to Claude:** *"Management fee accrues daily on the
   prior day's net assets at 75 basis points a year, actual/365, payable
   quarterly in arrears. Don't accrue on a non-business day — roll it forward."*
3. **The draft appears.** Typed, readable, obviously not English any more.
4. **The checks run.** Six pass. Two come back as questions — one of them
   catching that the draft contradicts something said in passing. *This is the
   moment the demo is about.* Answer them.
5. **Approve.** A config hash appears. Point out that the model could not do
   this step.
6. **Post ten thousand transactions.** The trial balance moves on screen and
   the difference stays `0.00`.
7. **Ask Claude to explain a figure.** It reads the postings back and cites the
   config hash — reporting, not deciding.

The line to land: *the model wrote the rule, and still could not have made the
books wrong.*

---

## UI screens

Three. Not four.

| Screen | Why it exists |
|---|---|
| **Trial balance / ledger** | The demo's payoff and the wedge's evidence. Live-updating, drill from a total to the postings behind it. |
| **Break report** | The wedge's actual deliverable — the thing a customer pays for. Each break: our figure, their figure, the cause, the config hash. |
| **Rule and its checks** | What was approved, by whom, which checks passed, which questions were asked and answered. Shown in the demo, used in the sale. |

No portal, no dashboard, no settings screens. The MCP conversation *is* the
authoring interface; building a rule editor would be building the thing the
demo exists to make unnecessary.

---

## Explicitly not building

Named so they stop being tempting. Every one of these is on the website as a
destination, and none of them earns a dollar in the next six months:

control-plane UI and epoch machinery beyond a version hash · the workload
planner · anything GPU · multi-currency and FX · tax lots and cost basis ·
corporate actions · performance reporting and attribution · the client portal ·
CRM connectors · a rule language parser · Postgres · Kubernetes.

⚠️ **The website describes the destination; the build is the first five per
cent of it.** That is defensible — the roadmap page says so — but it means
every claim on the site must stay in the future tense until it isn't.

---

## The control plane: geetch and crova

**The architecture is right; the timing is not.** Worth writing down properly,
because the instinct is correct and the reason to defer it is not obvious.

`fastverk/geetch` is an API-only git forge (Rust, gRPC, Bazel, serving
`forge.v1.ForgeService` over real git storage). `tomato-bazel/crova` is a
unified content-addressed store with per-namespace retention, built precisely
on the observation that *a build cache must evict and a depot must never lose
an object*.

Line those up against what the platform page claims the control plane is:

| Control plane, as described | geetch / crova |
|---|---|
| content-addressed artifacts | crova, exactly |
| versioned, diffable history | git commits |
| reviewed before it takes effect | pull requests |
| promoted atomically | a ref move |
| never loses a historical config | crova namespace retention |
| identical rule sets stored once across funds | crova dedup |

That is not an approximation. It is the same object, and it already exists.

### Why not now

- **Both carry open defects that would land on day one.** geetch's shipped
  image cannot exec `git http-backend` — the hermetic git 2.49 needs GLIBC 2.38
  and the bookworm base has 2.36, so the smoke test is red on main. crova has
  **no consumers at all**, its `v0.0.1` tag is ten commits behind the
  reconciliation that fixed its store trait, and the registry entry is
  unmerged. Adopting either means owning its operational problems before Ratio
  has a customer.
- **They need infrastructure Ratio does not have.** geetch expects an IdP for
  JWT, a running service, S3 mirroring. That is a platform to operate, on
  nights and weekends, in service of a feature no first customer is asking for.
- **For one fund, the control plane is a file and a hash.** The wedge needs to
  answer "which rules produced this figure?" — a SHA-256 over canonical TOML
  and a pointer file answers it completely. Review, branching and dedup are
  answers to problems that arrive with the *second* customer.

This is the same judgement as *do not build a language* in Stage 1: the site
describes the destination, and the destination is right, but the first version
of each piece should be the smallest thing that is not a lie.

### What to do instead — define the seam now

Put the interface in from the start, so the swap is a new implementation rather
than a refactor:

```
ConfigStore
  put(bytes)        -> Digest        content-addressed, canonical encoding
  get(Digest)       -> bytes
  set_active(Digest)                 atomic; the only way policy moves
  active()          -> Digest
  history()         -> [Digest]      newest first
```

- **v1** — a directory, SHA-256, and a pointer file. An afternoon.
- **later** — crova behind `put`/`get`, geetch behind `history` and the review
  flow that gates `set_active`.

Every posting already records the digest it ran under, so nothing downstream
changes when the implementation does. This is the same move as `.brando` being
the seam between producing a brand and wearing one: state the boundary in the
types, and the swap costs a day instead of a quarter.

**Trigger to adopt them:** a second customer (dedup and shared rule sets start
paying), or a compliance requirement for reviewed change control that a pointer
file cannot satisfy. Not before.

---

## Commercial mechanics

- **First engagement:** a paid pilot. Fixed fee, one fund, one quarter shadowed.
  Not free — a free pilot gets a free pilot's attention, and the point of
  charging is to find out whether the pain is real.
- **What they get:** a break report per period, and the reproducibility to take
  any figure back to the rules that made it.
- **Who to call first:** fund administrators serving small funds, and family
  offices — both feel reconciliation pain directly and neither has a platform
  team defending an incumbent. A boutique administrator is the best first call:
  they carry the pain, they buy quickly, and they have many funds behind them.
- **Do not start with the obvious warm intros.** Several of the people closest
  to this work at a firm competing in the category. Selling into or through
  them creates a problem for them before it creates revenue for you.

---

## Risks worth naming

- ⚠️ **IP assignment.** This is being built part-time while employed. Depending
  on the employment agreement and state law, an employer can have a claim on
  work done on personal time — particularly where the subject matter is
  adjacent. **Get the agreement read before the first customer dollar**, not
  after. This is the single risk that can retroactively destroy the asset, and
  it costs one conversation with a lawyer to close out.
- **False breaks.** Covered above; the reason the first fund type is narrow.
- **Coverage creep.** Every prospect will have one instrument that needs one
  more feature. The discipline is to say no and pick a different prospect.
- **Solo bus factor.** At part-time, six months of elapsed work is a small
  amount of built work. The plan is only viable because the wedge is small; it
  stops being viable the moment the scope is the platform.
- **The site is ahead of the product.** Anyone technical who reads the platform
  page and then the repository will notice. The roadmap page is the mitigation
  and it must stay honest.

---

## What "done" looks like

**Six months, part-time:** a demo that closes meetings, and one paying shadow
engagement with a real fund's data reconciling.

Not a platform. Not parity. One customer who can tell you their books were
wrong and that Ratio is how they found out.
