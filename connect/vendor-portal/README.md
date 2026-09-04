# Connect app — Project vendor / GC portal

Issue [#172](https://github.com/mattmarshall/ratio/issues/172). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PROJECT**.

Vendor-facing billing and retainage reads live **here**. They do not
live in `ratio watch`, the operations console, or a new kernel RPC.
`/billing` and `/budget` stay core.

This is a scaffold. A green cite is not a live Connect token and
not a vendor user directory.

## What landed

- Scope declaration using the frozen catalog names only:
  `billing:read`, `budget:read`, `statements:read`, `journals:post`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical write grant: `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)).
  Catalog near-misses `projects:billing:read` /
  `projects:budget:read` are refused.
- Vendor statement cites project billed / earned / retainage /
  collections from kernel reads. Unset stays unset — an unbilled
  job is not billed-zero. Treating billed as 0 would print the
  whole contract as remaining. A posted `"0.00"` is a figure.
- Remaining to bill is revised − billed. Collections vs billed is
  cash against AR (billed − outstanding receivable − retainage
  held). Unheld retainage is 0 for the subtraction and stays blank
  on the retainage *line*. Same doors as `/billing`.
- Billed is Progress billings. Earned is Project revenue. They can
  diverge. No percentage (that is a rounded figure).
- Vendor invoices post **only** as allowlisted `journals:post` for
  already-seeded `vendor_invoice*` templates. Empty allowlist
  refuses every post. Closed-through refuses the batch.
- No new `Method` / `Order` / `lot_method` variant.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` refuse. Connect access tokens are
  not accepted on `/v1`.
- `render_g702()` refuses. AIA G702 product UI is #184.
- `eac()` / `forecast()` refuse. Those stay on #169.
- `vendor_directory()` refuses. No vendor user directory in core.

`bazel test //connect/vendor-portal:portal_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the project administrator or a vendor the
administrator grants), so the flow is `authorization_code`, not M2M
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
| Requested scopes | `billing:read` `budget:read` `statements:read` `journals:post` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`. `journals:post` is only exercised for allowlisted `vendor_invoice*`. |
| Issuer / JWKS | The AuthKit environment that already signs session JWTs. Verification is the resource server's job (`/v1`), and that authorizer is **not built**. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This job portal is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. Accepting the token is
still leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a vendor invoice that posted without one would
attribute a payable to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #22.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). The authorizer still does not accept a
   Connect token — leftover on #22.
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
   The named templates are already in the book's approved RuleSet.
5. Closed-through, conservation, bounds. A scope does not waive a
   proof.

Until (1) and (2) land, `fetch_cites()` and `deliver()` are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture job of original $10,000 / CO $500 / billed
$1,000 / retainage $100 / AR $500 as collections $400 and remaining
$9,500, an unbilled job leaving billed and remaining unset rather
than inventing the whole contract as leftover, a site invoice
mapping to `vendor_invoice_site`, a closed March refusing a 15 March
invoice, an empty allowlist refusing everything, and
`journal:append` / `projects:billing:read` being rejected as scopes.

It cannot show a Connect token opening a book, a live OAuth grant, a
vendor user directory in core, AIA G702 product UI (#184), EAC /
forecast (#169), or a posting that reached `/v1`. BookKind PROJECT
chrome is unchanged. `screensFor` is not forked. `/billing` and
`/budget` stay the core cites.

## Leftovers — this does not close #172

1. **API authorizer accepts Connect access tokens** with these scopes
   (#150 / leftover #22). `/v1` still proves an AuthKit session JWT.
   Write-route actor binding landed (#151); this app does not reopen it.
2. **Live Connect OAuth.** Dashboard registration, redirect, and a
   token that `/v1` accepts.
3. **Vendor user directory in Ratio core.** Never. Membership is the
   subject's AuthKit `sub`. This app refuses to invent one.
4. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
5. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `billing:read`, `budget:read`, and `journals:post` and does not
   open the door.

Does not close #22 (grant-path leftovers stay there). Does not close
#184 (AIA G702 product UI). Does not close #169 (EAC / forecast).
Does not close #161 (LP portal). Does not close #165, #166, #168, or
#150. Does not grow `ratio watch` or Console chrome for vendor
product UI.
