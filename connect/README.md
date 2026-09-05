# First-party Connect apps

Scaffolds under this tree are WorkOS Connect applications, not kernel
RPCs. They share [`grant.py`](grant.py): a verified Connect access
token pulls cites and delivers against **ConnectApiUrl**, never DemoUrl.

## Live OAuth shape

Not a second IdP. Token mint is [WorkOS Connect](https://workos.com/docs/authkit/connect)
against the AuthKit custom domain.

| Flow | When | Endpoint |
|---|---|---|
| `authorization_code` | User-actor first-party apps (audit-export, lp-portal, tax-pack, bank-feed, …) | `{WORKOS_CONNECT_ISSUER}/oauth2/authorize` then `POST …/oauth2/token` |
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
