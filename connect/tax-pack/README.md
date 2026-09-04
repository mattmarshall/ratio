# Connect app — household tax pack export

Issue [#166](https://github.com/mattmarshall/ratio/issues/166). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Tax packing and 8949-ish / CSV export live **here**. They do not live
in `ratio watch`, the operations console, or a new kernel RPC.

This is a scaffold. A green pack builder is not a filed return.

## What landed

- Scope declaration using the frozen catalog names only:
  `lots:read`, `statements:read`, `config:read`.
- Read-only relative to the journal. `journals:post` is not requested.
- Lot + wash + lot-terms cites → Form 8949-ish CSV, plus companion
  sheets (`unclassified.csv`, `wash_cites.csv`, `lot_terms.csv`).
- No new `Method` / `Order` / `lot_method` variant.
  `lot_method = "min_tax"` / `"specific_id"` / `"average_cost"` /
  `"wash"` stay refused.
- Holding-period category when acquisition dates **agree**:
  `the_threshold_day_is_long_term`. Held exactly the threshold is LONG.
- When dates **disagree**, the category stays unset. The kernel
  rule is [`Ratio.Lots.PoolPeriod`](../../lean/Ratio/Lots/PoolPeriod.lean);
  this file cites it. The pack does not invent FIFO's oldest date
  or two Form 8949 boxes. Those rows land on `unclassified.csv`
  with the ambiguity named.
- Wash is a `WashRestatement` cite (code `W` + adjustment). The
  strike is not rewritten.
- Unset elections stay unset: `wash_window_days` is not a silent 30,
  `wash_keep_holding_period` is not a silent keep, `average_cost` is
  not a silent true.
- Money is minor units, split on the point, never a float.
- `fetch_cites()` refuses. Connect access tokens are not accepted
  on `/v1`.
- `submit()` refuses. No IRS e-file, no CPA portal, no MeF.

`bazel test //connect/tax-pack:pack_test` is the gate.

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
| Requested scopes | `lots:read` `statements:read` `config:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journals:post`. |
| Issuer / JWKS | The AuthKit environment that already signs session JWTs. Verification is the resource server's job (`/v1`), and that authorizer is **not built**. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household pack is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a tax pack that exported without one would
attribute a return to a client secret.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #151.
2. AuthKit `sub` is in the book's membership — leftover on #151.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof.

Until (1) and (2) land, `fetch_cites()` is the honesty: it refuses
with the leftover named.

## What a walk-through can and cannot show

It can show a fixture disposal mapping to an 8949 SHORT or LONG row,
a mixed-date pool landing on `unclassified.csv` rather than inventing
FIFO's oldest date, a wash cite as code `W`, and `lot_method = "wash"`
being rejected.

It cannot show a Connect token opening a book, an IRS e-file, or a
CPA portal. Mixed dates stay unclassified — that is
`Ratio.Lots.PoolPeriod`, not an invented box. BookKind PERSONAL
chrome is unchanged. `screensFor` is not forked.

## Leftovers — this does not close #166

1. **API authorizer accepts Connect access tokens** with these scopes
   (#150 / #151 / leftover #22). `/v1` still proves an AuthKit session
   JWT.
2. **Live CPA / IRS submission.** Never in core. A filed return, a
   MeF transmission, and a CPA portal stay leftover on this issue.
3. **Pooled holding-period category.** Mixed acquisition dates stay
   unset. The kernel rule is `Ratio.Lots.PoolPeriod`; this file
   cites it. The leftover that stays open is the grant path and
   IRS e-file (#166), not the category itself.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `lots:read` and `config:read` and does not open the door.

Does not close #165 (grant-path + live bank OAuth leftovers stay
on #165). Does not start #168 (net-worth goals). Does not close
#150. The category rule is `Ratio.Lots.PoolPeriod`; this file
does not reopen #9.
