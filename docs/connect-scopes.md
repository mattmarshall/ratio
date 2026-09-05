# WorkOS Connect — scope catalog

**Frozen 2026-09-04.** Issue [#150](https://github.com/mattmarshall/ratio/issues/150).
PLAN amendment of the same date. This file is the contract.

⭐ **A scope is a grant, not an RPC.** Ratio's core stays a thin book of
record: the journal, lots and relief, statements, kind-aware chrome, AuthKit
tenancy. Breadth — portals, bank feeds, CRM, tax packs, vendor apps — is a
[WorkOS Connect](https://workos.com/docs/authkit/connect) application that
presents a subset of the scopes below. Growing `ratio watch` / `Console` to
match every app is the failure this catalog exists to prevent.

⚠ **The catalog is frozen. Connect tokens accepted with catalog scopes.**
A verified Connect-shaped JWT that carries a frozen name can open the
matching `/v1` door, after membership. An AuthKit session JWT is
unchanged (login, `/v1/books` without a session, authenticated console
walks). A Connect-shaped token never takes `RATIO_DEMO_OPEN` and never
matches `org:{id}` (#151). Hard non-scopes and aliases stay refused.
#150 leftovers: the `journals:post` allowlist, reserved RPCs, and the
read-only reference skeleton. `RATIO_DEMO_OPEN` defaults off on the
deployed demo. first-party Connect apps call ConnectApiUrl.
Issue 22 stays open for `DEMO_MEMBERS` naming a live WorkOS
`sub` and WorkOS dashboard registration. unused Cognito
CloudFormation resources removed. API Gateway JWT verifies Connect tokens
on the Connect HTTP API (AuthKit custom-domain issuer). This
file does not close #150.

---

## Grant contract

A scope is grantable only when **all** of the following hold:

1. The token is a WorkOS Connect access token (OAuth `authorization_code` or
   M2M `client_credentials`), verified against the environment JWKS — not an
   invented bearer, not a session cookie replayed as a Connect grant.
2. The AuthKit subject (`sub`) is in the book's membership. A book is the
   tenant. An `org_id` claim is not membership, and a first-party app does
   not inherit every book in an org. #151: Connect tokens must not bypass
   book ACLs.
3. The requested action is in this catalog. A string that is not here is
   refused, including a plausible near-miss (`journal:read`, `billing:write`).
4. Write scopes that name a template (`journals:post`, `calls:post`,
   `fees:accrue`) pass the per-`client_id` allowlist. An empty allowlist
   refuses every post.

M2M tokens carry `org_id` and no user. They still need a membership row —
`org:{id}` is an operator grant already, never implied by the creator sitting
in an org. First-party vs third-party is a Connect application flag; it does
not widen the catalog.

---

## Frozen scopes

`resource:action`. One pair is one grant. Read does not imply write. Write
does not imply read. A missing kernel door stays missing — the scope reserves
the grant for when that door exists; it does not mint the RPC.

| Scope | Grants | Kernel today | Stay out of the kernel |
|---|---|---|---|
| `books:read` | List/get books the subject can see | `ListBooks` / `GetBook` | — |
| `books:write` | Create a book; update sidecar metadata | `CreateBook` | Not a fund, not a WorkOS org, not a fifth kind |
| `books:ingest` | Deliveries into the ingest plane | `Ingest` | Not a bank-OAuth product |
| `journals:read` | Entries, digests, prefix | journal fold / prefix reads | — |
| `journals:post` | Post **allowlisted** templates for this `client_id` | `ApplyEvent` | Not a general append; see [allowlist](#journalspost-allowlist) |
| `statements:read` | Sheet / P&L / cash-flow / aging / close status / Personal cash forecast | kind-selected chrome | Not a bank-balance predictor, not a credit score |
| `views:read` | As-of cuts; which views a book declares | `ListViews` / `GetView` | — |
| `positions:read` | Holdings | positions fold | — |
| `lots:read` | Open lots, realized gains, wash flags when cited | lot book | Not a Method, not an Order |
| `lots:elect` | SpecID names on a sale (`identified_lots`) where the RuleSet allows | entry field | ⛔ Not `lot_method = "specific_id"`. Not MinTax. Not average cost. Those are elections, not this scope |
| `nav:read` | NAV strikes, roll-forward | strike / roll-forward reads | — |
| `nav:strike` | Request a strike | `ratio strike` is CLI-only | The write RPC does not exist; the scope does not invent it |
| `partners:read` | Partner master, capital, commitments | `/capital` | Not a waterfall, not IRR / TVPI |
| `partners:write` | Partner master (write carefully) | sidecar / partner rows | Not capital-call notices as a product (the citeable notice object is core / `/capital`; e-sign / CRM / LP portal stay Connect); not an LP portal |
| `capital:read` | Capital activity, undrawn | `/capital` | — |
| `commits:read` | Commitments / undrawn | `/capital` | — |
| `calls:post` | Capital calls through allowlisted `call_*` templates | `ApplyEvent` + `call_lp` / `call_gp` | Not a future call schedule |
| `fees:read` | Fee terms | RuleSet fee rules | Not management-fee billing as a desk product |
| `fees:accrue` | Accrual post through allowlisted fee templates | `ApplyEvent` + fee rules | Not a billing engine |
| `budget:read` | Household or project budget vs actual | `/budget` | Not EAC, not a forecast |
| `billing:read` | Project billed / earned / retainage / collections | `/billing` | Not AIA G702 product UI |
| `breaks:read` | Exception queue | break report | — |
| `breaks:explain` | Person-attributed explanations | `Mark` / accept | The explainer is a person; a Connect app attributes, it does not invent |
| `closes:read` | Close records, closed-through day | `ListPeriodCloses` / `GetPeriodClose` | Not `ratio close` — that stays a person at a terminal |
| `config:read` | RuleSet / lot-terms cites, config digests | configuration screens | Not an editor |
| `audit:export` | Evidence pack | Connect scaffold (`connect/audit-export/`); first-party Connect apps call ConnectApiUrl; leftover is WorkOS dashboard registration | A pack is a read of cites, not a rewrite |
| `deliveries:write` | Fact-plane deliveries | `Ingest` | High trust; same membership door |
| `facts:admit` | Admit facts | `Admit` | High trust; provenance stays required |
| `webhooks:journal` | Subscribe to append / close events | not built | Not a second journal |

### Aliases that are not grantable

The issue body and the mapper-pass comment named the same doors twice. The
left column is refused as a scope string so a near-miss cannot sneak through.

| Do not grant | Canonical |
|---|---|
| `journal:read` | `journals:read` |
| `journal:append` | `journals:post` |
| `projects:budget:read` | `budget:read` |
| `projects:billing:read` | `billing:read` |

`budget:read` is kind-selected chrome — Personal household and Project job
figures share a URL pattern. Prefixing `projects:` would refuse a household
app that only needs the budget page. The mapper-pass names stay as aliases
so a reviewer can see the merge; they are not a second pair of grants.

---

## Hard non-scopes

Named so they stop being tempting. **Absence is the fence** — the same
reason `approve_rule` is not a tool. A permission check somebody could later
relax is not a refusal.

| Must not be minted | Why |
|---|---|
| `rules:approve` | Approval is `ratio approve` at a terminal. A Connect scope that approved a rule would be a write tool a model (or a vendor app) can call. |
| `config:promote` | `set_active` is the only way policy moves. A Connect app that promoted a digest would put a vendor's name on the administration agreement. |
| portal impersonation | An app that acts as the operator is a second identity. Membership is the subject's, not the client's. Do not mint `portal:impersonate` or `impersonate`. |
| payment initiation | A separate compliance app, if it is ever built. Do not mint `payments:initiate`. Bank OAuth stays refused. |

Rule approval is the same row as `rules:approve`. It is listed twice in the
issue because it is the fence this repository is for.

---

## Stay Connect-apps, not core

The issue's refuse list, said here so a later RPC does not "just" add them:

client portal · bank OAuth · CRM · tax e-file · vendor portal · waterfall
engine · GC/sub marketplace · e-signature · AIA G702 product UI · payroll ·
inventory / COGS · credit score · bank balance predictors · equalization
calculation · drip subscription packaging · side-pocket ops ·
FX rate providers (fact apps; #178).

Each of those may *read* through a scope above. None of them becomes a
kernel method.

⭐ **equalization, drip, and side-pocket stay Connect**
([#177](https://github.com/mattmarshall/ratio/issues/177)). Not core:
none of the three changes conservation or journal integrity. Equalization
is a valuation of NAV and the named cut; the books half is already
`subscribe_*`. A drip is `distribute_*` then `subscribe_*` plus an LP
election (#161). A side pocket is a share-class / instrument partition
and a named partner cut — a silent 1/N of pocket NAV is the defect #180
already refused. Do not mint `equalization:*` or `sidepocket:*`. Existing
scopes: `nav:read`, `partners:read`, `capital:read`, `positions:read`,
`config:read`, `journals:post`. Drip elections stay on #161. Equalization
and side-pocket first-party apps are not filed. The PLAN amendment
closes the decision card; this file does not close #161 or #150.

---

## `journals:post` allowlist

Documented. Not implemented.

`journals:post` is not `Journal::append` with a Connect sticker on it. A
client that can post any dated entry can write a sale, a close, a wash, or
a partner allocation. The grant is **this `client_id` may instantiate these
already-approved template ids, on books the subject administers**.

Shape when it lands (leftover, not a schema in this PR):

```
client_id → { template_id, … }
```

Rules that hold whether or not the map has been wired:

- **Empty allowlist refuses every post.** Silence is not "all templates".
  A first-party app that has not been listed cannot post.
- **The template must already be in the book's approved RuleSet.** A
  `client_id` that lists `call_lp` on a Personal book is refused — CreateBook
  never wrote that rule. Kind-aware, not a shared menu. Same reason
  `ingest_template_ids` will not offer `custodian-positions` to a household.
- **The entry still hits every door the kernel already has.** Closed-through,
  conservation, identified lots, wash window, bounds. A scope does not
  waive a proof.
- **`calls:post` and `fees:accrue` are the same shape** over a narrower
  set (`call_*`, fee-accrual templates). They do not bypass this allowlist;
  they are a tighter grant of the same verb.

Illustrative ids — what CreateBook seeds, **not** a grant table. An
implementation copies from the book's live RuleSet, not from this list.

| Kind | Ingest mappings (for `books:ingest`) | Posting templates a first-party app might be listed for |
|---|---|---|
| Personal | `bank-statement`, `loan-payment`, `brokerage-statement`, `brokerage-positions` | `living_expense`, `household_income`, `card_charge`, transfers, `spend_*`, `receive_income`, named-loan interest/principal, `forecast_*`, `scheduled_*` |
| Investment | `custodian-positions`, `prime_equity_trades`, `capital-calls`, `subscriptions` | `contribute_*`, `distribute_*`, `allocate_*`, `commit_*`, `call_*`, `subscribe_*`, `redeem_*`, `equity_purchase`, `disposal_proceeds` |
| Project | `project-invoices`, `change-orders`, `purchase-orders` | `project_cost*`, `vendor_invoice*`, `progress_bill`, `pay_vendor`, `earn_progress`, `collect_receivable`, `approve_co*`, `award_commitment*` |
| Operating | (kind chart; no fund feed) | `invoice_customer`, `collect_receivable`, `vendor_bill`, `pay_vendor` |

A tax-lot Method or Order is not a template id and is not listable here.

---

## Leftovers (do not close #150)

The catalog is this file and the PLAN amendment. The rest of #150 is still
open:

1. **API authorizer accepts Connect access tokens** with these scopes —
   Built. Write-route actor = WorkOS `sub` and the in-process ACL fence
   landed in #151. A Connect-shaped token is always `scoped`, does not
   inherit `org:{id}`, and needs the matching frozen name. API
   Gateway JWT verifies Connect tokens on the Connect HTTP API
   (AuthKit custom-domain `iss`; session tokens stay on DemoUrl).
   leftover #22 is now `DEMO_MEMBERS` naming a live WorkOS `sub`
   and WorkOS dashboard registration — unused Cognito
   CloudFormation resources removed — not the in-process grant, not the
   gateway issuer, not the deployed open-demo dial
   (`RATIO_DEMO_OPEN` defaults off on DemoUrl), and not the
   first-party Connect grant helper (first-party Connect apps
   call ConnectApiUrl).
2. **Book ACL on every Connect grant.** Built with the authorizer.
   Authorized-empty for a book the subject does not administer. An
   `org_id` claim is not membership.
3. **`journals:post` allowlist** keyed by `client_id` — the map above,
   enforced at `ApplyEvent`, empty-refuses.
4. **Reference Connect app skeleton** — read-only `books:read` +
   `statements:read`, proving the door opens without a new RPC.
   A first-party bank-feed scaffold lives at `connect/bank-feed/`
   ([#165](https://github.com/mattmarshall/ratio/issues/165)). It is not
   this leftover: it requests `journals:post`, first-party Connect
   apps call ConnectApiUrl, and this file still does not close #150 or #165.
   A first-party tax-pack scaffold lives at `connect/tax-pack/`
   ([#166](https://github.com/mattmarshall/ratio/issues/166)). It is not
   this leftover either: it requests `lots:read` and `config:read`,
   first-party Connect apps call ConnectApiUrl, and this file still does not close
   #166.
   A first-party net-worth goals scaffold lives at `connect/goals/`
   ([#168](https://github.com/mattmarshall/ratio/issues/168)). It is not
   this leftover either: it requests `journals:post` for opt-in
   scenario journals, first-party Connect apps call ConnectApiUrl, and this file
   still does not close #168.
   A first-party AIA pay-app scaffold lives at `connect/aia-pay-app/`
   ([#184](https://github.com/mattmarshall/ratio/issues/184)). It is not
   this leftover either: it requests `billing:read` and `budget:read`,
   first-party Connect apps call ConnectApiUrl, and this file still does not close
   #184.
   A first-party vendor / GC portal scaffold lives at
   `connect/vendor-portal/`
   ([#172](https://github.com/mattmarshall/ratio/issues/172)). It is not
   this leftover either: it requests `billing:read`, `budget:read`, and
   `journals:post` for allowlisted `vendor_invoice*` templates;
   first-party Connect apps call ConnectApiUrl, and this file still does not close #172.
   A first-party EAC / forecast scaffold lives at
   `connect/eac-forecast/`
   ([#169](https://github.com/mattmarshall/ratio/issues/169)). It is not
   this leftover either: it requests `budget:read` and `billing:read`,
   first-party Connect apps call ConnectApiUrl, and this file still does not close
   #169.
   A first-party program roll-up scaffold lives at
   `connect/program-rollup/`
   ([#179](https://github.com/mattmarshall/ratio/issues/179)). It is not
   this leftover either: it requests `books:read`, `budget:read`, and
   `billing:read`, first-party Connect apps call ConnectApiUrl, and this file still
   does not close #179.
   First-party Personal cash-forecast predictor scaffolds live at
   `connect/bank-balance-predictor/` and `connect/calendar-bills/`
   ([#163](https://github.com/mattmarshall/ratio/issues/163)). They are
   not this leftover either: they request `journals:post` for
   allowlisted `forecast_*` / `scheduled_*` templates;
   first-party Connect apps call ConnectApiUrl, and this file still does not close #163.
   A first-party audit-export scaffold lives at
   `connect/audit-export/`
   ([#185](https://github.com/mattmarshall/ratio/issues/185)). It is
   not this leftover either: it requests `audit:export` plus
   `closes:read`, `breaks:read`, `breaks:explain`, `nav:read`,
   `journals:read`, `config:read`, and `books:read`, it is a read
   of cites (not a write RPC and not a kernel blob store);
   first-party Connect apps call ConnectApiUrl, and this file still does not close
   #185.
   A first-party LP / investor portal scaffold lives at
   `connect/lp-portal/`
   ([#161](https://github.com/mattmarshall/ratio/issues/161)). It is
   not this leftover either: it requests `partners:read`,
   `statements:read`, `nav:read`, and optionally `books:read`, it
   is a read of cites (not a write RPC and not a kernel portal);
   first-party Connect apps call ConnectApiUrl, and this file still
   does not close #161.
   A first-party fund-ops-alerts scaffold lives at
   `connect/fund-ops-alerts/`
   ([#162](https://github.com/mattmarshall/ratio/issues/162)). It is
   not this leftover either: it requests `webhooks:journal`,
   `breaks:read`, `nav:read`, `views:read`, and `books:read`, it
   is a read of cites (not a write RPC and not a kernel notifier);
   first-party Connect apps call ConnectApiUrl, and this file still
   does not close #162.
   A first-party Operating bank-rec scaffold lives at
   `connect/bank-rec/`
   ([#174](https://github.com/mattmarshall/ratio/issues/174)). It is
   not this leftover either: it requests `statements:read` and
   `journals:post` (the issue body still says `journal:append`; that
   alias is refused), the recon report is a read of TB / statement /
   aging cites (missing cites stay unset — never a silent
   reconciled-empty, a fake $0.00 that looks cleared, or
   empty-digest-as-success), opt-in adjustments are allowlisted
   cash-moving Operating templates, payroll / tax filing stay
   leftovers, first-party Connect apps call ConnectApiUrl, and this
   file still does not close #174.
5. **`webhooks:journal`, `nav:strike` as a write RPC** —
   reserved scopes; the surfaces are not built. `audit:export` has
   a first-party scaffold at `connect/audit-export/` (#185); that
   is a ZIP of cites, not a write RPC, and it does not close #185.
   A first-party fund-ops-alerts scaffold lives at
   `connect/fund-ops-alerts/` (#162); it requests
   `webhooks:journal` and polls `breaks:read` + `nav:read`, it is
   not a kernel webhook surface, and it does not close #162.

This file does not close #150. Nothing here finishes issue 22
(DEMO_MEMBERS naming a live WorkOS sub, WorkOS dashboard
registration). unused Cognito CloudFormation resources removed.
RATIO_DEMO_OPEN defaults off on the deployed demo.
first-party Connect apps call ConnectApiUrl.
API Gateway JWT verifies Connect tokens. Nothing here closes #5
(console wash flag), #9 (lot-relief UI cites / pooled holding-period
leftover), #163, #166, #168, #169, #172, #184, #179, #185, #161, #162, or #174.
