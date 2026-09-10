# Connect app — bank feed into Personal books

Issue [#165](https://github.com/mattmarshall/ratio/issues/165). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Bank OAuth and transaction → journal mapping live **here**. They do not
live in `ratio watch`, the operations console, or a new kernel RPC.

The mapper, Plaid Link boundary, and bounded Transactions Sync adapter are
built. A green fixture run is not a live institution authorization.

## What landed

- Scope declaration using the frozen catalog names only:
  `books:read`, `statements:read`, `journals:post`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical: `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)).
- Feed row → already-seeded Personal templates
  (`living_expense`, `household_income`, `card_charge`, `xfer_*`,
  `spend_*`, `receive_income`). No new `Method` / `Order` /
  `lot_method` variant.
- `journals:post` allowlist keyed by `client_id` in
  [`app.json`](app.json). **Empty allowlist refuses every post.**
- Closed-through: a dated row on or before the book's closed-through
  day refuses the **batch**. An undated row is refused so it cannot
  sneak past the gate.
- Conservation: each instantiated template is two legs of opposite
  weight in one currency. `[USD +100, EUR −100]` is not balanced.
  Money is minor units, split on the point, never a float.
- `deliver()` calls ConnectApiUrl with a verified Connect access token.
  Membership is still required.
- Plaid `/transactions/sync` pagination is bounded and advances its opaque
  cursor only after a complete, mutation-consistent batch is accepted.
- Provider JSON money is decoded as `Decimal`. ISO currency, exact posted day,
  stable transaction/account ids, and no more than two decimal places are
  required.
- A caller supplies a `RuleChoice` for every settled Plaid transaction. Neither
  Plaid category nor amount sign selects an accounting rule.
- Pending rows are returned for review and never reach `map_batch`. Modified or
  removed rows refuse the batch until a correction/reversal policy exists.
- A stable digest of the Plaid transaction id becomes the Ratio event id, so an
  exact retry is idempotent without exposing the provider id in the journal.
- `/item/remove` disconnects the runtime client. It does not rewrite prior
  journal facts. The activation runner decrypts an Item token only for one
  adapter call; tokens remain redacted.
- `/link/token/create` asks only for `transactions`, explicit countries and a
  language. The WorkOS subject plus Ratio book is hashed into Plaid's stable
  non-PII `client_user_id`; neither identifier is sent in clear text.
- Link completion requires an unpredictable, expiring, one-time state. The
  public token is exchanged once, then its server-only `LinkedItem` moves the
  access token directly into `PlaidClient` without returning it to the browser.

```
bazel test //connect/bank-feed:mapper_test //connect/bank-feed:plaid_test //connect/bank-feed:plaid_link_test
```

This is the focused gate. Retained provider
fixtures exercise numeric JSON money, multi-page pending behavior, Link token
creation, and public-token exchange.

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
| Requested scopes | `books:read` `statements:read` `journals:post` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household feed is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a household feed that posted without one would
attribute a card charge to a client secret.

### Two OAuths, do not collapse them

| | Who | What it grants |
|---|---|---|
| **WorkOS Connect** | The bank-feed app, talking to Ratio | Catalog scopes on books the subject administers |
| **Bank / custodian** | The household, talking to Plaid | Link and Transactions Sync protocol boundaries built; live activation pending. |

The Plaid boundary speaks `/link/token/create`, `/item/public_token/exchange`,
`/transactions/sync`, and `/item/remove`, then hands explicitly classified rows
to the mapper (`dated`, `amount` as decimal text, `currency`, `kind`, optional
`from`/`to` for transfers). `connect/personal_activation.py` now binds that
lifecycle to the selected WorkOS subject/book, AES-256-GCM seals the Item token,
cursor, and pending index, survives restart, records pending-to-posted links,
and revokes before deleting custody. Exact production configuration is in
[`docs/personal-provider-activation.md`](../../docs/personal-provider-activation.md).
Production credentials, Dashboard redirect registration, webhooks, and a live
institution authorization remain operator work on #165.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #151.
2. AuthKit `sub` is in the book's membership — leftover on #151.
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
5. The template is already in the book's approved RuleSet. `call_lp`
   on a Personal book is refused even if a client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

Until Dashboard registration lands, a live walk-through stays leftover. `deliver()` calls ConnectApiUrl; a missing token refuses with
the leftover named.

## What a walk-through can and cannot show

It can show a fixture row mapping to `living_expense`, a closed March
refusing a 15 March row, an empty allowlist refusing everything, and
`journal:append` being rejected as a scope.

It cannot show a third-party token opening a book, a live bank login,
or a posting that reached `/v1`. BookKind PERSONAL chrome is unchanged.
`screensFor` is not forked.

## Leftovers — this does not close #165

1. **WorkOS dashboard registration**
   (#150 / leftover on issue 22). API Gateway JWT verifies Connect
   tokens on ConnectApiUrl. In-process `/v1` accepts catalog scopes
   after membership. Dashboard registration, redirect, and a live
   token stay leftover. Write-route actor binding landed (#151).
2. **Live bank evidence.** Production credentials, Dashboard redirect
   registration, webhook operation, and institution evidence. Link
   creation/exchange, encrypted per-membership Item-token custody, bounded
   Transactions Sync, pending-to-posted state, and disconnect are built.
3. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one is not that
   leftover.

Does not start #166 (tax pack) or #168 (net-worth goals). Does not
close #150.
