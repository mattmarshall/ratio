# Connect app — audit evidence ZIP

Issue [#185](https://github.com/mattmarshall/ratio/issues/185). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application.
Kind-aware cites, not a chrome fork: closes and digests apply to every
`BookKind`; NAV strikes and breaks stay unset on kinds that do not wear
fund-ops.

Evidence packing lives **here**. It does not live in `ratio watch`, the
operations console, a new kernel blob store, or a replacement for
`ratio close`.

This is a scaffold. `fetch_cites()` / `deliver()` call ConnectApiUrl
when a verified Connect access token is presented. A green cite is
not a live WorkOS dashboard registration.

## What landed

- Scope declaration using the frozen catalog names only:
  `audit:export`, `closes:read`, `breaks:read`, `breaks:explain`,
  `nav:read`, `journals:read`, `config:read`, `books:read`.
- Catalog aliases `journal:read` / `journal:append` are refused.
- Read-only relative to the journal. `journals:post` is not requested.
  The empty allowlist in [`app.json`](app.json) would refuse every
  post anyway.
- Pack contents are kernel cites already on the book:
  `PeriodClose` (closed-through, prefix, journal digest, config
  digest, surplus), `NavStrike` (NAV, prefix, digest, wash
  qualification / `WashRestatement` cite — the strike is not
  rewritten), `Break` / `BreakExplanation` (person-attributed;
  this app does not invent an explainer), and the `config:read` /
  `journals:read` pins.
- Unset stays unset. A missing cite is named in `manifest.json` +
  `unset.csv`, not a silent empty `closes.csv` / `strikes.csv` /
  `breaks.csv` that looks complete. An empty journal digest is
  unset, not history-intact. A missing NAV is unset, not 0.00.
  A missing `BreakReport` is unset, not a silent reconciled-empty
  file. A cited report with no lines is **cited-empty** — that is
  the kernel's "the period reconciled", and the manifest says so.
  A posted `"0.00"` is a figure.
- `books:read` membership. An `org_id` claim is not membership.
- No new `Method` / `Order` / `lot_method` variant.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` call ConnectApiUrl with a verified
  Connect access token (`connect/grant.py`). Membership still
  required. A missing token is a missing token, not "the grant
  path is not built". `deliver` writes the ZIP locally after a
  `/v1` pull — it does not POST a blob to the kernel.
- `store_blob()`, `close_period()`, `lp_portal()`, `esign()`,
  and `second_journal()` refuse.

`bazel test //connect/audit-export:pack_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the administrator who already administers the
book), so the flow is `authorization_code`, not M2M
`client_credentials`.

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
| Requested scopes | `audit:export` `closes:read` `breaks:read` `breaks:explain` `nav:read` `journals:read` `config:read` `books:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journals:post` or `journal:read`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This evidence pack is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. first-party Connect apps call ConnectApiUrl. WorkOS dashboard registration stays leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and an evidence ZIP that exported without one would
attribute an audit pack to a client secret.

## Grant contract this app honors

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — API Gateway JWT verifies Connect tokens on ConnectApiUrl.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). Live Connect OAuth (Dashboard registration,
   redirect) stays leftover on #22.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post. Empty allowlist refuses every post.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof. `audit:export` does not replace `ratio close`.

Env: `RATIO_CONNECT_API_URL` (ConnectApiUrl, never DemoUrl),
`WORKOS_CONNECT_ISSUER` (default `https://auth.ratio.marsh.build`),
`WORKOS_CLIENT_ID` (audience), `WORKOS_CONNECT_CLIENT_ID` /
`WORKOS_CONNECT_CLIENT_SECRET` (Connect application credentials),
`WORKOS_CONNECT_REDIRECT_URI` (`authorization_code`),
`RATIO_CONNECT_ACCESS_TOKEN` (already-minted token). See
[`connect/README.md`](../README.md).

## What a walk-through can and cannot show

It can show a fixture `PeriodClose` (closed 2026-03-31, prefix 12,
digest `cafe…`, surplus `$100.00`) mapping to `closes.csv`, a
`NavStrike` of `$1,000.00` mapping to `strikes.csv` without
rewriting a `WashRestatement`, a `Break` with a person-attributed
explanation mapping to `breaks.csv`, a missing close / strike /
break cite leaving that sheet **out of the ZIP** and naming the
gap on `unset.csv`, an empty digest staying unset rather than
history-intact, a cited-empty `BreakReport` named as reconciled
rather than missing, `journal:read` being rejected as a scope,
and a non-member book with a matching `org_id` being refused.

It can show `fetch_cites()` / `deliver()` presenting a Connect
access token against ConnectApiUrl. It cannot show a live walk-through
without WorkOS dashboard registration, a kernel blob store, a
period-close replacement, an LP portal, e-sign, or a second journal.
Chrome is unchanged. `screensFor` is not forked. `/close`, `/asof`,
Exceptions, and NAV strikes stay the core cites.

## Leftovers — this does not close #185

1. **WorkOS dashboard registration** (leftover on issue 22).
   first-party Connect apps call ConnectApiUrl. In-process `/v1`
   accepts catalog scopes after membership. API Gateway JWT verifies
   Connect tokens. A human still has to register the Connect
   application and present a live token. `DEMO_MEMBERS` naming a
   live WorkOS `sub` and unused Cognito CloudFormation resources
   stay leftover. Write-route actor binding landed (#151); this app
   does not reopen it.
2. **A live walk-through ZIP** still needs that Dashboard
   registration. Unit tests inject a transport. A green cite is
   not a live token.
3. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `audit:export` and the read scopes above.

Leaves issue 22 open. Leaves #150
open. Does not reopen #151. Does not start #161 (LP portal).
Does not grow `ratio watch` or Console chrome for an audit ZIP.
Does not add a blob store to the kernel.
