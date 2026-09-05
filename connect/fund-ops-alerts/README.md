# Connect app — fund ops alerts

Issue [#162](https://github.com/mattmarshall/ratio/issues/162). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **INVESTMENT** fund-ops cites.

Ops notification lives **here**. It does not live in `ratio watch`, the
operations console, a chatbot, or a kernel notification service. Core
already has breaks, `nav_gate`, and unpriced cites (#188). This app
polls them.

This is a scaffold. A green cite is not a live Connect token and
not a Slack / email / PagerDuty delivery.

## What landed

- Scope declaration using the frozen catalog names only:
  `webhooks:journal`, `breaks:read`, `nav:read`, `views:read`,
  `books:read`.
- Catalog aliases `journal:append` / `journal:read` are refused.
- Read-only relative to the journal. `journals:post` is not requested.
  The empty allowlist refuses every post. `breaks:explain` is not
  requested — this app does not invent a break explanation.
- Subscribe via `webhooks:journal` is a reserved catalog grant. The
  kernel webhook surface is not built. `subscribe()` refuses. Poll
  `breaks:read` + `nav:read` + `views:read`.
- Alert contents are kernel cites already on the book:
  unexplained HIGH breaks (`breaks:read`), `nav_gate` on GetFund /
  GetView (the same `blocking_at` fold the console badge reads —
  unexplained break, unresolved trade, unpriced), and an optional
  `NavStrike` pin.
- Unset stays unset. A missing `BreakReport` is unset, not a silent
  empty list that looks reconciled. A cited report with no lines is
  **cited-empty** — the period reconciled, and the pack says so. A
  missing `nav_gate` is unset, not an all-clear gate. Unpriced stays
  empty unless a valuation date was named. A missing NAV strike is
  unset, not NAV 0.00. An empty journal digest is unset, not
  history-intact.
- `books:read` membership. An `org_id` claim is not membership.
- Kind-aware cites, not a chrome fork. Fund-ops cites stay unset on
  Personal / Project / Operating. `screensFor` is not forked.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` / `dry_run()` call ConnectApiUrl
  with a verified Connect access token (`connect/grant.py`).
  Membership still required. A missing token is a missing token,
  not "the grant path is not built". `deliver` writes a local cite
  pack after a `/v1` pull.
- `subscribe()`, `kernel_notify()`, `chatbot()`, `html_alerts()`,
  `explain_break()`, `rewrite_strike()`, `slack()`, `email()`, and
  `pagerduty()` refuse.

`bazel test //connect/fund-ops-alerts:alerts_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the fund administrator who grants), so the flow
is `authorization_code`, not M2M `client_credentials`.

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
| Requested scopes | `webhooks:journal` `breaks:read` `nav:read` `views:read` `books:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`, `journals:post`, or `breaks:explain`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This alerts app is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. first-party Connect apps call ConnectApiUrl. WorkOS dashboard registration stays leftover #22.

M2M (`client_credentials`) still needs a membership row —
`org:{id}` is never implied.

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

It can show a fixture unexplained HIGH break as an alert line, a
missing `BreakReport` leaving that sheet blank rather than looking
reconciled, a missing `nav_gate` leaving the pack unset rather than
all-clear, unpriced staying empty until a valuation date is named,
a missing NAV strike leaving that sheet blank rather than NAV 0.00,
`journal:append` being rejected as a scope, and Slack / email /
PagerDuty / a kernel notifier / a chatbot / an HTML alert UI being
refused.

It cannot show a live walk-through without WorkOS dashboard
registration, a live OAuth grant, a Connect token that opens a real
book, a kernel webhook subscription, a Slack / email / PagerDuty
product destination, or a posting that reached `/v1`. BookKind
INVESTMENT chrome is unchanged. `screensFor` is not forked. The
exception queue and `nav_gate` stay the core cites.

## Leftovers — this does not close #162

The scaffold is complete enough that only leftover #22 blocks the
claim of a live alerts walk-through with a Connect token. Product
Slack / email / PagerDuty destinations and operator WorkOS
dashboard registration remain named on this issue.

1. **WorkOS dashboard registration**
   (leftover on issue 22). API Gateway JWT verifies Connect tokens
   on ConnectApiUrl. In-process `/v1` accepts catalog scopes after
   membership. Dashboard registration, redirect, and a live token
   stay leftover. Write-route actor binding landed (#151); this app
   does not reopen it.
2. **Live fund-ops alerts walk-through** with a Connect token that
   opens a real book. A green cite is not a live token.
3. **Live Slack / email / PagerDuty product destinations.**
   `dry_run` writes a local cite pack. Those destinations stay
   leftover on this issue.
4. **Kernel `webhooks:journal` surface.** Reserved scope; the
   surface is not built. This app polls. #150 leftover 5 stays.
5. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `webhooks:journal`, `breaks:read`, `nav:read`, and `views:read`
   as well and leftover is WorkOS dashboard registration, not a
   missing `/v1` accept path.

Leaves issue 22 open for grant-path leftovers. Does not close
#162. Does not close #150. Does not reopen #151. Does not reopen
#188. Does not grow `ratio watch` or Console chrome for an alerts
product UI.
