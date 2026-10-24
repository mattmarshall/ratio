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
is made. Completed provider results follow below.

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
## Height and append run contract (#358)

The separate existing #358 worktree measures the actual production S3 height
method against the default full-list height at 5,396 and 65,536 entries.
Both datasets are complete, contiguous synthetic sequence journals in a fresh
`conformance-lab/<run>/358/` namespace. A bounded 32-worker fixture upload is
outside measurements. Three paired height samples and real conditional
appends record both curves; a regression on the smaller shape must be reported,
not hidden by the asymptotic bound. Occupied claims must refuse without
changing acknowledged bytes, and a separate missing-slot log must refuse
replay. This does not fold a figure or mutate the running #357 fixture.

### Initial live height result and correction

Source `8404c1a285741a1925c28c4aa7c8aef40a572a01`, three paired samples:

| Seed height | Full-list height median | One-key bounded height median | Full-list append | Bounded append |
|---|---:|---:|---:|---:|
| 5,396 | 823 ms | 1,419 ms | 1,207 ms | 1,456 ms |
| 65,536 | 10,111 ms | 1,727 ms | 9,194 ms | 1,848 ms |

The bounded-only implementation regresses the deployed smaller shape. Retain
this result; it is not a failed claim/conservation test. The initial measurement
also passed actual conditional overwrite refusal, unchanged acknowledged bytes,
and missing-first-slot replay refusal. Appends compare adjacent heights (N and
N+1), not two simultaneous writes against the same head.

The correction streams one 1,000-key page at a time for smaller logs and uses
a bounded search above a 32,768-entry threshold, selected between the measured
shapes rather than claimed optimal. Small logs never accumulate every key in
one vector; large logs avoid reading every page. The claim remains conditional.
A separate `adaptive` attempt remeasures the same retained fixtures after the
two acknowledged initial append probes (N+2), preserving the original record.
Shared-host/provider timings are observations, not a capacity guarantee.

### Completed adaptive height result (#358)

Source `8d1241d12b09efca057c7c346984b0ebb23881fd`, three paired samples on
the retained fixtures after the two initial appends:

| Actual height | Full-list height median | Adaptive height median | Full-list append | Adaptive append |
|---|---:|---:|---:|---:|
| 5,398 | 999 ms | 983 ms | 937 ms | 1,047 ms |
| 65,538 | 10,075 ms | 1,179 ms | 12,486 ms | 1,532 ms |

The smaller height curve is now near parity, while its single append sample
still includes roughly one extra probe's overhead. Do not claim a universal
latency win. The larger curve avoids materializing the entire listing and
substantially reduces observed latency. Actual occupied conditional claims,
unchanged acknowledged bytes, and missing-slot refusal passed again. Both
local storage/CLI suites passed after the correction; the known-lower-bound
search covers threshold, large and u64-boundary cases.

Both read-back-verified AES256/seven-day GOVERNANCE archives remain retained:

- Initial: `evidence/provider-358-initial.zip`, SHA-256
  `68c1796dfe0897924da09f9908e90075a97b7962e04c876a1c82318209f8b96e`.
- Corrected: `evidence/provider-358-adaptive.zip`, SHA-256
  `c64d24644bbd7c7599b1042d5ddfa8290cf5094b7dd1a9d586d9ccf520d78d30`.

These include seed, initial and corrected protobuf records and redacted logs.
The S3 LIST/conditional-PUT model is unchanged; positive checks passed and the
unconditional-write probe violated `NoWriteIsLost` for its named reason.
## Registered fixture provider companion (#359)

The mandatory cheap gate retains local JSONL, MemoryStore, DirStore, an
emulated S3 endpoint, named conditional/no-hole probes, checkpoint telemetry
privacy, and the ObjectStore source registry. Its manual provider companion
executes the identical reusable fixture against real S3 with endpoint overrides
refused. Exact journal bytes, all four prefixes (including empty), six side
planes, and trial balance must match. Each side plane receives an explicit
SHA-256 citation. Real occupied-claim and missing-slot checks run on all seven
sequence planes. No emulated result is presented as live evidence.

The registry explicitly classifies the manual counting and baseline-height
wrappers as delegating test doubles. A new unclassified ObjectStore still
fails the build. Completed provider and mutation results follow below.

### Completed registered fixture provider evidence (#359)

At source `9fc28b69c8973f96410c5fd5ac98309dac0a55b7`, the actual S3 run
passed in 9.69 seconds: exact journal bytes, four journal prefixes including
empty, six side-plane record sets, and trial balance matched the same local
JSONL fixture used by the mandatory gate. Every prefix was also checked
against `ratio_nav::prefix_digest`. Each side plane's SHA-256 is retained in
the protobuf. Actual occupied-claim refusal, unchanged acknowledged bytes,
ordered replay, and missing-slot refusal passed on all seven sequence planes.

Independent raw S3 readback reproduced the canonical local JSONL bytes and
preserved each prefix pin:

| Prefix | SHA-256 |
|---|---|
| 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| 1 | `3b9ca02ec032f16793d7ebd80671cf4f0dce6be4a7aa50026daffcfa3b928f05` |
| 2 | `157be2c623cdc738978cd13e7a39347bc4665d22f26b4e3be0950072bbb8528f` |
| 3 | `923bede0abad250c1ea53a8b12f2fe8fa7fbc17abf77de476b535545bbe04502` |

All seven mandatory gate targets passed. The initial registry run correctly
refused four previously merged test doubles; they were explicitly classified,
without weakening the new-backend refusal. A compilable temporary crate then
proved both protections: omitting its inventory target refused analysis, and
adding the inventory without conformance registration refused specifically for
`UnregisteredStore`. The temporary crate was removed and the full gate passed
again. Dynamic Bazel `subpackages()` inventory is retained; no additional
build aspect is needed. The earlier smaller source mutation was also refused
for its named unclassified-backend reason.

AES256/seven-day GOVERNANCE archives, each uploaded and read back with matching
SHA-256:

- `evidence/provider-359-final.zip`:
  `3d3399d2764997765b9ebf4fa1d0718a4b78c95fc09ef1a5e4344cba61b79b1e`.
- `evidence/provider-359-prefixes.zip`:
  `4518beafc2b9edabdd9a20d435075e19d3ba79e468be97d594a3cb61c5f660e5`.
- `evidence/provider-359-new-crate.zip`:
  `6aa5dabe64033ea1e79beec1376fa5ff9eea291ac55a468a30eef7792d846969`.

Actual provider results remain distinct from the in-process S3 endpoint.
Operational-evidence retention/selection is separately covered by accepted
#264/#300 recovery; mixed live/local book routing remains #360.
