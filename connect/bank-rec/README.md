# Connect app — Operating bank reconciliation

Issue [#174](https://github.com/mattmarshall/ratio/issues/174). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **OPERATING**.

Bank reconciliation lives **here**. It does not live in `ratio watch`, the
operations console, or a new kernel RPC. Sheet, cash-flow, aging, TB, and
period close stay core.

This is a scaffold. A green cite is not a live Connect token and not a
live bank feed.

Payroll and tax filing stay **Connect-shaped leftovers on #174**. They
are refused here. This PR does not ship a fake paycheck UI or a tax
table.

## What landed

- Scope declaration using the frozen catalog names only:
  `statements:read`, `journals:post`, and optionally `books:read`.
- The issue body still says `journal:append`. That string is an alias
  and is refused. Canonical: `journals:post`
  ([docs/connect-scopes.md](../../docs/connect-scopes.md)).
- The recon report cites TB / statement cash, open AR/AP
  (`AgingSchedule.control`), and the journal digest already on the
  Operating book. Unset stays unset — a missing book-cash cite is not
  a cleared $0.00. A missing bank statement is not a silent
  reconciled-empty. An empty journal digest is unset, not
  history-intact and not success. Open AR/AP are context, never
  silent reconciling items. A posted `"0.00"` is a figure.
- The report path is **read-only by default**. It needs
  `statements:read`, not `journals:post`.
- Recon adjustments instantiate CreateBook(Operating) cash-moving
  templates already on the book (`collect_receivable`, `pay_vendor`,
  `receive_revenue`, `pay_expense`, `contribute_equity`,
  `draw_equity`). `invoice_customer` / `vendor_bill` do not move cash
  and are not listed. No new `Method` / `Order` / `lot_method`
  variant.
- Adjustments post **only if the operator opts in**. Non-opt-in must
  not post.
- `journals:post` allowlist keyed by `client_id` in
  [`app.json`](app.json). **Empty allowlist refuses every post.**
- Closed-through: a dated opt-in post on or before the book's
  closed-through day refuses the **batch**. A report is not a
  mutation. An undated row is refused so it cannot sneak past the
  gate.
- Conservation: each instantiated template is two legs of opposite
  weight in one currency. `[USD +100, EUR −100]` is not balanced.
  Money is minor units, split on the point, never a float.
- `books:read` is optional membership listing. An `org_id` claim is
  not membership.
- `payroll()` / `tax_filing()` / `inventory()` / `bank_oauth()` /
  `kernel_recon()` refuse.
- `fetch_cites()` and `deliver()` call ConnectApiUrl. first-party
  Connect apps call ConnectApiUrl with a verified Connect access
  token. Membership still required.

`bazel test //connect/bank-rec:rec_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the operating-company administrator), so the flow
is `authorization_code`, not M2M `client_credentials`.

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
| Requested scopes | `statements:read` `journals:post` `books:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append`. `journals:post` is only exercised on opt-in. `books:read` is optional. |
| Issuer / JWKS | WorkOS Connect access tokens mint `iss` as the AuthKit custom domain (`https://auth.ratio.marsh.build`). API Gateway JWT verifies them on `ConnectApiUrl` `/v1` (audience = Ratio WorkOS project client). AuthKit session tokens stay on DemoUrl. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This operating recon app is first-party: the subject's
book membership is still the tenant. An `org_id` claim is not
membership. #151: a Connect token must not bypass book ACLs.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a recon adjustment that posted without one would
attribute a bank fee to a client secret.

### Two OAuths, do not collapse them

| | Who | What it grants |
|---|---|---|
| **WorkOS Connect** | The bank-rec app, talking to Ratio | Catalog scopes on books the subject administers |
| **Bank / custodian** | The operating company, talking to a feed provider | A normalized statement ending. **Not wired.** |

The second one is leftover on this issue / #165. The report accepts a
normalized ending balance (`bank_ending` as a decimal string). It does
not speak Plaid, MX, TrueLayer, or a bank's token endpoint. This app
does not absorb #165.

## Grant contract this app honors (and cannot yet exercise)

From the catalog, restated so a later RPC does not "just" add them:

1. Token is a Connect access token, verified against the environment
   JWKS — leftover on #22.
2. AuthKit `sub` is in the book's membership — leftover on #22;
   write-route actor binding landed (#151).
3. Action is in the catalog. Aliases refused.
4. `journals:post` passes the per-`client_id` allowlist. Empty refuses.
   The write is also gated on opt-in.
5. The template is already in the book's approved RuleSet. `call_lp`
   on an Operating book is refused even if a client listed it.
6. Closed-through, conservation, bounds. A scope does not waive a proof.

Until Dashboard registration lands, a live walk-through stays leftover.
`fetch_cites()` and `deliver()` call ConnectApiUrl; they are the
honesty: they refuse with the leftover named.

## What a walk-through can and cannot show

It can show a fixture book cash of $1,000 against a bank ending of
$800 as open $200, a named outstanding deposit of $200 tying the
report, a missing book-cash or bank-ending cite leaving status
**unset** rather than a cleared $0.00, an empty digest staying unset
rather than success, open AR/AP listed as context and not filling a
missing outstanding row, a closed March refusing an opted-in 15 March
post, an empty allowlist refusing everything, `journal:append` being
rejected as a scope, and payroll / tax filing / inventory / a kernel
BankRec RPC being refused.

It cannot show a live walk-through without WorkOS dashboard
registration, a live OAuth grant, live bank OAuth, a payroll run, a
tax filing, inventory/COGS, or a posting that reached `/v1`.
BookKind OPERATING chrome is unchanged. `screensFor` is not forked.
`/sheet`, `/cashflow`, `/aging`, `/accounts`, and `/close` stay the
core cites.

## Leftovers — this does not close #174

1. **WorkOS dashboard registration**
   (#150 / leftover on issue 22). API Gateway JWT verifies Connect
   tokens on ConnectApiUrl. In-process `/v1` accepts catalog scopes
   after membership. Dashboard registration, redirect, and a live
   token stay leftover. Write-route actor binding landed (#151).
2. **Live bank / custodian OAuth.** Provider SDK, token refresh, and
   statement pull. Stays on #174 / #165. This app does not absorb
   #165.
3. **Payroll.** Connect-shaped leftover on #174. No payroll engine in
   core, no payroll account on `chart_for(Operating)`, no tax tables,
   no fake paycheck UI.
4. **Tax filing.** Connect-shaped leftover on #174. Household tax-pack
   is #166. IRS e-file stays refused.
5. **`journals:post` allowlist enforced at `ApplyEvent`** — leftover
   on #150. This app checks its own list; the kernel does not yet key
   one by `client_id`.
6. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `journals:post` for opt-in recon adjustments and leftover is WorkOS
   dashboard registration, not a missing `/v1` accept path.

Does not close #174. Does not absorb #22, #152, #163, or #165. Does
not close #150. Does not reopen #151. Does not grow `ratio watch` or
Console chrome for a bank-rec / payroll / tax product UI. Does not
add inventory/COGS to Operating.
