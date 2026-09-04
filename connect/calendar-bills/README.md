# Connect app — calendar bills sync into Personal scheduled journals

Issue [#163](https://github.com/mattmarshall/ratio/issues/163). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Dated bill occurrences live **here**. They do not live in `ratio watch`,
the operations console, or a new kernel RPC. The citeable forecast fold
already landed in core (`/cashflow`, `filter=forecast-YYYY[-MM]`) — this
app posts the `scheduled_*` material that fold will name.

This is a scaffold. A green mapper is not a live calendar login and not
a Connect token that `/v1` accepts.

The bank-balance-predictor sibling is
[`connect/bank-balance-predictor/`](../bank-balance-predictor/).
Project EAC is [`connect/eac-forecast/`](../eac-forecast/) — do not
confuse or fork Personal into that tree.

## What landed

- Scope declaration using the frozen catalog names only:
  `statements:read`, `journals:post`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical: `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)).
- Dated occurrences map to CreateBook(Personal) `scheduled_income` /
  `scheduled_spend`. ApplyEvent marks `JournalEntry.kind` from the
  rule-id prefix. A future-dated `spend_cash` is still an actual and
  is refused here.
- Recurrence stays in the calendar. An `rrule` / `repeat` field is
  refused rather than expanded into the journal. Each occurrence is
  a dated row.
- `journals:post` allowlist keyed by `client_id` in
  [`app.json`](app.json). **Empty allowlist refuses every post.**
- Closed-through: a dated row on or before the book's closed-through
  day refuses the **batch**. An undated row is refused so it cannot
  sneak past the gate.
- Conservation: each instantiated template is two legs of opposite
  weight in one currency. `[USD +100, EUR −100]` is not balanced.
  Money is minor units, split on the point, never a float.
- Empty occurrence batch leaves scheduled net unset — not a measured
  $0.00. A net-zero pair of posts is a real zero.
- Payroll and envelope kinds refuse. A calendar "paycheck" is payroll
  invention, not a bill. #164 stays refused. No new `Method` /
  `Order` / `lot_method` variant.
- `fetch_statements()` and `deliver()` refuse. Connect access tokens
  are not accepted on `/v1`.

`bazel test //connect/calendar-bills:bills_test` is the gate.

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
| Issuer / JWKS | The AuthKit environment that already signs session JWTs. Verification is the resource server's job (`/v1`), and that authorizer is **not built**. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This household bills app is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a scheduled rent that posted without one would
attribute a bill to a client secret.

### Two OAuths, do not collapse them

| | Who | What it grants |
|---|---|---|
| **WorkOS Connect** | The bills app, talking to Ratio | Catalog scopes on books the subject administers |
| **Calendar** | The household, talking to a calendar provider | Dated occurrences. **Not wired.** |

The second one is leftover on #163. The mapper accepts a normalized
row (`dated`, `amount` as a decimal string, `currency`, `kind`). It
does not speak Google Calendar, Outlook, or a calendar token endpoint,
and it does not expand an `rrule` into the journal.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #22 / #150.
2. AuthKit `sub` is in the book's membership — leftover on #22.
   Write-route actor binding landed (#151).
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
5. The template is already in the book's approved RuleSet. `call_lp`
   or `scheduled_payroll` on a Personal book is refused even if a
   client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

Until (1) and (2) land, `fetch_statements()` and `deliver()` are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture rent mapping to `scheduled_spend`, a closed March
refusing a 15 March row, an `rrule` being rejected rather than
expanded, an empty allowlist refusing everything, `journal:append`
being rejected as a scope, and payroll / envelope kinds being refused.

It cannot show a Connect token opening a book, a live calendar login,
a posting that reached `/v1`, envelope coaching, or payroll. BookKind
PERSONAL chrome is unchanged. `screensFor` is not forked. The
`/cashflow` forecast cite stays the core fold from #218.

## Leftovers — this does not close #163

1. **API Gateway JWT authorizer still AuthKit-session only**
   (leftover on issue 22). In-process `/v1` accepts Connect catalog
   scopes after membership. A live Connect token can still 401 at
   the edge. Write-route actor binding landed (#151).
2. **Live calendar OAuth.** Provider SDK, token refresh, and
   occurrence pull. Recurrence expansion stays with the provider.
3. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `journals:post` for scheduled material and does not open the door.

Does not close #165. Does not start or reopen #164 (envelope
invention stays refused). Does not close #150. Does not redo the
#218 core cite. Does not grow `ratio watch` or Console chrome for
a bills calendar UI.
