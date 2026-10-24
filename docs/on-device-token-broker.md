# On-device Connect token broker decision

Related: #366. Decision accepted by Matt Marshall in the working session,
October 23–24, 2026. A limited server broker exists in the target architecture.
It holds per-membership Plaid Item tokens and Google refresh tokens and handles
provider responses. This document scopes that service; the current local
Personal runner is not a deployed implementation of the split.

## Custody boundary

| Material or operation | Owner in the proposed split |
|---|---|
| Plaid client secret, Item access token, Google client secret, refresh and transient access tokens | Broker, encrypted per membership; never returned to the device |
| OAuth state, PKCE verifier, consent, selected provider account/calendar, token rotation and revocation | Broker, bound to one authenticated membership and provider |
| Provider pagination cursors, complete raw response batches and undelivered outbox | Broker, encrypted, bounded retention, retried until device acknowledgment |
| Ratio authorization, journals, configuration, chart/account mapping, balances, NAVs, encryption keys, figure decisions and posting | Device; never sent to the broker |
| Original deliveries and durable imported/seen identifiers, pending-to-posted reconciliation and proposal outcomes | Device, recovered with the book |

Provider responses can contain financial facts and calendar content. The
broker therefore processes sensitive provider data; it must not receive
Ratio-derived balances or book content. Fetch only the selected accounts and
concrete calendar with the existing minimum scopes, and minimize retained
fields and retention. Logs, metrics, traces, crash reports and support exports
follow the same boundary. Do not put book names, configuration/prefix digests,
posting results or figures in delivery acknowledgments.

Enrollment needs a short-lived, audience-bound issuer-signed assertion with
an opaque membership binding, nonce, expiry and revocation epoch. It grants
provider custody only, with no authority to read or write a Ratio book. The
broker must not receive a Ratio Connect access token. Authentication, assertion
issuance, replay protection and revocation propagation are implementation and
deployment gates, not existing endpoints established by this decision.

## Preserve retry and revocation semantics

The existing `connect/personal_activation.py` runner verifies the WorkOS
subject, selected book and Personal kind before opening authenticated
AES-256-GCM custody. It combines provider access and Ratio processing locally.
Keep that supported path unchanged. Its vault currently includes Google seen
identifiers and Plaid pending state; the split explicitly moves the durable
import index and mapping decisions to the device, rather than copying this
combined runner onto a server with Ratio credentials.

The broker must durably store a complete encrypted provider batch before
committing its next cursor. It delivers batches at least once using opaque
stable delivery identifiers. The device acknowledges only after preserving
original bytes and processing state durably; a timeout or lost acknowledgment
replays the same batch. Device deduplication must reconcile any already-posted
outcome before attempting an append. Do not promise exactly-once network
delivery or treat a correlation identifier as an idempotency key.

Plaid pending transactions remain nonposting. Pending removal and posted
addition can occur on different pages; collect a complete successful sync cut
before advancing the cursor, preserve pending-to-posted links and stable
transaction identifiers, and restart from the prior cursor after pagination
mutation. The current adapter conservatively refuses any modified/removed batch with
its cursor unchanged; the existing pending-to-posted regression covers a posted
addition linked to its pending identifier. Preserve that refusal until a
separate correction policy is proved; this decision does not claim that all
real-world removal sequences already import successfully. Unmatched or changed
previously imported transactions still refuse rather than invent corrections. See the [provider transaction lifecycle](https://plaid.com/docs/transactions/transactions-data/)
and [sync contract](https://plaid.com/docs/api/products/transactions/).

A Google HTTP 410 resets the provider sync token and performs a full resource
sync, as required by [Google's incremental sync protocol](https://developers.google.com/workspace/calendar/api/guides/sync).
It must preserve the device's imported event/etag index and journal history.
Commit the replacement sync token only after all pages succeed. Exact retries
remain nonposting; changed/deleted imported events still require explicit
resolution. Only the existing typed bill metadata can propose amounts; titles
and descriptions do not decide figures.

Each operation checks current membership eligibility before decrypting or
releasing a delivery. Revocation disables new delivery immediately and fences
in-flight responses using the membership epoch. Provider token revocation is
retried on failure; encrypted retry state is retained until provider success,
then removed. Separate memberships never share tokens, cursors, outboxes or
acknowledgments. This specifies the future split's requirements, not a claim
that the current runner enforces a distributed revocation epoch.

## Recovery, deployment and cost gates

Recover the device's book, original deliveries, import index and pending state
together. Separately back up broker token ciphertext, wrapping-key references,
provider cursors, outbox and revocation state with restricted key custody.
Restore must reconcile undelivered batches and revoked memberships before
resuming. Never place broker credentials in a book export. The accepted
synthetic S3 recovery drill included synthetic Connect custody, not this future
service or real provider-token resumption. Set and prove broker RPO, RTO,
retention, operator and verifier before deployment.

The on-device tier still pays for server authentication, secret custody,
provider refresh/polling and quotas, encrypted outbox storage, backup, monitoring
and revocation operations. No zero-infrastructure or unmeasured price claim is
made. Choose the hosting/key-management service, deploy assertion verification,
implement the split protocol with protobuf DTOs, and prove crash windows,
cross-membership denial, revocation races, real provider pagination and restore
before calling the broker operational. No broker service is deployed here.

## Verification and scope

This is a decision/documentation slice; it changes no arithmetic, provider
code or authorization scopes. Existing Bazel regressions cover authenticated
per-membership ciphertext and tamper refusal, membership checks, Plaid restart
and pending-to-posted behavior, Google refresh/410 with retained seen history,
failed sync/revocation retry state and successful disconnect. They are
simulated provider tests, not live Plaid/Google or future broker evidence.

The envelope refusal in #164 remains in force. The Personal cash forecast
already delivered in #218 is unchanged. Neither scope is reopened. The kernel
continues to decide every amount; the broker supplies upstream facts only.

Verification on October 24: all eight Bazel targets passed: Personal activation,
bank-feed mapper/Plaid/Link, Calendar bills/Google, PLAN refusal inventory and
issue wording. Standalone site/font verification also passed. No provider
secrets or live provider calls were used.
