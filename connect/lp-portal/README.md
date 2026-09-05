# Connect app — LP / investor portal

Issue [#161](https://github.com/mattmarshall/ratio/issues/161). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **INVESTMENT**.

Partner capital, statement, and NAV reads live **here**. They do not
live in `ratio watch`, the operations console, or a new kernel RPC.
`/capital` and `/nav` stay core.

This is a scaffold. A green cite is not a live Connect token and
not an HTML LP portal.

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
  `[[partner_cut]]`. A silent 1/N of book NAV is refused. A figure
  that will not divide stays unset, not rounded.
- Commitments / undrawn stay unset when no commitment posted — not
  a callable zero. A fully-drawn line is a real zero.
- NAV cites `NavStrike` and the period roll-forward. A missing
  strike is unset, not NAV 0.00. An empty journal digest is unset,
  not history-intact. Commitment and undrawn cancel in NAV.
- Capital notices already on GetBook (digest + pinned cut + posted
  amounts). Empty is unset, not a silent waterfall.
- `statements:read` is how closed-through is read. An open period
  is unset, not a fake closed period.
- `books:read` is optional membership listing. An `org_id` claim is
  not membership.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` call ConnectApiUrl. first-party Connect apps call ConnectApiUrl with a verified Connect access token. Membership still required.
- `irr()` / `tvpi()` / `waterfall()` refuse.
- `drip()` / `drip_election()` refuse. Drip elections stay leftover
  on #161 / #177.
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
`scoped` and does not inherit `org:{id}`. first-party Connect apps call ConnectApiUrl. WorkOS dashboard registration stays leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and an LP statement that exported without one would
attribute a capital cite to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — API Gateway JWT verifies Connect tokens on ConnectApiUrl.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). Live Connect OAuth (Dashboard registration,
   redirect) stays leftover on #22.
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
into $24 / $6, a book that never committed leaving undrawn blank
rather than inventing a callable zero, a missing NAV strike
leaving that sheet blank rather than NAV 0.00, an empty digest
staying unset rather than history-intact, `journal:append` being
rejected as a scope, and IRR / TVPI / waterfall / drip / a kernel
HTML portal being refused.

It cannot show a live walk-through without WorkOS dashboard registration, a live OAuth grant, a
Connect token that opens a real book, an HTML LP portal inside
`ratio watch`, LP user tables, a document vault, IRR, TVPI, a
waterfall, a K-1 pack, a drip election, or a posting that reached
`/v1`. BookKind INVESTMENT chrome is unchanged. `screensFor` is
not forked. `/capital` and `/nav` stay the core cites.

## Leftovers — this does not close #161

The scaffold is complete enough that only leftover #22 blocks the
claim of a live LP walk-through with a Connect token. Product UX
beyond the cite CSV / JSON, drip elections (#177), and operator
WorkOS dashboard registration remain named on this issue.

1. **WorkOS dashboard registration**
   (leftover on issue 22). API Gateway JWT verifies Connect tokens
   on ConnectApiUrl. In-process `/v1` accepts catalog scopes after
   membership. Dashboard registration, redirect, and a live token
   stay leftover. Write-route actor binding landed (#151); this app
   does not reopen it.
2. **Live LP walk-through** with a Connect token that opens a real
   book. A green cite is not a live token.
3. **Drip elections.** Already Connect on #161 / #177. This app
   does not start them.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `partners:read` and `nav:read` as well and leftover is WorkOS
   dashboard registration, not a missing `/v1` accept path.

Leaves issue 22 open for grant-path leftovers. Does not close
#161. Does not close #150. Does not close #177. Does not reopen
#151. Does not grow `ratio watch` or Console chrome for an LP
product UI.
