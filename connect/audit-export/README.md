# Connect app — audit evidence ZIP

Issue [#185](https://github.com/mattmarshall/ratio/issues/185). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application.
Kind-aware cites, not a chrome fork: closes and digests apply to every
`BookKind`; NAV strikes and breaks stay unset on kinds that do not wear
fund-ops.

Evidence packing lives **here**. It does not live in `ratio watch`, the
operations console, a new kernel blob store, or a replacement for
`ratio close`.

This is a scaffold. A green cite is not a live Connect token and
not a live ZIP against `/v1`.

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
- `fetch_cites()` and `deliver()` refuse. Connect access tokens
  are not accepted on `/v1`.
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
| Issuer / JWKS | The AuthKit environment that already signs session JWTs. Verification is the resource server's job (`/v1`), and that authorizer is **not built**. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This evidence pack is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. Accepting the token is
still leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and an evidence ZIP that exported without one would
attribute an audit pack to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #22.
2. AuthKit `sub` is in the book's membership. Write-route actor =
   `sub` landed (#151). The authorizer still does not accept a
   Connect token — leftover on #22.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post. Empty allowlist refuses every post.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof. `audit:export` does not replace `ratio close`.

Until (1) and (2) land, `fetch_cites()` and `deliver()` are the
honesty: they refuse with the leftover named.

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

It cannot show a Connect token opening a book, a live OAuth grant,
a ZIP that reached `/v1`, a kernel blob store, a period-close
replacement, an LP portal, e-sign, or a second journal. Chrome is
unchanged. `screensFor` is not forked. `/close`, `/asof`,
Exceptions, and NAV strikes stay the core cites.

## Leftovers — this does not close #185

1. **API authorizer accepts Connect access tokens** with these scopes
   (#150 / leftover #22). `/v1` still proves an AuthKit session JWT.
   Write-route actor binding landed (#151); this app does not reopen it.
2. **Live Connect OAuth.** Dashboard registration, redirect, and a
   token that `/v1` accepts.
3. **Live `fetch_cites()` / `deliver()` of a ZIP against `/v1`.**
   Unit tests assert the refuse and the pack shape from fixtures.
   A green cite is not a live token.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `audit:export` and the read scopes above and does not open the
   door.

Does not close #22 (grant-path leftovers stay there). Does not
close #150. Does not reopen #151. Does not start #161 (LP portal).
Does not grow `ratio watch` or Console chrome for an audit ZIP.
Does not add a blob store to the kernel.
