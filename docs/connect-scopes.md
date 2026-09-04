# WorkOS Connect — scope catalog

**Frozen 2026-09-04.** Issue [#150](https://github.com/mattmarshall/ratio/issues/150).
PLAN amendment of the same date. This file is the contract.

⭐ **A scope is a grant, not an RPC.** Ratio's core stays a thin book of
record: the journal, lots and relief, statements, kind-aware chrome, AuthKit
tenancy. Breadth — portals, bank feeds, CRM, tax packs, vendor apps — is a
[WorkOS Connect](https://workos.com/docs/authkit/connect) application that
presents a subset of the scopes below. Growing `ratio watch` / `Console` to
match every app is the failure this catalog exists to prevent.

⚠ **The catalog is frozen. The grant path is not built.** A Connect access
token is not accepted on `/v1`. The API authorizer still proves an AuthKit
session JWT (#151 / leftover #22). Do not read a row below as a door that
opens. #150 stays open until the authorizer accepts these scopes without
bypassing book membership.

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
| `statements:read` | Sheet / P&L / cash-flow / aging / close status | kind-selected chrome | Not a forecast, not a credit score |
| `views:read` | As-of cuts; which views a book declares | `ListViews` / `GetView` | — |
| `positions:read` | Holdings | positions fold | — |
| `lots:read` | Open lots, realized gains, wash flags when cited | lot book | Not a Method, not an Order |
| `lots:elect` | SpecID names on a sale (`identified_lots`) where the RuleSet allows | entry field | ⛔ Not `lot_method = "specific_id"`. Not MinTax. Not average cost. Those are elections, not this scope |
| `nav:read` | NAV strikes, roll-forward | strike / roll-forward reads | — |
| `nav:strike` | Request a strike | `ratio strike` is CLI-only | The write RPC does not exist; the scope does not invent it |
| `partners:read` | Partner master, capital, commitments | `/capital` | Not a waterfall, not IRR / TVPI |
| `partners:write` | Partner master (write carefully) | sidecar / partner rows | Not capital-call notices as a product; not an LP portal |
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
| `audit:export` | Evidence pack | not built | A pack is a read of cites, not a rewrite |
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
inventory / COGS · credit score · cash forecast.

Each of those may *read* through a scope above. None of them becomes a
kernel method.

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
| Personal | `bank-statement`, `loan-payment` | `living_expense`, `household_income`, `card_charge`, transfers, `spend_*`, `receive_income`, named-loan interest/principal |
| Investment | `custodian-positions`, `prime_equity_trades`, `capital-calls` | `contribute_*`, `distribute_*`, `allocate_*`, `commit_*`, `call_*`, `equity_purchase`, `disposal_proceeds` |
| Project | change-orders, purchase-orders, … | `project_cost*`, `vendor_invoice*`, `progress_bill`, `collect_receivable`, `approve_co*`, `award_commitment*` |
| Operating | (kind chart; no fund feed) | `invoice_customer`, `collect_receivable`, `vendor_bill`, `pay_vendor` |

A tax-lot Method or Order is not a template id and is not listable here.

---

## Leftovers (do not close #150)

The catalog is this file and the PLAN amendment. The rest of #150 is still
open:

1. **API authorizer accepts Connect access tokens** with these scopes —
   depends on #151 / leftover #22 (write-route actor = WorkOS `sub`,
   production membership check, Cognito leftovers in deploy templates).
2. **Book ACL on every Connect grant.** A token with `books:read` still
   returns authorized-empty for a book the subject does not administer.
3. **`journals:post` allowlist** keyed by `client_id` — the map above,
   enforced at `ApplyEvent`, empty-refuses.
4. **Reference Connect app skeleton** — read-only `books:read` +
   `statements:read`, proving the door opens without a new RPC.
5. **`webhooks:journal`, `audit:export`, `nav:strike` as a write RPC** —
   reserved scopes; the surfaces are not built.

This file does not close #150. Nothing here closes #5 (console wash
flag), #9 (lot-relief UI cites), or #22.
