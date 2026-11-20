# First-party Connect apps

Scaffolds under this tree are WorkOS Connect applications, not kernel
RPCs. They share [`grant.py`](grant.py): a verified Connect access
token pulls cites and delivers against **ConnectApiUrl**, never DemoUrl.

⭐ Core stays a thin book of record. Breadth is WorkOS Connect. Live
registration leftover is #22. PLAN maps refuse phrases to Connect
issues or never.

## Apps

Each directory has its own README with its scopes, refusals and leftovers.
Tests run under Bazel (`bazel test //connect/...`).

| App | Book kind | What it does | What it refuses |
|---|---|---|---|
| [`bank-feed`](bank-feed/) ([#165](https://github.com/mattmarshall/ratio/issues/165)) | Personal | maps household bank rows onto Personal templates; local activation binds WorkOS membership to Plaid with encrypted custody | production webhooks and institution evidence remain |
| [`bank-balance-predictor`](bank-balance-predictor/) ([#163](https://github.com/mattmarshall/ratio/issues/163)) | Personal | posts bank-balance predictions as forecast journals | — |
| [`calendar-bills`](calendar-bills/) ([#163](https://github.com/mattmarshall/ratio/issues/163)) | Personal | syncs calendar bills into scheduled journals | — |
| [`ecb-rates`](ecb-rates/) ([#178](https://github.com/mattmarshall/ratio/issues/178)) | Personal | cited ECB reference rates | — |
| [`tax-pack`](tax-pack/) ([#166](https://github.com/mattmarshall/ratio/issues/166)) | Personal | 8949-ish CSV from lot, wash and lot-terms cites | mixed dates stay unclassified; IRS e-file |
| [`goals`](goals/) ([#168](https://github.com/mattmarshall/ratio/issues/168)) | Personal | net-worth goals and what-if scenarios from sheet / bridge / cash-flow cites; scenario journals post only on opt-in | not a cash forecast |
| [`lp-portal`](lp-portal/) ([#161](https://github.com/mattmarshall/ratio/issues/161)) | Investment | LP / investor portal: partner capital, statement and NAV reads | a new kernel RPC |
| [`fund-ops-alerts`](fund-ops-alerts/) ([#162](https://github.com/mattmarshall/ratio/issues/162)) | Investment | polls breaks, `nav_gate` and unpriced cites and alerts on them | unpriced stays empty without an as-of |
| [`audit-export`](audit-export/) ([#185](https://github.com/mattmarshall/ratio/issues/185)) | any | audit evidence ZIP: closes and digests on every kind; strikes and breaks where a kind has them | unset or empty cites |
| [`aia-pay-app`](aia-pay-app/) ([#184](https://github.com/mattmarshall/ratio/issues/184)) | Project | G702-ish / G703-ish CSV from billing and budget cites | missing cites stay unset; a licensed AIA form |
| [`vendor-portal`](vendor-portal/) ([#172](https://github.com/mattmarshall/ratio/issues/172)) | Project | vendor / GC portal citing billed, earned, retainage, collections; vendor invoices are allowlisted `journals:post` | a vendor user directory in core |
| [`eac-forecast`](eac-forecast/) ([#169](https://github.com/mattmarshall/ratio/issues/169)) | Project | EAC / ETC CSV; remaining to spend is revised − incurred − awarded | an unset EAC is blank, never a silent 0; `/budget` does not forecast |
| [`program-rollup`](program-rollup/) ([#179](https://github.com/mattmarshall/ratio/issues/179)) | Project | rolls budget and billing cites across membership-visible Project books | an `org_id` claim is not membership; no mega-book in the kernel |
| [`bank-rec`](bank-rec/) ([#174](https://github.com/mattmarshall/ratio/issues/174)) | Operating | bank reconciliation | — |

## Live OAuth shape

Not a second IdP. Token mint is [WorkOS Connect](https://workos.com/docs/authkit/connect)
against the AuthKit custom domain.

| Flow | When | Endpoint |
|---|---|---|
| `authorization_code` | User-actor first-party apps (audit-export, lp-portal, fund-ops-alerts, bank-rec, tax-pack, bank-feed, …) | `{WORKOS_CONNECT_ISSUER}/oauth2/authorize` then `POST …/oauth2/token` |
| `client_credentials` | M2M. Still needs a membership row — `org:{id}` is never implied | `POST {WORKOS_CONNECT_ISSUER}/oauth2/token` |

Default issuer is `https://auth.ratio.marsh.build` (`WORKOS_CONNECT_ISSUER`).
Audience on the Connect HTTP API is `WORKOS_CLIENT_ID` (the Ratio WorkOS
project client). Per-app credentials are `WORKOS_CONNECT_CLIENT_ID` /
`WORKOS_CONNECT_CLIENT_SECRET` from Dashboard → Applications → Connect.

| Surface | Who calls it |
|---|---|
| **ConnectApiUrl** (`RATIO_CONNECT_API_URL`) | Connect apps. JWT issuer = AuthKit custom domain. |
| **DemoUrl** (`RATIO_API_ORIGIN`) | AuthKit session console. Connect tokens are refused here at the gateway. |

Membership is still required at `/v1`. A Connect token never takes
`RATIO_DEMO_OPEN` and never matches `org:{id}` (#151).

Present a token as the `token=` argument or `RATIO_CONNECT_ACCESS_TOKEN`.
Without one, `fetch_*` / `deliver` refuse because no token was presented
— not because "the grant path is not built".

## Leftovers (issue 22 stays open)

Human WorkOS dashboard registration, `DEMO_MEMBERS` naming a live
WorkOS `sub`. Unused Cognito CloudFormation resources are removed. Bank / calendar
OAuth product UI, licensed AIA forms, IRS e-file, and a kernel blob
store stay refused on the apps that name them.

`ecb-rates/` is the bounded Personal FX reference-data provider. It uses the
official ECB Data API and the existing ingest/fact plane; it carries no journal
scope. Its live WorkOS registration and permitted-book activation remain on
#22 / #178.

`oauth.py` is the shared native WorkOS activation path for the Personal Connect apps.
It uses a public WorkOS OAuth client, PKCE S256, an exact
`http://127.0.0.1:8765/callback` loopback, and state verification. It returns
the bearer in memory and never prints or persists access, refresh, or ID
tokens. Each app's `app.json` remains the exact Ratio resource-scope list;
`openid` is added only as the OAuth protocol scope.

`personal_activation.py` composes that grant with Plaid Link/Transactions and
Google Calendar for one selected Personal membership. Provider credentials,
cursors, and idempotency indexes are AES-256-GCM encrypted in a durable local
mode-0600 store keyed by a hash of WorkOS `sub` plus book. Plaid and Google
grants remain independent of WorkOS authorization. Exact production setup and
secret names are in
[`docs/personal-provider-activation.md`](../docs/personal-provider-activation.md).
Live provider and signed-in book evidence remains on #163, #165, and #22.
