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
