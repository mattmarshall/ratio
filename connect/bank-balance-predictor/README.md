# Connect app — bank-balance predictor into Personal forecast journals

Issue [#163](https://github.com/mattmarshall/ratio/issues/163). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Predicted bank-balance movements live **here**. They do not live in
`ratio watch`, the operations console, or a new kernel RPC. The citeable
forecast fold already landed in core (`/cashflow`,
`filter=forecast-YYYY[-MM]`) — this app posts the `forecast_*` material
that fold will name.

This is a scaffold. A green mapper is not a live bank login and not a
Connect token that `/v1` accepts.

The calendar-bills sibling is [`connect/calendar-bills/`](../calendar-bills/).
Project EAC is [`connect/eac-forecast/`](../eac-forecast/) — do not
confuse or fork Personal into that tree.

## What landed

- Scope declaration using the frozen catalog names only:
  `statements:read`, `journals:post`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical: `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)).
- Predicted movements map to CreateBook(Personal) `forecast_income` /
  `forecast_spend`. ApplyEvent marks `JournalEntry.kind` from the
  rule-id prefix. A future-dated `spend_cash` is still an actual and
  is refused here.
- A predicted ending balance cites current cash (`statements:read`)
  and posts the delta. Unset cited cash is not a silent 0.00
  baseline. A predicted balance equal to cited cash is not a posting.
- `journals:post` allowlist keyed by `client_id` in
  [`app.json`](app.json). **Empty allowlist refuses every post.**
- Closed-through: a dated row on or before the book's closed-through
  day refuses the **batch**. An undated row is refused so it cannot
  sneak past the gate.
- Conservation: each instantiated template is two legs of opposite
  weight in one currency. `[USD +100, EUR −100]` is not balanced.
  Money is minor units, split on the point, never a float.
- Empty predicted batch leaves forecast net unset — not a measured
  $0.00. A net-zero pair of posts is a real zero.
- Payroll and envelope kinds refuse. #164 stays refused. No new
  `Method` / `Order` / `lot_method` variant.
- `fetch_statements()` and `deliver()` call ConnectApiUrl. first-party Connect apps call ConnectApiUrl with a verified Connect access token. Membership still required.

`bazel test //connect/bank-balance-predictor:predictor_test` is the gate.

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
| Type | OAuth (not M2M) |
| Trust | First-party — Ratio deploys this tree |
| Redirect URI | The app's callback. Must match the registered value exactly, including a trailing slash. |
| Credentials | `client_id` / `client_secret` from a Connect credential. Up to five. Shown once. |
| Requested scopes | `statements:read` `journals:post` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household predictor is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a forecast that posted without one would
attribute a predicted spend to a client secret.

### Two OAuths, do not collapse them

| | Who | What it grants |
|---|---|---|
| **WorkOS Connect** | The predictor app, talking to Ratio | Catalog scopes on books the subject administers |
| **Bank / custodian** | The household, talking to a feed provider | Predicted balances / upcoming movements. **Not wired.** |

The second one is leftover on #163. The mapper accepts a normalized
row (`dated`, `amount` as a decimal string, `currency`, `kind`) or a
predicted ending balance against a cited cash figure. It does not
speak Plaid, MX, TrueLayer, or a bank's token endpoint.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — API Gateway JWT verifies Connect tokens on ConnectApiUrl.
2. AuthKit `sub` is in the book's membership — leftover on #22.
   Write-route actor binding landed (#151).
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
5. The template is already in the book's approved RuleSet. `call_lp`
   or `forecast_payroll` on a Personal book is refused even if a
   client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

Until Dashboard registration lands, a live walk-through stays leftover. `fetch_statements()` and `deliver()` call ConnectApiUrl; they are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture predicted deposit mapping to `forecast_income`,
a predicted ending balance $500 above cited cash mapping to the same
template, a closed March refusing a 15 March row, an empty allowlist
refusing everything, `journal:append` being rejected as a scope, and
payroll / envelope kinds being refused.

It cannot show a live walk-through without WorkOS dashboard registration, a live bank login, a
posting that reached `/v1`, envelope coaching, or payroll. BookKind
PERSONAL chrome is unchanged. `screensFor` is not forked. The
`/cashflow` forecast cite stays the core fold from #218.

## Leftovers — this does not close #163

1. **WorkOS dashboard registration**
   (leftover on issue 22). API Gateway JWT verifies Connect tokens
   on ConnectApiUrl. In-process `/v1` accepts catalog scopes after
   membership. Dashboard registration, redirect, and a live token
   stay leftover. Write-route actor binding landed (#151).
2. **Live bank / custodian OAuth.** Provider SDK, token refresh, and
   predicted-balance pull.
3. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `journals:post` for forecast material and leftover is WorkOS dashboard registration, not a missing `/v1` accept path.

Does not close #165 (grant-path + live bank OAuth leftovers stay on
#165). Does not start or reopen #164 (envelope invention stays
refused). Does not close #150. Does not redo the #218 core cite.
Does not grow `ratio watch` or Console chrome for a predictor UI.
