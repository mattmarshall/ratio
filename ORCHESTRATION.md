# Orchestration — production agents and the proven kernel

Where production agents sit relative to the proofs: what verification owns,
what agents may propose, and the doors between them.

**Written 2026-08-13.** Prompted by an external roadmap describing fifteen AI
agents across a daily NAV production pipeline — trade capture through
reconciliation, pricing, and close. This document determines how those ideas
fit Ratio, and where the boundary between the formal verification and the
agentic workflows lies. It elaborates the one sentence
`paper/sections/control.tex` and `research.tex` assert and never explain —
*"an agent-orchestrated workflow coordinates intent capture, validation,
compilation, simulation, and approval"* — and it absorbs the aspiration in
`competitive/specs/ai-insights.tex`'s status box ("expand into a full Ratio
component spec") without replacing that file.

Two relationships worth stating before anything else:

| | |
|---|---|
| **PLAN.md** | owns sequencing, capacity, and refusals. This document commits to building **nothing**, and the wedge — a real customer's period reconciling — outranks everything in it. |
| **AGENTS.md** | is rules for an LLM working on this *repository*. This document is about agents that would touch a fund's *book*. The two must not be confused, which is why neither is named like the other. |

---

## What the roadmap wants, and what this document does with it

The roadmap maps fifteen purpose-built agents onto the four layers of a daily
NAV cycle — ingestion (trade capture, derivative lifecycle, security data,
shareholder activity), processing (corporate actions, accruals, expenses,
reconciliation), valuation (asset and derivative pricing), and close
(transaction close, valuation close, NAV validation, audit close) — plus a
cross-cutting governance agent. Each agent handles its domain on its own and
escalates to a person only when its confidence is insufficient. The goal is
operations that manage exceptions rather than screens.

The hard question in that roadmap is not what the agents do. It is **what
keeps an agent from being wrong in a way nobody notices.** The roadmap's
answer is a confidence threshold. Ratio's answer already exists, is tested,
and is different in kind: **the model proposes; the proven kernel disposes.**
A model can draft anything and decide nothing, because the deciding surface —
what enters the journal, what a figure folds from, what a sale relieves, when
a NAV can be struck — is theorems and refusals, not judgment. The fence is
enforced by absence: `approve_rule` is not in the tool list, not dispatched,
not reachable, and `demo/rehearse.sh` fails if it appears.

So this document does one thing: it takes each duty the roadmap assigns to an
agent and gives it exactly one of four dispositions.

| | |
|---|---|
| **theorem** | the duty is already a machine-checked property. No agent performs it, because nothing needs to. |
| **proposal** | the duty is genuinely agentic — interpretation, narration, drafting. It produces a content-addressed artifact that is inert until a person acts on it. |
| **human** | the duty is a verb that changes the books or elects a policy, and those are personed by construction. |
| **no substrate** | Ratio has nothing underneath the duty today, and this document says so rather than pretending. |

---

## The boundary

Three claims, then the asymmetry that makes the third one the important one.

**1. The kernel owns everything that can be a theorem.** Conservation on
every currency (`ledger_conserves`, `lean/Ratio/Core.lean:114`); the trial
balance as conservation restated, not a second check
(`trial_balance_ties`, `lean/Ratio/Chart.lean:75`); a balanced template
balanced at every amount (`balanced_template_balances`,
`lean/Ratio/Chart.lean:113` — the reason checking a rule once at approval
time is sound); a figure pinned to the journal prefix it was folded from
(`StrikeFoldsItsOwnPrefix`, `tla/Projection.tla:138`); a strike refused
exactly when something is unpriced
(`strike_refuses_exactly_when_something_is_unpriced`,
`lean/Ratio/Valuation.lean:323`); one answer per valuation day, no
restatement (`one_answer_per_day`, `lean/Ratio/Period.lean:93`); and the
door itself — an entry that does not conserve, or names a configuration that
is not stored, never reaches the record (`crates/ratio-store/src/lib.rs:768`).
No agent re-checks any of this, and no agent's judgment overrides a refusal.

**2. Agents own everything that is interpretation.** What a document means.
Which template fits a file nobody has seen before. Why a break exists. What a
corporate-action announcement implies. What a NAV movement means, in prose,
with the postings behind it cited. All of it is proposal-shaped:
content-addressed, citable, and inert until a person walks it through a door
the agent cannot reach. ⛔ Nothing in this document proposes an approval
tool, an approve button, or a permission-checked model write path. A fence
with a permission check is a fence with a door, and the repository has
already been down that road once: the original plan listed `approve_rule` as
a tool, and the fence won.

**3. ⭐ The privileged operation is not writing to the journal.** The door
refuses an unbalanced entry; that risk is closed by construction. The
residual risk is an agent that **supplies or influences a fact or a policy
input** — a price, an FX rate, a lot method, a chart role, a holding-period
threshold, a valuation date. Those are the inputs where the books tie, the
digest reproduces, and `ratio replay` reports *reproduced* — of the wrong
figure, permanently. HANDOFF.md names the shape ("the books tie and the
number is wrong") and prices it: $523,810.23 on a $134M fund, 0.39%, tied
the entire way. Every mitigation in this document is therefore the one Ratio
already uses — provenance, determinism, refusal, and human approval of
configuration — and never trust in the author.

```
        ABOVE THE FENCE — proposes                 BELOW THE FENCE — disposes
  ┌─────────────────────────────────────┐   ┌─────────────────────────────────────┐
  │ models, and whatever orchestrates    │   │ Lean theorems · Rust emitted from   │
  │ them                                 │   │ the Lean · TLA+-checked protocols   │
  │                                      │   │                                     │
  │  draft rules and templates           │   │  conservation, per currency         │
  │  narrate breaks · explain figures    │   │  balanced at the door               │
  │  propose entity setups · flag prices │   │  unpriced blocks the strike         │
  │  ask questions when facts are absent │   │  one answer per day                 │
  └──────────────────┬──────────────────┘   └──────────────────▲──────────────────┘
                     │ proposals: content-                     │ runs only from the
                     │ addressed, citable, inert               │ journal + the config
                     ▼                                         │ digest each entry pins
        ┌───────────────────────┐    ratio approve —  ┌────────┴───────────────┐
        │ the drafts plane      │    a person, at     │ configuration by digest │
        │ ActorKind::MODEL,     ├────a terminal──────▶│ facts with provenance   │
        │ author of DRAFTs only │                     │ the journal, at the door│
        └───────────────────────┘                     └─────────────────────────┘

     There is no arrow from the models box to the kernel box.
     That absence is the fence, and demo/rehearse.sh tests for it.
```

---

## The doors, as they exist today

Every surface an agent legitimately touches already exists, which is what
makes the mapping in the next section short. Nothing below is aspiration.

| | |
|---|---|
| **MCP tools** | eight, in `crates/ratio-mcp/src/lib.rs`: `list_accounts`, `propose_rule`, `propose_template`, `list_entities`, `check_rule`, `post_events`, `trial_balance`, `explain_figure`. Served over stdio by `ratio mcp` and over the network at `POST /mcp`. |
| **one dispatch table** | the Bedrock chat agent (`crates/ratio-agent`) reads `ratio_mcp::tools()` and dispatches through the same table, so the fence cannot reappear wrong in a second copy. |
| **the console contract** | 34 RPCs on `ratio.console.v1.Console`. All four writes — `ApplyEvent`, `IngestDelivery`, `AdmitFacts`, `MarkPositions` — take `validate_only`, a dry run through the same code path that commits. An agent can rehearse anything a person could do, without doing it. |
| **the planes of a book** | `journal.jsonl` · `config/<digest>` + `ACTIVE` + `HISTORY` · `deliveries` · `entities` · `facts` · `actions` · `reports/*.pb` · `NAVS` · `proposals/*.toml` · `CHANGELOG`. Everything an agent reads is citable by name. |
| **`ActorKind::MODEL`** | `proto/ratio/console/v1/console.proto:605`. A model appears in the change log only ever as the author of a DRAFT — the verb that changed the books always has a person on it. |
| **the person verbs** | `ratio approve` and `ratio strike` run at a terminal, signed with a name. Neither has a route, a button, or a tool. |

One property ties the table together: a single config digest fixes both how
bytes become events (templates) and how events become postings (rules). An
approved proposal therefore changes the book's *future* behavior
deterministically, and replay stays honest about its past.

---

## The translation table

The roadmap's vocabulary and Ratio's overlap less than the ideas do. Here is
the dictionary — including the entries where a roadmap concept is refused
rather than translated.

| the roadmap says | Ratio says |
|---|---|
| guarded autonomy | the fence. The guard is absence, not permission — there is nothing to relax. |
| confidence score, threshold escalation | a check passes, refuses, or asks a question. A numeric confidence on top of that is confidence nobody has earned, and nothing gates on it. |
| exception-management-only operations | the break list is a list of real data problems, ordered by money. It is kept short by being right, not by being filtered — a **false break** is the cardinal sin. |
| auto-resolve exceptions | a model-drafted explanation a person accepts. The break becomes *explained*, never *gone*. |
| learns from resolution history | may **cite** history in a proposal. Never becomes behavior — the book's conduct replays from the journal and the config digest, and from nothing else. |
| conversational, not navigational | the MCP conversation is the authoring interface. The console reads and previews, and deliberately has no approve, explain, or strike button. |
| auto-trigger close, auto-release | a NAV strike is a named person at a terminal, once per valuation day, and the kernel refuses it while anything is unpriced. |
| an agent per function | a verb over a plane. Fifteen job titles collapse into one propose/approve loop pointed at different planes. |
| four-eyes review package | the change log. The model authors the DRAFT; the person owns the verb. Two lines, two actors, one record. |
| final release approval "likely always human" | agreed, and generalized: **every** verb that changes the books is human. NAV release is not the exception; it is the rule everywhere. |

---

## The fifteen agents, mapped

Fifteen agents is an org chart. Mapped onto Ratio they are not fifteen
programs: they are one propose/approve loop pointed at different planes, plus
a set of duties the kernel has already made into theorems, plus a set of
domains Ratio has no substrate for and will not pretend to.

| # | roadmap agent | disposition | lands on |
|---|---|---|---|
| 1 | Trade Capture | proposal | deliveries plane, templates, `validate_only` |
| 2 | OTC Derivative Lifecycle | no substrate | refused as a domain, below |
| 3 | Cash & Asset Reconciliation | ⭐ proposal | recon, the break list, the missing explain verb |
| 4 | Security Data Management | proposal | entities plane, admitted facts |
| 5 | Corporate Actions | proposal + human | actions plane, the factor representation |
| 6 | Shareholder Activity | split | file half rides ingest; register half, no substrate |
| 7 | Income Accruals | theorem + proposal | rules in the config digest |
| 8 | Expense Management | proposal; payments refused | rules; budgets have no substrate |
| 9 | Asset Pricing | theorem + proposal | facts plane, the mark, the challenger role |
| 10 | OTC Derivative Pricing | no substrate | refused as a domain, below |
| 11 | Transaction Close | theorem + narration | the break list and the unpriced list |
| 12 | Valuation Close | theorem | the strike's own refusal |
| 13 | NAV Validation & Approval | theorem + narration | `explain_figure`, replay |
| 14 | Audit Close & Dissemination | theorem + human | the strike; dissemination has no substrate |
| 15 | Governance & Oversight | reads | the change log, `CHANGELOG`, the record itself |

### Ingestion interpretation — trade capture, and the file half of shareholder activity

**Attaches to:** the deliveries plane, `IngestDelivery` with `validate_only`,
ingest templates inside the config digest, `propose_template`, and pending
facts. **Theorem:** the "edit tests" of the roadmap are mostly the door —
an entry that does not conserve never posts — plus resolution monotonicity
(`resolved_never_becomes_absent`, `lean/Ratio/Ingest.lean:162`: adding master
data can never un-resolve a posted fact) and decimal parsing as scaling, with
no division for a rounding decision to hide in. **Proposal:** a template for
a file shape nobody has seen — the loop that already exists; extraction from
unstructured documents, *as a proposed delivery previewed through
`validate_only`*, never a direct post; narration of why a row was refused.
**Refused:** guessing an entity resolution. An AMBIGUOUS fact is a question,
and it clears itself only when the entity master gains the reference through
an approved admission. **Greenfield:** connectors, watchers, schedulers —
`Delivery.origin` names "a path, an S3 key, an SFTP URL" and nothing fetches
from any of them; template kinds beyond CSV; the extraction pipeline itself.

### Reference data — security data management

**Attaches to:** the entities plane, `AdmitFacts`, `list_entities`.
**Theorem:** an unknown instrument cannot corrupt the ledger — it produces a
pending fact, which is a question, not an error, and it self-clears when the
master catches up. **Proposal:** a full entity setup inferred from documents,
with the inference's sources cited; a classification; a rating or factor
change drafted *as a fact* with provenance. **Refused:** auto-applying
anything. An applied factor is a journal entry, and announcements must be
journal entries or replay answers differently as the world tells us more
(`replay_is_determined_by_the_prefix`, `lean/Ratio/Actions/Factor.lean:177`).
"Default detection from news" is narration attached to the entity — a flag
with citations, deciding nothing.

### Corporate actions

**Attaches to:** the actions plane and the factor representation.
**Theorem:** per-step application, never a composed ratio (the composed
factor silently swallows the half-share the holder was paid cash for);
at-most-once application with attributable staleness, checked in TLA+.
**Proposal:** interpreting an announcement into a proposed action; modeling a
voluntary election *as a memo with numbers* — which choice yields what, cited
to the terms. **Human:** the election itself, which is an economic choice,
and the tax treatment, which is configuration and is approved like any other
policy. **Greenfield:** a draft stage for proposed actions; today an action
enters through the CLI door with no proposal in front of it.

### Accruals and expenses — income accruals, expense management

**Attaches to:** posting rules in the config digest, `propose_rule` /
`check_rule`, the facts plane for rates and factors. **Theorem:** a balanced
template is balanced at every amount, so an accrual cannot unbalance the
book, ever. ⚠ The sharp edge belongs in the same sentence: **a buggy rule
emits the wrong *balanced* transaction** — the semantic error no theorem
catches — and that is exactly why a rule is a proposal a person approves,
with `check_rule` grounding the argument first. **Proposal:** drafting
accrual rules per asset type; variance commentary that cites the postings it
explains. **No substrate:** expense budgets. **Refused entirely:** payment
instructions and FX execution. Ratio keeps books; it does not move money.

### ⭐ Reconciliation — the wedge agent

The roadmap's reconciliation agent is the one that matters, because its
output is the thing the wedge sells: a break report a customer can argue
with.

**Attaches to:** `crates/ratio-recon` (the shadow run), `reports/*.pb`, and
the console break list, where three kinds of break converge — recon breaks
(our figure, their figure, the cause, the config hash), lot breaks (always
HIGH regardless of amount, because they corrupt the realized gain — the
figure with no counterparty), and pending facts. The scope gate is part of
the substrate: one row outside the declared scope and the run produces *no
breaks at all*, only exceptions naming every such row, because a partial
replay compared against whole positions manufactures false breaks. Exit
codes 0, 2, 3 — reconciled, differences, refused — and refusal is never
investigated as data.

**Theorem:** break *detection* is deterministic and stays below the fence.
The agent never decides what is or is not a break.

**Proposal:** everything the roadmap calls "auto-investigate." Narrating a
break's likely cause with citations to the postings, deliveries, and facts
behind it. Proposing a disposition (next section). Explaining the priority
order — the list is already sorted by money, and the agent may explain the
order, not change it.

**Refused:** auto-clearing, in any costume. The code already states the
doctrine, in the place where the temptation will arrive
(`crates/ratio-console/src/lib.rs:2227`):

> Nothing records an explanation yet, so nothing claims one. A break the
> software decided was fine is exactly the kind of thing this product exists
> not to do.

An accepted explanation makes a break *explained*, with a name attached. It
never makes the break gone. Also refused: "this break in this custody
relationship is always resolved by X" as *behavior*. History is evidence an
agent may cite; it never becomes an automatic rule, because the book's
conduct must replay from the journal and the config alone — and because
cross-client resolution history crosses the tenant boundary the auth round
just built.

**Greenfield:** the explain verb itself. `explained: false` is hardcoded;
there is no explanation store, no state machine, no RPC. This is the single
most load-bearing gap in the whole mapping, and the nearest one to the
wedge.

### Pricing — asset pricing, and the challenger role

**Attaches to:** the facts plane (prices and FX with provenance),
`MarkPositions` with `validate_only`, and the valuation theorems.
**Theorem:** the chosen price is never from the future
(`markPrice_is_not_after`, `lean/Ratio/Valuation.lean:140`); no price means
no mark, not a zero mark (`unpriced_yields_no_mark`,
`lean/Ratio/Valuation.lean:264`); and the strike's refusal is total and is
exactly the unpriced list. The roadmap's "pricing completeness monitoring"
is therefore not a monitor. It is a refusal the strike already performs.
**Proposal:** the challenger role — flagging a price that disagrees with a
secondary source or with the news, *as narration attached to the fact*,
deciding nothing; drafting a fair-value policy change for approval.
**Refused:** an agent supplying the winning price, and "dynamic tolerance
bands." Which source wins, and how much movement is tolerable, are terms of
an agreement — configuration a person approved. "Dynamic" is refused;
"proposed, versioned, approved" is the translation. **No substrate:**
derivative pricing, Greeks, credit events.

⭐ **Amended 2026-08-14 — the first half of that sequence landed.** This
section used to say tolerance was two hardcoded constants and that the honest
order was to move it into the config digest before letting anything propose
changes to it. It is now a `[tolerance]` on the rule set, beside the lot
method and the holding-period threshold; `Ratio.Tolerance` is the proof side,
and the grading decision is emitted from it. A break is graded by the
configuration **its report names**, not the one in force now — a severity is
part of a comparison, and regrading a report whose bytes never changed is the
same failure as an unpinned announcement. What the grader cannot read grades
HIGH, deliberately: a tolerance nobody could read cannot certify a difference
as small.

⛔ **And the second half is still refused, now mechanically rather than by
absence.** `ratio approve` REFUSES a proposal that declares a tolerance and
names `ratio config set` instead. The reason is this document's own boundary
claim: the dangerous verb is not writing to the journal, it is supplying a
policy input, and how big a difference has to be before it stops the NAV is
exactly that. Letting a drafted proposal move it — through a merge that,
before this, would have dropped it silently — is the shape to avoid. When
agents do propose tolerance changes, it will be through the control plane with
a person reading the whole document, and it will be its own change.

### The close — transaction close, valuation close, NAV validation, audit close

**Attaches to:** the NAV strike (CLI-only, signed by a named actor, one
answer per day), the `NAVS` ledger, `explain_figure`, and the fund state the
console reports. **Theorem:** most of the roadmap's close checklist. Classes
summing to the total is conservation over a partitioning dimension. Balance
sheet tying to income statement is the trial balance restated. A figure
citing the prefix it folded from is `StrikeFoldsItsOwnPrefix`. The audit
trail is not a feature to build — the append-only journal, the
content-addressed config, and the strike ledger *are* the trail. The
roadmap's "all impact tests performed automatically" is Ratio's founding
premise, achieved by proof rather than by test. **Proposal:** explaining the
NAV movement — extending the reach of `explain_figure` from a balance to a
day-over-day story with the trades and marks cited; a readiness *narration*
over the break list and the unpriced list. **Refused:** auto-triggering the
close, and a "ready-for-close" score. The break list and the unpriced list
**are** the readiness statement; a scalar summarizing them adds confidence
nobody earned and subtracts the citations. The strike stays a person.

⚠ One seam named rather than papered over: breaks do not mechanically block
a strike today. The BLOCKED state the console reports is derived for
display, not enforced at the strike. The fix, if the gap matters, is
enforcement below the fence — a kernel-side refusal like the unpriced one —
and never an agent's judgment layered on an unenforced gate, which would
launder the gap into prose.

**No substrate, and human where it exists:** dissemination to exchanges and
vendors, and rate confirmation to a transfer agency, do not exist here. The
roadmap's own concession — final release approval is "regulatory, likely
always human" — is quoted and generalized above: in Ratio that is every
verb, not this one.

### No substrate today — said once, plainly

OTC derivative lifecycle and pricing, the transfer-agency half of
shareholder activity (registers, redemption fees, as-of gain/loss policy),
and expense budgets have nothing underneath them in this repository. The
posture is the recon scope gate's, applied to domains: **refuse the domain
rather than pretend a partial mapping**, because a partial mapping
manufactures the domain equivalent of false breaks. Share reconciliation
could plausibly ride the existing recon engine the day shares are positions
in a fund's book; the rest is far-phase and deliberately not designed here.

### Governance and oversight

**Attaches to:** the read surfaces — the change log, `CHANGELOG`, the
journal, the strike ledger. Reporting over the record is legitimate and
boring, which is a compliment. Trend narration is proposal-shaped like
everything else. ⚠ One disambiguation, so a future reader neither violates
PLAN.md's refusal list nor over-reads it: the refused "performance reporting
and attribution" means *investment* performance — time-weighted returns,
attribution by sector. Operational reporting over the change log is not that.

---

## The shape of an agent's proposal

The precedent already exists and the design generalizes it rather than
inventing one. Today a model's output has exactly one shape:
`proposals/*.toml`, content-addressed, checked by `check_rule`, approved by
`ratio approve`, entering `HISTORY` under a new digest, with the model as
`ActorKind::MODEL` — the author of a DRAFT in a change log where the verb
always has a person on it.

Generalized to the rest of this document, an agent proposal is a
content-addressed artifact under the book that carries four things:

1. **the durable name of its subject** — a break's URL, an entity, a
   posting. Break names derive from the account dimension precisely so a
   link survives the report being regenerated.
2. **what the agent read** — the journal prefix and the config digest. A
   proposal is as-of; when the journal moves past it, it goes visibly stale
   instead of silently wrong.
3. **citations that resolve mechanically** — every factual claim points at a
   journal line, a fact, a delivery digest, or an entity, and a citation
   that does not resolve fails the proposal before any human reads it.
4. **exactly one disposition, from a closed set:**

| disposition | what accepting it does |
|---|---|
| an explanation | the break becomes explained, with the accepting person's name on it. It stays in the list. |
| a correcting entry draft | just events — walks through `post_events` / `ApplyEvent` with `validate_only` first, like anything else. |
| a config change draft | a rule or template, through the propose tools that already exist. |
| a question | a missing fact — lands in pending facts, where it clears itself when the master catches up. |

Acceptance is a person's verb at an existing kind of door. The change log
then shows two lines: the model authored the draft, the person performed the
verb.

⛔ **Nothing in this section is built by this document.** The additions it
implies — an explanation message and field on `Break` in the console proto,
one person-authenticated RPC to attach an accepted explanation, storage for
the new proposal kind — are deliberate future changes with their own review.
The hardcoded `explained: false` comes out *last*, when there is finally
something true to report. And there is no MCP tool for acceptance, ever;
this section exists so that when the verb is built, it is built to this
shape and not to a more convenient one.

---

## Phasing, against the plan and the site

| tier | what | status and gate |
|---|---|---|
| **built** | the authoring loop: say it, draft it, check it, answer back, approve. `propose_rule`, `propose_template`, `check_rule`, `explain_figure`, the fence, the chat agent on the same dispatch table. | PLAN.md Stage 2 ✅; `demo/rehearse.sh` asserts the fence. |
| **nearest, and wedge-serving** | break-explanation proposals over recon output — the explanation disposition first. The site already claims the assistant "proposes reconciliations" and that verb is not built; the wedge's deliverable *is* an explained break list. | the only agent work that moves the plan's one open gap; site roadmap phase two. |
| **later** | corporate-action interpretation; entity-setup proposals; the pricing challenger; tolerance into configuration, then proposed tolerance changes; connectors on `Delivery.origin`; `explain_figure` reaching NAV movements. | site phases two through four, each gated on a fund that needs it. |
| **refused** | the list below, permanently. | — |

None of this outranks the wedge. At one engineer, part-time, the only tier
that may consume hours before a real customer's period reconciles is the
second — and only because the site already claims it.

---

## Explicitly not building

Named so they stop being tempting, in the same spirit as PLAN.md's list —
and phrased so that nothing here proposes anything on that list.

- an approve tool, an approve button, or a permission-checked model write
  path. The fence works by absence.
- auto-clearing exceptions, in any costume — including "resolved, with high
  confidence."
- an agent that supplies a price, a rate, a method, a role, or a date as
  anything but a citable proposal.
- confidence scores as gates, anywhere.
- model state as an input to the book's behavior. The book replays from the
  journal and the config digest, full stop.
- payment execution, order routing, or dissemination to exchanges, vendors,
  or transfer agencies.
- fifteen separate agent programs. The mapping is verbs over planes, not
  headcount.

---

## Risks worth naming

- **Plausible-but-wrong narration is the false-break sin in new clothes.** A
  wrong explanation a person accepts converts an open question into a
  confident lie with a URL. The mitigations are structural: citations that
  must resolve before a human reads the proposal, as-of staleness on every
  proposal, and the explanation attributed to the accepting person, not the
  drafting model.
- **The explained-flag comment is a tripwire, not a TODO.** Whoever builds
  the explain verb will be tempted to flip `explained` from the agent's side
  "just for the demo." The comment quoted above is the product's identity;
  it comes out last, not first.
- **Learning versus replay.** The roadmap's most seductive idea — the agent
  that has seen this break before — is the one most at odds with a book
  whose conduct must reproduce from the journal and the config. Evidence,
  yes; behavior, never. And cross-client history crosses a tenant boundary
  that was built to hold.
- **The unenforced gate.** Breaks not blocking a strike is a kernel seam. An
  agent narrating readiness on top of it would inherit the gap and hide it.
  Enforcement first, narration second.
- **This document as coverage creep.** PLAN.md records four refused features
  built in the 48 hours after that file was written, with the list
  untouched. A mapping document is exactly the kind of artifact that quietly
  becomes a backlog. Hence the tier table, the sentence under it, and the
  fact that this document edits PLAN.md not at all: it creates no build
  permission.

---

## Done when

- A reader can place any of the fifteen roadmap agents into one of the four
  dispositions, with the door named.
- The paper's "agent-orchestrated workflow" sentence has a reference instead
  of a hand-wave.
- Nothing here requires editing PLAN.md's refusal list or
  `plan_refusals_test.sh`.
- "Autonomy" and "confidence" appear only where the roadmap is being
  described, translated, or refused — never as Ratio's own vocabulary.
- Every file path cited here resolves in the tree at the commit that adds
  this file.
