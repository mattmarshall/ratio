# Ratio — path to revenue

**Written 2026-08-07.** Supersedes `specs/iterations/iteration-1-mvp.md`, which
is an eight-week plan for a product nobody was buying.

> **Amended 2026-08-11 — the auth, tenancy and hosting round.** Before a prospect
> can be handed a login to their own book, the demo needed authentication (#22),
> a tenant boundary that actually holds (#23), durable writes (#24) and a
> Ratio-owned hostname (#25). None of these is on the *Explicitly not building*
> list — the nearest, "the client portal", is a customer self-service product,
> not authentication on the internal ops console — so this round adds no refused
> feature and `//:plan_refusals_test` is untouched. ⚠️ **Stated plainly, as it
> must be: none of this moves the wedge's one remaining open gap — a real
> customer's period reconciling.** It is prerequisite to a paid pilot, not a
> substitute for one. The tenancy, identity, attribution and security-header work
> landed and is CI-verified; it *activates* on the live demo only when the
> Cognito authorizer and durable store are deployed.

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
  rehearsal. **`demo/rehearse.sh` is that rehearsal**, wired as
  `//demo:rehearse_test` so CI runs the whole thing — all seven steps, ten
  thousand real events, asserting at each one. It fails if the fence opens:
  negative-tested by exposing `approve_rule` as a working tool, which the
  rehearsal caught at "must not appear in tools/list" and the unit test caught
  independently.

  The one thing not automated is the model itself. Steps 2–4 are a person
  talking to Claude, which drives the MCP tools; the rehearsal drives those
  same tools over the same stdio transport, so what is covered is the surface
  the model actually touches.

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

### Stage 3 — the wedge — ✅ BUILT (`crates/ratio-recon`, `ratio recon`)

- Ingest **one** file format. **Deviation, flagged:** there is no first
  prospect yet, so the format is a plain CSV with a documented column
  contract — the thing any prospect can hand over on a first call without an
  integration project. Columns are located by header name, not position,
  because a customer's export reorders them between runs. When a real prospect
  appears, their format replaces this one; it does not get generalized into a
  framework for both.
- Replay against the customer's reported positions and produce a break report:
  each difference, its cause, and the config hash that produced Ratio's figure.
  The report is `ratio.v1.BreakReport` — a proto, because it is the artifact
  the customer keeps and argues with. `--out` writes it.
- **Scope it to a fund type where coverage can be complete.** The scope is
  declared (`equity-long-only-single-ccy`) and is a **gate, not a best
  effort**: one row outside it and the run produces *no breaks at all*, only
  exceptions naming every such row. A partial replay compared against a whole
  period's positions manufactures a break for everything it skipped, and those
  breaks are Ratio's fault, not the customer's.
- **Done when:** a real period reconciles to zero differences, or every
  difference has an explanation the customer agrees with. *A synthetic quarter
  reconciles to zero in `demo/shadow-run.sh`; the second half waits on a real
  customer.*

#### Three design decisions worth keeping

**A disposal must carry its basis.** A sale relieves the investment at cost,
and without tax lots there is no way to know the cost. Relieving at proceeds
instead produces an entry that *balances* while leaving the investment figure
wrong — the worst failure available, because it ties. So a disposal without a
`basis` column is refused. Given one, no tax-lot engine is needed: the source
system already chose which lots to relieve, and taking that choice as input is
correct for a run that is trying to reproduce their books rather than replace
their decisions.

**A sale is two balanced events, not a three-legged rule.** A rule is a weight
vector applied to one amount and cannot carry two. Rather than extend the rule
model, `sell` compiles to `disposal_proceeds` at proceeds and `disposal_basis`
at basis; both conserve, so their sum does. This works because the ledger is a
monoidal fold over conserved vectors — the property `Ratio.Core` proves — and
it is why the rule model did not need changing.

**Three exit codes, because there are three outcomes.** `0` reconciled clean,
`2` reconciled with differences, `3` refused. Conflating 3 with 2 would let a
refusal be scripted as "breaks found" and quietly investigated as data.

#### Still open

- A zero-difference run on a *real* customer's period. Everything above is
  verified against a synthetic quarter.
- The `--post` path writes the shadow book, but there is no way to compare two
  runs under different configurations — which is what "we changed the fee rule,
  what moved?" actually needs.

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

## The demo, deployed — ✅ LIVE

**Two surfaces now, on two clouds, and the split is deliberate.**

`https://ratio-ims.vercel.app/` — the operations console. A Next.js application
with a route per resource, so a break, a NAV strike or a configuration version
can be sent to somebody rather than described. Sign-in is Cognito, open to
anyone; the browser never calls AWS, because the console's own server holds the
token and makes the call.

`https://1h4q8av2gb.execute-api.us-east-1.amazonaws.com/` — the API the console
reads, plus the three public screens (`/balance`, `/breaks`, `/rules`) and the
chat page, which need no account and are Rust string literals with no build
step. Plus the MCP tools at `POST /mcp` so a model can reach them over the
network rather than through a process on the caller's machine. Same six tools as
the stdio transport, same dispatcher, same fence; a test asserts `approve_rule`
is absent from the *public* endpoint's tool list.

AWS account `320473299741` (`ratio`), Platform OU. Lambda behind an HTTP API:
nothing bills at rest, and a demo used a few times a week rounds to zero
against a $5 budget. Deployed by GitHub Actions over OIDC on every push to
main, by image digest, with a smoke test that fails the deploy if the live site
serves an empty book. See `deploy/README.md` — including why a Function URL,
which would have been simpler, is refused in this account.

---

## UI screens — ✅ BUILT (`ratio watch`)

Three. Not four — and now five; see the 2026-08-13 amendment for the fourth
and fifth. All are served by `ratio watch` on loopback — one
binary, a fixed route table, reads and never writes. There is no button and no
form on any of them: the fence that keeps a model from approving a rule would
be worthless if the screen offered a second way round it, and a test asserts
that no screen has grown one.

| Screen | Why it exists |
|---|---|
| **Trial balance / ledger** | The demo's payoff and the wedge's evidence. Live-updating, drill from a total to the postings behind it. |
| **Break report** | The wedge's actual deliverable — the thing a customer pays for. Each break: our figure, their figure, the cause, the config hash. |
| **Rule and its checks** | What was approved, by whom, which checks passed, which questions were asked and answered. Shown in the demo, used in the sale. |

No portal, no dashboard, no settings screens. The MCP conversation *is* the
authoring interface; building a rule editor would be building the thing the
demo exists to make unnecessary.

The rules screen shows active rules and unapproved proposals as two separate
lists, deliberately: the gap between them is exactly what a person's approval
bought, and merging them would erase it. A test asserts a proposal never
appears among the active rules.

---

## Explicitly not building

Named so they stop being tempting. Every one of these is on the website as a
destination, and none of them earns a dollar in the next six months:

control-plane UI and epoch machinery beyond a version hash · the workload
planner · anything GPU · performance reporting and attribution · the client
portal · CRM connectors · a rule language parser · Kubernetes.

⚠️ **The website describes the destination; the build is the first five per
cent of it.** That is defensible — the roadmap page says so — but it means
every claim on the site must stay in the future tense until it isn't.

### ⛔ Four of these were built in the two days after this file was written

This list had eleven entries on **2026-08-07**. Four of them shipped on
**08-09 and 08-10**, and this file was not touched in between:

| refused | built | where |
|---|---|---|
| tax lots and cost basis | 08-09 → 08-10 | six lot methods, holding-period split, `Ratio.Lots.*`, `//tla:relief_engine_check` |
| multi-currency and FX | 08-10 | conservation per currency, translation on every read path, `Ratio.Chart.Dimensions` |
| corporate actions | 08-09 | the factor representation — nothing is rewritten |
| Postgres | 08-10 | spec only: `//tla:sql_projection_check`, no schema |

That is 48 hours, not slow drift, and it deserves a decision rather than a
footnote. **The list was either wrong on the day it was written, or the last two
days were off-plan.** Both are live readings:

- *The list was wrong.* The wedge sells independent verification of a fund's own
  book. A fund holding three currencies with a twenty-year lot history is not an
  edge case, it is the customer — and a shadow run that cannot relieve a lot or
  translate a balance produces **false breaks**, which this file names as the
  one failure that burns the whole proposition. On that reading these were
  never optional and the refusal was a scoping error.
- *The last two days were off-plan.* Ten hours a week does not survive four
  refused features, however good the reasons felt at the time. **Coverage creep**
  is named below as a risk, and this is exactly its shape — arriving from inside
  rather than from a prospect, which is the harder direction to refuse.

⚠️ **It is not all drift, and pretending otherwise would be its own distortion.**
One of the wedge's two open gaps — comparing two runs under different
configurations — closed in the same window (`ratio.v1.recon`, `baseline_*` /
`candidate_*`). The remaining gap is the one that matters: **a real customer's
period.** No amount of engine work substitutes for it, and none of the four
above moves it.

**The decision to take, explicitly:** either strike these from the refusal list
and say the wedge needs them, or stop and go get a fund's file. Leaving the
list as written meant the plan and the repository disagreed about what the
product is, and the repository was winning without anyone saying so.

### Amendment, 2026-08-13 — the console moved off the binary, and nothing on the refusal list moved

The operations console is a Next.js application in `console/`, deployed to
Vercel while the API keeps deploying to AWS. **This adds nothing from the list
above**, and it is worth saying why in the same place the list lives, because
"an operations console" and "a portal" are one word apart.

What changed is the ADDRESSING, not the surface. The console was one URL — every
screen was a `useState` in an 1801-line file — so a break an operator found could
be described and not sent. That is a strange thing for a product whose claim is
that a figure cites the journal prefix it was folded from and the configuration
it ran under. Every resource on `ratio.console.v1.Console` now has a URL, and
`console/scripts/route_manifest_test.py` holds it to that in both directions:
the console calls exactly the contract's routes, and no RPC goes unread by a
screen. (It runs in `console.yml`; Bazel does not build or test the console.)

What did NOT change, deliberately:

- **No approve button.** `approve_rule` is absent from the model's tool list on
  purpose and `//demo:rehearse_test` asserts it; approval is `ratio approve` at a
  terminal. The rules screen shows active rules and unapproved drafts as two
  lists that do not merge — the gap between them is what a person's approval
  bought. A console offering a second way round the fence would make the fence
  worth nothing, which is the argument the "three screens, not four" section
  already makes.
- **No control-plane UI, no configuration editor, no client portal.** The
  configuration screens READ versions and diff them. Editing one is control-plane
  UI and stays on the list.
- **No new RPC.** `Console::apply_action` still has no route, `ratio strike` is
  still CLI-only, and a proposal's rendered form still is not on the contract.
  Each is a proto change and each was refused here rather than smuggled in as
  page work.

⚠️ The cost that was paid: `bazel test //...` stopped being the whole gate. See
`CONTRIBUTING.md` and the ⛔ in `MODULE.bazel`.

### Amendment, 2026-08-13 — a fifth screen, because the scale claim was unshowable

`ratio watch` now serves **five** screens. The section above says "Three. Not
four.", and the fourth (`chat`) already had its paragraph; this is the fifth's,
written here rather than left for somebody to find in a test diff.

**`/scale` — what a period end reads, at a size you choose.** It is a public,
unauthenticated screen with dials over `Ratio.Closure`, and beside the answer it
carries the recorded cold build of the twenty-million-lot book issue #6
measured.

**Why it is not drift.** `deploy/seed-demo-funds.sh` names the gap in as many
words: *"AND THE SCALE ARGUMENT WAS UNSHOWABLE FOR THE SAME REASON. 'A NAV does
not read the tax lots' is a claim about a fund with a lot of them."* The demo's
largest fund holds eight hundred lots — deliberately, because the journal is
copied into a Lambda's `/tmp` on every cold start and `/tmp` is 512 MB. So the
system's central claim was true, proved, measured, and had nowhere to be seen.
`site/platform.src.html` sells that claim to the world; this is the screen where
a reader can check it instead of taking it.

**What it is NOT, and what holds it there:**

- **No write, no form, no button that spends money.** It reads. The existing
  assertion that no screen has grown a form applies to it unchanged — the dials
  are a plain `div` producing a query string on a GET, precisely so that fence
  did not have to be widened to admit "a read-only form".
- **It is not "performance reporting and attribution"** from the refusal list
  below. That entry means *investment* performance — returns, benchmarks,
  attribution against an index — and none of it is here. This is the engine's
  own cost, which `ratio bench` and `ratio closure` have reported at a terminal
  since before this file was written. ⚠ The two are one word apart, which is why
  the distinction is written down rather than assumed.
- **No new RPC.** `ratio.console.v1.Console` is untouched, the proto is
  untouched, and `console/` is untouched. The screen lives in the binary's own
  route table beside `/balance`, which is why the console's route manifest and
  `//proto:mirrors_test` have nothing to say about it.

⛔ **Both curves are on the screen, and that is the whole point of it.** The
dial answers what a NAV reads, which is flat in the tax lots and is the
persuasive number; the panel beside it is the cold build, which is 995 seconds
and a gigabyte at twenty million lots and does not flatten. `ratio bench` exists
to make quoting the second as though it were the first hard, and a screen
showing only the flat curve would have undone that at a public URL.

⚠️ **What is still NOT built: a visitor cannot RUN the twenty-million-lot fold.**
A cold build of that book is ~66× the function's timeout, ~2× its memory and
~80× its `/tmp`, so it cannot happen in a request. Doing it live needs compute
that does not exist yet — a one-shot task, a queue, and a progress surface — and
none of that is in this repository. The screen is honest about the distinction:
the estimate is live and calibrated on the serving machine, the 995 seconds is
labelled as a recorded run.

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
- ⛔ **Coverage creep — this one has already happened.** The risk was written as
  arriving from a prospect: one instrument that needs one more feature, and the
  discipline is to say no and pick a different prospect. It arrived from
  **inside** instead, which is the harder direction, because there is no
  prospect to decline and every step is individually defensible. Four entries
  left the refusal list in the 48 hours after this file was written. See
  *Explicitly not building*; the mitigation is not "say no to prospects", it is
  that this file gets edited in the same commit as the feature that contradicts
  it.
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
