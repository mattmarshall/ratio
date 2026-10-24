# Recovery readiness — October 22, 2026

Related: #264. This preserves the historical readiness audit, not recovery acceptance.
The later owner-approved [synthetic AWS drill](recovery-drill-2026-10-23.md)
supersedes pending lab objectives and verifier assignment below. Recovery
PRs #375/#376 and shell PRs #4/#5 have merged; dated initial states remain
for provenance.

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
`ratio` and `ratio-ro` AWS profiles initially failed STS identity checks because
their SSO tokens expired. After the owner renewed SSO, the `ratio` profile
successfully identified the intended account and read the live deployment.
Stack `ratio-demo-app` in `us-east-1` reports `UPDATE_COMPLETE`. Its journal
bucket has AES-256 encryption and all four public-access blocks enabled.
GetBucketVersioning returned no enabled versioning state;
GetObjectLockConfiguration returned `ObjectLockConfigurationNotFoundError`.
The only expiration lifecycle rules cover `runs/progress-` and `runs/await-`
at 30 days, plus a one-day incomplete-multipart-upload abort rule. They do
not establish journal backup retention. The known synthetic
`personal-tax-walkthrough` book prefix contains five objects totaling 1,524
bytes. No production object body was read, no production write occurred, and
no original-delivery or Connect-state recovery was established by these reads.

Prepare a disposable representative book after the owner approves the rehearsal contract. A proposed
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
| AWS identity and source | Identity, deployment, protections, and known synthetic namespace inspected read-only; drill source still to be created | Access verified; representative source pending |
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

## iOS coordination — latest verified state

The owner selected private visibility and explicitly authorized a linked
kernel build PR followed by an exact pin update in existing shell PR #4.
Kernel [PR #381](https://github.com/mattmarshall/ratio/pull/381) merged as
`7b81c1d6f5648ba902d69f30401bf18eedcaf6e4`. It adds the missing device and
simulator dependency platforms and refreshes the Bazel lock without changing
Cargo.lock or accounting code. This supersedes the earlier target-analysis
failure at the old `2a20e7c1b6d288da8930becef4d0cb38997c7363` pin.

Shell [PR #4](https://github.com/mattmarshall/ratio-ios/pull/4) now pins that
exact merged revision. [CI 39845836493](https://github.com/mattmarshall/ratio-ios/actions/runs/39845836493)
passed device/simulator XCFramework builds, an APFS app-sandbox no-replace
contention/reopen check, and replay of the simulator-written journal using
the actual pinned CLI. Both reproduced prefix 2, NAV 10000 minor units, and
digest `5e0d32d7d553a694752e4018502b59f17d2a7226345daf59f3a7ade087319c3b`.
A memo-only mutation reproduced the arithmetic but refused rewritten history.

Shell [PR #5](https://github.com/mattmarshall/ratio-ios/pull/5), stacked on #4,
adds the protobuf C ABI and generated typed Swift client for all 49 current
Console RPCs. [CI 39850325396](https://github.com/mattmarshall/ratio-ios/actions/runs/39850325396)
passed the ABI/RPC-contract/mirror tests, both archive builds, APFS check,
pinned CLI replay, complete Swift regeneration, and simulator Swift response
and refusal checks. Swift retained the exact authenticated-creator refusal
and the kernel's non-dividing average-cost break. No shell accounting code
or fabricated authenticated identity was introduced. On-device provisioning
remains unresolved.

Neither shell PR is merged. Simulator execution does not prove physical-device
execution; Apple Development signing access is unavailable. Source-license
and contribution decisions remain pending on
[ratio-ios #3](https://github.com/mattmarshall/ratio-ios/issues/3).
The repository remains private. TestFlight and outside contributions remain
gated by #3. None of this native evidence proves live S3 recovery or provider
conformance.

## Pending owner actions

After explicit owner approval, recovery PR #375 merged into main as
`e1e9f6a792602b12f0e367769b62ef734ab98e9a`. Its updated head passed
[build, tests, and live Postgres](https://github.com/mattmarshall/ratio/actions/runs/39891140110),
[site](https://github.com/mattmarshall/ratio/actions/runs/39891140246), and
closure checks. The earlier automatic approval rejection was resolved by that
explicit approval. Issues #264 and #300 remain open. Operational-evidence
PR #376 now targets main and incorporates the merged base; it remains
unmerged. The table above preserves the initial audit, not current PR states.

A proposed synthetic rehearsal uses two disposable books, disjoint source and
restore namespaces, a separate encrypted backup bucket, no production writer
interruption, RPO 0 at the quiescent cut, RTO 60 minutes, and seven-day lab
evidence retention. These are proposed lab objectives, not an agreed customer
recovery contract. Approval and a named independent verifier remain pending;
no resources or retention policies were created from this proposal. Original
deliveries and Connect state still require the inventory and custody evidence
listed above. The Plaid/Google credential broker owner decision remains open.
