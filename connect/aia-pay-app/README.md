# Connect app — AIA G702/G703-ish pay-app pack

Issue [#184](https://github.com/mattmarshall/ratio/issues/184). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PROJECT**.

Pay-app packing and G702-ish / G703-ish CSV export live **here**. They
do not live in `ratio watch`, the operations console, or a new kernel
RPC. `/billing` and `/budget` stay core.

This is a scaffold. A green pack builder is not a filed pay-app and
not a licensed AIA form.

## What landed

- Scope declaration using the frozen catalog names only:
  `billing:read`, `budget:read`, `statements:read`.
- The catalog's near-misses `projects:billing:read` /
  `projects:budget:read` and `journal:append` are refused.
- Read-only relative to the journal. `journals:post` is not requested.
- Project `/billing` + `/budget` cites → G702-ish application CSV plus
  G703-ish schedule-of-values, companions (earned, collected, vendor
  retainage), and an `unset.csv` naming what the journal cannot support.
- Revised contract is original + approved when `[project] budget` is
  set. An unposted change order leaves the *change* line unset; revised
  equals the original.
- Remaining to bill is revised − billed. An unbilled job is not
  billed-zero — that invention would print the whole contract as
  remaining.
- Previous certificates and current payment due stay unset until a
  prior-application cut is cited. An omitted prior is not 0.00.
- Retainage that has never been held is 0 for billed-less-retainage
  (no hold is not an unknown hold) and stays blank on the retainage
  *line*.
- Billed is Progress billings. Earned is a companion. Phase cost is
  incurred, not completed-and-stored. No percentage (that is a rounded
  figure).
- Unset stays unset: billed, retainage, and CO zeros are not invented
  to fill a form. A posted `"0.00"` is a real zero.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` refuse. Connect access tokens are
  not accepted on `/v1`.
- `render_form()` refuses. No licensed AIA PDF, no vendor portal, no
  packing inside core.

`bazel test //connect/aia-pay-app:pack_test` is the gate.

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
| Requested scopes | `billing:read` `budget:read` `statements:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journals:post`. |
| Issuer / JWKS | The AuthKit environment that already signs session JWTs. Verification is the resource server's job (`/v1`), and that authorizer is **not built**. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This job pack is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. Accepting the token is
still leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a pay-app that exported without one would
attribute an application to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #22.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). The authorizer still does not accept a
   Connect token — leftover on #22.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof.

Until (1) and (2) land, `fetch_cites()` and `deliver()` are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture job of original $10,000 / CO $500 / billed
$1,000 / retainage $100 mapping to G702-ish rows, a missing billed
cite leaving billed and remaining unset rather than inventing the
whole contract as leftover, an omitted prior leaving previous
certificates unset, phase cost staying off completed-and-stored, and
`projects:billing:read` / `journal:append` being rejected as scopes.

It cannot show a Connect token opening a book, a licensed AIA PDF, a
vendor portal, live G702 product UX beyond the pack, or EAC / forecast
(#169). BookKind PROJECT chrome is unchanged. `screensFor` is not
forked. `/billing` and `/budget` stay the core cites.

## Leftovers — this does not close #184

1. **API authorizer accepts Connect access tokens** with these scopes
   (#150 / leftover #22). `/v1` still proves an AuthKit session JWT.
   Write-route actor binding landed (#151); this app does not reopen it.
2. **Live Connect OAuth.** Dashboard registration, redirect, and a
   token that `/v1` accepts.
3. **Licensed AIA G702/G703 PDF / product UX.** Never in core. A filed
   pay-app, a vendor portal (#172), and a copyrighted form stay leftover
   on this issue.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `billing:read` and `budget:read` and does not open the door.

Does not close #165, #166, or #168 (grant-path leftovers stay on those
issues). Does not start #169 (EAC / forecast). Does not start #172
(vendor user directory). Does not close #150 or leftover #22. Does
not reopen #151.
