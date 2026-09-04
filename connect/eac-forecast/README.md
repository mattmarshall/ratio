# Connect app — Project EAC / forecast

Issue [#169](https://github.com/mattmarshall/ratio/issues/169). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PROJECT**.

Estimate-at-completion and cost-to-complete live **here**. They do
not live in `ratio watch`, the operations console, or a new kernel
RPC. `/budget` remaining-to-spend stays core: revised − incurred −
awarded.

This is a scaffold. A green pack builder is not a live Connect token
and not a silent EAC of 0.

## What landed

- Scope declaration using the frozen catalog names only:
  `budget:read`, `billing:read`, `statements:read`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical write grant would be `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)), but the
  catalog has no forecast template. Posting `project_cost*` as a
  what-if would mix a forecast into the journal of record. This app
  is read-only: CSV / JSON export.
- Catalog near-misses `projects:budget:read` /
  `projects:billing:read` are refused.
- Remaining to spend is revised − incurred − awarded — the same
  door `/budget` already cites. Treating awarded as 0 would print
  budget − actual as headroom.
- When that cut can be cited: ETC = remaining + awarded, EAC =
  incurred + ETC (= revised). The assumption is written on the row.
  When it cannot: EAC and ETC stay blank — never a silent 0.00.
- A posted `"0.00"` (revised, incurred, and awarded all zero) is a
  real zero. Unset is not that.
- Billed / earned are companions, not substitutes for incurred.
- No percentage. A CPI / percent-complete EAC is refused.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` refuse. Connect access tokens are
  not accepted on `/v1`.
- `post_forecast()` refuses. No invented write scope.
- `cpi_eac()` refuses.

`bazel test //connect/eac-forecast:pack_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the project administrator), so the flow is
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
| Requested scopes | `budget:read` `billing:read` `statements:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append` or `journals:post`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This job pack is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. API Gateway JWT verifies Connect tokens on ConnectApiUrl. Live OAuth stays leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and an EAC that exported without one would
attribute a forecast to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — API Gateway JWT verifies Connect tokens on ConnectApiUrl.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). Live Connect OAuth (Dashboard registration,
   redirect) stays leftover on #22.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post. The catalog has no forecast template.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof.

Until live OAuth lands, `fetch_cites()` and `deliver()` are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture job of original $10,000 / CO $500 / incurred
$2,000 / awarded $1,500 mapping to remaining $7,000, ETC $8,500,
and EAC $10,500 with the assumption written on the row, an
unawarded job leaving remaining and EAC unset rather than printing
budget − actual as headroom, a missing incurred cite leaving EAC
blank rather than inventing 0.00, billed / earned staying off the
EAC inputs, and `journal:append` / `projects:budget:read` being
rejected as scopes.

It cannot show a Connect token opening a book, a live OAuth grant,
EAC fields on `/budget`, a CPI / percent-complete dashboard, a
forecast line that reached `/v1`, or a Personal cash forecast
(#163). BookKind PROJECT chrome is unchanged. `screensFor` is not
forked. `/budget` and `/billing` stay the core cites.

## Leftovers — this does not close #169

1. **Live Connect OAuth**
   (leftover on issue 22). API Gateway JWT verifies Connect tokens
   on ConnectApiUrl. In-process `/v1` accepts catalog scopes after
   membership. Dashboard registration, redirect, and a live token
   stay leftover. Write-route actor binding landed (#151); this app
   does not reopen it.
2. **Live EAC product UX.** Never in core. A `/budget` EAC column, a
   CPI dashboard, and a forecast journal template stay leftover on
   this issue.
3. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `budget:read` and `billing:read` and does not open the door.

Does not close #165, #166, or #168 (grant-path leftovers stay on those
issues). Does not close #172 (vendor user directory). Does not close
#184 (AIA G702 product UI). Does not close #170 (ingest change
orders / awards). Does not close #173 (collections chrome). Does
not close #163 (Personal cash forecast). Does not close #150 or
leftover #22. Does not reopen #151. Does not grow `ratio watch` or
Console chrome with EAC fields.
