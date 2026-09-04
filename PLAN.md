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
> substitute for one. The identity, attribution and security-header work
> landed and is CI-verified. **#23 — per-book tenant isolation — is enforced
> in CI** at `Console::open_book` (not only handlers): membership is the
> creator's WorkOS `sub`, not an implied org; `ListBooks` / `ListFunds`
> distinguish an authorized empty set from an unreadable membership file;
> every live `ROUTES` entry is classified and a caller scoped to book A
> cannot read book B. It is not a client portal. Authenticated writes
> record actor = WorkOS `sub` (#151). Connect tokens accepted with
> catalog scopes on `/v1` (membership still required; never
> `RATIO_DEMO_OPEN`; never `org:{id}`). API Gateway JWT verifies
> Connect tokens on a second HTTP API (Connect `iss` =
> `https://auth.ratio.marsh.build`; AuthKit session `iss` stays
> `/user_management/{client_id}` — one authorizer cannot OR those).
> `RATIO_DEMO_OPEN` defaults off on the deployed demo — AuthKit
> sessions isolate via membership. first-party Connect apps call
> ConnectApiUrl. The demo API Lambda hydrates
> ScaleBucket `journals/` (`RATIO_JOURNAL_BUCKET` /
> `RATIO_JOURNAL_PREFIX`) so CreateBook survives a
> cold start. Hydrate 503 is transient only. The
> 40GB scale fold stays on Fargate ScaleTask. Scale
> keeps ScaleBucket. Unused Cognito CloudFormation resources are
> removed — AuthKit is the sole IdP. Live leftovers on issue 22
> are `DEMO_MEMBERS` naming a live WorkOS `sub` and WorkOS
> dashboard registration. Durable writes are #24 (closed). The
> Cognito-era activation sentence is historical.

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
- Comparing two configurations is `compare_configs` (same events, two
  digests, in memory). A parallel mutable shadow book is refused — do
  not keep a second journal to answer "what moved?"

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

`https://ratio.marsh.build/` — the operations console. A Next.js application
with a route per resource, so a break, a NAV strike or a configuration version
can be sent to somebody rather than described. Sign-in is WorkOS AuthKit, not
Cognito; the browser never calls AWS, because the console's own server holds
the token and makes the call. AuthKit is the code path (#63). Vercel
Production has the env (#68 closed). Write-route actor binding landed
(#151): `applyEvent` / `ingest` / `admit` / `mark` / CreateBook / period
close record actor = WorkOS `sub`. Connect tokens accepted with
catalog scopes on `/v1`. `RATIO_DEMO_OPEN` defaults off on the
deployed demo. first-party Connect apps call ConnectApiUrl.
The demo API Lambda hydrates ScaleBucket `journals/`
(`RATIO_JOURNAL_BUCKET` / `RATIO_JOURNAL_PREFIX`) so
CreateBook and other writes survive a cold start. Hydrate
503 (“the journal is still hydrating”) is transient only
— accept-during-hydrate / orTransient still apply. The
40GB scale fold stays on Fargate ScaleTask. Scale still
uses ScaleBucket. Unused Cognito CloudFormation resources are
removed. Live leftovers remain on issue 22 — `DEMO_MEMBERS`
naming a live WorkOS `sub`, and WorkOS dashboard registration.
Do not
read this paragraph as production-complete.
The retired `https://ratio-ims.vercel.app` host still resolves;
`deploy.yml` refuses it as `CONSOLE_ORIGIN`.

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
  page work. ⚠️ **This stopped being true on 2026-08-13** — see *multi-view
  books* below, which adds three. ⚠ The `/scale` amendment between here and
  it re-asserts the bullet for ITS OWN change and is still right to: that
  screen lives in the binary's route table and touches no proto.

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

### Amendment, 2026-08-13 — multi-view books

A fund keeps more than one book of record. The accounting book recognises a
trade when it is struck; a settlement book recognises it when cash and stock
move, T+1 or T+2 later. Both are the fund's books, and until now this system
could hold only one of them.

**What changed.** A view is declared in the same content-addressed TOML the
rules are, and it overrides one thing: WHEN an entry is recognised. One journal,
folded more than one way, in one pass, from one prefix. The console grew a
book-of-record switch beside the currency, a `views/{view}` layer in every URL
that names a folded figure, and a reconciliation screen that shows what two
views disagree about entry by entry.

**What this is NOT, because two entries on the refusal list are adjacent to it
and both still stand:**

- Not **performance reporting and attribution**. A settlement-basis NAV is the
  same fold under a different recognition convention. Attribution is a return
  decomposed against a benchmark, and nothing here computes a return at all.
- Not **control-plane UI**. A view is declared in the configuration document a
  person already writes and `ratio approve` already promotes. There is still no
  editor, and the configuration screens still only READ.

**What it cost.** Three new RPCs — `ListViews`, `GetView`, `ReconcileViews` —
so the bullet above is now wrong and says so. Eleven figures moved off `Fund`
onto `View`, because a NAV that does not name its recognition convention is a
figure that does not say which question it answers. And the twenty-million-lot
memory figures in `HANDOFF.md` became ONE-VIEW figures: each view carries its
own lot book, and nothing has re-measured two.

**What it did NOT buy, and this is the honest half of the entry.** The console's
maintained projection still folds the whole journal with no cut, so ten
view-scoped screens and `ReconcileViews` REFUSE for anything but a `recorded`
view. `ratio strike --view` cuts and is correct; the screens are not. Merged in
that state deliberately — a refusal that names the gap is a smaller lie than the
recorded view's figures under another name.

#### The per-view fold, and the one decision it turns on

Written down before the code, because this split was started once and reverted:
a pending queue is easy to describe and the hard part is what BOUNDS it.

A settlement figure is determined by **three** things — the journal prefix, the
view, and the DAY. `AsOf<T>` carries one of them today. That is the whole
difficulty: a projection has no clock, so it cannot decide on its own whether a
trade placed on Tuesday has been recognised yet.

Three designs, and what kills two of them:

1. **Fold every view to the head, no cut.** ⛔ Vacuous.
   `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_gap` — folded to the end
   of history every view agrees, because everything eventually settles. This is
   what the projection does today and why it must refuse rather than answer.
2. **Retain the entries and filter by recognition day on read.** ⛔ Reintroduces
   holding the journal, which `follow`'s one streaming pass exists to avoid — it
   was three copies of a thing that only ever needed walking once.
3. **A monotonic cut per view, and a band of what is not yet recognised.** ⭐ The
   one that survives. Each `ViewFold` carries `recognised_through` and a
   `BTreeMap<Day, Vec<PendingEntry>>` of entries placed after it. A read at day
   `d` drains the band up to `d` and advances the cut; `d` earlier than the cut
   REFUSES rather than unfolding, which is `advance` using `max` one layer out.

⛔ **And what bounds the band is not the settlement lag on its own.** The lag
bounds it only once the cut is moving. A cold build over twenty years starting
from a cut of zero would put the WHOLE JOURNAL in the band — the failure mode
design 2 was rejected for, arrived at by a different road. So the fold advances
`recognised_through` to the highest recognition day it has SEEN as it reads, and
the band holds only what is placed beyond that. A journal is roughly
chronological, so that is days of trades rather than years.

`AsOf<T>` gains `view` and `through`, which is a compile-error-driven sweep
across four crates and is the point: with two views, `nav("abor")` and
`nav("ibor")` are otherwise structurally interchangeable.

Measured surface, so this is planned against the real thing: 24 field touches in
`fold` (`:504`), `positions` (`:707`), `fold_lots` (`:741`), `realized`
(`:922`), the accessors (`:940`–`:963`) and `nav` (`:989`); ~39 test call sites,
all of which take `UNDECLARED_VIEW` because the seeded books declare none.

⛔ The test that decides whether it is real is
`the_recorded_view_folds_exactly_what_the_projection_used_to`, compared against
`ratio_nav::strike`'s independent fold rather than against itself. And
`the_pending_queue_is_bounded_by_the_settlement_lag_not_by_the_journal`, which is
the paragraph above as an assertion.

**Landed, 2026-08-14, with one refinement the design note missed.** The frontier
that advances the cut is the highest **trade** day seen — the journal's own
clock — not the highest recognition day: advanced on recognition days, the band
drains the moment anything enters it (the latest-settling entry always settles
last), every view agrees at every prefix, and the vacuity of design 1 is rebuilt
one layer up. With the cut on trade days, "in flight" means exactly *struck but
not yet settled relative to the journal's own frontier*. `ratio reconcile A B`
shows the list; the reconciliation itself **refuses** when the per-entry effects
cannot sum to the NAV difference (integer translation does not distribute over a
sum), when a view has entries only IT cannot place, or when a view was declared
after the fold read past its history — and shows, rather than omits, what
NEITHER view can place. The ten view-scoped screens and `ReconcileViews` answer
per view; the projection-cannot-answer refusal is deleted.

And the deployed console taught one more rule on the same day: a refusal THROWN
out of a server component reaches production as `Minified React error #441` —
Next redacts the message — so the API's one explanatory sentence is exactly what
got hidden, on every view screen of the dual-basis demo fund. Every view-scoped
page now treats a `Refused` as a value and renders the sentence
(`withRefusal`); `error.tsx` had stated the rule all along.

### Amendment, 2026-08-14 — a plan screen, and the thirty-sixth RPC

The console can show how a NAV was computed: `/funds/{fund}/views/{view}/
strikes/{strike}/plan` draws the strike's steps, what each one reads, and what
the plans not taken would have cost. ⭐ `ExplainNavStrike`, and it is a custom
method on GET for the reason `:replay` already is.

**The premise this was asked under was false, and finding that out was the first
half of the work.** The request was to visualize "the nav strike query plan and
optimization" — and there is no query planner in this repository. No plan node
type, no rewrite registry, nothing that chooses a plan at runtime:
`ratio_nav::strike` folds the journal, `Projection::nav` reads maintained
totals, and a caller picks one by CALLING it. `Ratio.Plan` proves the two agree
and is not emitted into Rust at all. So this screen does not render a plan the
engine built; it writes down what the two paths already do and attaches the cost
the model proves or the fold measures. **That distinction is on the screen in as
many words**, because a diagram implying a planner that does not exist would be
checked by nothing and believed anyway — which is the shape of both false
premises `HANDOFF.md` opens with.

**What it cost.** One RPC, so the "No new RPC" bullet in the 08-13 amendment
above is wrong for the second time and says so here rather than in a test diff.
Four messages (`NavStrikePlan`, `PlanNode`, `PlanEdge`, `PlanDials`), three
accessors on `Projection`, and a counters struct on `NavFold`.

**What it is NOT, because the adjacent refusal is one word away:**

- **Not "the workload planner"** from the list below. That entry means
  scheduling compute across workers — `crates/ratio-exec`, a proved crate
  nothing in this repository depends on, and this change leaves it unwired.
  This is an EXPLAIN of one figure's derivation, which is the category
  `ratio explain ACCOUNT` and the `explain_figure` MCP tool have been in since
  before that list was written.
- **Not performance reporting and attribution.** The `/scale` amendment already
  drew this line: that entry means *investment* performance. This is the
  engine's own cost.
- **No write, no form, no button that spends money.** The two dials are a
  `router.replace` producing a query string on a GET — the same shape `/scale`
  used precisely so the no-forms fence did not have to be widened.
- **No new dependency.** The layout is arithmetic and the diagram is inline SVG
  over the design tokens `site/style.css` already publishes. A graph library
  would have been a client-only package in a five-package application whose
  colours would still have to be forced back through `tokens_test`.

⛔ **BOTH CURVES ARE ON IT AND STAY ON IT WHEN THE REJECTED STEPS COLLAPSE.**
The plans not taken are hidden by default — they are noise on a first read — but
the three costs are rendered whatever the dials say. `ratio bench` "reports two
curves and both must be quoted", and a plan screen showing only the flat one
would be that overclaim drawn as a diagram. `screens.test.tsx` asserts the strip
in both states, and the case was negative-tested by gating it on the toggle.

---

### Amendment, 2026-08-15 — the demo page becomes the front door, and captures a lead

`/scale` left the operator nav and became a standalone lead-gen page: one claim
("twenty million tax lots, folded cold in seventeen minutes, struck in twelve
microseconds"), an email gate, one button. A prospect who leaves an address gets
the fold run for them and a follow-up email whose permalink —
`/scale/runs/{id}` — shows every figure from THEIR run and the `ratio bench`
line that reproduces it byte for byte.

**Why this is not on the refusal list, said precisely because it is two words
away from two entries:**

- **Not "the client portal."** That entry is a customer self-service product —
  a signed-in surface where a CLIENT operates on their own fund. This is a
  marketing page with a mailing-list gate; nothing behind it belongs to the
  visitor, nothing authenticates, and the run report is a public read-only
  permalink. The 2026-08-11 amendment drew the same line for the ops console's
  auth, in the same words.
- **Not "CRM connectors."** The mailing list is objects under the demo's own S3
  prefix, written by the same `Store` the run lock uses — no Mailchimp, no
  HubSpot, no third-party anything. SES sends one transactional report per run,
  capped in code. Export is `aws s3 ls`; a CRM integration remains refused.

⚠️ The standing caveat, restated: none of this moves the wedge's one open gap —
a real customer's period reconciling. It widens the funnel's mouth; the funnel
still ends at a conversation, not a sign-up.

### Amendment, 2026-09-03 — the book-centric turn

The plan of record was a fund-admin console on a Cognito demo. The repository
has been something else for four merged PRs, and this file was not touched in
between. That is the same failure the refusal-list protocol exists for.

**What a Book is.** A Book is a journal plus content-addressed configuration.
It is the unit the console addresses. A Fund and a WorkOS organization are
optional layers, not parents: absence is independence, not an error.
`CreateBook` writes no fund and no organization. Kind selects the chart
`chart_for` writes and the chrome `screensFor` offers — one list, not a fork
of the kernel. `UNSPECIFIED` is the proto default, not a domain and not a
hidden fifth kind.

**Why it exists independently of Funds and Orgs.** A household, a construction
job, and an operating company are not funds that forgot to file. Filing every
book under a Fund so the old URLs kept working would have made "fund" mean
"directory", and a Personal book's NAV would have been a fake label on fund-ops
screens. The kernel already treated a directory as a book (`FileBook`). The
sidecar (`book.toml`) is the control-plane fact the directory never had to
carry.

**Which domains it serves.** Four kinds, named as the wire names them so a
kind the console offers cannot go unrecorded here again:

- `PERSONAL` — household: sheet, period P&L, net-worth bridge, cash-flow
  (the same-day amendment below), budget vs actual, named-loan roll-forward.
  Ingest: `bank-statement`, `loan-payment`, `brokerage-statement`,
  `brokerage-positions`. `[personal] currencies` is the election —
  empty is unset, not a silent USD. Lot relief stays unset until
  `[personal] lot_relief` elects the engines already on the book.

- `INVESTMENT` — fund administration as one kind of four, not the product:
  partner capital (beginning → contributions → distributions → allocated
  plugs → ending; CreateBook writes `[[partner_cut]]` LP 80 / GP 20;
  omitted table stays unset; never an equal split of book NAV),
  commitments / undrawn, period NAV roll-forward, then the
  ABOR warehouse. Does not file a Fund.
- `PROJECT` — a job, not an entity: original vs revised contract, awarded
  committed cost, remaining to spend, remaining to bill, collections vs
  billed. Unset until the journal can support them. Project `/billing`
  posts a cash application (`collect_receivable`). Ingest:
  `project-invoices` (job-cost / AP / progress-bill), `change-orders`,
  `purchase-orders`. The budget page does not forecast.
- `OPERATING` — an ordinary operating business: cash, AR, AP, operating
  revenue / expense, owner equity. Sheet, period income statement, and
  period cash-flow (the same-day amendment below) that tie to the trial
  balance. Investing stays unset — the chart has no PPE account.
  AR/AP aging at `/aging` (#117, the same-day amendment below) cites
  due-date buckets when the journal carries a due date and an
  application; unset when it cannot — never a fake current bucket.
  Buckets foot to the AR/AP control.

Period close at `/close` (the same-day amendment below) is on every
kind that wears a trial balance, including the fund operations
surface. Unset / provisional when the period is not closed — never a
fake closed period.

**The wedge is still the revenue path.** A paid shadow run on a real
customer's period is the first dollar. What changed is the *unit*. The
fund-admin Cognito demo is not the plan of record; independent Books
across four kinds are. The first engagement is still a fund
administrator's quarter, because that is who feels reconciliation pain
and who this file already named. That is a decision, written here rather
than inferred from the commit log.

**What this is NOT, because two entries on the refusal list are adjacent
to it and both still stand:**

- **Not "the client portal."** That entry is a customer self-service
  product — a signed-in surface where a CLIENT of a firm operates on
  their own fund (an LP on a capital account, a household as someone
  else's customer). The 2026-08-11 and 2026-08-15 amendments drew that
  line for console auth and for `/scale`. A Personal, Project, or
  Operating book is the operator of that book on the same ops console,
  behind the same fence (no approve button, no control-plane editor,
  kind selects chrome). Growing kinds on the operator surface is not a
  second product for clients.
- **Not "performance reporting and attribution."** The `/scale`
  amendment already drew this line: that entry means *investment*
  performance — returns, benchmarks, attribution against an index.
  Partner capital is who put money in and what remains callable.
  A NAV roll-forward is beginning → plugs → ending on the same journal.
  Neither computes a return. The multi-view amendment said the same
  about a settlement-basis NAV.

**Sign-in is WorkOS AuthKit**, on `https://ratio.marsh.build`. Cognito
is not the code path. AuthKit is in the code (#63). Vercel Production
has the env (#68 closed). Write-route actor binding landed (#151).
Connect tokens accepted with catalog scopes on `/v1`. API Gateway
JWT verifies Connect tokens on the Connect HTTP API (AuthKit
custom-domain issuer). `RATIO_DEMO_OPEN` defaults off on the
deployed demo. first-party Connect apps call ConnectApiUrl.
Unused Cognito CloudFormation resources are removed. Live
leftovers remain on issue 22 — `DEMO_MEMBERS` naming a live
WorkOS `sub`, and WorkOS dashboard registration. Do not
read this paragraph as production-complete, and do
not read a walk-through as demo-ready (#27).

**What it cost.** A Book proto resource; `book.toml`; `CreateBook` /
`ListBooks` / `GetBook`; kind-selected charts and ingest templates;
`screensFor` as the one chrome list; the AuthKit callback (`/sign-in`,
`/callback`) in place of Cognito's Hosted UI. The domain figures above
composed onto existing URLs — `/capital`, `/billing`, `/budget`,
`/sheet`, `/pnl`, `/cashflow`, `/aging`, `/close` — rather than minting
a chrome list per issue.

**What it did NOT buy, and this is the honest half.** Period close
(#114) is the Stage 1 door ("no effect on a closed period"); this
amendment does not land it and does not refuse it. The same-day
amendment below records that #114 / #116 landed. Envelope coaching,
bank OAuth, a credit score, a cash forecast, and a client portal stay
refused or on their own issues. The remaining wedge gap is still a
real customer's period.

### Amendment, 2026-09-03 — a household cash-flow statement, and nothing on the refusal list moved

Personal books already cited a sheet, a period P&L, a net-worth bridge, budget
vs actual, and a loan roll-forward. The bridge explained why **net worth**
moved. It did not answer where **cash** went. Operators rebuilt that story in a
spreadsheet beside a book that already tied.

What landed is a citeable period figure at `/books/{id}/views/{view}/cashflow`,
folded from the same journal as every other household figure (`filter=
cashflow-YYYY[-MM]`, the Loan-shaped window the bridge already uses). No new
RPC, no second store, no proto resource. Kind still selects the chrome from
one `screensFor` list — Fund, Project, and Investment books do not wear
the household statement. Operating composes onto the same URL (amendment
below); it does not fork `screensFor`.

**The split the chart can support.** Operating / investing / financing,
reconstructed from each non-cash account's period net (cash from an account
= −(debit − credit)), which is conservation itself:

- **Operating** — income, expenses (including loan interest), and Credit cards
  as working capital. A card charge is Dr expense / Cr cards and does not
  move cash; omitting the card plug would make a charge look like a cash
  outflow.
- **Investing** — Investments, the same account the net-worth bridge names as
  a transfer. Asset purchases stay unset: `chart_for(Personal)` has no
  purchase account distinct from that transfer.
- **Financing** — named-loan principal and draws (`[personal.loan]`, the same
  plug `/loans` cites) and Opening equity (household in/out).

A liability the book never named as a loan is a residual line an operator can
open, not silent absorption into financing. Beginning cash plus classified
movement equals ending cash when both cuts exist.

**Unset stays unset.** Beginning cash is unset when every account's beginning
is 0 (no dated prefix before the window). Ending cash is unset when nothing
dated has landed. An empty journal is not a measured $0.00 cash. Spending
down to zero is a real zero.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show beginning and ending cash for a month or year, the three activity
classes, and the conserved tie. It cannot show a cash forecast, envelope
coaching, bank OAuth, a credit score, a fire number, or a client portal.
Household budget vs actual is at `/budget`; loan schedules at `/loans`;
the net-worth bridge at `/bridge`. Period close is the same-day
amendment below. None of those are on the *Explicitly not building*
list, and this amendment adds none of them.

### Amendment, 2026-09-03 — an operating-company cash-flow statement, and nothing on the refusal list moved

OPERATING books already cited a balance sheet, a period income statement, and
a trial balance (#108). PERSONAL books already cited a period cash-flow
statement at `/cashflow` (#98). An ordinary operating company still could not
answer where **cash** went for the same period its P&L covers. Operators
rebuilt that story in a spreadsheet beside a book that already tied.

What landed is the same citeable period figure at
`/books/{id}/views/{view}/cashflow`, folded from the same journal
(`filter=cashflow-YYYY[-MM]`). No new RPC, no second store, no proto
resource. Kind still selects the chrome from one `screensFor` list — the
screen is added to `OPERATING_SCREENS`. Fund, Project, and Investment books
do not wear it. This is not a bank reconciliation and not a forecast.
Aging is the same-day amendment below; period close is the same-day
amendment after that.

**The split the Operating chart can support.** Operating / investing /
financing, reconstructed from each non-cash account's period net (cash from
an account = −(debit − credit)), which is conservation itself:

- **Operating** — operating revenue, operating expenses, Accounts receivable
  and Accounts payable as working capital. An invoice is Dr AR / Cr revenue
  and does not move cash; a vendor bill is Dr expense / Cr AP. Omitting
  those plugs would make accrual look like cash.
- **Investing** — unset. `chart_for(Operating)` has no PPE / securities
  account. A 0.00 investing class would invent one.
- **Financing** — Owner equity (contribution and draw). There is no
  `[personal.loan]`. A chart without Owner equity leaves financing unset,
  not a silent 0.00 draw.

A move the chart cannot name is a residual line an operator can open, not
silent absorption. Beginning cash plus classified movement equals ending
cash when both cuts exist.

**Unset stays unset.** Beginning cash is unset when every account's beginning
is 0 (no dated prefix before the window). Ending cash is unset when nothing
dated has landed. An empty journal is not a measured $0.00 cash. Spending
down to zero is a real zero.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show beginning and ending cash for a month or year, the operating and
financing classes the journal supports, and the conserved tie. It cannot
show a bank reconciliation, a cash forecast, payroll, tax filing,
inventory/COGS, payment initiation, bank OAuth, or a client portal.
Sheet / P&L stay on #108; aging is the same-day amendment below; period
close is the same-day amendment after that. None of those are on the
*Explicitly not building* list, and this amendment adds none of them.

### Amendment, 2026-09-03 — operating AR/AP aging by due-date bucket, and nothing on the refusal list moved

OPERATING books already cited a balance sheet, a period income statement,
a trial balance, and a period cash-flow statement (#108 / #118). AR and AP
were control-account totals. They did not say which invoices and bills
were current, past due, or undated. Operators rebuilt that story in a
spreadsheet beside a book that already tied.

What landed is a citeable as-of figure at
`/books/{id}/views/{view}/aging`, folded from the same journal
(`filter=` as-of, the sheet-shaped cut). The journal carries optional due
date and open-item application. `OperatingAging` is a fold, not a second
store — there is no OperatingAging resource. Kind still selects the
chrome from one `screensFor` list — the screen is added to
`OPERATING_SCREENS`. Fund, Project, and Investment books do not wear it.
Project `/billing` is one job's billed/earned/collections, not
entity-wide aging. This is not a bank reconciliation, not payroll, not
inventory/COGS, and not payment initiation.

**The buckets the journal can support.** Current / 1–30 / 31–60 / 61–90 /
older than 90 / undated, remaining open items against the AR and AP
controls:

- **Current** — due on or after the as-of day.
- **1–30 / 31–60 / 61–90 / older** — days past due.
- **Undated** — remaining with no due date. A missing due date is
  undated, not current.

Dated buckets plus the undated residual equal the control on the same
cut.

**Unset stays unset.** A schedule is unset when remaining items have no
due date, or a collection/payment does not name the invoice or bill it
applies to — no FIFO, no equal split, no silent current bucket. Empty
dated buckets on a set schedule are `"0"` (nothing in that window).
`undated` empty is no residual line. An empty journal is not a schedule
of zeros.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show aged AR and AP that foot to the control, an undated residual that
is not current, and unset when the journal cannot support the cut. It
cannot show a bank reconciliation, payroll, tax filing, inventory/COGS,
payment initiation, bank OAuth, or a client portal. Sheet / P&L stay on
#108; cash-flow stays on #118; period close is the same-day amendment
below. None of those are on the *Explicitly not building* list, and this
amendment adds none of them.

### Amendment, 2026-09-03 — a citeable period close, and nothing on the refusal list moved

Stage 1 named "no effect on a closed period" as a required check. The rule
compiler never asked. `FileBook::trial_balance` folded the whole journal.
The sheet showed `surplus = income + expenses` as the residual that makes
the books foot **while they have not closed**, and said so. There was no
close verb, no closed-through day, and no roll into equity.

What landed is a citeable close boundary per Book / view / period, and a
retained-earnings roll-forward, for Personal, Investment, Project,
Operating, and the fund operations surface. Kind still selects the chrome
from one `screensFor` list. Operating rolls into retained earnings (25),
not Owner equity — the same distinction Personal keeps between Opening
equity and the residual. Operating `/cashflow` stays the period cash
statement (#118); this amendment does not change that classification.

**The door is on the journal, not the rule set.** A `CloseRecord` lives on
`Plane::Closes` (`closes.jsonl`), beside the journal the way an explanation
does. The closing *posting* is a conserved journal entry — that is what
makes the post-close trial balance cite the same legs as the P&L. The
record is the evidence: view, closed date, journal prefix, configuration
digest, actor, time. `Journal::append` refuses a dated entry on or before
any view's closed-through day. Append-only storage is not this.
`Ratio.Close`. `//tla:period_close_check`. The probe
`//tla:backdated_post_check` flips the door off and
`AClosedPeriodRefusesABackdatedPost` goes red.

**The operator verb is `ratio close --through YYYY-MM-DD`.** Same fence as
`ratio accept` and `ratio approve`. The console is read-only evidence at
`/books/{id}/views/{view}/close` (`filter=close-YYYY[-MM]`, the Loan-shaped
window the bridge already uses) plus `ListPeriodCloses` / `GetPeriodClose`.
No write RPC.

**Unset stays unset.** `[close] equity_destination` names the dest; a
missing key refuses the close rather than defaulting to Opening equity or
Funding. A period with no income or expense still closes (the door holds)
and leaves `closing_entry` / `surplus` absent — not a measured zero. A
missing beginning prefix leaves the roll-forward unset. The proto surplus
field is a string so empty is empty.

**Post-close statements agree.** The P&L Activity fold skips the closing
entry, so March still shows March. The sheet as-of includes it, so
temporaries are zero and retained earnings holds the surplus. The next
period does not silently include the prior I/E. An open period may preview
unclosed surplus; it says provisional rather than presenting the period as
closed.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show a close against a named view, prefix and digest, a back-dated post
refused, a roll-forward that ties to the period P&L and the post-close
trial balance, and unset when the journal cannot support a close. It
cannot show a control-plane configuration editor, a client portal,
performance reporting, a tax-filing workflow, or a general
workflow engine. Aging is the same-day amendment above. The close may
remain CLI/API.

### Amendment, 2026-09-03 — wash sales in the lot-relief engine, and nothing on the refusal list moved

The lot engine relieved under the method a book elected. A repurchase
inside a jurisdiction's window did not disallow the loss or attach it
to the replacement. Conservation held, the trial balance tied, and the
realized gain was somebody else's — the figure with no counterparty.
This file named tax lots and cost basis as built and said nothing
about wash sales.

What landed is the wash window as a term of the configuration, the
write onto an open replacement, and the fold that applies both
halves, on main via #133 (Lean / TLA) and #138 (Rust).

**The window is a jurisdiction's number, not a constant in the
arithmetic.** `RuleSet.wash_window_days: Option<i64>` is resolved per
entry the way `lot_method` and `long_term_days` are. A fund that wants
US wash writes `wash_window_days = 30`. `Ratio.Lots.Wash`.
`//tla:wash_engine_check`. The window reaches both ways. The probe
`//tla:unattached_wash_check` flips the write off and
`ADisallowedLossIsOnTheReplacement` goes red.

**Attach, then re-rank.** `attachTo` / `Holding::attach` writes the
disallowed amount onto an open replacement — never a lot the sale
took. Ranked methods re-key so later HIFO/LOFO sees the new basis;
FIFO keeps sequence order. A negative deferral is refused (that would
be washing a gain). `a_gain_is_never_washed`.
`disallowing_without_attaching_destroys_the_loss`.

**The fold plans, then writes.** After a sale, match oldest in-window
open lots; after a purchase, match `pending_wash` against the new
lots. A split that will not divide refuses before any write lands.
Holding-period transfer is the US rule already named in Lean
(`replacementAcquired`); the replacement's acquisition date becomes
the original lot's, not the repurchase's.

**Unset stays unset.** A book that never elected a window has no
window — not a silent 30. Silence does not serialize (digest-stable).
A silent 30 would have restated every in-window loss on every
existing book. A negative window is refused.

**What a walk-through can and cannot show** (demo readiness, #27). It
can elect a wash window, attach a deferred loss to an open
replacement, and show a later sale taking the adjusted basis. It
cannot show `WashRestatement` (a struck figure that a later repurchase
moved must not change silently), a wash flag on the console or in the
demo, or a non-US holding-period variant. Those remain on #5. This
amendment does not close #5. MinTax stays on #9. None of those are on
the *Explicitly not building* list, and this amendment adds none of
them.
*(WashRestatement as a citeable record, and the non-US holding-period
variant as an election, are later amendments.)*

### Amendment, 2026-09-03 — MinTax is a ranking at a price, not a Method

Tax lots left the refusal list in the 48 hours after this file was
written. Six ordering methods and a holding-period split shipped then.
MinTax was modelled in `Ratio.Lots.Methods` as the shape that is **not**
an ordering — `a_tax_minimising_method_is_not_a_function_of_the_lots` —
and deliberately not implemented, because adding it as a `Method` /
`Order` variant is the mistake that file exists to prevent.

What landed is the decision surface and the engine, as its own election:

- Lean: `Ratio.Lots.MinTax`. Ranking takes a sale **price**. Close
  bases flip (10 short / 12 long: at 50 take the long lot, at 5 take
  the short). Far bases do not. No `Order` reproduces both answers.
  A price that will not divide is refused. Missing dates refuse.
- TLA: `//tla:mintax_engine_check`. The probe
  `//tla:sort_and_walk_mintax_check` treats MinTax as a Method and
  `TheLotTakenMinimisesTax` goes red.
- Rust: `min_tax_short_weight` on the rule set. `None` means nobody
  said — not a silent 2. `lot_method = "min_tax"` stays refused.
  Electing both `lot_method` and `min_tax_short_weight` refuses.
  The fold ranks at the cash posting's per-unit proceeds.

**What this is NOT, because two shapes named in #9 stay named:**

- Not **specific identification**. That is a per-sale selection the
  client supplies, possibly from the middle of a holding. Still
  modelled, not an engine. *(superseded by the next amendment.)*
- Not **average cost**. That pools the holding and carries a rounding
  decision no ordering has. Still modelled, not an engine.
  *(superseded two amendments later.)*

Nothing on the *Explicitly not building* list moved. Tax lots already
left it. This extends that engine with the one method that cannot be a
sort. Issue #9 stays open until those two siblings land.
Wash sales stay as the previous amendment recorded them; #5 stays
open for `WashRestatement` and the console cite.

### Amendment, 2026-09-03 — SpecID is a named selection, not a Method

Specific identification was modelled in `Ratio.Lots.Methods` as the
shape that is **not** an ordering — the client names the lots, possibly
from the middle — and `selectFirst` put those names first. That walk
cannot refuse a contradictory instruction. Adding SpecID as a
`Method` / `Order` / `lot_method` variant is the mistake that file
exists to prevent.

What landed is the decision surface and the engine, as an attested
per-sale choice:

- Lean: `Ratio.Lots.SpecId`. Named lots are relieved exactly. Unknown,
  overspecified, insufficient, duplicate and unnamed selections refuse.
  No `Order` takes the middle lot of 10 / 40 / 70. `selectFirst` of
  nothing is FIFO; SpecID of nothing refuses. Conservation is inherited.
  A husk is still a husk.
- TLA: `//tla:specid_engine_check`. The probe
  `//tla:sort_and_walk_specid_check` treats SpecID as FIFO and
  `TheLotTakenIsTheOneNamed` goes red.
- Rust: `JournalEntry.identified_lots: Option<Vec<u64>>`. `None` means
  this sale is not SpecID. `Some([])` is elected and unnamed — refuse,
  not FIFO. `lot_method = "specific_id"` stays refused. Naming lots on
  a min-tax book refuses (two answers). The fold reads the names on
  the entry.

**What this is NOT, because one shape named in #9 stays named:**

- Not **average cost**. That pools the holding and carries a rounding
  decision no ordering has. Still modelled, not an engine.
  *(superseded by the next amendment.)*

Nothing on the *Explicitly not building* list moved. Issue #9 stays
open for average cost. WashRestatement and the console cite stay on
#5. This amendment closes nothing.

### Amendment, 2026-09-03 — Average cost is a pool, not a Method

Average cost was modelled in `Ratio.Lots.Methods` as the shape that is
**not** an ordering — the holding is pooled, so "which lot" is not a
question it answers, and the figure divides. Adding it as a
`Method` / `Order` / `lot_method` variant is the mistake that file
exists to prevent. `average_cost_is_not_a_lot_walk` is the theorem;
10 / 40 / 70 pools to 40, which equals the middle lot by coincidence.

What landed is the decision surface and the engine, as its own election:

- Lean: `Ratio.Lots.AverageCost`. 10 / 20 / 60 pools to 30 — a basis
  no lot carries and no `Order` gives up. The remainder is one pooled
  lot, not the other lots left intact (the 10 / 40 / 70 coincidence
  with SpecID of lot 2: same taken cost, different remainder). A
  figure that will not divide is refused. A husk is absorbed into the
  pool. Conservation is arithmetic.
- TLA: `//tla:average_cost_engine_check`. The probe
  `//tla:sort_and_walk_average_cost_check` treats average cost as a
  Method and `TheBasisTakenIsThePooledBasis` goes red.
- Rust: `average_cost: Option<bool>` on the rule set. `None` means
  nobody said — not a silent true. `Some(false)` is refused at read;
  omit the field. `lot_method = "average_cost"` stays refused.
  Electing both `lot_method` and `average_cost`, or both min-tax and
  average cost, refuses. Naming lots on an average-cost book refuses
  (two answers). The fold pools; treating it as
  `held.relieve(method, …)` is the TLA probe.

**What this is NOT, because the engine is not the walk-through:**

- Not a **console / proto UI** for electing the pool, nor MinTax or
  SpecID screens. Those stay leftovers on #9.
  *(the console cites are a later amendment.)*
- Not a **holding-period category rule** for the pool (US single vs
  double category). A shared acquisition date is carried when every
  lot agrees; mixed or missing dates stay unset. No category is
  invented.
- Not **WashRestatement**, a wash flag on the console, or a non-US
  holding-period variant. Those stay on #5.
  *(WashRestatement as a citeable record, and the non-US
  holding-period variant as an election, are later amendments.)*

Nothing on the *Explicitly not building* list moved. Issue #9 stays
open for the UI cite and the holding-period leftover. This amendment
does not close #9. It does not close #5.

### Amendment, 2026-09-03 — WashRestatement is a citeable record

A wash sale can land after a NAV has already reported the loss. The
strike read the whole journal, pinned its prefix, and computed the
only correct answer available at the time. Days later a repurchase
reaches backwards and the same sale's realized gain is a different
number — for a period that is closed. Conservation holds, the trial
balance ties, the digest reproduces. The figure that moved is the
one with no counterparty.

The obligation is not "do not let it change". The tax rule genuinely
reaches back, and an engine that held March fixed would report a
number the rule disagrees with. The obligation is that a figure
somebody was paid on must not change **silently**.

What landed is the record, not a console screen:

- Lean: `Ratio.Lots.WashRestatement`. A strike of a realized gain
  carries a citeable prefix identity. Qualification is written at
  strike time iff the wash window is still open. Restatement is a
  new record that cites that identity and the original number —
  `Ratio.Period`'s "new kind of thing", for this rule. Rewriting
  the struck figure in place keeps the id and changes the number;
  that is the defect, named so it cannot be confused with
  restatement. No `Order` / `Method` / `lot_method = "wash"`
  variant. `a_struck_gain_that_moved_says_so`.
- TLA: `//tla:wash_restatement_check` already said a moved figure
  was qualified or restated. It now also says the restatement cites
  the strike it supersedes, and that the struck figure is not
  rewritten. The probe `//tla:silent_wash_restatement_check` skips
  the record and `AStruckGainThatMovedSaysSo` goes red. The probe
  `//tla:mutating_wash_restatement_check` overwrites the figure and
  `AStruckFigureIsNotRewritten` goes red.
- Rust: `strike_gain` / `restate` / `rewrite_in_place` on the lot
  surface, matching the proofs. `Projection::open_wash_windows`
  is what a strike reads to qualify — leftovers whose window is
  still open on a named day, not a leftover that outlived the
  window. A wrapped window-close is refused. The posted gain in
  the journal is not rewritten; a restatement cites, it does not
  mutate a historical strike digest.

**What this is NOT, because two leftovers stay named on #5:**

- Not a **wash flag on the console or in the demo**. Lot proto and
  the operations screens do not grow a field for this. Adding one
  is a cite, not a restatement, and it is still ahead.
  *(the console wash-flag cite is a later amendment.)*
- Not a **non-US holding-period variant**. `replacementAcquired`
  is the US transfer already named in `Ratio.Lots.Wash`. A
  jurisdiction that does not transfer the period is a different
  rule; it does not fall out of this record and is not invented
  here.
  *(the non-US holding-period variant is an election — the next
  amendment.)*

Nothing on the *Explicitly not building* list moved. This
amendment does not close #5. It does not close #9.

**What a walk-through can and cannot show** (demo readiness, #27).
It can qualify a strike taken while a wash window is open, and
produce a restatement that cites that strike when a later
repurchase moves the figure. It cannot show a wash flag on the
console or in the demo, a non-US holding-period variant, or a
lot-relief UI screen. Those remain on #5 (wash leftovers) and #9
(engine UI cites).
*(the console wash-flag cite is a later amendment.)*
*(the non-US holding-period variant is an election — the next
amendment.)*

### Amendment, 2026-09-03 — the non-US holding-period variant is an election

`Ratio.Lots.Wash.replacementAcquired` is the US rule: the
replacement's acquisition date for holding-period purposes becomes
the original lot's, not the repurchase's. A jurisdiction that does
not transfer the period is a different rule. Assuming the US
transfer everywhere classifies that disposal at the US rate.
Conservation holds, the trial balance ties, the deferred loss
still attaches. The figure that goes wrong is the RATE — short
or long — which no reconciliation reaches.

What landed is the election, as its own shape:

- Lean: `Ratio.Lots.WashHolding`. `PeriodRule` is `transfer` or
  `keep`, not an `Order` / `Method` / `lot_method = "wash"`.
  Day 0 / repurchase 300 / dispose 400 / threshold 365: transfer
  is long, keep is short. Same basis. Assuming
  `replacementAcquired` when the election is `keep` is the named
  defect. `choosing_the_wrong_rule_flips_the_rate`.
- TLA: `//tla:wash_holding_check`. The probe
  `//tla:universal_us_transfer_check` hardcodes the US transfer
  and `TheReplacementKeepsItsOwnDate` goes red.
- Rust: `wash_keep_holding_period` on the rule set. `None` means
  nobody said — not a silent keep. Silence leaves the US transfer
  that already landed. `Some(true)` elects keep. `Some(false)` is
  refused at read; omit the field. Electing keep without
  `wash_window_days` refuses. `lot_method = "wash"` stays refused.

**What this is NOT, because one leftover stays named on #5:**

- Not a **wash flag on the console or in the demo**. Lot proto
  and the operations screens do not grow a field for this. Adding
  one is a cite, not this election, and it is still ahead.
  *(the console wash-flag cite is a later amendment.)*

Nothing on the *Explicitly not building* list moved. This
amendment does not close #5. It does not close #9. MinTax /
SpecID / average-cost UI cites stay on #9.

**What a walk-through can and cannot show** (demo readiness, #27).
It can elect `wash_keep_holding_period = true` beside a wash
window, attach a deferred loss without transferring the period,
and show a later sale classified from the replacement's own date.
Silence stays unset, not a silent keep. It cannot show a wash
flag on the console or in the demo, or a lot-relief UI screen.
Those remain on #5 (console cite) and #9 (engine UI cites).
*(the console wash-flag cite is the next amendment.)*

### Amendment, 2026-09-03 — the console cites the wash election

The engine halves of #5 were already on main: the wash window
(`wash_window_days`; silence stays unset, not a silent 30),
`WashRestatement` as a citeable record, and the non-US keep
(`wash_keep_holding_period = true`; unset stays the US transfer;
`Some(false)` is refused at read). A live walk-through still
could not point at the election the way it can point at lot
method. Built named the engine; cannot-show still said the
console could not show a wash flag. That honesty gap was the
product leftover.

What landed is the cite, not new arithmetic:

- The fund lot-terms screen reads `wash_window_days` and
  `wash_keep_holding_period` from the configuration already on
  `RuleSet`. Unset stays unset — the days are not printed when
  nobody wrote a window, and keep is not printed as a third
  meaning. `Some(true)` elects keep. `lot_method = "wash"` stays
  refused.
- The proto / wire / fixtures / `fields_test` needles / rendered
  screens hold the phrases, so the rows cannot silently
  disappear. The demo seed writes `wash_window_days = 30` on the
  recon books and leaves keep unset (US transfer).

**What this is NOT, because #9 leftovers stay on #9:**

- Not a **tax-lot planner UI**, and not MinTax / SpecID /
  average-cost console cites. Those stay leftovers on #9.
  *(the console cites are a later amendment.)*

Nothing on the *Explicitly not building* list moved. This
amendment closes the #5 leftover. It does not close #9.

**What a walk-through can and cannot show** (demo readiness, #27).
A fund admin walk-through can point at the wash window on the
fund lot-terms screen the way it can point at lot method, and
can point at keep when a book writes it. Silence stays unset, not
a silent 30, and unset keep stays the US transfer. It cannot show
lot-relief UI screens for MinTax, SpecID, or average cost. Those
remain on #9.
*(the console cites are the next amendment.)*

### Amendment, 2026-09-04 — the console cites the MinTax, SpecID, and average-cost elections

The engine halves of #9 were already on main: MinTax as a ranking
at a price (`min_tax_short_weight`; unset stays unset, not a
silent 2; `lot_method = "min_tax"` stays refused), SpecID as a
named selection (`identified_lots` on the sale; unnamed or
overspecified refuse; `lot_method = "specific_id"` stays
refused), and average cost as a pool (`average_cost = true`;
unset stays unset, not a silent true; `lot_method =
"average_cost"` stays refused). A live walk-through still could
not point at those elections the way it can point at lot method
and the wash window. Built named the engine; cannot-show still
said the console could not show the cites. That honesty gap was
the product leftover.

What landed is the cite, not new arithmetic:

- The fund lot-terms screen reads `min_tax_short_weight` and
  `average_cost` from the configuration already on `RuleSet`,
  sharing the elected-term claim with lot method and wash.
  Unset stays unset — the weight is not printed when nobody
  wrote one (not a silent 2), and the pool is not printed as a
  silent true. `Some(true)` elects the pool. The two cannot
  share a configuration with each other or with `lot_method`.
- SpecID is per-sale, not a fund term. The journal-entry page
  cites `identified_lots` when a sale carries names, and says
  so when it does not. Unnamed (`Some([])`) is elected and
  refuses — not FIFO.
- Proto / wire / fixtures / `fields_test` needles / rendered
  screens hold the phrases, so the rows cannot silently
  disappear. The demo seed writes `min_tax_short_weight = 2`
  on Calderwood (empty, so the weight cannot restate a sale),
  `average_cost = true` on Kestrel (a book that cannot share
  the pool with lot_method or min-tax), and a zero-gain SpecID
  sale on the recon books so a walk-through can open the
  names. Harbourline leaves min-tax and the pool unset.

**What this is NOT, because one leftover stays named on #9:**

- Not a **holding-period category rule** for the average-cost
  pool (US single vs double category). A shared acquisition
  date is carried when every lot agrees; mixed or missing
  dates stay unset. No category is invented.
- Not a **tax-lot planner UI**. This is a field cite, the same
  shape as lot method and wash.

Nothing on the *Explicitly not building* list moved. This
amendment does not close #9. The pooled holding-period leftover
stays.

**What a walk-through can and cannot show** (demo readiness, #27).
A fund admin walk-through can point at the min-tax weight on
Calderwood's lot-terms screen and the average-cost pool on
Kestrel's, the same way it points at lot method and the wash
window, and can point at named lots on a SpecID sale's journal
entry. Silence stays unset — not a silent 2, not a silent true,
not a silent FIFO. It cannot show a pooled holding-period
category (mixed dates stay unset). That remains on #9.
*(the pooled holding-period category is a date — a later
amendment.)*

### Amendment, 2026-09-04 — Phase-four marketplace begins as WorkOS Connect scopes

The destination page still sells a third-party marketplace. The temptation
is to grow `ratio watch` / `Console` until every portal, feed, and vendor
app has a kernel method. That is RPC sprawl. Breadth belongs in
[WorkOS Connect](https://workos.com/docs/authkit/connect) applications that
present a scoped token. The core stays the journal, the lots, the
statements, and AuthKit tenancy.

**What this amendment freezes.** The resource:action catalog in
[`docs/connect-scopes.md`](docs/connect-scopes.md)
([#150](https://github.com/mattmarshall/ratio/issues/150)). A Connect app
is granted a subset of those scopes, and only after the AuthKit subject is
in the book's membership. A scope is not a new RPC. A missing kernel door
stays missing.

**Hard non-scopes, by absence.** `rules:approve`, `config:promote`, and
portal impersonation are not in the catalog and must not be added. Payment
initiation is a separate compliance app, if it is ever built. The fence
that keeps a model from approving a rule is the same fence: a write that
must be a person is not a scope somebody could later relax.

**What this is NOT, because the adjacent refusals are one word away:**

- **Not live provider OAuth.** API Gateway JWT verifies Connect
  tokens on ConnectApiUrl. Live OAuth is leftover #22. This file
  does not close #150.
- **Not a reference Connect app.** A read-only books + statements skeleton
  is leftover on #150.
- **Not the client portal, bank OAuth, CRM connectors, tax e-file, a
  vendor portal, or a waterfall.** Those stay Connect-apps or stay
  refused. Nothing on the *Explicitly not building* list moved.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show the frozen table and the refuse list. It cannot show a third-party
token opening a book. Live activation leftovers remain on #22.

### Amendment, 2026-09-04 — the partner allocation cut is named weights, not 1/N

PLAN / `/capital` already said allocated income / expense / unrealized
stay **unset** without a partner cut — never a silent equal split of
book NAV. Capital accounts and commitments were Built (#70 / #82 /
#102). The cut that fills those plugs was the gap (#180).

What landed is the cut, as config plus journal facts:

- Lean: `Ratio.Partners.Cut`. A cut is named weights, not a partner
  count. `None` and an empty list are unset (`no_cut_is_unset`). A
  figure that will not divide is refused rather than rounded
  (`a_slice_is_exactly_pro_rata`). When every slice divides, the
  shares sum to the figure. A standing special replaces the default
  for one kind. A journal fact is an exact amount; `Some []` refuses
  the SpecID way.
- Rust: `[[partner_cut]]` / `[[special_allocation]]` on the rule set.
  Empty is unset, not 1/N. Zero or duplicate weights refuse at read.
  `allocate` checks the product before the divisibility guard.
  `JournalEntry.special_allocations` carries per-entry exact amounts;
  `Some([])` refuses at the door.
- `/capital` fills allocated lines only when GetBook cites a cut and
  the figure divides. CreateBook does not write a cut.

**What this is NOT, because leftovers stay named on #180:**

- Not a **fold of per-entry specials into the `/capital` plugs**.
  Standing config specials apply; journal facts are stored and
  refused when unnamed, and the statement does not yet walk them.
- Not a **seeded demo cut**. CreateBook(Investment) still writes no
  `[[partner_cut]]`. The live demo's allocated plugs stay unset.
- Not an **LP portal, K-1 pack, or waterfall**. Those stay Connect
  (#161 / #150). This amendment does not close #181.

Nothing on the *Explicitly not building* list moved. This amendment
does not close #180. It does not close #181.

**What a walk-through can and cannot show** (demo readiness, #27).
It can write `[[partner_cut]]` LP 80 / GP 20, cite allocated income
of a dividing figure on `/capital`, and leave plugs unset when the
table is omitted or the figure will not divide. It cannot show a
seeded demo cut, a fold of per-entry specials, IRR, a waterfall, an
LP portal, or K-1 packaging. Those remain on #180 (cut leftovers)
and Connect.

### Amendment, 2026-09-04 — a Personal bank-feed Connect app, and the grant path still does not open

[#165](https://github.com/mattmarshall/ratio/issues/165) asked for bank-feed
OAuth into Personal books without stuffing OAuth into `ratio watch`. The
catalog already said bank OAuth stays a Connect app. What landed is that
app as a sibling tree, not a kernel method.

**What this amendment records.** [`connect/bank-feed/`](connect/bank-feed/)
is a first-party WorkOS Connect OAuth application for `BookKind` PERSONAL.
It declares `books:read`, `statements:read`, and `journals:post` — the
frozen names, not the issue body's stale alias `journal:append`. Feed rows
map to CreateBook(Personal) templates already on the book
(`living_expense`, `household_income`, `card_charge`, transfers, `spend_*`,
`receive_income`). `journals:post` is allowlisted per `client_id`; an empty
allowlist refuses every post. A dated row on or before closed-through
refuses the batch. Instantiated legs must conserve in every currency;
`[USD +100, EUR −100]` is not a posting. Money is minor units, split on
the point. No new `Method` / `Order` / `lot_method` variant. PERSONAL
chrome is unchanged — `screensFor` is not forked.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect tokens
  on ConnectApiUrl. `deliver()` still refuses. Live OAuth is leftover
  #22 / #150. This file does not close #150.
- **Not live bank OAuth.** No Plaid / MX / TrueLayer token. The mapper
  accepts a normalized row. Provider wiring stays leftover on #165.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `journals:post` and does not open the door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.** The
  append ACL and the closed-through gate stay where they are.

Nothing on the *Explicitly not building* list moved. Client portal, CRM,
tax e-file, vendor portal, and waterfall stay Connect-apps or stay
refused. This file does not close #165 — grant path and live provider
remain. It does not start #166 or #168.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show a fixture expense mapping to `living_expense`, a closed March
refusing a 15 March row, and `journal:append` being rejected as a scope.
It cannot show a Connect token opening a book or a live bank login.

### Amendment, 2026-09-04 — a Personal tax-pack Connect app, and e-file still does not happen here

[#166](https://github.com/mattmarshall/ratio/issues/166) asked for household
tax-pack export from lot and wash cites without stuffing packing into
`ratio watch` or growing a CPA portal in core. The catalog already said
tax e-file stays a Connect app. What landed is that app as a sibling
tree, not a kernel method.

**What this amendment records.** [`connect/tax-pack/`](connect/tax-pack/)
is a first-party WorkOS Connect OAuth application for `BookKind` PERSONAL.
It declares `lots:read`, `statements:read`, and `config:read` — the frozen
names. It is read-only relative to the journal: `journals:post` is not
requested. Lot, wash, and lot-terms cites become an 8949-ish CSV plus
companion sheets (`unclassified.csv`, `wash_cites.csv`, `lot_terms.csv`).
Wash is a `WashRestatement` cite (code `W` + adjustment); the strike is
not rewritten. Unset elections stay unset. No new `Method` / `Order` /
`lot_method` variant. PERSONAL chrome is unchanged — `screensFor` is not
forked. **a Personal tax-pack Connect app** is the Built phrase this
amendment adds.

**Holding-period category.** When acquisition dates agree, the threshold
day is long-term (`the_threshold_day_is_long_term`). When they disagree
— the average-cost pool leftover on
[#9](https://github.com/mattmarshall/ratio/issues/9) — the category stays
unset. The pack does not invent FIFO's oldest date or two Form 8949
boxes. Those rows land on the unclassified companion with the ambiguity
named. PR #154 may still be open; this amendment does not close #9.
*(the kernel category is `Ratio.Lots.PoolPeriod` — a later
amendment.)*

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect tokens
  on ConnectApiUrl. `fetch_cites()` still refuses. Live OAuth is leftover
  #22 / #150. This file does not close #150.
- **Not IRS e-file, not a CPA portal, not MeF.** `submit()` refuses.
  Live submission stays leftover on #166.
- **Not a pooled holding-period category rule in the kernel.** Mixed
  dates stay unset here; the engine leftover stays on #9.
  *(superseded by a later amendment.)*
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `lots:read` and `config:read` and does not open the door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**

Nothing on the *Explicitly not building* list moved. Client portal, CRM,
tax e-file, vendor portal, and waterfall stay Connect-apps or stay
refused. This file does not close #166 — grant path, live CPA/IRS
submission, and the #9 leftover remain. It does not close #165. It does
not start #168. It does not close #9.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show a fixture disposal mapping to an 8949 SHORT or LONG row, a mixed-date
pool landing unclassified rather than inventing FIFO's oldest date, and
`lot_method = "wash"` being rejected. It cannot show a Connect token
opening a book, an IRS e-file, or a pooled holding-period category when
dates disagree.
*(the kernel category is a date — a later amendment.)*

### Amendment, 2026-09-04 — a Personal net-worth goals Connect app, and the grant path still does not open

[#168](https://github.com/mattmarshall/ratio/issues/168) asked for
net-worth goals and what-if scenarios on Personal books without stuffing
goal-tracking into `ratio watch` or growing a kernel RPC. Sheet, bridge,
and cash-flow stay core. The catalog already said breadth is a Connect
app. What landed is that app as a sibling tree, not a kernel method.

**What this amendment records.** [`connect/goals/`](connect/goals/)
is a first-party WorkOS Connect OAuth application for `BookKind` PERSONAL.
It declares `statements:read` and `journals:post` — the frozen names,
not the issue body's stale alias `journal:append`. Goal progress cites
the sheet net worth already in core against a named target. Unset stays
unset. A scenario overlays discrete CreateBook(Personal) templates
already on the book (`living_expense`, `household_income`, `card_charge`,
transfers, `spend_*`, `receive_income`). Scenario journals post only
when the household administrator opts in. `journals:post` is allowlisted
per `client_id`; an empty allowlist refuses every post. A dated opt-in
post on or before closed-through refuses the batch. Instantiated legs
must conserve in every currency; `[USD +100, EUR −100]` is not a
posting. Money is minor units, split on the point. Required monthly
savings and a FIRE number refuse — this is not a cash forecast. No new
`Method` / `Order` / `lot_method` variant. PERSONAL chrome is unchanged
— `screensFor` is not forked. **a Personal net-worth goals Connect app**
is the Built phrase this amendment adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect tokens
  on ConnectApiUrl. `fetch_statements()` and `deliver()` still refuse.
  Live OAuth is leftover #22 / #150. This file does not close #150.
- **Not a cash forecast, not a FIRE number, not envelope coaching.**
  Those stay refused or cannot-show. A scenario is discrete hypothetical
  posts, not a compounding path.
- **Not a kernel Goal RPC, not `ratio watch`, not Console product UI.**
  Sheet / bridge / cash-flow stay the core cites.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `journals:post` for opt-in scenario posts and does not open
  the door.

Nothing on the *Explicitly not building* list moved. Client portal, CRM,
tax e-file, vendor portal, and waterfall stay Connect-apps or stay
refused. This file does not close #168 — grant path and live OAuth
remain. It does not close #165 or #166.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show a fixture sheet of $50,000 against a $75,000 target as short, an
unset sheet leaving progress unset, extra income raising projected net
worth, a card charge that does not move cash, a closed March refusing an
opted-in 15 March post, and `journal:append` being rejected as a scope.
It cannot show a Connect token opening a book, a cash forecast, or a
FIRE number.

### Amendment, 2026-09-04 — the pooled holding-period category is a date, not a Method

Average cost pools the basis. It did not say what DATE the
remainder and the slice carry. US single-category invents FIFO's
oldest date on a mixed pool and classifies the sale long-term.
Double-category invents two pools. Both invent a short-vs-long
answer the lots do not support. Conservation holds, the trial
balance ties, the digest reproduces. The figure that goes wrong
is the RATE — which no reconciliation reaches.

The tax-pack Connect app already refused to invent a box when
dates disagree. That is the export citing the leftover, not the
kernel rule.

What landed is the date rule, as its own shape:

- Lean: `Ratio.Lots.PoolPeriod`. `poolAcquired` carries a shared
  date when every lot agrees; mixed or missing dates stay unset.
  Day 0 / day 400 / dispose 400 / threshold 365: FIFO is long,
  the other lot is short, the pool is neither. No `Order` /
  `Method` / `lot_method` variant. Unset is not a silent long
  and not a silent short. Not two Form 8949 boxes.
  `treating_mixed_dates_as_an_order_invents_a_category`.
- TLA: `//tla:pool_period_engine_check`. The probe
  `//tla:sort_and_walk_pool_period_check` treats the rule as
  FIFO and `ThePoolDateStaysUnset` goes red.
- Rust: `pool_acquired` on the lot surface, cited from the
  proofs. A shared date classifies. Mixed or missing dates
  leave the gain in `unclassified`. No RuleSet field — this is
  how pooling works, not an election between invented
  categories. `lot_method = "average_cost"` stays refused.
- The tax-pack cites the same rule. Mixed dates still land on
  `unclassified.csv`. It does not invent FIFO's oldest date or
  two boxes.

**What this is NOT:**

- Not a **tax-lot planner UI**. That was never a leftover of
  #9; it is a different product, the same shape as lot method
  and wash.
- Not **#166 closed**. Grant path and IRS e-file stay leftover
  on the tax-pack.
- Not **#168 closed**. Grant path and live OAuth stay leftover
  on the goals app.

Nothing on the *Explicitly not building* list moved. This
amendment closes #9. The named leftovers on that issue — MinTax,
SpecID, average cost, their console cites, and the pooled
holding-period category rule — have all landed. It does not
close #5 (already closed for wash). It does not close #166.
It does not close #168.

**What a walk-through can and cannot show** (demo readiness, #27).
A fund admin walk-through can pool a holding whose lots share
an acquisition date and see the sale classified from that date,
and can pool lots with mixed or missing dates and see the gain
stay unclassified — not a silent long, not a silent short, not
two Form 8949 boxes. The tax-pack walk-through cites the same
unset. It cannot show a tax-lot planner UI. That is not a
leftover of #9.

### Amendment, 2026-09-04 — subscriptions and redemptions are unit movements

Period NAV already cited contribution / distribution *money*.
Unitization was the other half: a subscription issues units, a
redemption retires them, and units in issue stay **unset** until a
unit event posts. A silent 0 on a PE-style contribution is the
defect. After a full redemption, `"0"` is a real zero.

What landed is the movement, as journal facts plus a measured
quantity — not a lot, not an equal-split of book units:

- Lean: `Ratio.Partners.Units`. A movement names cash and a
  non-zero unit count of the same sign. Money conserves; units do
  not enter conservation. `unitsInIssue [] = none`. Redeeming when
  unset, redeeming zero, and over-redemption refuse. Allocating
  units across partners is the named cut, not 1/N.
- Rules: `subscribe` / `redeem` (book-level, no 1/N split) and
  `subscribe_lp` / `subscribe_gp` / `redeem_lp` / `redeem_gp`.
  `measured = true` on the capital / equity leg. The operator types
  a positive count; credit issues `+q`, debit retires `−q`.
  `contribute_*` stays money-only.
- `/capital` and `/nav` cite ending units from the same folds they
  already use. Empty is unset. Console explain counts journal
  entries that posted a quantity onto a capital account and shows
  that count on the capital node — a contribution without units
  does not count. `shape_of` without a chart still leaves the node
  blank.

**What this is NOT, because leftovers stay named on #181:**

- Not a **CreateBook / live-demo seed of unitized capital**. The
  rules exist; the demo's opening `sub-0001` is still a money-only
  post onto Capital contributions, with no quantity. Units on that
  book stay unset — correctly — and the walk-through still has to
  record `subscribe_lp` itself.
- Not a **period issued / redeemed plug or a per-share NAV**.
  `/nav` cites ending units in issue and the same contribution /
  distribution *money* it already named. Issued and redeemed this
  window, and NAV per unit, stay unbuilt.
- Not an **LP portal, drip, or payment initiation** (Connect —
  #161 / #150).
- Not a **seeded demo cut** or a fold of per-entry specials (#180).
- Not an ingest template for subscriptions.

Nothing on the *Explicitly not building* list moved. This amendment
does not close #181. It does not close #180.

**What a walk-through can and cannot show** (demo readiness, #27).
It can record `subscribe_lp` with a unit count, cite those units on
`/capital` and `/nav`, redeem down to a real zero, and leave units
unset on a contribute-only book — including the live demo, whose
opening subscription has no units. It cannot show a seeded unit
movement, a per-share figure, period issued/redeemed as their own
plugs, an LP portal, or a 1/N split of book units. Those remain on
#181 (seed / NAV leftovers) or Connect.

### Amendment, 2026-09-04 — write-route actor is the WorkOS `sub`, not a caller string

#151. The JWT authorizer already proved audience and issuer. The leftover
was the subject reaching the write handlers as the journal/audit actor
PLAN requires for period close, and a Connect-shaped token riding
`RATIO_DEMO_OPEN` or an `org:{id}` grant.

What landed is the production constructor and the handler tests, not a
new identity product:

- `Console::for_request` is the one path `ratio watch` builds for `/v1`.
  An AuthKit session may take the open demo. A Connect token (`azp`,
  `scope`, or a `client_id` that is not this app) is always `scoped`.
- `applyEvent` / `ingest` / `admit` / CreateBook / period close record
  `ChangeLogEntry.actor` = the verified `sub`. A body field named
  `actor` is ignored. CreateBook grants the creator's `sub`, not their
  org, and writes a `created` line.
- User A creates a book and posts; user B gets authorized-empty / "no
  fund", never A's journal. ListBooks / ListFunds still distinguish
  that empty set from an unreadable membership file.
- A Connect token does not match `org:{id}` membership. That line is
  an AuthKit operator grant, not a third-party inheritance.

**What this is NOT, because leftovers stay named on #22:**

- **Not live provider OAuth.** `/v1` now proves AuthKit session JWTs
  on DemoUrl and Connect tokens on ConnectApiUrl. Dashboard
  registration, redirect, and bank / calendar OAuth stay leftover.
  This file does not close #22.
- The `RATIO_DEMO_OPEN` leftover named here is recorded by the
  later deployed-demo dial amendment.
- **Not removing Cognito CloudFormation resources.** They stay unused
  so a stack update does not destroy the live pool.

Nothing on the *Explicitly not building* list moved. This amendment
closes the #151 write-route leftover. It does not close #22.

**What a walk-through can and cannot show** (demo readiness, #27). It
can show a signed-in operator's `sub` on a posted event and a period
close, and a second subject receiving `[]` / refuse for that book
when the console is `scoped`. The open-dial leftover named here is
recorded by the later amendment. It cannot show live bank or
calendar OAuth.

### Amendment, 2026-09-04 — a Project AIA pay-app Connect app, and G702 product UI still does not live here

[#184](https://github.com/mattmarshall/ratio/issues/184) asked for an
AIA G702/G703 pay-app pack from Project billing figures without stuffing
G702 product UI into `ratio watch` or growing a vendor portal in core.
`/billing` and `/budget` stay core. The catalog already said AIA G702
product UI stays a Connect app. What landed is that app as a sibling
tree, not a kernel method.

**What this amendment records.** [`connect/aia-pay-app/`](connect/aia-pay-app/)
is a first-party WorkOS Connect OAuth application for `BookKind` PROJECT.
It declares `billing:read`, `budget:read`, and `statements:read` — the
frozen names, not the catalog aliases `projects:billing:read` /
`projects:budget:read`. It is read-only relative to the journal:
`journals:post` is not requested. Billing, budget, retainage, and
change-order cites become a G702-ish application CSV plus a G703-ish
schedule of values, companions (earned, collected, vendor retainage),
and an unset sheet naming what the journal cannot support. Revised
contract is original plus approved when `[project] budget` is set. An
unposted change order leaves the change line unset; remaining to bill
stays unset until billed posts — never the whole contract as a fake
remainder. An omitted prior application is not previous-certificates
0.00. Billed is Progress billings; earned and phase cost are not
substitutes. No percentage. Unset stays unset. Money is minor units,
split on the point. No new `Method` / `Order` / `lot_method` variant.
PROJECT chrome is unchanged — `screensFor` is not forked. **a Project
AIA pay-app Connect app** is the Built phrase this amendment adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect
  tokens on ConnectApiUrl. `fetch_cites()` and `deliver()` still
  refuse. Write-route actor binding landed (#151). Live OAuth is
  leftover #22 / #150. This file does not close #22. It does not
  reopen #151.
- **Not a licensed AIA PDF, not a vendor portal, not G702 product UX.**
  `render_form()` refuses. Live form / portal leftovers stay on #184
  and #172.
- **Not EAC / forecast.** Those stay on #169. `/budget` still does not
  forecast.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `billing:read` and `budget:read` and does not open the door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**
  `/billing` and `/budget` stay the core cites.

Nothing on the *Explicitly not building* list moved. Client portal, CRM,
tax e-file, vendor portal, and waterfall stay Connect-apps or stay
refused. This file does not close #184 — grant path (leftover #22),
live OAuth, and a licensed AIA form remain. It does not close #165,
#166, or #168. It does not close #150. It does not close #22. It
does not close #181. It does not reopen #151. It does not start
#169 or #172.

**What a walk-through can and cannot show** (demo readiness, #27). It can
show a fixture job of original $10,000 / CO $500 / billed $1,000 mapping
to G702-ish rows, a missing billed cite leaving billed and remaining
unset rather than inventing the whole contract as leftover, an omitted
prior leaving previous certificates unset, and `projects:billing:read`
being rejected as a scope. It cannot show a Connect token opening a
book, a licensed AIA PDF, or live G702 product UX beyond the pack.

### Amendment, 2026-09-04 — management-fee accrual posts receivable/expense

Stage 1 already had the accrual *rule*: `rate_bp`, a day-count, a
balanced template. Posting **accrual → receivable/expense** on the
journal was the books half (#182). Invoice PDF / LP statements /
payment collection stay Connect (`fees:read` / `fees:accrue`).

What landed is the election plus the conserved posting:

- Lean: `Ratio.Fees.Accrual`. Terms are an election (`None` is
  unset, not a silent 0 bp). A zero rate is not well-formed. The
  posting is expense debit and receivable credit of one amount
  (`a_posting_conserves`). Same-sign legs are not an accrual. A
  zero amount is not a posting. The citeable receivable stays
  unset without terms — even if some other rule moved the payable
  — and an empty journal stays unset. Accrued then paid is a real
  zero.
- Rust: `RuleSet.fee_terms` reads `management_fee_accrual`.
  CreateBook writes no fee rule. `accrue` returns the conserved
  pair or `None`. `compile` of the elected rule is that pair.
  GetBook.`fee_receivable` is empty without terms or without a
  post. ApplyEvent refuses a zero-day no-op and a same-sign
  template.
- `/capital` cites the receivable. Empty is unset.

**What this is NOT:**

- Not **invoice packaging, LP email, or payment collection**.
  Those stay Connect (#150 / `fees:read` / `fees:accrue`).
- Not a **seeded demo fee**. CreateBook writes no
  `management_fee_accrual`, so the live demo's receivable stays
  unset — correctly.
- Not a **`screensFor` fork**, and not a `Method` / `Order` /
  `lot_method` variant.
- Not #181 leftovers (demo seed money-only unitization, NAV
  issued/redeemed plugs, per-share NAV).
- Not #180 leftovers (specials fold, seed cut, no LP portal).

Nothing on the *Explicitly not building* list moved. This
amendment closes #182. Invoice / LP leftovers were never this
issue's — they stay Connect.

**What a walk-through can and cannot show** (demo readiness, #27).
It can write `management_fee_accrual` at 75 bp act/365, accrue a
dividing basis, cite a conserved expense / payable on the journal
and the receivable on `/capital` / GetBook, and leave the figure
unset on a book that never elected the rule — including the live
demo. It cannot show an invoice PDF, an LP statement, or a
payment. Those remain Connect.

### Amendment, 2026-09-04 — a Project vendor-portal Connect app, and the grant path still does not open

[#172](https://github.com/mattmarshall/ratio/issues/172) asked for a
vendor / GC portal that reads Project billing and retainage without
stuffing vendor chrome into `ratio watch` or growing a vendor user
directory in core. `/billing` and `/budget` stay core. The catalog
already said a vendor portal stays a Connect app. What landed is that
app as a sibling tree, not a kernel method.

**What this amendment records.** [`connect/vendor-portal/`](connect/vendor-portal/)
is a first-party WorkOS Connect OAuth application for `BookKind` PROJECT.
It declares `billing:read`, `budget:read`, `statements:read`, and
`journals:post` — the frozen names, not the issue body's stale alias
`journal:append` and not the catalog aliases `projects:billing:read` /
`projects:budget:read`. Vendor-facing billed / earned / retainage /
collections cite the kernel cuts already on `/billing`. Unset stays
unset — an unbilled job is not billed-zero; treating billed as 0
would print the whole contract as remaining. Remaining to bill is
revised − billed. Collections vs billed is cash against AR. Vendor
invoices post only as allowlisted `journals:post` for already-seeded
`vendor_invoice*` templates. Empty allowlist refuses every post. A
dated invoice on or before closed-through refuses the batch.
Instantiated legs must conserve in every currency. Money is minor
units, split on the point. No percentage. No new `Method` / `Order`
/ `lot_method` variant. PROJECT chrome is unchanged — `screensFor`
is not forked. **a Project vendor-portal Connect app** is the Built
phrase this amendment adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect
  tokens on ConnectApiUrl. `fetch_cites()` and `deliver()` still
  refuse. Write-route actor binding landed (#151). Live OAuth is
  leftover #22 / #150. This file does not close #22. It does not
  reopen #151.
- **Not a vendor user directory in Ratio core.** Membership is the
  AuthKit `sub` on the book. `vendor_directory()` refuses.
- **Not AIA G702 product UI.** That door is #184. `render_g702()`
  refuses. The pay-app pack is `connect/aia-pay-app/`. `/billing`
  and `/budget` stay the core cites.
- **Not EAC / forecast.** Those stay on #169. `/budget` still does
  not forecast. `eac()` / `forecast()` refuse.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `billing:read`, `budget:read`, and `journals:post` and
  does not open the door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**

Nothing on the *Explicitly not building* list moved. Client portal,
CRM, tax e-file, AIA G702 product UI, and waterfall stay
Connect-apps or stay refused. This file does not close #172 —
grant path (leftover #22), live OAuth, and a vendor user directory
remain. It does not close #184, #169, #161, #165, #166, or #168.
It does not close #150. It does not close #22. It does not reopen
#151. It does not reopen #182.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show a fixture job of original $10,000 / CO $500 / billed
$1,000 mapping to collections and remaining-to-bill, an unbilled
job leaving billed and remaining unset rather than inventing the
whole contract as leftover, a site invoice mapping to
`vendor_invoice_site`, a closed March refusing a 15 March invoice,
and `journal:append` being rejected as a scope. It cannot show a
Connect token opening a book, a vendor user directory, AIA G702
product UI, or EAC / forecast.

### Amendment, 2026-09-04 — a Project EAC / forecast Connect app, and `/budget` still does not forecast

[#169](https://github.com/mattmarshall/ratio/issues/169) asked for
estimate-at-completion and cost-to-complete from Project budget
figures without stuffing EAC fields into `/budget` or growing a
forecast RPC in core. Remaining to spend stays revised − incurred −
awarded. Unset remaining stays unset — never a silent forecast of 0.
The catalog already said a cash forecast stays a Connect app. What
landed is that app as a sibling tree, not a kernel method.

**What this amendment records.** [`connect/eac-forecast/`](connect/eac-forecast/)
is a first-party WorkOS Connect OAuth application for `BookKind` PROJECT.
It declares `budget:read`, `billing:read`, and `statements:read` — the
frozen names, not the issue body's stale alias `journal:append` and
not the catalog aliases `projects:budget:read` /
`projects:billing:read`. It is read-only relative to the journal:
`journals:post` is not requested. The catalog has no forecast
template; posting `project_cost*` as a what-if would mix a forecast
into the book of record. Budget, billing, and statements cites
become an EAC CSV / JSON pack with the assumption written on the
row, companions (billed / earned / remaining-to-bill), a
work-package sheet, and an unset sheet naming what the journal
cannot support. Remaining to spend is revised − incurred − awarded
when those three can support the cut. Treating awarded as 0 would
print budget − actual as headroom. When remaining can be cited:
ETC = remaining + awarded, EAC = incurred + ETC (= revised). When
it cannot: EAC and ETC stay blank — never a silent 0.00. A posted
`"0.00"` is a real zero. Billed / earned are not substitutes for
incurred. No percentage. No CPI / SPI EAC. Money is minor units,
split on the point. No new `Method` / `Order` / `lot_method`
variant. PROJECT chrome is unchanged — `screensFor` is not forked.
`/budget` still does not forecast. **a Project EAC / forecast
Connect app** is the Built phrase this amendment adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect
  tokens on ConnectApiUrl. `fetch_cites()` and `deliver()` still
  refuse. Write-route actor binding landed (#151). Live OAuth is
  leftover #22 / #150. This file does not close #22. It does not
  reopen #151.
- **Not EAC fields on `/budget`.** Remaining to spend is still
  revised − incurred − awarded. Unset stays unset. A silent forecast
  of 0 is refused. `post_forecast()` and `cpi_eac()` refuse.
- **Not a forecast journal write.** `journal:append` is an alias.
  `journals:post` would need an allowlisted template the catalog
  does not have. Export is CSV / JSON so the journal stays the
  system of record.
- **Not AIA G702 product UI, not a vendor portal.** Those doors are
  #184 and #172. `connect/aia-pay-app/` and `connect/vendor-portal/`
  stay the sibling trees.
- **Not a Personal cash forecast.** That door is #163.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `budget:read` and `billing:read` and does not open the
  door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**
  `/budget` and `/billing` stay the core cites.

Nothing on the *Explicitly not building* list moved. Client portal,
CRM, tax e-file, vendor portal, AIA G702 product UI, and waterfall
stay Connect-apps or stay refused. This file does not close #169 —
grant path (leftover #22), live OAuth, and live EAC product UX
remain. It does not close #172, #184, #170, #173, #163, #165, #166,
or #168. It does not close #150. It does not close #22. It does
not reopen #151.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show a fixture job of original $10,000 / CO $500 / incurred
$2,000 / awarded $1,500 mapping to remaining $7,000 and EAC $10,500
with the assumption on the row, an unawarded job leaving remaining
and EAC unset rather than printing budget − actual as headroom, a
missing incurred cite leaving EAC blank rather than inventing
0.00, and `journal:append` being rejected as a scope. It cannot
show a Connect token opening a book, EAC fields on `/budget`, or
live EAC product UX beyond the pack.

### Amendment, 2026-09-04 — unitized seed, period issued/redeemed, per-share

#196 landed subscribe / redeem as conserved unit movements.
Leftovers stayed on #181: the live demo's `sub-0001` was still
money-only, `/nav` had no period issued / redeemed plug and no
per-share figure, and CreateBook seeded no ingest mapping.

What landed is those leftovers, cited honestly:

- Lean: `Ratio.Partners.periodIssued` / `periodRedeemed`. Empty, or
  a window with only the other kind of movement, is unset — not a
  silent zero issue or a silent zero redemption. Issued minus
  redeemed is the signed net ending units already sum.
  `Ratio.Closure.perShare` was already the one division that must
  round; residual is accounted for.
- Live demo: `sub-0001` posts 500,000 units on Capital
  contributions, dated 2026-01-01. Units in issue are citeable.
  CreateBook still writes no journal history — the seed is the
  `subscriptions` ingest mapping plus this opening post.
- `/nav` cites period issued / redeemed from the Loan fold's
  window, and per-share from ending NAV / units in issue
  (Euclidean, matching Lean). Unset when the window has no unit
  event, or units are unset or zero — never a fake zero plug or a
  divided-by-zero per-share. Residual stays with the fund.
- Ingest: `subscriptions` maps Kind × Amount × Quantity onto
  `subscribe` / `subscribe_lp` / `subscribe_gp` / `redeem` /
  `redeem_lp` / `redeem_gp`. Quantity is decimal hundredths →
  whole units on admit. `contribute_*` stays money-only.

**What this is NOT:**

- Not an **LP portal, drip, or payment initiation** (Connect —
  #161 / #150).
- Not a **seeded demo cut** or a fold of per-entry specials (#180).
- Not a **`screensFor` fork**, and not a `Method` / `Order` /
  `lot_method` variant.
- Not a CreateBook journal posting. CreateBook writes the
  template; the live demo posts `sub-0001`.

Nothing on the *Explicitly not building* list moved. This
amendment closes #181. It does not close #180. It does not
close #172, #169, or #22.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open the live demo and cite units in issue, January issued,
and per-share NAV on `/nav`; ingest or record `subscribe_lp`;
leave issued / redeemed / per-share unset on a contribute-only
window or a book with no units. It cannot show an LP portal, a
1/N split of book units, or a seeded partner cut. Those remain on
Connect or #180.

### Amendment, 2026-09-04 — Project `/budget` posts and ingests change orders / awards

[#170](https://github.com/mattmarshall/ratio/issues/170) asked for an
approved change order or award to be posted from the existing Project
`/budget` chrome, with phase keys, unset until posted, and without a
second budget store. Change orders and awarded commitments were already
journal facts (`approve_co_*` / `award_commitment_*`); the leftover was
that the walk-through had to leave `/budget` to `/record` or `/ingest`
to write them.

**What this amendment records.** Project `/budget` posts a change order
or award through the same `ApplyEvent` `/record` uses. Kind × phase
selects a rule CreateBook already seeded — `approve_co` /
`deduct_co` / `award_commitment` / `release_commitment`, optionally
`_site` / `_structure` / `_finishes`. A tampered form that sends
`project_cost` or `equity_purchase` is refused. CSV ingest on the same
page uses the same `change-orders` / `purchase-orders` templates;
`listTemplates` is not a fourth upstream call — those two ids are the
kind-selected catalog. Facts stay unset until posted:
`postingCount === "0"` is still the distinction. Treating an unposted
award as 0 would print budget − actual as headroom. No new journal
kind. No new `Method` / `Order` / `lot_method` variant. PERSONAL /
INVESTMENT / OPERATING `screensFor` paths are unchanged. `/budget`
still does not forecast. **change-order and award ingest on /budget**
is the Built phrase this amendment adds.

**What this is NOT:**

- **Not a Connect estimating app.** Estimating tools push later via
  `journals:post` (the frozen name; `journal:append` is an alias) plus
  `budget:read`. Grant path leftover #22 / #150. This file does not
  close #22. It does not close #150. It does not reopen #151.
- **Not EAC / forecast, not a vendor portal, not collections chrome.**
  Those doors are #169, #172, #173. `/budget` still does not forecast.
- **Not a second budget store, not a `screensFor` fork.**
- **Not a kernel RPC.** The write is `ApplyEvent` on the seeded rules.

Nothing on the *Explicitly not building* list moved. This amendment
closes #170. It does not close #169, #172, #173, #184, #104, #91,
#150, or #22.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open a Project `/budget`, leave approved / awarded unset
before anything posts, preview then post `approve_co_site` or
`award_commitment_site`, cite the posted figure on the same page,
and ingest a `change-orders` / `purchase-orders` CSV under those
templates. It cannot show a Connect token opening a book, an
estimating-tool OAuth grant, EAC fields on `/budget`, or a vendor
portal.

### Amendment, 2026-09-04 — CreateBook writes the cut, and `/capital` folds journal specials

[#180](https://github.com/mattmarshall/ratio/issues/180) leftovers after
#191: CreateBook / the seeded demo still wrote no `[[partner_cut]]`, so
live allocated plugs stayed unset, and per-entry `special_allocations`
were stored but not walked by `/capital`. The cut engine itself
(`Ratio.Partners.Cut`, named weights, refuse undividable figures) was
already Built.

What landed is the seed and the fold:

- CreateBook(Investment) writes `[[partner_cut]]` LP 80 / GP 20. The
  live demo seed writes the same table. Two partners is not 50/50.
  Personal / Project / Operating still write no cut. A book that
  omits the table stays unset — not a silent 1/N.
- GetBook cites journal `allocation_facts` (partner, kind, amount,
  trade date). GetEntry cites per-entry specials the SpecID way
  (`special_allocations_declared`). `Some([])` still refuses at the
  store door.
- `/capital` folds those facts (`Ratio.Partners.applyFacts`): named
  amounts first, remainder under the cut. Facts that cover the
  figure are the allocation. An overshoot refuses. A remainder
  without a cut stays unset. A dated window drops undated and
  out-of-window facts. Standing config specials still replace the
  default cut for one kind.

**What this is NOT:**

- Not an **LP portal, K-1 pack, or waterfall**. Those stay Connect
  (#161 / #150).
- Not a **`screensFor` fork**, and not a `Method` / `Order` /
  `lot_method` variant.
- Not a CreateBook journal posting. CreateBook writes the cut as
  configuration; the journal stays empty until someone posts.

Nothing on the *Explicitly not building* list moved. This
amendment closes #180. It does not close #161. It does not
reopen #181 or #191.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook(Investment) or open the live demo and cite
allocated income of a dividing figure on `/capital` under LP 80 /
GP 20; write a journal special and see the fold (named amount,
then the remainder cut); leave plugs unset when the table is
omitted, the figure will not divide, or a leftover has no cut.
It cannot show an LP portal, a K-1, a waterfall, or IRR. Those
remain Connect.

### Amendment, 2026-09-04 — equalization, drip, and side-pocket stay Connect

[#177](https://github.com/mattmarshall/ratio/issues/177) asked whether
equalization, drip subscriptions, and side pockets belong in core
because buyers name them and the kernel already has partners,
commitments, capital accounts, unit movements, and NAV roll-forward.
That premise is the coverage-creep shape this file already named:
each is valuation or ops packaging on top of proofs that already
hold. The default bias holds — **Connect or refuse; keep the kernel
slim.** Core only if conservation or journal integrity would change.
None of the three do.

**The decision.** **equalization, drip, and side-pocket stay Connect**
is the phrase this amendment records — a decision, not a landing.

| Feature | Decision | Why not core |
|---|---|---|
| Equalization | **Connect** | A late-subscriber credit, equalization share, or series price is a valuation of NAV and the named cut. The books half is already `subscribe_*` at a price, maybe a share-class partition. No new conserved dimension. |
| Drip subscriptions | **Connect** | A drip is `distribute_*` then `subscribe_*` of the same amount, plus an LP election. Two existing conserved postings packaged as a workflow. Already named Connect (#161 / #150). |
| Side pockets | **Connect** | A pocket is *where* an illiquid asset sits — share class / instrument / a named partner cut, already partitioning (`Ratio.Chart.Dimensions`). A transfer into a restricted class conserves. A silent 1/N of pocket NAV is the defect #180 already refused. |

**Refuse in the kernel, for all three.** No `equalize` / `drip` /
`side_pocket` rule primitive. No `Method` / `Order` / `lot_method`
variant. No `screensFor` fork. No chrome on `/nav` or `/capital` that
invents an equalization factor, a reinvestment election, or a
SidePocket type. Minting `equalization:*` or `sidepocket:*` as a
Connect scope is the same chrome with a different sticker — use the
frozen catalog in [`docs/connect-scopes.md`](docs/connect-scopes.md).

**What Connect reads and writes.** Existing scopes, no new grants:

- Equalization: `nav:read`, `partners:read`, `capital:read`,
  `journals:post` (allowlisted `subscribe_*`).
- Drip: `partners:read`, `nav:read`, `journals:post` (allowlisted
  `distribute_*` then `subscribe_*`). The LP election lives on #161.
- Side pocket: `positions:read`, `partners:read`, `nav:read`,
  `config:read`, `journals:post`. Participation is a named cut, not a
  new primitive. No cut → allocated plugs stay unset.

**What this is NOT, because this card is the decision:**

- **Not an implementation.** No Lean, no Rust, no Connect app tree.
  Do not implement until a later Connect card names the app.
- **Not new scopes.** The catalog already covers these doors.
- **Not a refuse of the product door.** Buyers ask; Connect is the
  answer. Waterfall is the same shape.
- **Not #161.** Drip was already Connect there. This file does not
  close #161.
- **Not #155**, and not a seeded equalization or side-pocket demo.

Nothing on the *Explicitly not building* list moved. This amendment
closes #177 — the decision is the whole card. It does not close
#161. It does not close #150. It does not close #22. It does not
close #155. Equalization and side-pocket first-party Connect apps
are not filed; drip stays on #161.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show this table and the refuse-in-kernel list. It cannot
show an equalization credit, a drip election, or a side-pocket
class in `ratio watch`. Those remain Connect, unbuilt.

### Amendment, 2026-09-04 — Project `/billing` posts a cash application

[#173](https://github.com/mattmarshall/ratio/issues/173) asked for a
collection (cash applied to AR) to be posted from the existing Project
`/billing` chrome, unset until billed and receivable can support the
cut, and without a payment-processor integration in the kernel.
Collections vs billed was already a citeable figure; the leftover was
that the walk-through had to leave `/billing` to `/record` to write
the cash application.

**What this amendment records.** Project `/billing` posts a collection
through the same `ApplyEvent` `/record` uses. The rule is
`collect_receivable` CreateBook already seeded — cash up, receivable
down. A tampered form that sends `progress_bill`, `project_cost`, or
`equity_purchase` is refused. `listRules` is not a fourth upstream
call — the id is the kind-selected seed, the way `/budget` hardcodes
its two ingest templates. No new journal kind. No new ingest
template — `project-invoices` is vendor cost / AP, not customer cash.
Facts stay unset until billed and AR can support the cut: an unbilled
job is not collected 0.00, and a billed figure with no AR posting
stays unset. Billed but uncollected is a real zero collected. No new
`Method` / `Order` / `lot_method` variant. PERSONAL / INVESTMENT /
OPERATING `screensFor` paths are unchanged. Stripe / ACH / payment
processors stay Connect. **cash application on /billing** is the
Built phrase this amendment adds.

**What this is NOT:**

- **Not a payment processor.** Stripe / ACH settlement journals stay
  Connect apps posting through `journals:post`. Grant path leftover
  #22 / #150. This file does not close #22. It does not close #150.
  It does not reopen #151.
- **Not billed-vs-earned / retainage / remaining-to-bill math.**
  Those cuts already land. This page does not invent them.
- **Not EAC / forecast, not a vendor portal, not AIA G702.** Those
  doors are #169, #172, #184.
- **Not a `screensFor` fork**, and not a kernel RPC. The write is
  `ApplyEvent` on the seeded rule.

Nothing on the *Explicitly not building* list moved. This amendment
closes #173. It does not close #169, #172, #184, #104, #100, #85,
#180, #177, #155, #161, #150, or #22.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open a Project `/billing`, leave collected unset before AR
has posted, preview then post `collect_receivable`, and cite the
moved collections vs billed / outstanding receivable on the same
page with journal provenance. It cannot show a Stripe or ACH
processor, a Connect token opening a book, AIA G702 product UI, a
client portal, or a vendor portal.

### Amendment, 2026-09-04 — live custodian statement ingest

[#155](https://github.com/mattmarshall/ratio/issues/155) asked for the
first live feed end-to-end: one statement ingest path into the journal
with refuse-on-unbalanced / unidentified, scaled enough to show the
recon gate (a difference at its own address), without a raw sideload.
CreateBook already seeded `custodian-positions` (recorded, never
posted) and `prime_equity_trades`. The Stage 3 wedge (`ratio recon`
over a different CSV contract, `--post` inventing journal history)
was the demo's blocked-NAV story, not the live path.

**What this amendment records.** The live path is ingest → admit →
`ratio recon --from-ingest`. Identified trades post through the
seeded rules; unidentified facts stay pending and do not post; the
kernel still refuses an unbalanced entry at the door. Ingested
`custodian-positions` facts are compared to the journal's
Investments carrying value — only that account, because a holdings
snapshot is not a statement about cash. One unidentified or
foreign-currency holding refuses the whole run and writes no
breaks. A difference is a `BreakReport` the NAV gate already reads,
at `funds/{id}/views/{view}/breaks/1`. No new `Method` / `Order` /
`lot_method` variant. No `screensFor` fork. Lot method and wash stay
the elections Config/RuleSet already cites. **live custodian
statement ingest** is the Built phrase this amendment adds.

**What this is NOT:**

- **Not broker OAuth or multi-custodian adapters.** Those stay
  Connect apps with `journal:append` + `breaks:read` (#150).
- **Not a client document portal** (Connect `partners:read` / files).
- **Not an LP portal, equalization, drip, or side-pocket.** Those
  stay Connect (#161 / #177). This file does not reopen #177.
- **Not reconciliation at a fund's volume.** That Phase two line
  stays open.
- **Not a second store, and not a shadow-run sideload.** The demo
  seed still posts recon history on a *different* book so the
  blocked-NAV story has a break. A blank CreateBook book does not.

Nothing on the *Explicitly not building* list moved. This
amendment closes #155. It does not close #150, #161, or #177. It
does not reopen #180 or #181.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook(Investment) or `ratio init --kind investment`,
ingest `prime_equity_trades` and `custodian-positions`, admit the
identified trades, run `ratio recon --from-ingest`, open the
Investments difference at its own address, and see `ratio strike`
refuse while that break is unexplained. It cannot show a broker
OAuth grant, a multi-custodian adapter, an LP portal, or a fund-
volume feed. Those remain Connect or the Phase two leftover.

### Amendment, 2026-09-04 — Project job-cost / AP statement ingest

[#171](https://github.com/mattmarshall/ratio/issues/171) asked for a
BookKind-aware statement ingest into the journal + breaks for Project
books — subcontractor AP / progress-bill feeds — at #72 parity with
the fund trade loop, without inventing retainage. `project-invoices`
already mapped unpartitioned `cost` / `invoice`. The leftover was the
closed loop and the progress-bill / phase kinds.

**What this amendment records.** CreateBook(Project) still seeds one
`project-invoices` mapping — not a second recon engine. Kind now
picks `project_cost*` / `vendor_invoice*` (phase suffix names the
work package), `progress_bill`, `pay_vendor`, and `earn_progress`.
An unidentified vendor pends, the same shape as a missing instrument
on the fund path. `hold_retainage` / `capitalize_wip` /
`collect_receivable` in Kind are refused, not posted: retainage and
WIP stay unset until a hold or capitalization is recorded, and
customer cash stays on `/billing` (#173). Unbalanced entries still
meet the journal door. No new journal kind. No new
`Method` / `Order` / `lot_method` variant. PERSONAL / INVESTMENT /
OPERATING `screensFor` paths are unchanged. Vendor portals and GC
SaaS sync stay Connect. **job-cost / AP statement ingest** is the
Built phrase this amendment adds.

**What this is NOT:**

- **Not a vendor portal or GC SaaS sync.** Those doors are #172 /
  Connect. Grant path leftover #22 / #150. This file does not close
  #22. It does not close #150. It does not reopen #151.
- **Not retainage invention, not EAC / forecast, not AIA G702.**
  Retainage / WIP figures stay the Built `/record` path. EAC is
  #169. G702 product UI is #184.
- **Not a project-only recon engine**, and not a `screensFor` fork.
  The write is `ingest` / `admit` on the seeded template.

Nothing on the *Explicitly not building* list moved. This amendment
closes #171. It does not close #169, #172, #184, #155, #150, or #22.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook a Project book, ingest a job-cost / AP /
progress-bill CSV under `project-invoices`, admit the identified
rows onto `journal.jsonl`, leave an unmatched vendor pending, cite
billed and phase cost from those posts, and show retainage and WIP
still unset. It cannot show a Connect token opening a book, a
vendor portal, AIA G702 product UI, or a silent retainage split.

### Amendment, 2026-09-04 — multi-view FX / translation refuses stay citeable

The 2026-08-13 multi-view amendment already said `ratio reconcile A B`
refuses when per-entry effects cannot sum to the NAV difference
(integer translation does not distribute over a sum), when a view has
entries only it cannot place, or when a view was declared after the
fold read past its history — and shows, rather than omits, what
neither view can place. The leftover was a silent zero: unplaceable
rows left the BFF as `net_asset_value_effect: "0"`, and nothing proved
the residue / missing-rate refuses against a published 0.00
difference that looks like agreement.

What landed is the cite, not a new Method:

- Engine: `Projection::reconcile` still refuses a translation
  residue, a missing rate, an entry only one view can place, and a
  view declared after the fold. Tests now fail if those become a
  silent zero.
- API / CLI: `ReconcileViews` and `ratio reconcile` propagate those
  sentences. Unplaceable rows carry `why` and an **empty** effect —
  unset, not `"0"`.
- Console: `/books/{id}/views/{view}/reconcile` cites the refuse
  sentence (`withRefusal`) and renders unplaceable as the why, never
  `money("")` padded to `0.00`.

No new `Method` / `Order` / `lot_method` variant. FX rate vendor
apps stay Connect — they supply facts; they do not own translation.
No `screensFor` fork. **multi-view FX / translation refuses stay citeable**
is the Built phrase this amendment adds.

**What this is NOT:**

- **Not an FX rate vendor Connect app.** That door is not opened
  here.
- **Not #159 (Postgres).** Stage E stays blocked.
- **Not #158 (control / fact seam).** Next after this; this file
  does not expand into it. The seam landed in the amendment below.
- **Not a `screensFor` fork** for Personal or Project.

Nothing on the *Explicitly not building* list moved. This
amendment closes #160. It does not close #159 or #158. It does
not close #171. It does not reopen #155, #180, or #181.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open `/reconcile?against=` on a dual-basis book, see the
in-flight list add to the NAV difference, and see an unplaceable
row cite why rather than 0.00. A book whose in-flight EUR legs
will not translate into the NAV difference refuses the screen with
the residue sentence. It cannot show a rate-vendor Connect app or a
Postgres projection. Those remain Connect or #159. The
control/fact seam is the next amendment.

### Amendment, 2026-09-04 — the ConfigStore / fact-plane seam

[#158](https://github.com/mattmarshall/ratio/issues/158) asked for the
phase-one control / fact plane still open from historical #74: a
typed ConfigStore seam so authored rules compile and get checked
without smuggling a second ledger, and a fact plane whose prices and
FX an operator can open from a figure. PLAN already sketched
`put` / `get` / `set_active` / `active` / `history` with v1 as a
directory, SHA-256, and a pointer file. geetch / crova stay later.

**What this amendment records.** The seam is real, not only prose:

- **Control plane.** `ConfigStore` is the five methods this file
  already named. v1 is `DirectoryConfigStore` — a directory, SHA-256,
  an atomic `ACTIVE` pointer, `HISTORY` newest-first. `put` does not
  promote. An unstored digest cannot become active. FileBook
  delegates here so the journal and the control plane do not share
  one type.
- **Fact plane.** `FactStore` is append-only `facts.jsonl`
  (`Plane::Facts`). A fact without provenance is refused — a figure
  cannot open it. The same id is refused, not overwritten; a
  correction is a new fact. Ingest writes through `record_typed_fact`.
  Vendors push via scoped ingest (Connect); they do not own the
  journal.
- **Refuse a parallel mutable shadow book.** `--post` may write
  reconstructed entries onto a book that *is* the Stage 3 wedge (no
  facts, no `shadow/` sibling). It refuses a `shadow/` directory
  beside the book, a book sitting at `shadow/` under a live book, or
  posting recon history beside ingested facts. `compare_configs`
  answers "what moved?" in memory. **the ConfigStore / fact-plane
  seam** is the Built phrase this amendment adds.

**What this is NOT:**

- **Not geetch or crova.** Open defects, ops burden, and the
  trigger (a second customer, or compliance-reviewed change control
  a pointer file cannot satisfy) stay as this file already wrote
  them. Do not adopt either in this tree until that trigger.
- **Not #159 (Postgres).** Stage E stays blocked. The journal is
  still the store.
- **Not #161 (LP portal).** Connect. External research / price
  vendors stay Connect too — they push facts via scoped ingest.
- **Not a `screensFor` fork**, and not a `Method` / `Order` /
  `lot_method` variant.
- **Not a second store.** The refuse is the point.

Nothing on the *Explicitly not building* list moved. This
amendment closes #158. It does not close #159 or #161. It does
not reopen #155, #160, #74, or #150. geetch / crova wiring stays
named here as later.

**What a walk-through can and cannot show** (demo readiness, #27).
It can `ratio approve` (pointer moves, history records), ingest a
price or FX fact and open it from a figure, run `ratio recon` on an
empty wedge book with `--post`, and see `--post` refuse once facts
exist or a `shadow/` directory is present. It cannot show reviewed
change control, a forge, crova dedup, a Postgres projection, or an
LP portal.

### Amendment, 2026-09-04 — a Project program-rollup Connect app, and no mega-book in the kernel

[#179](https://github.com/mattmarshall/ratio/issues/179) asked for a
multi-contract / program roll-up of Project budget and billing
figures without stuffing a program URL into `ratio watch` or growing
a mega-book that breaks BookKind independence. `/budget` and
`/billing` stay per-book core cites. The catalog already said
breadth stays a Connect app. What landed is that app as a sibling
tree, not a kernel method.

**What this amendment records.** [`connect/program-rollup/`](connect/program-rollup/)
is a first-party WorkOS Connect OAuth application for `BookKind` PROJECT.
It declares `books:read`, `budget:read`, and `billing:read` — the
frozen names, not the catalog aliases `projects:budget:read` /
`projects:billing:read` and not `journal:append`. It is read-only
relative to the journal: `journals:post` is not requested.
`books:read` lists books the subject can see. The roll-up keeps
PROJECT rows with membership. An `org_id` claim is not membership —
a first-party app does not inherit every book in an org. Per-book
budget and billing cites become a program CSV / JSON that sums
**only the books that cited the figure**. Unset stays unset. An
unbilled job is not billed-zero; treating billed as 0 would print
the whole contract as remaining. An uncollected job is not
collected-zero. A book that cannot support a cut does not contribute
0.00 to the program total. Program remaining / collected are the
sum of the per-book cuts — never recomputed from mixed program
totals. A posted `"0.00"` is a real zero. Money is minor units,
split on the point. No new `Method` / `Order` / `lot_method`
variant. PROJECT chrome is unchanged — `screensFor` is not forked.
No fifth kind. **a Project program-rollup Connect app** is the Built
phrase this amendment adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect
  tokens on ConnectApiUrl. `fetch_cites()` and `deliver()` still
  refuse. Write-route actor binding landed (#151). Live OAuth is
  leftover #22 / #150. This file does not close #22. It does not
  reopen #151.
- **Not a mega-book.** `mega_book()` and `merge_journals()` refuse.
  Books stay independent. A concatenated journal would break the
  prefix a figure must pin.
- **Not EAC / forecast, not AIA G702 product UI, not a vendor
  portal.** Those doors are #169, #184, and #172. `eac()`,
  `render_g702()`, and `vendor_directory()` refuse. `/budget` still
  does not forecast.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `budget:read` and `billing:read` as well and does not
  open the door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**
  `/budget` and `/billing` stay the per-book core cites.

Nothing on the *Explicitly not building* list moved. Client portal,
CRM, tax e-file, vendor portal, AIA G702 product UI, and waterfall
stay Connect-apps or stay refused. This file does not close #179 —
grant path (leftover #22), live OAuth, and a live `ListBooks`
filtered to PROJECT + membership remain. It does not close #169,
#172, #184, #165, #166, or #168. It does not close #150. It does
not close #22. It does not reopen #151.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show two fixture jobs mapping to per-book remaining and a
program billed that sums only the books that cited billed, an
unbilled job leaving that book's billed / remaining / collected
blank rather than inventing 0.00, a non-member PROJECT book with a
matching `org_id` staying out, and `journal:append` being rejected
as a scope. It cannot show a Connect token opening a book, a
mega-book in the kernel, EAC / forecast, AIA G702 product UI, or a
vendor portal.

### Amendment, 2026-09-04 — Personal brokerage statement ingest

[#167](https://github.com/mattmarshall/ratio/issues/167) asked for
BookKind-aware brokerage / custodian CSV ingest into the journal for
Personal books — #72 parity with the fund trade loop (#155) and the
Project job-cost loop (#171) — without a second recon engine, without
lot relief on household Investments, and without broker OAuth.

**What this amendment records.** CreateBook(Personal) seeds
`brokerage-statement` and `brokerage-positions` beside
`bank-statement` and `loan-payment`. The column contract is the same
custodian / broker CSV the fund path already reads (`B/S`, ISIN /
ticker, consideration). Identified buys and sells post as household
transfers (`xfer_cash_investments` / `xfer_investments_cash`) onto
the Investments asset — not `equity_purchase`. Holdings are
recorded and never posted. `ratio recon --from-ingest` is the same
engine: unidentified or foreign-currency holdings refuse the whole
run and write no breaks; a difference is a `BreakReport` at the
Investments account; exit 2 is a break, exit 3 is a refusal. The
kernel still refuses an unbalanced entry at the door. No new
`Method` / `Order` / `lot_method` variant. No `screensFor` fork.
Lot relief stays unset (#187). A Personal book has no NAV gate —
`ratio strike` is not this path. Broker OAuth stays Connect.
**Personal brokerage statement ingest** is the Built phrase this
amendment adds.

**What this is NOT:**

- **Not broker OAuth or multi-broker adapters.** Those stay Connect
  apps (#165 / #150). Grant path leftover #22 / #150. This file does
  not close #22. It does not close #150. It does not reopen #151.
- **Not optional lot relief on household Investments.** That door
  is #187. A buy that opened a lot would be that issue wearing an
  ingest sticker.
- **Not cash forecast, envelope depth, or household multi-currency.**
  Those doors are #163, #164, #178.
- **Not a second recon engine**, and not a `screensFor` fork. The
  write is `ingest` / `admit` on the seeded templates; the compare
  is `recon --from-ingest`.

Nothing on the *Explicitly not building* list moved. This amendment
closes #167. It does not close #187, #165, #163, #164, #178, #155,
#150, or #22.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook a Personal book, ingest a brokerage / custodian
CSV under `brokerage-statement` and `brokerage-positions`, admit
the identified transfers onto `journal.jsonl`, leave an unmatched
instrument pending, run `ratio recon --from-ingest`, and see an
unidentified or foreign-currency holding refuse the whole run
(exit 3, no breaks). It cannot show a broker OAuth grant, a lot
opening on Investments, a household NAV, a cash forecast, or
envelope coaching.

### Amendment, 2026-09-04 — Personal books declare currencies

[#178](https://github.com/mattmarshall/ratio/issues/178) asked for
multi-currency on PERSONAL books without a silent USD and without a
second FX engine. #7 already made postings carry currency and
conservation hold per code. #160 already made a translation residue,
a missing rate, an entry only one view can place, and a view declared
after the fold refuse rather than print 0.00. The leftover was the
label: ListBooks / GetBook filled the fund reporting constant on
every household, so an undeclared book wore USD.

What landed is the declaration, not a Personal FX method:

- Configuration: `[personal] currencies = ["EUR", "GBP"]`. First
  code is the reporting base `Rates` translates into. Empty is unset
  — not a silent USD. CreateBook writes no list.
- API: `Book.currency_code` and `Book.currencies` cite that
  election. ApplyEvent carries an optional `currency`; a declared
  household that omits it, names a code that is not on the list, or
  an undeclared household that names a code, refuses. Compiled legs
  are stamped. The journal door still refuses `[USD +100, EUR −100]`.
- Translation: the same `Rates` / missing-rate refuse #160 already
  cites. No new `Method` / `Order` / `lot_method` variant. No
  `screensFor` fork. FX rate providers stay Connect fact apps.
  **Personal books declare currencies** is the Built phrase this
  amendment adds.

**What this is NOT:**

- **Not an FX rate-vendor Connect app.** That door is not opened
  here. Live rate facts stay leftover on #178.
- **Not a console `/trade` currency picker.** Chrome is unchanged.
- **Not #163 (cash forecast), #164 (envelope budget), or #187
  (household lots).** Those doors stay where they are.
- **Not a `screensFor` fork**, and not a Personal-only FX engine.

Nothing on the *Explicitly not building* list moved. This
amendment does not close #178 — live FX rate Connect apps and a
currency picker remain. It does not close #160 (already closed).
It does not start #163, #164, or #187. It does not close #165,
#166, or #168.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook a Personal book, leave currency unset, write
`[personal] currencies`, post a named EUR expense that conserves,
and see `[USD +100, EUR −100]` refused at the journal door. A
USD-base household holding EUR without a rate fact refuses the
sheet with the same missing-rate sentence the fund path uses. It
cannot show a rate-vendor Connect app, a `/trade` currency
picker, a cash forecast, or envelope coaching.

### Amendment, 2026-09-04 — NAV gate reasons stay citeable

[#188](https://github.com/mattmarshall/ratio/issues/188) asked the
console to cite why a fund refuses its own NAV — unexplained break,
unpriced, unresolved trade — instead of a bare HTTP 400. The gate
already existed (`Console::blocking_at`; #156). Chrome printed
BLOCKED and left the reasons on the CLI.

What landed is the cite, not a new gate:

- Engine: `blocking_at` is still the one fold. GetFund / GetView
  carry it as `nav_gate` (unexplained breaks, unresolved trades,
  unpriced). ListFunds / ListViews leave it unset — those indexes
  do not fold. Unpriced stays empty unless a valuation date was
  named, the same limit a bare `ratio strike` already has. Tests
  fail if the field drifts from the fold, or if unpriced appears
  without as-of.
- Console: fund overview and view chrome cite the three reasons
  (`NavGateCite` / `withRefusal`) before the NAV tile. A thrown
  `Refused` is the sentence, not a status number. `fields_test`
  needles and the render suite hold the phrases.

No new `Method` / `Order` / `lot_method` variant. No `screensFor`
fork. No new gate semantics — what blocks did not change. **NAV
gate reasons stay citeable** is the Built phrase this amendment
adds.

**What this is NOT:**

- **Not #26 (console buildout).** Broader screens stay on #26.
- **Not #186 (point-in-time / restatement browser).**
- **Not #157 (capital-call notice).**
- **Not #159 (Postgres).** Stage E stays blocked.
- **Not a `screensFor` fork** for Personal or Project. Those
  books do not strike a NAV.

Nothing on the *Explicitly not building* list moved. This
amendment closes #188. It does not close #26, #186, or #157. It
does not close #159. It does not reopen #156, #160, #180, or
#181.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open a blocked Investment book and read unexplained break /
unpriced / unresolved trade as refuse copy on the fund overview
and on every view screen, rather than a bare HTTP 400. It cannot
show a point-in-time restatement browser, a capital-call notice,
or the rest of the #26 console buildout. Those remain #186 /
#157 / #26.

### Amendment, 2026-09-04 — optional lot relief on household Investments

[#187](https://github.com/mattmarshall/ratio/issues/187) asked for an
optional election so PERSONAL books can use the wash / MinTax /
SpecID / average-cost lot engines already on main — for tax cites —
without fund ABOR chrome. #167 posted brokerage buys as household
transfers and left this door named.

**What this amendment records.** `[personal] lot_relief = true` elects
the existing engines. `None` stays unset — CreateBook, bank ingest,
and an unelected brokerage admit still post transfers, and the lot
book stays empty. `Some(false)` is refused at read — omit the field.
The election is not a `Method` / `Order` / `lot_method` variant;
`lot_method = "wash" | "min_tax" | "average_cost" | "specific_id"`
stays refused. Electing requires `[chart_roles]` (investments / cash
/ realized_gain on `chart_for(Personal)`), or a sale cannot post a
gain the chart has not named. Brokerage-statement still *names*
`xfer_cash_investments` / `xfer_investments_cash`; admit remaps onto
`equity_purchase` / `equity_disposal` only when someone wrote the
election. GetFund cites wash / min-tax / average-cost the same way
the fund path does; SpecID stays per-sale on the journal entry.
`screensFor` is not forked — PERSONAL does not wear Positions / NAV
/ Exceptions. Bank OAuth and tax e-file stay Connect.
**optional lot relief on household Investments** is the Built phrase
this amendment adds.

**What this is NOT:**

- **Not a new Method / Order / lot_method variant.** MinTax, SpecID,
  average cost, and wash keep the election shapes already on main
  (`min_tax_short_weight`, `identified_lots`, `average_cost`,
  `wash_window_days` / `wash_keep_holding_period`).
- **Not fund ABOR chrome on a Personal book.** Positions, NAV
  strikes, and Exceptions stay off `screensFor(PERSONAL)`.
- **Not broker OAuth or tax e-file.** Those doors are #165 / #166 /
  Connect. This file does not close #165. It does not close #166.
- **Not household multi-currency, cash forecast, or envelope
  coaching.** Those doors are #178, #163, #164.

Nothing on the *Explicitly not building* list moved. This amendment
closes #187. It does not close #165, #166, #163, #164, #178, #167,
#155, #150, or #22.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook a Personal book, leave lot relief unset (a
brokerage buy is still a transfer), elect `[personal] lot_relief =
true` with `[chart_roles]`, post or admit an instrumented buy that
opens a lot, dispose it under the engines already on main, and
point at wash / min-tax / average-cost on GetFund and at
`identified_lots` on the journal entry. It cannot show Positions /
NAV chrome on that household, a broker OAuth grant, IRS e-file, a
household NAV, a cash forecast, or envelope coaching.

### Amendment, 2026-09-04 — a capital-call / distribution notice is a citeable document

[#157](https://github.com/mattmarshall/ratio/issues/157) asked for a
capital-call / distribution notice a walk-through can cite. The
journal already had partner capital activity (`call_*` /
`distribute_*`). `/capital` cited remaining undrawn. What was
missing was the **document**: digest + partner cut + amounts, not
a waterfall.

What landed is the notice, not a waterfall engine:

- Lean: `Ratio.Partners.Notice`. `issue` is pro-rata of a named
  total (`allocate`) — no preferred return, no catch-up, no carry.
  `fromPosted` cites the amounts the journal posted. Applying
  `issue` to a partner-scoped LP call invents the other partners
  (`fromPosted` of 250_000 on LP is not 80/20 of 250_000). No cut,
  a zero amount, or an empty posted list stays unset. Rewriting
  amounts is a different document.
- Rust: `notice_from_posted` / `issue_notice` / `notice_digest`.
  GetBook walks `call_*` / `distribute_*` (cash + partner capital,
  call draws undrawn; unit movements stay off the list). The cut
  is the configuration the entry pinned — not `active()`. Index
  stays empty.
- Console: `/capital` lists notices (digest, cut, posted amounts)
  and links each to the journal entry. Empty is unset. A dated
  window drops undated and out-of-window rows. No `screensFor`
  fork.

**What this is NOT:**

- Not an **LP portal, e-sign, or CRM sync**. Those stay Connect
  (#161 / #150). `partners:write` is still not notices-as-a-product.
- Not a **waterfall, preferred return, or carry** in the kernel.
- Not #186 (restatement browser), #26 (broader console), or #188
  (already closed). #159 stays blocked on Postgres.

Nothing on the *Explicitly not building* list moved. This
amendment closes #157. It does not close #161, #186, or #26.
It does not reopen #82, #180, or #181.

**What a walk-through can and cannot show** (demo readiness, #27).
It can CreateBook(Investment), record `commit_lp` then `call_lp`,
and cite the notice on `/capital`: digest, LP 80 / GP 20, LP's
posted amount — not 80/20 of that LP call. A distribution posts
its own notice. A book that has never called shows unset. It
cannot show an LP portal, e-sign, CRM, a future call schedule,
IRR, or a waterfall. Those remain Connect.

### Amendment, 2026-09-04 — point-in-time / restatement reporting stays citeable

[#186](https://github.com/mattmarshall/ratio/issues/186) asked the
console to browse point-in-time / restatement reporting the core
already has: a pinned journal prefix + config digest on a close
or a strike, and WashRestatement as a citeable record. The engine
could cite those. Chrome had no period / prefix picker that
surfaced them honestly.

What landed is the browser, not a new figure:

- Engine: `ListNavStrikes` / `GetNavStrike` carry `wash_qualified`
  and the two restatement strings (`wash_restatement_original` /
  `wash_restatement_moved`). Empty is unset — not a silent 0.
  A book that never wrote `wash_window_days` leaves all three
  unset, not a silent 30. A later wash produces a restatement
  that cites the strike (`Ratio.Lots.WashRestatement.restate`).
  `net_asset_value` is the number somebody was paid on and is
  never rewritten.
- Console: `/asof` is one screen on every `screensFor` list —
  not a kind fork, not composed onto `/close` (that page is
  already at the three-RPC ceiling). Period chips and a prefix
  picker (`now` / a close / a strike). A close or a strike cites
  journal position, journal digest, and config digest. `now`
  cites the maintained fold's position and leaves digest and
  config unset — the head is not a historical pin. An empty
  journal, a missing close, or a missing strike stays unset.
  WashRestatement cites original → moved when present; otherwise
  "Unset — no wash restatement". `fields_test` needles and the
  render suite hold the phrases.

No new `Method` / `Order` / `lot_method` variant. No
`screensFor` fork. **point-in-time / restatement reporting stays
citeable** is the Built phrase this amendment adds.

**What this is NOT:**

- **Not performance attribution.** That stays refused.
- **Not rewriting a struck figure.** #146 / `rewrite_in_place`
  stay refused. A restatement is a new record that cites the
  strike.
- **Not #26 (console buildout).** Broader screens stay on #26.
- **Not #161 (Connect LP portal) or #159 (Postgres).** #159
  stays blocked.
- **Not a `screensFor` fork.** Kind still selects one list.

Nothing on the *Explicitly not building* list moved. This
amendment closes #186. It does not close #26. It does not
close #161 or #159. It does not reopen #146, #157, or #188.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open `/asof`, pick a period, pin a close or a strike, and
read the journal prefix + digest + config digest that record
already named. A strike taken while a wash window was open
shows qualified; a later repurchase that moved the realized
gain shows WashRestatement citing that strike — the struck NAV
is unchanged. A book with no close, no strike, or no
restatement shows unset, not 0.00. It cannot show performance
attribution, a rewritten struck figure, an LP portal, or the
rest of the #26 console buildout. Those remain refused /
Connect / #26.

### Amendment, 2026-09-04 — a Personal cash forecast, unset when none exist

[#163](https://github.com/mattmarshall/ratio/issues/163) asked for a
forward-looking household cash figure that is not a fake zero. Period
cash-flow at `/cashflow` already cited actuals (`filter=cashflow-YYYY[-MM]`).
A walk-through could not show a forecast. Inventing one from envelopes,
payroll, or a bank-balance predictor would have been a silent current
bucket on a book that had posted none of those things.

What landed is a citeable forecast at the same `/cashflow` URL, folded
from the same journal (`filter=forecast-YYYY[-MM]`). No new RPC, no
second store, no proto resource, no `screensFor` fork. Personal only.
Operating keeps period cash-flow and does not wear the forecast cite.

**The material the fold will name.** Posted `scheduled_*` / `forecast_*`
journal kinds only (`scheduled_income`, `scheduled_spend`,
`forecast_income`, `forecast_spend`). ApplyEvent sets
`JournalEntry.kind` from the rule-id prefix. A future-dated
`spend_cash` is still an actual. Actuals folds (sheet, P&L, bridge,
period cash-flow, the maintained projection) skip forecast material so
a scheduled rent is not this month's operating cash.

**Unset stays unset.** No scheduled or forecast entry in the window is
not a measured $0.00. A posted income and spend that net to zero is a
real zero. Payroll and envelope kinds refuse at the door — they are
not invented.

**What this amendment records.** CreateBook(Personal) writes the four
templates. `forecast-YYYY[-MM]` is Activity-shaped over those kinds.
The console cites scheduled net cash on `/cashflow`. Bank-balance
predictors and calendar bills sync stay Connect (`journals:post` +
`statements:read`; `journal:append` is a refused alias). No new
`Method` / `Order` / `lot_method` variant. **a Personal cash forecast**
is the Built phrase this amendment adds.

**What this is NOT:**

- **Not envelopes or payroll.** Those stay refused. Envelope coaching
  is #164. No `forecast_payroll` / `forecast_envelope` rule, no
  payroll account on `chart_for(Personal)`.
- **Not a bank-balance predictor, not calendar bills sync.** Those
  stay Connect. This file does not close #165.
- **Not a `screensFor` fork, not `ratio watch` product UI.** The cite
  composes onto `/cashflow`. Fund / Project / Operating / Investment
  do not wear it.
- **Not #178 leftovers (FX Connect /trade).** This file does not
  absorb them.

Nothing on the *Explicitly not building* list moved. This file does
not close #163 — bank-balance predictors and calendar bills sync
remain. It does not close #164, #165, #168, or #150. It does not
reopen #98.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show beginning / operating / investing / financing / ending
actuals as before, and a scheduled net cash forecast when
`scheduled_*` or `forecast_*` entries exist in the window. An empty
book, or a book of actuals only, leaves the forecast unset — not
0.00. A net-zero pair of scheduled posts is a real zero. It cannot
show envelope coaching, payroll, a bank-balance predictor, calendar
bills sync, or a client portal.
### Amendment, 2026-09-04 — kind-aware IA

[#175](https://github.com/mattmarshall/ratio/issues/175) asked for an
audit of `screensFor` + nav after #65 closed personal IA. The lists
were already kind-selected: Personal / Project / Operating did not
offer Exceptions / Positions / NAV as tabs. A typed URL, a legacy
`/breaks` redirect, or a palette "Open by id" row still rendered
fund-ops chrome on those books — a fake label on fund screens,
the same defect #65 named.

What landed is the gate, not a second chrome list:

- `wearsFundOps` is the one predicate. Investment and the
  unspecified operations surface (proto default, not a domain)
  wear Exceptions / Positions / NAV strikes, trade / mark, corporate
  actions, and dual-view recon. Personal / Project / Operating 404.
- Palette deep-links drop "as an exception" / "as a position" /
  "as a NAV strike" / "as a corporate action" on those kinds.
  Exact AIP names still translate; the page 404s.
- The period-close footer no longer offers Period P&L / Balance
  sheet on every kind — that was leftover household chrome on
  Project and Investment. Trial balance stays: it is the
  conservation view every kind already wears.
- One `screensFor` list. Kind selects chrome. Investment keeps
  the warehouse. **kind-aware IA** is the Built phrase this
  amendment adds.

**Leftover fund-only terms found on Personal / Project (not
absorbed):** typed `/wip` and `/billing` still render on a
Personal book (project figures, not fund-ops — #66 / #85 / #26).
Typed `/sheet` and `/pnl` still render on Investment (household
figures). The books-header "Funds" link is workspace nav, not
book chrome. Those stay on #26.

**What this is NOT:**

- **Not #26 (console buildout).** Broader screens stay on #26.
- **Not a `screensFor` fork.** Kind still selects one list.
- **Not a new Method / Order / lot_method variant.**
- **Not Connect #161 / #162.** Drip / LP portal stay Connect.
- **Not #159 (Postgres).** Stage E stays blocked.

Nothing on the *Explicitly not building* list moved. This
amendment closes #175. It does not close #26. It does not
close #161, #162, or #159. It does not reopen #65, #66, #85,
#186, or #157.

**What a walk-through can and cannot show** (demo readiness, #27).
It can open a Personal, Project, or Operating book and not see
Exceptions / Positions / NAV / trade / mark in the hub, the
palette, or a typed fund-ops URL (those 404). It can open an
Investment book and still reach capital, NAV roll-forward,
Exceptions, Positions, and NAV strikes. It cannot show the rest
of the #26 console buildout, a Connect LP portal, or Postgres.
Those remain #26 / Connect / #159.

### Amendment, 2026-09-04 — Personal cash-forecast Connect predictors, and the grant path still does not open

[#163](https://github.com/mattmarshall/ratio/issues/163) asked for bank
balance predictors and calendar bills sync as Connect apps after the
core cite landed (`/cashflow`, `filter=forecast-YYYY[-MM]`; unset when
none). The issue body still says `journal:append`. That string is an
alias. The catalog already said a cash forecast stays a Connect app
and that `journals:post` is the write grant. What landed is those two
apps as sibling trees, not a kernel method and not a `screensFor` fork.

**What this amendment records.**
[`connect/bank-balance-predictor/`](connect/bank-balance-predictor/)
and [`connect/calendar-bills/`](connect/calendar-bills/) are first-party
WorkOS Connect OAuth applications for `BookKind` PERSONAL. Both declare
`statements:read` and `journals:post` — the frozen names, not the issue
body's stale alias. The predictor instantiates CreateBook(Personal)
`forecast_income` / `forecast_spend`. The bills app instantiates
`scheduled_income` / `scheduled_spend`. ApplyEvent already marks
`JournalEntry.kind` from the rule-id prefix; a future-dated
`spend_cash` stays an actual and is refused here. `journals:post` is
allowlisted per `client_id`; an empty allowlist refuses every post. A
dated row on or before closed-through refuses the batch. Instantiated
legs must conserve in every currency; `[USD +100, EUR −100]` is not a
posting. Money is minor units, split on the point. Payroll and
envelope kinds refuse — #164 stays refused, and this file does not
rebuild envelope chrome. No new `Method` / `Order` / `lot_method`
variant. PERSONAL chrome is unchanged — `screensFor` is not forked.
Project EAC stays [`connect/eac-forecast/`](connect/eac-forecast/).
**a Personal bank-balance predictor Connect app** and **a Personal
calendar-bills Connect app** are the Built phrases this amendment adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect tokens
  on ConnectApiUrl. `fetch_statements()` and `deliver()` still refuse.
  Live OAuth is leftover #22 / #150. Write-route actor binding
  landed (#151). This file does not close #150.
- **Not live bank or calendar OAuth.** No Plaid / MX / TrueLayer token,
  no Google Calendar / Outlook grant. The mappers accept a normalized
  predicted movement or a dated occurrence. Provider wiring stays
  leftover on #163.
- **Not the #218 core cite.** `/cashflow` already folds posted
  `forecast_*` / `scheduled_*`. This file does not redo that slice.
- **Not envelopes or payroll.** Those stay refused. Envelope coaching
  is #164. This file does not reopen #164.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**
  The forecast fold and the closed-through gate stay where they are.
- **Not #165's live bank-feed OAuth leftover.** That door stays on
  #165. This file does not close #165.

Nothing on the *Explicitly not building* list moved. Client portal, CRM,
tax e-file, vendor portal, and waterfall stay Connect-apps or stay
refused. This file does not close #163 — grant path and live provider
OAuth remain. It does not close #164, #165, #168, or #150. It does
not reopen #98 or #218.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show a fixture predicted deposit mapping to `forecast_income`,
a fixture rent mapping to `scheduled_spend`, a closed March refusing a
15 March row, an empty allowlist refusing everything, `journal:append`
being rejected as a scope, and payroll / envelope kinds being refused.
It cannot show a Connect token opening a book, a live bank or calendar
login, envelope coaching, payroll, or a posting that reached `/v1`.

### Amendment, 2026-09-04 — an audit-export Connect app, and the grant path still does not open

[#185](https://github.com/mattmarshall/ratio/issues/185) asked for an
audit evidence ZIP of period closes, NAV strikes, break reports /
explanations, and digests without a kernel blob store and without
replacing period close. The catalog already reserved `audit:export`
as a read of cites. What landed is that app as a sibling tree, not
a kernel method and not a `screensFor` fork.

**What this amendment records.** [`connect/audit-export/`](connect/audit-export/)
is a first-party WorkOS Connect OAuth application. It declares
`audit:export`, `closes:read`, `breaks:read`, `breaks:explain`,
`nav:read`, `journals:read`, `config:read`, and `books:read` — the
frozen names, not the catalog aliases `journal:read` /
`journal:append`. It is read-only relative to the journal:
`journals:post` is not requested. The empty allowlist refuses every
post. Pack contents are kernel cites already on the book
(`PeriodClose`, `NavStrike`, `Break` / `BreakExplanation`, the
`journals:read` prefix, the `config:read` RuleSet pin). Unset stays
unset. A missing cite is named in the pack manifest, not a silent
empty file that looks complete. An empty journal digest is unset,
not history-intact. A missing NAV strike is unset, not NAV 0.00.
A missing `BreakReport` is unset, not a silent reconciled-empty
file. A cited report with no lines is cited-empty — the kernel's
"the period reconciled", and the manifest says so. A posted
`"0.00"` is a real zero. `books:read` membership; an `org_id`
claim is not membership. Kind-aware cites, not a chrome fork:
closes and digests apply to every kind; NAV strikes and breaks stay
unset on kinds that do not wear fund-ops. No new `Method` /
`Order` / `lot_method` variant. No blob store, no period-close
replacement, no LP portal, no e-sign, no second journal.
**an audit-export Connect app** is the Built phrase this amendment
adds.

**What this is NOT:**

- **Not live Connect OAuth.** API Gateway JWT verifies Connect
  tokens on ConnectApiUrl. `fetch_cites()` and `deliver()` still
  refuse. Write-route actor binding landed (#151). Live OAuth is
  leftover #22 / #150. This file does not close #22. It does not
  reopen #151.
- **Not a live ZIP against `/v1`.** Unit tests assert the refuse and
  the pack shape from fixtures. A green cite is not a live token.
- **Not a kernel blob store, not `ratio close`.** `store_blob()` and
  `close_period()` refuse. Period close stays a person at a terminal.
- **Not #161 (LP portal), not e-sign, not a second journal.**
  `lp_portal()`, `esign()`, and `second_journal()` refuse.
- **Not #150's read-only reference skeleton.** That leftover is
  `books:read` + `statements:read` proving the door opens. This app
  requests `audit:export` and the read scopes above and does not
  open the door.
- **Not a kernel RPC, not `ratio watch`, not Console product UI.**
  `/close`, `/asof`, Exceptions, and NAV strikes stay the core cites.

Nothing on the *Explicitly not building* list moved. Client portal,
CRM, tax e-file, vendor portal, AIA G702 product UI, and waterfall
stay Connect-apps or stay refused. This file does not close #185 —
grant path (leftover #22) and live ZIP delivery remain. It does not
close #161, #165, #166, #168, #169, #172, #184, or #179. It does
not close #150. It does not close #22. It does not reopen #151.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show a fixture `PeriodClose` mapping to `closes.csv`, a
`NavStrike` mapping to `strikes.csv` without rewriting a
`WashRestatement`, a missing cite leaving that sheet out of the ZIP
and naming the gap on `unset.csv`, an empty digest staying unset
rather than history-intact, and `journal:read` being rejected as a
scope. It cannot show a Connect token opening a book, a live ZIP
against `/v1`, a kernel blob store, a period-close replacement, an
LP portal, or e-sign.

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
  **Landed** (#158): [`ConfigStore`](crates/ratio-store/src/lib.rs) and
  [`DirectoryConfigStore`]. The fact-plane half is [`FactStore`]
  (`facts.jsonl`, append-only, provenance required). A parallel
  mutable shadow book is refused on the recon `--post` path.
- **later** — crova behind `put`/`get`, geetch behind `history` and the review
  flow that gates `set_active`. Not this PR.

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

### Amendment, 2026-09-04 — Connect tokens accepted with catalog scopes

The leftover on issue 22 after #151 was the `/v1` authorizer still
proving an AuthKit session JWT only. A Connect-shaped token that
arrived was `scoped` and did not match `org:{id}`, but it had no
catalog grant: `books:read` was not a door.

What landed is the in-process authorizer, not a new RPC and not a
second identity product:

- `from_request_context` reads the `scope` claim on a Connect-shaped
  JWT and keeps only frozen names from
  [`docs/connect-scopes.md`](docs/connect-scopes.md). Aliases
  (`journal:read`, `journal:append`) and hard non-scopes
  (`rules:approve`, `config:promote`, portal impersonation) never
  enter the grant set.
- `transcode::serve` requires one of those names before a handler
  opens a book. AuthKit sessions and `Local` skip the table —
  membership stays their grant. Every live `ROUTES` entry names a
  door; a forgotten route would skip the grant.
- A Connect token still never takes `RATIO_DEMO_OPEN` and still
  never matches `org:{id}` (#151). Authorized-empty / "no fund" for
  a book the subject does not administer. An `org_id` claim is not
  membership.
- Write scopes that name a template (`journals:post`, `calls:post`,
  `fees:accrue`, `lots:elect`) share `ApplyEvent`. Read does not
  imply write.

**Connect tokens accepted with catalog scopes** is the Built phrase
this amendment adds.

**What this is NOT, because leftovers stay named on issue 22:**

- The `RATIO_DEMO_OPEN` leftover named here is recorded by the
  later deployed-demo dial amendment.
- **Not removing unused Cognito CloudFormation resources.**
- **Not live provider OAuth.** In-process, verified Connect claims
  with catalog scopes are accepted. Dashboard registration,
  redirect, and bank / calendar OAuth stay leftover. First-party
  Connect scaffolds still refuse `fetch` / `deliver` until those
  leftovers move. The API Gateway JWT leftover named here is
  recorded by the next amendment. This amendment does not finish
  issue 22.
- **Not the `journals:post` allowlist** keyed by `client_id`. Empty
  still refuses every post at the app; the kernel map stays on #150.
- **Not reserved RPCs** (`webhooks:journal`, `nav:strike` as a write,
  a kernel blob store). `audit:export` reads the change log; it does
  not mint a ZIP.

Nothing on the *Explicitly not building* list moved. This amendment
does not close #150. It does not reopen #151. It does not finish
#163, #165, #166, #168, #169, #172, #184, #179, or #185.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show a Connect-shaped token with `books:read` listing a book
the subject administers, `breaks:read` opening the exception queue,
`audit:export` reading the change log, and a token without the
scope / with `journal:read` / with `rules:approve` being refused.
It can show an AuthKit session still walking `/v1/books` without
Connect scopes. The open-dial leftover named here is recorded by
the later amendment. It cannot show live bank or calendar OAuth,
or a ZIP that reached `/v1`.

### Amendment, 2026-09-04 — API Gateway JWT verifies Connect tokens

The leftover on issue 22 after #224 was the HTTP API JWT authorizer
still proving only the AuthKit session `iss`
(`https://api.workos.com/user_management/{client_id}`). A
Connect-shaped token that arrived in-process was accepted with
catalog scopes, but WorkOS Connect access tokens mint a different
`iss` — the AuthKit custom domain
(`https://auth.ratio.marsh.build`) — and API Gateway refused them
at the edge. Connect apps (#163 / #185 / #184 / #169 / #179 / …)
never reached `/v1`.

**The exact refuse of one authorizer.**
`AWS::ApiGatewayV2::Authorizer` `JwtConfiguration.Issuer` is a
single string. CloudFormation will not OR two issuers. Pointing
the console authorizer at the Connect domain 401s every AuthKit
session. Pointing a Connect authorizer at the session path 401s
every Connect token. A route-key split on the console API would
invent a second URL prefix the in-process accept path does not
serve.

What landed is a second HTTP API, not a new identity product and
not a Lambda authorizer:

- `ConnectApi` (`ratio-demo-connect`) proxies the same Lambda.
  `ANY /v1/{proxy+}` requires JWT. Public screens stay on DemoUrl.
- `ConnectAuthorizer` (`workos-connect-jwt`) proves
  `WorkOsConnectIssuer` (default `https://auth.ratio.marsh.build`,
  OIDC discovery and `jwks_uri` `/oauth2/jwks` verified 2026-09-04)
  and audience `WorkOsClientId` — Connect `aud` is the Ratio
  WorkOS project client, not `azp`.
- AuthKit sessions stay on DemoUrl / `WorkOsIssuer`. Connect apps
  call `ConnectApiUrl`. Same `/v1` path. Same in-process fence:
  membership required, never `RATIO_DEMO_OPEN`, never `org:{id}`.
- `//deploy:iac_test` fails if the gateway is AuthKit-issuer-only
  again (no `WorkOsConnectIssuer`, Connect authorizer citing
  `WorkOsIssuer`, or missing Connect API / `/v1` route).

**API Gateway JWT verifies Connect tokens** is the Built phrase
this amendment adds.

**What this is NOT, because leftovers stay named on issue 22:**

- The `RATIO_DEMO_OPEN` leftover named here is recorded by the
  next amendment.
- **Not removing unused Cognito CloudFormation resources.** They
  stay unused so a stack update does not destroy the live pool.
- **Not live provider OAuth.** Dashboard registration, redirect,
  and bank / calendar OAuth stay leftover. First-party Connect
  scaffolds still refuse `fetch` / `deliver`. This file does not
  close #22.
- **Not the `journals:post` allowlist**, reserved RPCs, or a
  kernel blob store. Those stay on #150 / the app issues.

Nothing on the *Explicitly not building* list moved. This
amendment does not close #150. It does not reopen #151. It does
not finish #163, #165, #166, #168, #169, #172, #184, #179, or
#185.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show an unauthenticated request to ConnectApiUrl `/v1`
returning 401, and a Connect-shaped token whose `iss` is the
AuthKit custom domain reaching the same `/v1` accept path that
#224 opened. It can show an AuthKit session still walking DemoUrl
`/v1`. The open-dial leftover named here is recorded by the
next amendment. It cannot show live bank or calendar OAuth, or
a ZIP that reached `/v1`.

### Amendment, 2026-09-04 — RATIO_DEMO_OPEN defaults off on the deployed demo

The leftover on issue 22 after #226 was the shared DemoUrl still
setting `RATIO_DEMO_OPEN`, so any AuthKit session received every
fund. Isolation already held on the `scoped` path in CI. A live
two-user walk-through on execute-api could not show isolation
until that dial was off.

What landed is the production-safe default, not a new identity
product:

- `deploy/app.yaml` no longer sets `RATIO_DEMO_OPEN`. Unset (or
  empty) is off — `ratio watch` already treated a missing value
  that way. AuthKit sessions on DemoUrl go through
  `Console::for_request(..., false)` and isolate via
  `MEMBERSHIP.tsv`.
- `RATIO_DEMO_MEMBER` still seeds that file from the `DemoMember`
  / `DEMO_MEMBERS` list (WorkOS `sub` and/or email). CreateBook
  still grants the creator's `sub`. A second AuthKit subject not
  on the list sees authorized-empty / "no fund", never the first
  subject's journal.
- The dial remains: `RATIO_DEMO_OPEN=1` on a local `ratio watch`
  or a CI job restores the shared rail for AuthKit sessions only.
  A Connect-shaped token still never takes it and still never
  matches `org:{id}` (#151). AuthKit stays on DemoUrl; Connect
  tokens stay on ConnectApiUrl.
- `//deploy:iac_test` fails if the function Environment sets
  `RATIO_DEMO_OPEN` again, and fails if `RATIO_DEMO_MEMBER` is
  dropped so the seed vanishes with the dial.

**RATIO_DEMO_OPEN defaults off on the deployed demo** is the
Built phrase this amendment adds.

**What this is NOT, because leftovers stay named on issue 22:**

- **Not live provider OAuth.** Dashboard registration, redirect,
  bank / calendar OAuth, and a Connect-app two-user walk-through
  stay leftover. First-party Connect scaffolds still refuse
  `fetch` / `deliver`. This file does not close #22.
- **Not removing unused Cognito CloudFormation resources.** They
  stay unused so a stack update does not destroy the live pool.
- **Not rewriting `DEMO_MEMBERS`.** The default remains
  `demo@ratio.fastverk.dev`. AuthKit access tokens always carry
  `sub`; email is optional. If that Cognito-era address never
  appears on the token, seeded funds stay authorized-empty until
  `DEMO_MEMBERS` names a live WorkOS `sub` (or an email the token
  carries). Isolation still holds — nobody gets every fund. Two
  operators can each CreateBook and not see the other's book
  without any variable change.
- **Not the `journals:post` allowlist**, reserved RPCs, or a
  kernel blob store. Those stay on #150 / the app issues.

Nothing on the *Explicitly not building* list moved. This
amendment does not close #150. It does not reopen #151. It does
not finish #163, #165, #166, #168, #169, #172, #184, #179, or
#185.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show two AuthKit subjects isolated on DemoUrl: the subject
named in `RATIO_DEMO_MEMBER` (or the creator of a book) sees that
rail; a second session sees authorized-empty / refuse, not every
fund. It can still show a Connect-shaped token refused the open
dial. first-party Connect apps call ConnectApiUrl. It cannot show
live bank or calendar OAuth, a Connect dashboard registration, or
unused Cognito resources removed.

### Amendment, 2026-09-04 — first-party Connect apps call ConnectApiUrl

The leftover on issue 22 after #227 was first-party Connect
scaffolds still refusing `fetch_*` / `deliver` solely because
"the grant path is not built", and comments that still said
Connect tokens are not accepted on `/v1`. In-process catalog
accept (#224) and API Gateway Connect JWT (#226) were already
on main. The shared DemoUrl dial was already off (#227).

What landed is the live OAuth grant helper, not a second IdP
and not a kernel blob store:

- `connect/grant.py` presents a verified Connect access token
  (argument or `RATIO_CONNECT_ACCESS_TOKEN`) or mints one via
  WorkOS Connect `authorization_code` / `client_credentials`
  against `WORKOS_CONNECT_ISSUER` (default
  `https://auth.ratio.marsh.build`). Per-app credentials are
  `WORKOS_CONNECT_CLIENT_ID` / `WORKOS_CONNECT_CLIENT_SECRET`.
  `WORKOS_CLIENT_ID` is the audience, not a second issuer.
- HTTP goes to `RATIO_CONNECT_API_URL` (ConnectApiUrl). DemoUrl
  (`RATIO_API_ORIGIN`) is refused as a collision. A Connect
  token never takes `RATIO_DEMO_OPEN` and never matches
  `org:{id}` (#151). Membership is still required at `/v1`.
- `connect/audit-export/` `fetch_cites` pulls book / close /
  strike / break / config cites; `deliver` writes the ZIP after
  that pull — it does not POST a blob. Sibling scaffolds use
  the same helper for the identical refuse→grant.
- Bank OAuth, calendar OAuth, licensed AIA forms, IRS e-file,
  silent empty ZIPs, and a kernel blob store stay refused.

**first-party Connect apps call ConnectApiUrl** is the Built
phrase this amendment adds.

**What this is NOT, because leftovers stay named on issue 22:**

- **Not WorkOS dashboard registration.** A human still registers
  the Connect application, redirect, and a live token. Unit
  tests inject a transport.
- **Not rewriting `DEMO_MEMBERS`.** The default remains
  `demo@ratio.fastverk.dev`. If that address never appears on
  the token, seeded funds stay authorized-empty until the list
  names a live WorkOS `sub`.
- **Not removing unused Cognito CloudFormation resources.**
  They stay unused so a stack update does not destroy the live
  pool.
- **Not live bank / calendar OAuth product UI**, licensed AIA
  PDF, IRS e-file, or a kernel blob store.
- **Not the `journals:post` allowlist**, reserved RPCs, or
  #150's read-only reference skeleton.

Nothing on the *Explicitly not building* list moved. This
amendment does not close #150. It does not reopen #151. It does
not finish #163, #165, #166, #168, #169, #172, #184, #179, or
#185. leftover #22 stays open.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show `fetch_cites` / `deliver` presenting a Connect
access token against ConnectApiUrl, and a missing token / DemoUrl
collision / 401 membership refuse. It cannot show a live
two-app walk-through without Dashboard registration, unused
Cognito resources removed, or bank / calendar OAuth.

### Amendment, 2026-09-04 — demo API Lambda does not hydrate the journal from S3

⚠ **Superseded the same day.** The unset stopped lasting
`/books` 503s and wiped CreateBook on cold start
(Household). A later amendment restores ScaleBucket
`journals/` in the template.

Ops cleared `RATIO_JOURNAL_BUCKET` and `RATIO_JOURNAL_PREFIX`
from live Lambda `ratio-demo` (account `320473299741`,
us-east-1) after production `/books` showed “the journal is
still hydrating” (orTransient on API 503). Timeout was already
60. `/v1/books` then returned 401, not 503. The app stack still
set both env vars on the Function, so the next CloudFormation
deploy would restore the hang.

What landed is the durable unset, not a new store:

- `deploy/app.yaml` no longer sets `RATIO_JOURNAL_BUCKET` or
  `RATIO_JOURNAL_PREFIX` on the `ratio-demo` Function
  (Connect is the same Lambda, second HTTP API). Unset is
  `/tmp` journals — the local `ratio watch` shape.
- `Timeout: 60` stays. ScaleBucket, the scale cluster / task,
  and `RATIO_SCALE_*` stay. The journal bucket policy stays
  so a later durable-write path can; it is not the API
  hydrate dial.
- `//deploy:iac_test` fails if the Function Environment sets
  either journal var again, and fails if `RATIO_SCALE_BUCKET`
  is dropped with them.

This amendment does not close #22 (unused Cognito resources,
`DEMO_MEMBERS` naming a live WorkOS `sub`, WorkOS dashboard
registration). It does not reopen #24. It does not close #150.

### Amendment, 2026-09-04 — the lots/positions projection

[#153](https://github.com/mattmarshall/ratio/issues/153) is the
delivery slice under umbrella
[#8](https://github.com/mattmarshall/ratio/issues/8): a projection
schema for derived lots, positions, and aggregates, replayed from
the journal digest, so interactive scale toward the 20M-lot claim
on [#159](https://github.com/mattmarshall/ratio/issues/159) has a
place to sit. The journal stays the system of record. Postgres
does not become one.

**What this amendment records.** The schema is real, not only TLA:

- **Schema.** `crates/ratio-sql-project/schema.sql` names
  `projection_watermark`, `lots`, `positions`, and `aggregates`.
  One watermark, not one per table —
  `//tla:sql_projection_check`'s `AFigureIsFoldedFromOnePrefix`.
  `acquired` NULL is unset, not a default. `ORDER BY seq` is not
  FIFO relief.
- **Replay.** `SqlProjection::replay_book` folds the journal
  through the proved `Projection` (each entry's pinned config
  names the method) and replaces every table plus the watermark
  in one commit. The watermark is the prefix and
  `ratio_nav::prefix_digest`. A replaced journal at the same
  height refuses. A rebuild does not append onto existing rows —
  `//tla:rebuild_double_counts_check`.
- **Fail closed.** A read that pins the journal head while the
  snapshot lags (or leads, or disagrees on the digest) refuses.
  `//tla:unpinned_projection_check`. Zeros from an empty store
  are also a refuse — they look like a fund.
- **Relief stays the proved walk.** `SqlProjection::relieve`
  loads the rows and calls `ratio_project::relief::relieve_by`
  under the elected method. Physical storage is seq-keyed, so a
  silent SQL FIFO would take the cheap lot on a HIFO book; the
  test that names that sabotage stays red if the walk is
  replaced by the index. MinTax, SpecID, average cost, and wash
  stay elections — no new `Method` / `Order` / `lot_method`
  variant.

**the lots/positions projection** is the Built phrase this
amendment adds.

**What this is NOT:**

- **Not a live Postgres engine.** The crate is the denotational
  store the schema names. Applying the SQL to a server, planner
  pushdown proved against `Pg.Rel.Semantics`, and the measured
  20M-lot interactive claim stay #8 / #159. `Ratio.Exec` still
  holds: a database does not change the IO floor.
- **Not moving authority off `journal.jsonl`.** Replay and
  content-addressed digests remain the product.
- **Not CRM, a reporting warehouse in core, a client portal, or
  a `screensFor` fork.** Connect `statements:read` / `lots:read`
  apps can warehouse. Equalization, drip, and side-pocket stay
  Connect (#177).
- **Not wiring the console or the API through the store.** The
  in-memory `Projection` remains the running read model. Stage E
  is the schema and the refuse.

Nothing on the *Explicitly not building* list moved. This
amendment closes #153. It does not close #8 or #159. It does
not reopen #160, #158, or #151. leftover #22 stays on WorkOS.

**What a walk-through can and cannot show** (demo readiness, #27).
It can replay a book into the snapshot, see HIFO take the dear
lot while a seq scan would take the cheap one, and see a stale
watermark refuse. It cannot show twenty million lots as a
routine Postgres table, a planner rewrite, or a console screen
that reads the store. Those remain #8 / #159.
The demo API hydrates ScaleBucket `journals/` so CreateBook
survives a cold start; this amendment does not reopen the
#230 `/tmp`-only wipe.

### Amendment, 2026-09-04 — unused Cognito CloudFormation resources removed

The leftover on issue 22 after #230 was unused Cognito
UserPool / UserPoolClient / UserPoolDomain /
UserPoolIdentityProvider resources still declared in
`deploy/app.yaml`, with outputs `UserPoolId` /
`UserPoolClientId` / `HostedUiDomain`, and a `DEMO_MEMBERS`
default of `demo@ratio.fastverk.dev` — a Cognito-era address
that never appears on an AuthKit token. AuthKit was already
the sign-in path. The unused pool was left so a stack update
would not destroy it. Nothing live referenced it.

What landed is the teardown, not a second IdP:

- `deploy/app.yaml` no longer creates or exports those Cognito
  resources, nor the Google Client parameters that existed only
  for the unused Hosted UI. The next stack update deletes the
  unused live pool. WorkOS JWT authorizers, DemoUrl,
  ConnectApiUrl, and ScaleBucket stay. Journal env is
  restored in a later amendment.
- `DemoMember` defaults to empty. `deploy.yml` passes
  `DEMO_MEMBERS` with an empty fallback — not
  `demo@ratio.fastverk.dev`. Activation is a WorkOS `sub`
  (email optional). Empty writes no membership seed.
- Bootstrap keeps `cognito-idp:Delete*` so CloudFormation can
  tear the unused pool down, and drops `Create*`.
- `//deploy:iac_test` fails if a Cognito type, output, Google
  parameter, or the Cognito-era email default returns.

**unused Cognito CloudFormation resources removed** is the
Built phrase this amendment adds.

**What this is NOT, because leftovers stay named on issue 22:**

- **Not WorkOS dashboard registration.** A human still
  registers first-party Connect applications, redirect, and a
  live token. This repository does not invent those clicks.
- **Not naming a live WorkOS `sub` in `DEMO_MEMBERS`.** The
  template and docs are WorkOS-`sub` based. Empty is the
  honest default. Setting the repository variable to a live
  `sub` is operator work — this file does not invent one.
- **Not live bank / calendar OAuth product UI**, licensed AIA
  PDF, IRS e-file, or a kernel blob store.
- **Not #152 custom domain, #8 live Postgres, #159 scale, or
  Connect app features.**

Nothing on the *Explicitly not building* list moved. This
amendment does not finish #150. It does not reopen #151. It
does not finish #163, #165, #166, #168, #169, #172, #184,
#179, or #185. leftover #22 stays open.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show the app stack with no Cognito resources and
AuthKit / Connect JWT authorizers still in place. It cannot
show a live two-app walk-through without Dashboard
registration, or seeded funds granted to a live WorkOS `sub`
until an operator sets `DEMO_MEMBERS`.

### Amendment, 2026-09-04 — demo API Lambda hydrates ScaleBucket journals/ for CreateBook durability

This supersedes the same-day “does not hydrate” amendment
(#230). That unset stopped lasting `/books` 503s and wiped
user-created books on cold start. Matthew created PERSONAL
book “Household” (slug `household`) on ratio.marsh.build;
after refresh `/books` showed empty (Books · 0). Live
Lambda had Bucket/Prefix null; Household never appeared
under `s3://ratio-demo-scale-320473299741/journals/`.

Ops already restored live Lambda env
(`RATIO_JOURNAL_BUCKET=ratio-demo-scale-320473299741`,
`RATIO_JOURNAL_PREFIX=journals/`). `GET /v1/books` then
returned 401 immediately, not 503. `journals/` is ~1971
small objects (~0.5 MB), not the 40GB scale fold.

What landed is the durable restore in the template, so the
next CloudFormation deploy keeps persistence:

- `deploy/app.yaml` sets `RATIO_JOURNAL_BUCKET: !Ref
  ScaleBucket` and `RATIO_JOURNAL_PREFIX: journals/` on the
  `ratio-demo` Function (Connect is the same Lambda).
- `Timeout: 60` stays. Scale Fargate / `RATIO_SCALE_*`
  stay. The 40GB scale fold stays on ScaleTask, not this
  Lambda.
- accept-during-hydrate / orTransient (#136/#137) still
  apply: a book route during hydrate 503s with Retry-After;
  `/healthz` and `/version` never wait; unauthenticated
  `/v1` 401s without waiting for the book.
- `//deploy:iac_test` fails if either journal var is
  absent, and fails if `RATIO_SCALE_BUCKET` is dropped.

This amendment does not close #22 (`DEMO_MEMBERS` naming a
live WorkOS `sub`, WorkOS dashboard registration). It does
not reopen Cognito teardown. It does not close #150. It
does not reopen #24.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show CreateBook surviving a cold start because the
journal is on ScaleBucket `journals/`. It cannot show a
lasting `/books` hang from hydrating the 40GB scale fold
on this Lambda — that fold stays on Fargate.

### Amendment, 2026-09-04 — the projection schema applies to a live engine

[#8](https://github.com/mattmarshall/ratio/issues/8) leftover after
[#153](https://github.com/mattmarshall/ratio/issues/153): apply
`crates/ratio-sql-project/schema.sql` to a real server and exercise
replay plus fail-closed watermark reads against the same tables.
The journal stays the system of record. This slice does that. It
leaves the umbrella open.

**What this amendment records.**

- **Apply.** `PgProjection::apply_schema` runs the contract on a
  live engine through `psql`. The crate stays off the Cargo
  workspace — no rust-postgres member, no crate_universe churn.
- **NULL-as-unset survives the engine.** A `PRIMARY KEY` that
  included `instrument` or `currency` made those columns `NOT
  NULL` on Postgres; the rest map and an unset currency could not
  be stored. `UNIQUE NULLS NOT DISTINCT` is the uniqueness the
  denotational store already had. `acquired` NULL is still unset.
- **Replay / refuse / relief.** One transaction replaces every
  table and the watermark. A stale pin refuses.
  `PgProjection::relieve` loads the rows and calls `relieve_by`.
  Physical seq order is not FIFO. CI runs
  `//crates/ratio-sql-project:pg_engine_test` against a Postgres
  16 service container; `RATIO_PG_URL` unset is a refuse, not a
  skip. The denotational store still runs in `bazel test //...`.

**the projection schema applies to a live engine** is the Built
phrase this amendment adds.

**What this is NOT:**

- **Not planner pushdown.** `Pg.Rel.Semantics` / tomato-bazel
  `rules_postgres` stay leftover on #8.
- **Not wiring the console or the API through the store.** The
  in-memory `Projection` remains the running read model.
- **Not the measured 20M-lot claim.** That stays #159.
- **Not moving authority off `journal.jsonl`.** Replay and
  content-addressed digests remain the product.
- **Not claiming Postgres as the interactive-scale engine.** The
  public roadmap still names that word on the spec side.

Nothing on the *Explicitly not building* list moved. This
amendment leaves #8 and #159 open. It does not reopen #153. leftover
#22 stays on WorkOS.

**What a walk-through can and cannot show** (demo readiness, #27).
It can apply the schema to a server, replay a book, see a NULL
rest-map row land, see HIFO take the dear lot, and see a stale
watermark refuse. It cannot show a planner rewrite, a console
screen that reads the store, or twenty million lots as a routine
table. Those remain #8 / #159.
The demo API hydrates ScaleBucket `journals/` so CreateBook
survives a cold start; this amendment does not reopen the
#230 `/tmp`-only wipe.

### Amendment, 2026-09-04 — console/API reads through the store

[#8](https://github.com/mattmarshall/ratio/issues/8) leftover after
the live-engine apply: the running read model was still the
in-memory `Projection`. This slice wires lots, positions, and
Current aggregates through the Stage E store when configured.
The journal stays the system of record. This slice does that.
It leaves the umbrella open.

**What this amendment records.**

- **Config.** `RATIO_PG_URL` (optional `RATIO_PG_SCHEMA`, default
  `ratio_proj`) is the dial. Empty or missing is the in-memory
  fold — not localhost, not an empty fund. The network server
  opts in via `Console::with_stage_e_from_env`. A URL that cannot
  be reached refuses the request; it does not silently fall back.
- **Pin.** `ProjectionReads::catch_up` pins `journal.jsonl`
  (`ratio_nav::prefix_digest`) and rebuilds a lagging snapshot
  from the journal. A missing watermark refuses rather than
  answering with `lots: []` or a silent empty trial balance.
  Unset `acquired` / instrument / currency stay unset.
  `//tla:projection_check` / `//tla:unpinned_projection_check`.
- **Handlers.** `list_lots`, `list_positions`, and Current
  `list_accounts` read the store when configured. Period folds
  still walk the journal — the store is not a time-travel table.
  Chart labels and mark cites still read the journal.

**console/API reads through the store** is the Built phrase this
amendment adds.

**What this is NOT:**

- **Not planner pushdown.** `Pg.Rel.Semantics` / tomato-bazel
  `rules_postgres` stay leftover on #8.
- **Not the measured 20M-lot claim.** That stays #159.
- **Not moving authority off `journal.jsonl`.** Replay and
  content-addressed digests remain the product.
- **Not claiming Postgres as the interactive-scale engine.** The
  public roadmap still names that word on the spec side.

Nothing on the *Explicitly not building* list moved. This
amendment leaves #8 and #159 open. It does not reopen #153.
leftover #22 stays on WorkOS.

**What a walk-through can and cannot show** (demo readiness, #27).
It can set `RATIO_PG_URL`, list lots / positions / Current
accounts from the snapshot, see a stale store rebuild from the
journal, and see a never-replayed store refuse. It cannot show
a planner rewrite or twenty million lots as a routine table.
Those remain #8 / #159.
The demo API hydrates ScaleBucket `journals/` so CreateBook
survives a cold start; this amendment does not reopen the
#230 `/tmp`-only wipe.

### Amendment, 2026-09-04 — planner pushdown vs Pg.Rel.Semantics

[#8](https://github.com/mattmarshall/ratio/issues/8) leftover after
the live-engine apply and console/API store reads: a planner rewrite
that is a theorem, not a SQL string. tomato-bazel
`rules_postgres#9` already had the denotation. This slice
instantiates it on the Stage E catalog. The journal stays the
system of record. This slice does that. It leaves the umbrella
open.

**What this amendment records.**

- **Denotation.** `lean/Pg/Rel/Semantics.lean` is `Pg.Rel.Semantics`
  (bags, three-valued logic, `≡`) carried in-tree so a rewrite
  elaborates here. The sound rewrite is
  `pushdown_into_the_preserved_side_is_sound`. The refused one is
  `pushdown_below_an_outer_join_is_unsound`. A predicate and its
  negation do not reassemble a table that holds a null.
- **Stage E instances.** `lean/Ratio/Sql/Pushdown.lean` names the
  catalog. A pin that reads only the width-3 watermark prefix
  pushes into the watermark scan of watermark ⋉ lots. An
  `acquired` filter on that outer join cannot move into the lots
  scan — Stage E's witness is a watermark row with no partner and
  a null `acquired` (unset, not a default day).
  `seq_scan_is_not_hifo`: cheap-then-dear, `ORDER BY seq` is not
  HIFO. `an_empty_pin_is_not_an_empty_holding`: two scans, not an
  INNER JOIN that returns `[]` and looks like a sold-out fund.
  `//tla:sql_projection_check`, `//tla:stale_method_relief_check`,
  `//tla:unpinned_projection_check`.
- **Store door.** `crates/ratio-sql-project/src/plan.rs` denotes
  the same plans, applies the sound rewrite, refuses the unsound
  one, and emits filter-over-scan SQL. `PgProjection` lots /
  positions / aggregates / watermark reads go through that emit.
  Relief is still `relieve_by`. Physical seq order is not FIFO.

**planner pushdown vs Pg.Rel.Semantics** is the Built phrase this
amendment adds.

**What this is NOT:**

- **Not the measured 20M-lot claim.** That stays #159. A proved
  rewrite is not a measured fold.
- **Not moving authority off `journal.jsonl`.** Replay and
  content-addressed digests remain the product.
- **Not claiming Postgres as the interactive-scale engine.** The
  public roadmap still names that word on the spec side.
- **Not CRM, a reporting warehouse in core, a client portal, or
  a `screensFor` fork.** Connect apps can warehouse.
  Equalization, drip, and side-pocket stay Connect (#177).
- **Not a silent SQL FIFO.** `ORDER BY seq` is display order on
  a seq-keyed table. HIFO still takes the dear lot.

Nothing on the *Explicitly not building* list moved. This
amendment leaves #8 and #159 open. It does not reopen #153,
#234, or #235. leftover #22 stays on WorkOS.

**What a walk-through can and cannot show** (demo readiness, #27).
It can show a pin filter push into the watermark scan, see an
`acquired` push below the outer join refused, see HIFO take the
dear lot while a seq scan would take the cheap one, and see a
join-shaped SQL emit refuse. It cannot show twenty million lots
as a routine table. That remains #159.
The demo API hydrates ScaleBucket `journals/` so CreateBook
survives a cold start; this amendment does not reopen the
#230 `/tmp`-only wipe.

### Amendment, 2026-09-04 — the measured 20M-lot fold

[#159](https://github.com/mattmarshall/ratio/issues/159) leftover after
the Stage E interactive path (#153 / #234 / #235 / #236): a visitor
still could not RUN a twenty-million-lot fold as a routine
measurement. The geometry HANDOFF already named is 10,000 securities
× 2,000 lots = 20,000,000 rows, not `ratio closure`'s 500 × 40,000
dial. This slice measures that projection fold in-repo. It leaves
the #8 umbrella open.

**What this amendment records.**

- **Geometry.** `Geometry::HANDOFF` is 10,000 × 2,000. A 500 × 40,000
  product is a different fund (`//:scale_shapes_test`). Acquired
  stays unset. Relief is `relieve_by` (HIFO takes the dear lot;
  a seq scan takes the cheap one). MinTax, SpecID, average cost,
  and wash stay elections.
- **Path.** `//crates/ratio-sql-project:fold_scale_test` generates
  the open-lot projection, digests every row, relieves every
  holding, and publishes
  `crates/ratio-sql-project/fold_scale.recorded.json`. Measured
  on a 4 vCPU / 15 GiB Linux fastbuild: **17.4 s**, digest
  `bbf896400835916d0902f9ea175609bccd84be4801f71cc9fc57140f8a60a5d3`.
  Small geometries load the same rows into `SqlProjection` /
  `PgProjection`; the HANDOFF row refuses `load_scale` (that
  would be 20M string-keyed INSERTs, not the claim).
- **Journal stays SoR.** This is a projection load of the open-lot
  geometry, not a replay of the ~140 million entries / ~40 GB
  journal. That fold stays on Fargate ScaleTask / ScaleBucket.
  `Ratio.Exec` still holds: a database does not change the IO
  floor. Pushdown vs `Pg.Rel.Semantics` was already Built.

**the measured 20M-lot fold** is the Built phrase this amendment
adds.

**What this is NOT:**

- **Not closing #8.** The umbrella stays open. The public roadmap
  still does not call Postgres the interactive-scale engine.
- **Not the 140M-entry / 40GB journal fold.** ScaleTask /
  ScaleBucket stay the place that fold runs. This process does
  not have those secrets and does not claim it ran that task.
- **Not claiming the demo Lambda hosts 20M lots.** A visitor
  still cannot RUN the forty-gigabyte journal in a request.
- **Not moving authority off `journal.jsonl`.** Replay and
  content-addressed digests remain the product.
- **Not CRM, a reporting warehouse in core, a client portal, or
  a `screensFor` fork.** Connect apps can warehouse.
  Equalization, drip, and side-pocket stay Connect (#177).
- **Not a silent SQL FIFO.** `ORDER BY seq` is display order.
  HIFO still takes the dear lot.
- **Not re-deriving planner pushdown.** That phrase is already
  Built.

Nothing on the *Explicitly not building* list moved. This
amendment closes #159. It does not close #8. It does not reopen
#153, #234, #235, or #236. leftover #22 stays on WorkOS.

**What a walk-through can and cannot show** (demo readiness, #27).
It can run `bazel test //crates/ratio-sql-project:fold_scale_test`,
see 20,000,000 lots, see digest
`bbf896400835916d0902f9ea175609bccd84be4801f71cc9fc57140f8a60a5d3`,
see HIFO cost 2,000,000 against a seq-scan cost of 1,000, and
cite the recorded JSON from the scale screen. It cannot show a
visitor folding the 140-million-entry journal inside a Lambda
request, or twenty million lots as a browser table. Those remain
the #8 umbrella / ScaleTask path.
The demo API hydrates ScaleBucket `journals/` so CreateBook
survives a cold start; this amendment does not reopen the
#230 `/tmp`-only wipe.

### Amendment, 2026-09-04 — Postgres as the interactive-scale engine

[#8](https://github.com/mattmarshall/ratio/issues/8) leftover after
the Stage E interactive path (#153 / #234 / #235 / #236) and the
measured 20M-lot fold (#159 / #237): the public roadmap still named
Postgres on the spec side. The projection path is already Built.
This amendment records the claim. The journal stays the system of
record.

**What this amendment records.**

- **Claim.** The public roadmap moves Postgres for a live relational
  engine at interactive scale from Designed to Built. Schema apply,
  console/API store reads, planner pushdown vs `Pg.Rel.Semantics`,
  and the measured 20M-lot fold are already Built. This phrase is
  the honesty that closes the umbrella.
- **Journal stays SoR.** Replay and content-addressed digests remain
  the product. Postgres holds the derived transactional state.

**Postgres as the interactive-scale engine** is the Built phrase
this amendment adds.

**What this is NOT:**

- **Not reopening #159.** The measured 20M-lot fold is already
  closed. This file does not reopen #153, #234, #235, #236, or
  #159.
- **Not the 140M-entry / 40GB journal fold.** ScaleTask /
  ScaleBucket stay the place that fold runs. This process does
  not invent a 40GB ScaleTask run.
- **Not claiming the demo Lambda hosts 20M lots.** A visitor
  still cannot browse twenty million lots as a browser table.
- **Not moving authority off `journal.jsonl`.** Replay and
  content-addressed digests remain the product.
- **Not ACM / custom domain (#152).**
- **Not leftover #22.** `DEMO_MEMBERS` naming a live WorkOS
  `sub` and WorkOS dashboard registration stay WorkOS operator
  work.
- **Not Connect apps.** Client / LP / vendor portals, bank
  OAuth, G702, tax packs, EAC stay Connect.

Nothing on the *Explicitly not building* list moved. This
amendment closes #8. It does not reopen #153, #234, #235,
#236, or #159. leftover #22 stays on WorkOS.

**What a walk-through can and cannot show** (demo readiness, #27).
It can cite the Stage E store, the proved pushdown, and the
recorded 20M-lot fold. It cannot show a visitor folding the
140-million-entry journal inside a Lambda request, or twenty
million lots as a browser table. Those remain the ScaleTask
path.
The demo API hydrates ScaleBucket `journals/` so CreateBook
survives a cold start; this amendment does not reopen the
#230 `/tmp`-only wipe.
