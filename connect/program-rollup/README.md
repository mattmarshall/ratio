# Connect app — Project program roll-up

Issue [#179](https://github.com/mattmarshall/ratio/issues/179). First-party
[WorkOS Connect](https://workos.com/docs/authkit/connect) application for
`BookKind` **PROJECT**.

Multi-contract / program views live **here**. They do not live in
`ratio watch`, the operations console, or a new kernel RPC. `/budget`
and `/billing` stay per-book core cites.

This is a scaffold. A green roll-up builder is not a live Connect
token and not a mega-book.

## What landed

- Scope declaration using the frozen catalog names only:
  `books:read`, `budget:read`, `billing:read`.
- Catalog near-misses `projects:budget:read` /
  `projects:billing:read` and `journal:append` / `journal:read` are
  refused.
- Read-only relative to the journal. `journals:post` is not requested.
- `books:read` lists books the subject can see. The roll-up keeps
  **PROJECT** rows with membership. An `org_id` claim is not
  membership — a first-party app does not inherit every book in an
  org. Non-PROJECT kinds the subject can see stay out.
- Per-book `/budget` + `/billing` cites (revised contract, billed,
  remaining to bill, collections vs billed, remaining to spend) plus
  a program CSV / JSON that sums **only the books that cited the
  figure**.
- Unset stays unset. An unbilled job is not billed-zero. An
  uncollected job is not collected-zero. A book that cannot support a
  cut does not contribute 0.00 to the program total. Treating billed
  as 0 would print the whole contract as remaining. A posted `"0.00"`
  is a figure.
- Program remaining / collected are the sum of the per-book cuts —
  never recomputed from mixed program totals (that invention would
  treat an unset book as billed-zero).
- Money is minor units, split on the point, never a float.
- `fetch_cites()` and `deliver()` refuse. Connect access tokens are
  not accepted on `/v1`.
- `mega_book()` / `merge_journals()` refuse. No fifth kind. No
  `screensFor` fork.
- `eac()` / `render_g702()` / `vendor_directory()` refuse. Those
  doors stay on #169, #184, and #172.

`bazel test //connect/program-rollup:rollup_test` is the gate.

## WorkOS Connect — application shape

This app is a **first-party OAuth** Connect application. Ratio owns it.
The actor is a user (the program administrator), so the flow is
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
| Requested scopes | `books:read` `budget:read` `billing:read` — plus `openid` if the library requires an OIDC discovery scope. Do not request `journal:append` or `journals:post`. |
| Issuer / JWKS | The AuthKit environment that already signs session JWTs. Verification is the resource server's job (`/v1`), and that authorizer is **not built**. |

A third-party flag would prompt AuthKit consent and bind the app to an
Organization. This program view is first-party: the subject's book
membership is still the tenant. An `org_id` claim is not membership.
#151 landed the write-route ACL fence: a Connect-shaped token is
`scoped` and does not inherit `org:{id}`. Accepting the token is
still leftover #22.

M2M (`client_credentials`) is the wrong shape here. There is no user
on an M2M token, and a program roll-up that exported without one
would attribute a cite to a client secret.

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

It can show two fixture jobs (original $10,000 / CO $500 / billed
$1,000 and original $4,000 / billed $500) mapping to per-book
remaining and a program billed of $1,500, an unbilled job leaving
that book's billed / remaining / collected blank rather than
inventing 0.00, the program remaining staying the sum of per-book
remaining (not program-revised minus program-billed), a
non-member PROJECT book with a matching `org_id` staying out, a
PERSONAL book the subject can see staying out, and
`journal:append` / `projects:budget:read` being rejected as scopes.

It cannot show a Connect token opening a book, a live OAuth grant,
a mega-book in the kernel, EAC / forecast (#169), AIA G702 product
UI (#184), a vendor portal (#172), or a posting that reached
`/v1`. BookKind PROJECT chrome is unchanged. `screensFor` is not
forked. `/budget` and `/billing` stay the per-book core cites.

## Leftovers — this does not close #179

1. **API authorizer accepts Connect access tokens** with these scopes
   (#150 / leftover #22). `/v1` still proves an AuthKit session JWT.
   Write-route actor binding landed (#151); this app does not reopen it.
2. **Live Connect OAuth.** Dashboard registration, redirect, and a
   token that `/v1` accepts.
3. **Live `ListBooks` filtered to PROJECT + membership** against a
   real authorizer. The scaffold holds the filter; the door is not
   open.
4. **#150's read-only reference skeleton** (`books:read` +
   `statements:read` only) is a different app. This one requests
   `budget:read` and `billing:read` as well and does not open the
   door.

Does not close #169 (EAC / forecast). Does not close #172 (vendor
user directory). Does not close #184 (AIA G702 product UI). Does
not close #150 or leftover #22. Does not reopen #151. Does not
grow `ratio watch` or Console chrome with a program URL.
