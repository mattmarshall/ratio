# Personal provider activation

`connect/personal_activation.py` is the local activation boundary for one
Personal book membership. It composes the existing WorkOS public-PKCE, Plaid
Link/Transactions, and Google Calendar contracts. It is not a kernel service
and it does not add a hosted callback or a console screen.

The grants are independent:

1. WorkOS Connect proves the signed-in subject can reach the selected Ratio
   book through `ConnectApiUrl`.
2. Plaid Link grants one bank Item, or Google grants one concrete calendar.
3. The provider credential and retry state are encrypted under the hash of the
   WorkOS `sub` plus book resource name. Neither an organization claim nor a
   provider grant supplies Ratio membership.

The runner checks the WorkOS access-token `sub` locally, then reads the selected
book through `ConnectApiUrl`. API Gateway performs JWT verification; the local
JWT decode is only the binding check. The returned book must be the selected
`KIND_PERSONAL` book before any provider credential is opened.

The local UI imports `TokenVault.from_environment()`, calls
`authorize_workos(...)` with the selected app manifest, subject, and book, then
uses the returned `PersonalActivation`. For Plaid it sends
`begin_plaid(...).link_token` only to Plaid Link and hands Link's one-time
`public_token`, state, and Item id directly to `complete_plaid(...)`. For Google
it calls `connect_google(...)`, which opens the browser and owns the fixed local
callback. Sync and disconnect use `sync_plaid` / `disconnect_plaid` or
`sync_google` / `disconnect_google`. Do not route provider credentials through
command-line arguments or JSON output.

## Production configuration

### WorkOS Connect

Create two first-party public OAuth applications, one from each app manifest:

| Application | Ratio scopes |
|---|---|
| `connect/bank-feed/app.json` | `books:read statements:read journals:post` |
| `connect/calendar-bills/app.json` | `books:read statements:read journals:post` |

For both applications:

- Flow: authorization code; public native client; PKCE S256; no client secret.
- Authorized redirect URI:
  `http://127.0.0.1:8765/callback` exactly.
- The runner adds only the protocol scope `openid`.
- Issuer: `WORKOS_CONNECT_ISSUER`, normally
  `https://auth.ratio.marsh.build`.
- Client id: `WORKOS_CONNECT_CLIENT_ID`, set to the selected app's public
  `client_…` value.
- Ratio API: `RATIO_CONNECT_API_URL` must be the deployed `ConnectApiUrl`, not
  `RATIO_API_ORIGIN` (DemoUrl).
- `WORKOS_CLIENT_ID` remains the Ratio project audience used by the API
  authorizer. It is not the app client id.

Do not set `WORKOS_CONNECT_CLIENT_SECRET`: this path is a public client and
never reads, sends, or stores one.

### Plaid

Use these environment names:

| Name | Value |
|---|---|
| `PLAID_CLIENT_ID` | Plaid application client id |
| `PLAID_SECRET` | Secret for the selected Plaid environment |
| `PLAID_ENV` | `production` (`sandbox` and `development` are accepted for testing) |
| `PLAID_REDIRECT_URI` | `https://ratio.marsh.build/connect/plaid` |

Add `https://ratio.marsh.build/connect/plaid` exactly to Plaid Dashboard →
Developers → API → Allowed redirect URIs. Production Plaid OAuth redirects must
be HTTPS and must not contain a query or fragment. Pass that value to
`PersonalActivation.begin_plaid(..., redirect_uri=...)` for institutions that
use OAuth. Plaid Link requests only the `transactions` product. The local Link
view passes its one-time public token directly to `complete_plaid`; it must not
place that value on a command line or in a log.

Plaid's Item access token is sealed immediately. The encrypted record also
holds the opaque Transactions cursor and pending transaction index. A complete
bounded page set commits the new cursor. Transport failure, pagination
mutation, mapping refusal, or page-bound refusal leaves the old cursor.
`pending_transaction_id` removes the corresponding pending row when Plaid
later supplies its posted transaction. Disconnect calls `/item/remove` before
deleting custody, so a failed revoke remains retryable.

### Google Calendar

Create a Google Cloud **Desktop app** OAuth client, enable the Google Calendar
API, and configure the consent screen. Desktop loopback redirects do not need
an Authorized redirect URI entry in Google Cloud; the exact redirect sent by
Ratio is:

`http://127.0.0.1:8766/google/callback`

Use these environment names:

| Name | Value |
|---|---|
| `GOOGLE_OAUTH_CLIENT_ID` | Desktop OAuth client id |
| `GOOGLE_OAUTH_CLIENT_SECRET` | Desktop OAuth client secret value |

The Google desktop flow uses PKCE S256; the verifier remains in the local
attempt and is sent only to the token endpoint. The only Google API scope is
`https://www.googleapis.com/auth/calendar.events.readonly`. The authorization
request uses offline access and explicit consent so initial activation must
return a refresh token. The runner binds state to the same WorkOS
subject/book, selects one concrete calendar (never the implicit `primary`
alias), refreshes expired access tokens, and preserves a rotated refresh token
when Google returns one.

A complete occurrence page commits the final sync token and event-id/etag
index. A `410 Gone` retries once as a full sync while retaining that index, so
already imported occurrences do not post again. Disconnect revokes the refresh
token before deleting custody.

## Encrypted custody

| Name | Meaning |
|---|---|
| `RATIO_PERSONAL_TOKEN_KEY` | URL-safe base64 encoding of exactly 32 random bytes |
| `RATIO_PERSONAL_TOKEN_STORE` | Optional path; default `~/.local/share/ratio/personal-providers.json` |

Generate the key with a password manager or secret manager, for example:

```bash
python3 -c 'import base64,secrets; print(base64.urlsafe_b64encode(secrets.token_bytes(32)).decode())'
```

Install the local Python dependency with
`python3 -m pip install -r connect/requirements.txt`. Custody uses AES-256-GCM
with a fresh nonce and membership/provider-bound authenticated data. The
directory is mode `0700`, writes are atomic, and the store and lock file are
mode `0600`. Back up the key separately: loss of the key makes grants
unreadable, while reuse of the store with a different key is an authentication
failure. To rotate, revoke grants while the old key is loaded, replace the key
and store, then reconnect each provider.

Provider tokens are never returned by status methods, printed, or logged.
Status exposes only connection metadata, cursor presence, and bounded counts.

## Focused verification

```bash
bazel test \
  //connect:oauth_test \
  //connect:personal_activation_test \
  //connect/bank-feed:mapper_test \
  //connect/bank-feed:plaid_link_test \
  //connect/bank-feed:plaid_test \
  //connect/calendar-bills:bills_test \
  //connect/calendar-bills:google_calendar_test
```

These are fake-provider and fake-ConnectApiUrl tests. They prove local
orchestration, restart durability, membership isolation, callback refusal,
retry state, pending-to-posted handling, 410 recovery, and revoke ordering.
They are not redacted production walkthrough evidence.
