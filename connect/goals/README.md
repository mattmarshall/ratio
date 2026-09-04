# Connect app — net-worth goals and scenarios

Issue [#168](https://github.com/mattmarshall/ratio/issues/168). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Goals and what-if scenarios live **here**. They do not live in
`ratio watch`, the operations console, or a new kernel RPC. Sheet,
bridge, and cash-flow stay core.

This is a scaffold. A green overlay is not a live Connect token and
not a cash forecast.

## What landed

- Scope declaration using the frozen catalog names only:
  `statements:read`, `journals:post`.
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
- `fetch_statements()` and `deliver()` refuse. Connect access tokens
  are not accepted on `/v1`.

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
| Type | OAuth (not M2M) |
| Trust | First-party — Ratio deploys this tree |
| Redirect URI | The app's callback. Must match the registered value exactly, including a trailing slash. |
| Credentials | `client_id` / `client_secret` from a Connect credential. Up to five. Shown once. |
| Requested scopes | `statements:read` `journals:post` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`. `journals:post` is only exercised on opt-in. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household goals app is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a scenario journal that posted without one would
attribute a what-if to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #151.
2. AuthKit `sub` is in the book's membership — leftover on #151.
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
   The write is also gated on opt-in.
5. The template is already in the book's approved RuleSet. `call_lp`
   on a Personal book is refused even if a client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

Until (1) and (2) land, `fetch_statements()` and `deliver()` are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture sheet of $50,000 against a $75,000 target as
short, an unset sheet leaving progress unset, extra income raising
projected net worth, a card charge that does not move cash, a closed
March refusing an opted-in 15 March post, an empty allowlist refusing
everything, and `journal:append` being rejected as a scope.

It cannot show a Connect token opening a book, a live OAuth grant, a
cash forecast, a FIRE number, or a posting that reached `/v1`.
BookKind PERSONAL chrome is unchanged. `screensFor` is not forked.
Sheet, bridge, and cash-flow stay the core cites.

## Leftovers — this does not close #168

1. **Live Connect OAuth**
   (#150 / leftover on issue 22). API Gateway JWT verifies Connect
   tokens on ConnectApiUrl. In-process `/v1` accepts catalog scopes
   after membership. Dashboard registration, redirect, and a live
   token stay leftover. Write-route actor binding landed (#151).
2. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
3. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `journals:post` for opt-in scenario posts and does not open the
   door.

Does not close #165 (grant-path + live bank OAuth leftovers stay
on #165). Does not close #166 (grant path, IRS e-file, #9 leftover).
Does not close #150. Does not grow `ratio watch` or Console chrome
for goals product UI.
