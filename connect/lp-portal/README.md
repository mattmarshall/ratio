# Connect app — LP / investor portal

Issue [#161](https://github.com/mattmarshall/ratio/issues/161). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **INVESTMENT**.

Partner capital, statement, and NAV reads live **here**. They do not
live in `ratio watch`, the operations console, or a new kernel RPC.
`/capital` and `/nav` stay core.

This is a first-party Connect app. A green cite is not a hosted
live walk-through and not an HTML portal inside `ratio watch`.

## What landed

- Scope declaration using the frozen catalog names only:
  `partners:read`, `statements:read`, `nav:read`, and optionally
  `books:read`.
- Catalog aliases `journal:append` / `journal:read` are refused.
- Read-only relative to the journal. `journals:post` is not requested.
  The empty allowlist refuses every post.
- Partner capital cites `/capital` already on the book (beginning,
  contributions, distributions, allocated plugs, ending, units).
  Unset stays unset — an unposted partner is not ending-zero.
  Allocated income / expense / unrealized stay unset without a named
  `[[partner_cut]]`. Journal specials fold first; a remainder uses
  the cut. A silent 1/N of book NAV is refused. A figure that will
  not divide stays unset, not rounded.
- GetBook cites the rest of the partner / statement surface already
  on the book: `partner_cut`, `special_allocations`,
  `allocation_facts`, `fee_receivable`, `trial_balance_difference`,
  `config_digest`. Missing fee terms stay unset, not a silent zero
  receivable. `statement_from_getbook` / `cite_from_fetch` compose
  those fields from ConnectApiUrl JSON (camelCase, Int64 money as
  minor-unit digits).
- Commitments / undrawn stay unset when no commitment posted — not
  a callable zero. A fully-drawn line is a real zero.
- NAV cites `NavStrike` (valuation time, trial-balance difference,
  qualification, digest) and the period roll-forward. A missing
  strike is unset, not NAV 0.00. An empty journal digest is unset,
  not history-intact. Commitment and undrawn cancel in NAV.
- Capital notices already on GetBook (digest + pinned cut + posted
  amounts + trade date). Empty is unset, not a silent waterfall.
- `statements:read` is how closed-through is read. An open period
  is unset, not a fake closed period.
- `books:read` is optional membership listing. An `org_id` claim is
  not membership.
- Money is minor units, split on the point, never a float. GetBook
  wire digits are already minor units (`"7500"` is $75.00).
- `fetch_cites()` and `deliver()` call ConnectApiUrl. first-party Connect apps call ConnectApiUrl with a verified Connect access token. Membership still required. This app's Connect grant is proven.
- `as_html()` is a read-only cite-backed page in this tree. Blanks
  stay em-dash. It is not a kernel route.
- `irr()` / `tvpi()` / `waterfall()` refuse.
- `drip()` / `drip_election()` refuse. Drip elections stay leftover
  on #161 / #177. Equalization and side-pocket stay Connect/#177.
- `kernel_portal()` / `html_portal()` / `lp_directory()` /
  `document_vault()` / `payments_initiate()` refuse.

`bazel test //connect/lp-portal:portal_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the LP or the fund administrator who grants),
so the flow is `authorization_code`, not M2M `client_credentials`.

Docs, not a Dashboard click-path:

- [Connect](https://workos.com/docs/authkit/connect)
- [OAuth applications](https://workos.com/docs/authkit/connect/oauth)

Registration notes (WorkOS Dashboard → Applications → Connect):

| | |
|---|---|
| Type | OAuth (not M2M) |
| Trust | First-party — Ratio deploys this tree |
| Redirect URI | The app's callback. Must match the registered value exactly, including a trailing slash. |
| Credentials | `client_id` / `client_secret` from a Connect credential. Up to five. Shown once. |
| Requested scopes | `partners:read` `statements:read` `nav:read` `books:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append` or `journals:post`. `books:read` is optional. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This LP portal is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. first-party Connect apps call ConnectApiUrl. This app's Connect grant is proven. leftover #22 is other Connect apps.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and an LP statement that exported without one would
attribute a capital cite to a client secret.

## Grant contract this app honors

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — API Gateway JWT verifies Connect tokens on ConnectApiUrl.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). This app's Connect grant is proven. leftover
   #22 is other Connect apps and a console signed-in walk-through.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof.

Env names are in [`connect/README.md`](../README.md) / `connect/grant.py`. A missing token is a missing token, not "the grant path is not built".

## What a walk-through can and cannot show

It can show a fixture LP of beginning $100 / contributions $40 /
distributions $10 as ending $130, allocated income staying blank
until `[[partner_cut]]` LP 80 / GP 20 divides a $30 book figure
into $24 / $6, journal specials folding before that remainder,
GetBook `partnerCut` LP 80 / GP 20 and a tied trial balance of 0
without inventing NAV, a book that never committed leaving undrawn
blank rather than inventing a callable zero, a missing NAV strike
leaving that sheet and the Connect-side HTML blank rather than
NAV 0.00, an empty digest staying unset rather than history-intact,
`journal:append` being rejected as a scope, and IRR / TVPI /
waterfall / drip / a kernel HTML portal / LP directory / document
vault / payment initiation being refused.

It cannot show a hosted live LP walk-through, an HTML LP portal
inside `ratio watch`, LP user tables, a document vault, IRR, TVPI,
a waterfall, a K-1 pack, a drip election, or a posting. This app's
Connect grant is proven; registering *other* Connect apps stays
leftover on #22. BookKind INVESTMENT chrome is unchanged.
`screensFor` is not forked. `/capital` and `/nav` stay the core
cites.

## Leftovers — this does not close #161

1. **Hosted live LP walk-through** — a person clicking through a
   deployed portal with a Connect token. `as_html` is a cite file,
   not that product. A green cite is not a hosted walk-through.
2. **Drip elections.** Already Connect on #161 / #177. This app
   does not start them. Equalization and side-pocket stay
   Connect/#177 — not kernel primitives.
3. **#22** stays open for registering *other* Connect apps and a
   console signed-in walk-through. Do not invent WorkOS Dashboard
   clicks here. This app's grant path is proven.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `partners:read` and `nav:read` as well.

Leaves issue 22 open. Does not close #161. Does not close #150.
Does not close #177. Does not reopen #151. Does not grow
`ratio watch` or Console chrome for an LP product UI.
