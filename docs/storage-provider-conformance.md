# Live provider conformance

Recovery prerequisite: Matt Marshall accepted the documented representative
AWS drill; #264/#300 are complete for that scope. Main `a90889d` passed
application deployment and smoke. This work reuses PR #377 (#357), then #378
(#358), #379 (#359), and #374 (#360), each on its existing branch.

## Checkpoint run contract

The manual `//crates/ratio:checkpoint_live_test` uses the production S3 adapter
without an endpoint override. Source is committed before each operator run.
Only a new `conformance-lab/` prefix in the approved disposable lab bucket is
allowed; the retained recovery snapshot and production books are untouched.

Separate processes seed the deployed 20-security/40-lot/3-currency geometry,
measure a checkpoint miss/full replay, load the published checkpoint, and
append/replay one capital tail. The in-memory projection and NAV prefix digest
are compared to the same generated local fixture; no second accounting engine
is introduced. Every declared-view trial balance must match. Checkpoints must
leave the authoritative book tree byte-identical. A bounded 16-worker fixture
upload is outside fold timings.

Corrupt-content, truncated, version-refused, and mismatched-pin checkpoint cases
use a small local journal with checkpoint publication/retrieval against live
S3; their results are not presented as large-journal measurements. Actual
telemetry lines must match only hit/miss/reason and integer lengths. Protobuf
records preserve source SHA, timings, assertion results, adapter-call counts,
and limitations. Counts are not paginated HTTP-request counts. Serial prefix
verification still reads journal bodies on a checkpoint hit; no one-GET claim
is made. Provider results are pending execution.

## Completed live checkpoint evidence (#357)

Source `cbd805c8db1e2c990ff1c67be6205544eabf473f`; production S3 in
`us-east-1`, operator host Apple M4 Pro / 24 GiB / macOS arm64. These are
single-run shared-host observations, including other build/upload activity,
not Lambda or API Gateway cold-start guarantees. Baked configuration remains
local as in the deployed seed image; journal bodies and checkpoints are S3.

| Separate process | Fold time | S3 adapter reads | Telemetry |
|---|---:|---:|---|
| Missing checkpoint / full replay | 792,031 ms | 10,792 | miss=missing, height=5,396, tail=5,396 |
| Published checkpoint / empty tail | 355,170 ms | 5,400 | hit=1, prefix=5,396, tail=0 |
| Published checkpoint / one new entry | 352,426 ms | 5,400 | hit=1, prefix=5,396, height=5,397, tail=1 |

Each process compared all declared-view balances to a full local fold and its
checkpoint pin to `ratio_nav::prefix_digest`. The original prefix is
`70e476310d24354c03de924285f7df0fe5e3c1ad40fb44d6ca51748b60ae9138`.
Each book-tree byte fingerprint stayed unchanged by checkpoint use. All actual
telemetry lines passed the strict metric-only pattern. The small eight-entry
negative fixture used live S3 checkpoints and reported the precise
`version_refused`, `corrupt`, `digest_mismatch`, and `prefix_mismatch` reasons;
each fallback reproduced every view. Directory/emulated evidence is separate.

Provider archive: backup `evidence/provider-357-final.zip`, SHA-256
`2e147f6f33b0a53b4375d0691c3b6f3ac51e393c2eb61e40d8c2188384916f38`;
upload/readback matched with AES256 and seven-day GOVERNANCE retention. It
includes every phase's protobuf/log and the named `NoWriteIsLost` TLA probe.
Five local checkpoint/protobuf/planning suites, both positive TLA checks and
the expected-red unconditional-write probe passed their intended checks.
The checkpoint reduces folding and body passes; it still reads the prefix.
The measured minutes remain well beyond the gateway's 30-second cap.
