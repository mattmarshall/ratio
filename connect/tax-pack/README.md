# Connect app — household tax pack export

Issue [#166](https://github.com/mattmarshall/ratio/issues/166). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Tax packing and 8949-ish / CSV export live **here**. They do not live
in `ratio watch`, the operations console, or a new kernel RPC.

This is a cited export application. A green pack builder is not a filed return.

## What landed

- Scope declaration using the frozen catalog names only:
  `books:read`, `lots:read`, `statements:read`, `closes:read`, `config:read`.
- Read-only relative to the journal. `journals:post` is not requested.
- `ListDisposals` replays the selected view and exposes the exact lots relieved
  by the proof-backed engine, including a later repurchase's wash adjustment.
  The tax app does not reconstruct basis from balanced journal postings.
- Lot + wash + historically pinned lot-terms cites → Form 8949-ish CSV, plus
  companion sheets (`unclassified.csv`, `wash_cites.csv`, `lot_terms.csv`,
  `relieved_lots.csv`, and `citations.csv`).
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
- `fetch_live_pack()` selects one permitted Personal book and calls
  ConnectApiUrl. Each row retains its entry id, journal prefix, configuration
  digest, relieved lots, wash result, and holding-period threshold. A later
  active configuration cannot reclassify an earlier disposal.
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
| Type | Public OAuth native client with PKCE S256 (not M2M) |
| Trust | First-party — Ratio deploys this tree |
| Redirect URI | `http://127.0.0.1:8765/callback` exactly. WorkOS permits loopback HTTP for production native clients. |
| Credentials | Public `client_id` only. No client secret is created, stored, printed, or sent. |
| Requested scopes | `books:read` `lots:read` `statements:read` `closes:read` `config:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journals:post`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household pack is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a tax pack that exported without one would
attribute a return to a client secret.

## Grant contract this app honors

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment JWKS.
2. AuthKit `sub` is in the book's membership.
3. Action is in the catalog. Aliases refused.
4. Read-only. No `journals:post` allowlist, because this app does
   not post.
5. Closed-through, bounds, no invented Method. A scope does not
   waive a proof.

The dedicated first-party tax-pack application is registered in WorkOS with
the loopback callback and the exact `books:read`, `lots:read`,
`statements:read`, `closes:read`, and `config:read` scopes. A production OAuth
walk-through exported the permitted deterministic `personal-tax-walkthrough`
book on 2026-09-14. That deployment seed
contains a loss disposal followed by a replacement inside the elected wash
window and a separate missing-date disposal; it contains no customer data.

## What a walk-through can and cannot show

It can show a cited disposal mapping to an 8949 SHORT or LONG row,
a mixed-date pool landing on `unclassified.csv` rather than inventing
FIFO's oldest date, a wash cite as code `W`, and `lot_method = "wash"`
being rejected.

The production OAuth walk-through produced the cited wash-adjusted and
unclassified rows; the repository tests continue to verify those shapes
without depending on production. IRS e-file and a
CPA portal remain refused. Mixed dates stay unclassified — that is
`Ratio.Lots.PoolPeriod`, not an invented box. BookKind PERSONAL
chrome is unchanged. `screensFor` is not forked. Household lots stay
unset until `[personal] lot_relief = true` (#187); this pack cites
those engines and does not elect them.

## Remaining product work

1. **CPA / IRS submission.** Never in core. A filed return, a MeF
   transmission, and a CPA portal remain refused product decisions.
2. **Pooled holding-period category.** Mixed acquisition dates stay
   unset. The kernel rule is `Ratio.Lots.PoolPeriod`; this file
   cites it. The category itself is complete.
3. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `lots:read` and `config:read`; the tax-pack application is separately
   registered and proven against its permitted synthetic book.

Does not close #165 (grant-path + live bank OAuth leftovers stay
on #165). Does not start #168 (net-worth goals). Does not close
#150. The category rule is `Ratio.Lots.PoolPeriod`; this file
does not reopen #9.
