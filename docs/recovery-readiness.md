# Recovery readiness — October 22, 2026

Related: #264. This is a readiness audit, not recovery acceptance.

## Verified starting point

GitHub `main` and fetched `origin/main` both name
`9cc92b9caa1a2a15c1768ccc9123e7742e87f156`. The working directory initially
contained the clean `codex/object-store-height` branch at `59f0074`, not main.
The following implementation PRs are still open; none is merged:

| Issue | Existing PR | State at audit |
|---|---|---|
| #264 | [#375](https://github.com/mattmarshall/ratio/pull/375) | Draft; recovery commands and disposable directory evidence |
| #300 | [#376](https://github.com/mattmarshall/ratio/pull/376) | Draft; operational evidence and configured writers |
| #357 | [#377](https://github.com/mattmarshall/ratio/pull/377) | Draft; object checkpoints |
| #358 | [#378](https://github.com/mattmarshall/ratio/pull/378) | Draft; append height |
| #359 | [#379](https://github.com/mattmarshall/ratio/pull/379) | Draft; conformance with simulated S3 |
| #360 | [#374](https://github.com/mattmarshall/ratio/pull/374) | Open; explicit per-book storage |
| #361 | [#380](https://github.com/mattmarshall/ratio/pull/380) | Open; based on the existing per-book storage branch |

The existing `codex/book-scoped-object-store` branch contains #360 work.
Reuse these slices; do not recreate them from their issue descriptions.
Local and simulated-provider results in their PRs remain separate from live
S3 evidence. The user requires recovery and live conformance to pass before
#362 starts, followed by #363–#366 in order.

## Operational inputs

The owner has not designated a recovery book or agreed objectives. Existing
`ratio` and `ratio-ro` AWS profiles both failed STS identity checks because
their SSO tokens expired. An SSO sign-in was initiated; no AWS read or write
has succeeded during this audit. Deployment source names the demo account,
`us-east-1`, stack `ratio-demo-app`, and its ScaleBucket. These are discovery
coordinates, not evidence of their live settings or authorization to damage
the deployed book.

Prepare a disposable representative book after access is restored. A proposed
rehearsal should include two book IDs, all journal and side planes, two config
promotions, grants and revocations, NAV strikes, report ordering, reviewed
proposals, audit events, a close, and an accepted explanation. Retain original
delivery bytes alongside their recorded digest and byte count. Inventory
Connect registration, per-membership credential custody, retry cursors,
pending-to-posted state, Google sync/index state, encryption-key custody, and
revocation state separately. Use synthetic provider credentials for the
representative rehearsal; that cannot prove recovery of live provider state.

| Required input | Next concrete action | Acceptance state |
|---|---|---|
| Book and known figures | Select an owner-approved representative book and independently recorded expected figures | Pending |
| AWS identity and source | Complete SSO, verify account/role, inspect deployment and source prefixes read-only | Pending |
| Independent backup | Select a separately controlled backup location; inspect encryption, versioning, access, and retention | Pending |
| Isolated restore | Select an empty namespace and serving root with outbound Connect writes disabled | Pending |
| Writer quiescence | Enumerate HTTP, CLI, MCP, ingest, membership, and config writers; record how each stops and drains | Pending |
| RPO / RTO | Agree acknowledged-write loss and verified-service recovery objectives before timing acceptance | Not agreed |
| Retention / cadence | Agree backup frequency, evidence retention, copies, and separation; change no production retention by inference | Not agreed |
| Operator / verifier | Assign executor, alternate, and independent figure/access verifier | Not assigned |

## Evidence to preserve

Store restricted evidence outside public issues and CI logs. Record exact
kernel/image revisions, UTC start/end timestamps per phase, namespace
identities, object/file counts and bytes, the protobuf capture manifest and
its hash, journal and side-plane lengths/digests, all config hashes and control
predecessors, expected figures, NAV replay results, report selection, and
membership read/write outcomes for both books. Preserve failures as well as
successful runs. A fresh process must reproduce the same citations after
source loss. Test corruption and missing planes only on disposable copies.

Live-provider acceptance additionally needs checkpoint publication followed
by cold load and tail replay, privacy-safe telemetry, full prefix-digest
equivalence, conditional-claim contention, hole refusal, and a two-book
backend-isolation check. Compare listing-based and height-based append at the
same nontrivial height, recording request counts and latency separately.
Do not infer live S3 behavior from the in-process S3 endpoint.

No clean live backup/failure/restore/replay drill, measured RPO/RTO result,
independent verification, or provider-conformance result exists from this
audit. Issues #264 and #300 remain open. No production storage, retention,
membership, repository visibility, or provider-token custody was changed.

The readiness documentation passed the site build with vendored-font checks,
site verification, shared console-token checks, and `git diff --check`.
These are documentation checks, not recovery or provider evidence.

Current-main Bazel invocation `a1626869-2497-43c2-8dbc-a4f5f51394dd`
passed `//proto:mirrors_test` and `//:plan_refusals_test`. The accompanying
`//crates/ratio-console:recovery_test` build was interrupted when shared APFS
free space fell to 2.3 GB. That test has no result from this audit. Do not
carry the two passing source checks forward as a passing recovery suite.

## iOS coordination

The owner selected private visibility. Source-license and contribution
decisions remain pending on
[ratio-ios #3](https://github.com/mattmarshall/ratio-ios/issues/3).
Existing [PR #4](https://github.com/mattmarshall/ratio-ios/pull/4) pins kernel
`2a20e7c1b6d288da8930becef4d0cb38997c7363`. Its recorded blocker is the
kernel's crate-universe platform list omitting iOS device and simulator.
A device build was run again during this audit (invocation
`deb452f2-184b-4cda-9857-a55688075174`). It failed target analysis after
362.132 seconds: `@crates//:anyhow` was incompatible with `//:ios_device`.
The pinned kernel's crate-universe default platform list omits both iOS
triples. No archive or xcframework was produced. The C ABI issue #2 depends
on the pinned build. Keep all figure behavior in Ratio; do not patch a kernel
copy in the shell. TestFlight and outside contributions remain gated by #3.
