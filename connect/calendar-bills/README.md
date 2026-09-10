# Connect app — calendar bills sync into Personal scheduled journals

Issue [#163](https://github.com/mattmarshall/ratio/issues/163). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PERSONAL**.

Dated bill occurrences live **here**. They do not live in `ratio watch`,
the operations console, or a new kernel RPC. The citeable forecast fold
already landed in core (`/cashflow`, `filter=forecast-YYYY[-MM]`) — this
app posts the `scheduled_*` material that fold will name.

The mapper and bounded Google Calendar Events adapter are built. A green
fixture run is not a live calendar login or production consent grant.

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
- `fetch_statements()` and `deliver()` call ConnectApiUrl. first-party Connect apps call ConnectApiUrl with a verified Connect access token. Membership still required.
- Google Calendar access uses only
  `https://www.googleapis.com/auth/calendar.events.readonly` and one concrete,
  explicitly selected calendar. `primary` is refused.
- Events sync is bounded and uses stable `singleEvents=true`,
  `showDeleted=true`, and page-token parameters. Google expands recurrence;
  only dated instances reach the mapper. The final sync token is returned only
  after every page and proposal passes.
- Only private event metadata `ratio_amount`, `ratio_currency`, and
  `ratio_kind` (`bill` or `income`) can select a posting. Titles, descriptions,
  organizers, and attendees never do.
- Untagged, tentative, and newly cancelled events remain visible and
  non-posting. A changed or deleted previously imported event refuses until a
  correction/reversal policy exists. Event id plus etag makes exact retries
  non-posting.
- A `410 Gone` requires full sync while retaining the prior event-id/etag
  index. It never clears or silently reposts journal history.

```
bazel test //connect/calendar-bills:bills_test //connect/calendar-bills:google_calendar_test
```

This is the focused gate.

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
| Requested scopes | `statements:read` `journals:post` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

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
| **Calendar** | The household, talking to Google Calendar | Bounded Events sync built; live OAuth activation pending. |

The Google adapter reads Events pages and hands explicitly tagged dated
instances to the mapper (`dated`, `amount` as decimal text, `currency`, `kind`).
Google expands recurring series with `singleEvents=true`; the adapter never
hands an `rrule` to the journal. Production OAuth credentials/consent and token
custody remain #163; an Outlook provider would be a separate slice.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — API Gateway JWT verifies Connect tokens on ConnectApiUrl.
2. AuthKit `sub` is in the book's membership — leftover on #22.
   Write-route actor binding landed (#151).
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
5. The template is already in the book's approved RuleSet. `call_lp`
   or `scheduled_payroll` on a Personal book is refused even if a
   client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

Until Dashboard registration lands, a live walk-through stays leftover. `fetch_statements()` and `deliver()` call ConnectApiUrl; they are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture rent mapping to `scheduled_spend`, a closed March
refusing a 15 March row, an `rrule` being rejected rather than
expanded, an empty allowlist refusing everything, `journal:append`
being rejected as a scope, and payroll / envelope kinds being refused.

It cannot show a live walk-through without WorkOS dashboard registration, a live calendar login,
a posting that reached `/v1`, envelope coaching, or payroll. BookKind
PERSONAL chrome is unchanged. `screensFor` is not forked. The
`/cashflow` forecast cite stays the core fold from #218.

## Leftovers — this does not close #163

1. **WorkOS dashboard registration**
   (leftover on issue 22). API Gateway JWT verifies Connect tokens
   on ConnectApiUrl. In-process `/v1` accepts catalog scopes after
   membership. Dashboard registration, redirect, and a live token
   stay leftover. Write-route actor binding landed (#151).
2. **Live calendar OAuth.** Google production credentials and consent, durable
   encrypted token custody/refresh, and an authorized calendar. The bounded
   occurrence pull is built; recurrence expansion stays with Google.
3. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `journals:post` for scheduled material and leftover is WorkOS dashboard registration, not a missing `/v1` accept path.

Does not close #165. Does not start or reopen #164 (envelope
invention stays refused). Does not close #150. Does not redo the
#218 core cite. Does not grow `ratio watch` or Console chrome for
a bills calendar UI.
