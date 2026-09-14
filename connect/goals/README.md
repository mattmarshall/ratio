# Connect app — net-worth goals and scenarios

Issue [#168](https://github.com/mattmarshall/ratio/issues/168). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Goals and what-if scenarios live **here**. They do not live in
`ratio watch`, the operations console, or a new kernel RPC. Sheet,
bridge, and cash-flow stay core.

The registered public-PKCE app and live sheet reader are the activation path.
A green overlay is still not a cash forecast or permission to post.

## What landed

- Scope declaration using the frozen catalog names only:
  `books:read`, `statements:read`, `journals:post`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical: `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)).
- Goal progress cites the sheet net worth already in core against a
  named target. Unset stays unset — an empty journal is not a
  measured $0.00. A real zero is a figure. No percentage (that is a
  rounded figure).
- Scenario overlay instantiates CreateBook(Personal) templates
  already on the book (`living_expense`, `household_income`,
  `card_charge`, transfers, `spend_*`, `receive_income`). No new
  `Method` / `Order` / `lot_method` variant.
- Scenario journals post **only if the household administrator opts
  in**. Non-opt-in must not post.
- `journals:post` allowlist keyed by `client_id` in
  [`app.json`](app.json). **Empty allowlist refuses every post.**
- Closed-through: a dated opt-in post on or before the book's
  closed-through day refuses the **batch**. An overlay is not a
  mutation. An undated row is refused so it cannot sneak past the
  gate.
- Conservation: each instantiated template is two legs of opposite
  weight in one currency. `[USD +100, EUR −100]` is not balanced.
  Money is minor units, split on the point, never a float.
- Required monthly savings and a FIRE number refuse. This is not a
  cash forecast.
- `fetch_live_statement()` verifies one permitted Personal Book, reads its
  period `sheet-*` account resources, and retains the exact book, filter, and
  account names. A sheet with no posted asset or liability stays unset; an
  observed zero remains a figure. Exact Book metadata is read before and after
  the sheet; a changed view, currency, or configuration refuses. The account
  response does not carry a configuration pin, so the app does not attach the
  surrounding Book digest to the account fold.
- `evaluate_live_goal()` requires that provenance wrapper and returns the goal
  result with its exact book, filter, and account resource names. The fixture
  evaluator remains available for deterministic unit tests.
- `fetch_statements()` and `deliver()` call ConnectApiUrl. First-party Connect
  apps call ConnectApiUrl with a verified Connect access token. Membership
  remains required.

`bazel test //connect/goals:goals_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the household administrator), so the flow is
`authorization_code`, not M2M `client_credentials`.

Docs, not a Dashboard click-path:

- [Connect](https://workos.com/docs/authkit/connect)
- [OAuth applications](https://workos.com/docs/authkit/connect/oauth)

Registration notes (WorkOS Dashboard → Applications → Connect):

| | |
|---|---|
| Type | Public OAuth native client with PKCE S256 (not M2M) |
| Trust | First-party — Ratio deploys this tree |
| Client | `client_01M287GBM6W5NB79T9ZQG7M741` (public; no client secret) |
| Redirect URI | `http://127.0.0.1:8765/callback` exactly. WorkOS permits loopback HTTP for production native clients. |
| Credentials | Public `client_id` only. No client secret is created, stored, printed, or sent. |
| Requested scopes | `books:read` `statements:read` `journals:post` — plus `openid` as the protocol scope. Do not request `journal:append`. `journals:post` is only exercised on opt-in. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household goals app is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a scenario journal that posted without one would
attribute a what-if to a client secret.

## Grant contract this app honors

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment JWKS.
2. AuthKit `sub` is in the book's membership.
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
   The write is also gated on opt-in.
5. The template is already in the book's approved RuleSet. `call_lp`
   on a Personal book is refused even if a client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

The WorkOS application is registered. `fetch_live_statement()` verifies the
BookKind and selected resource before measuring a goal; `deliver()` remains
behind the explicit opt-in and the API's client-template policy.

## What a walk-through can and cannot show

It can show a fixture sheet of $50,000 against a $75,000 target as
short, an unset sheet leaving progress unset, extra income raising
projected net worth, a card charge that does not move cash, a closed
March refusing an opted-in 15 March post, an empty allowlist refusing
everything, and `journal:append` being rejected as a scope.

It cannot show a cash forecast, a FIRE number, or treat an overlay as a posting.
BookKind PERSONAL chrome is unchanged. `screensFor` is not forked.
Sheet, bridge, and cash-flow stay the core cites.

## Remaining acceptance

1. **Live cited goal.** Complete public PKCE, read one named Personal sheet,
   and retain redacted goal, unset, and refusal evidence on #168.
2. **Scenario writes remain opt-in.** The API client-template fence is built.
   No scenario is posted during activation without a separate explicit user
   instruction; a disallowed template and a closed date must refuse.
3. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `journals:post` for opt-in scenario posts.

Does not close #165 (live bank OAuth stays on #165). Does not reopen #166.
Does not close #150. Does not grow `ratio watch` or Console chrome
for goals product UI.
