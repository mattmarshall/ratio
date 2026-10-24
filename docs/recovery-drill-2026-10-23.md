# Approved synthetic AWS recovery drill — October 23, 2026

Related: #264 and #300. This is an approved synthetic rehearsal, not a customer
recovery guarantee. The owner approved the proposed AWS drill and designated
Matt Marshall as verifier. Codex executes under the approved `ratio` AWS session;
Matt Marshall is the human alternate operator and independent result reviewer.

## Contract established before execution

- RPO: zero acknowledged writes lost at the quiescent cut.
- RTO: 60 minutes from deliberate source-loss start through fresh-process
  figure, digest, evidence, and membership verification.
- Evidence: seven-day lab retention. The separate backup bucket has AES-256
  encryption, versioning, seven-day GOVERNANCE Object Lock, public-access
  blocking, and a policy denying manual deletion and retention bypass.
- Scope: two synthetic books; no production writers are interrupted. Source
  and restore use disjoint prefixes in a new lab bucket. Backup uses a separate
  new bucket in the same AWS account and `us-east-1`; this does not prove
  cross-account or cross-region disaster recovery.
- Cadence: this one approved rehearsal. No customer backup schedule or
  production retention setting is implied.

Run `20261023-f2fdf6e8` uses:

| Purpose | Location |
|---|---|
| Source | `s3://ratio-recovery-lab-320473299741-20261023-f2fdf6e8/recovery-lab/20261023-f2fdf6e8/source/` |
| Backup | `s3://ratio-recovery-backup-320473299741-20261023-f2fdf6e8/snapshot/` |
| Restore | `s3://ratio-recovery-lab-320473299741-20261023-f2fdf6e8/recovery-lab/20261023-f2fdf6e8/restore/` |
| Evidence | Separate `evidence/` prefix in the backup bucket; protobuf records and redacted operator/AWS logs |

`deploy/recovery-lab.yaml` provisions only these synthetic resources. It retains
the buckets on stack deletion; lifecycle cleanup follows the agreed lab
retention. The retained backup must not be edited for negative tests.

## Independently stated fixture expectations

Alpha is an Investment book owned by synthetic subject `alice`: capital
50,000 plus 10,000 minor units, acknowledged journal prefix 2, expected NAV
60,000. Two configuration promotions produce three configuration versions.
Beta is a Personal book owned by `bob`: one capital entry of 7,000 minor units,
prefix 1, expected NAV 7,000. These constants precede execution and are checked
against the existing kernel's recorded/replayed figures, not computed by a
second accounting implementation.

Alpha includes every side plane, a future legacy action, original CSV delivery
bytes and their delivery digest/size, two accepted-explanation fixture records,
two close records, signed NAV evidence, ordered report bytes, a reviewed draft,
and audit lines. Control history grants and then revokes `guest`. Restore must
preserve both grant and revocation, distinguish book kinds, retain report
selection, refuse a backdated post, and claim the next journal slot for a valid
next-period post without changing beta.

The separate `external/connect/` protobuf inventory records synthetic app
registration, book/subject binding, credential reference, retry cursor,
pending-to-posted mapping, Google sync/index state, revocation state, and
disabled outbound writes. No actual Plaid or Google token is present. Byte
custody of these fixtures is tested; live provider resumption and real token/key
custody remain separate decisions and evidence requirements.

## Quiescence and failure procedure

The only writer to the random lab source namespace is the foreground seed
process. It must exit before capture. No HTTP listener, stdio MCP service,
ingest worker, membership/config worker, or Connect consumer is started for the
lab. All mutations are synchronous and acknowledged before the cut. The lab
does not change a deployed writer's environment or pause production traffic.

An incomplete S3 multipart upload targets the next journal slot but is never
completed; it is not an acknowledged entry or a visible object. This probes an
unpublished upload, not a post-acknowledgment network timeout. The backup copies
only published objects, publishes its manifest last, reads bytes back, and
retains exact SHA-256/length inventory. A separately named damaged lab copy
changes a journal memo without changing postings; restore must refuse it before
writing any destination objects. The retained backup remains untouched.

Source failure removes only this lab source prefix and its local serving root,
then aborts the unpublished upload. A restore starts from an empty disjoint
namespace and an absent serving root. A new process compares every captured
object, both figures/prefix digests, configuration history, all cited evidence,
original delivery bytes, and synthetic Connect custody. Membership probes use
the existing application boundary for allowed/denied reads and denied writes;
synthetic gateway claims do not prove JWT signature verification.

## Reproduction and acceptance

Build with Bazel only:

```sh
bazel build //crates/ratio:ratio //crates/ratio:recovery_live_test --jobs=2
```

`deploy/recovery-drill.py` uses short-lived credentials only in child process
environments. Generate the Python representation of the same recovery proto
with the Bazel-provisioned protoc; then pass its generated module directory and
the operator's explicit lab environment file. The script refuses an existing
run directory, non-lab names, a dirty source tree, or occupied namespaces. Do not
restart it to hide a failed attempt. Preserve failed phase protobuf/logs and
inspect authoritative state before an explicitly planned continuation.

Measured results and failures will be recorded here after execution. Verifier
acceptance remains pending; automated checks cannot accept evidence for Matt
Marshall. Issues #264 and #300 remain open until their full criteria are proved.


## Preserved first-attempt result and continuation plan

The live fixture and clean capture passed: 33 objects, 50,749 bytes; alpha
NAV 60,000 at prefix 2 with three configurations; beta NAV 7,000 at prefix 1.
All 23 negative checks passed (balanced memo corruption and missing/corrupt
bytes for each of 11 protected planes). Source/root loss and clean S3 restore
passed. Fresh-process verification matched the exact object inventory, figures,
digests, and configuration histories, then stopped because the fixture expected
two audit lines. CreateBook correctly adds an attributed creation line before
the two explicit review lines, so three is the required count.

This is a fixture assertion failure, not evidence of lost or extra restored
bytes: the complete inventory comparison passed first. The original failure
record and logs are retained in `evidence/attempt-1.zip` in the backup bucket,
SHA-256 `42880ebcc0a1f3799832ffbae9e9b1084e2dae28acde209dbe54e7b1f3b0ebc8`.
The third line was inspected and names `alice`, `created`, and `alpha`.

The continuation corrects the expected count and asserts all three lines'
ordering/attribution. It will use another absent serving root against the
already restored namespace; the original backup, failure evidence, and prior
serving root remain intact. RTO continues from the original source-loss start,
including diagnosis and repair. It is not reset to hide the failed assertion.
No final verification or verifier acceptance is claimed by this amendment.

## Measured continuation result

The explicit continuation passed against another absent serving root. It
verified all 33 captured objects byte-for-byte, both recorded/replayed figures,
all configuration histories, close/explanation metadata, report selection,
reviewed draft, all three audit lines, original delivery bytes, and synthetic
Connect custody. Reads and attempted writes by another book's member, revoked
`guest`, and an outsider refused. A backdated post still refused; a next-period
post claimed alpha's next slot and left beta unchanged.

| Measurement | Result |
|---|---|
| Clean capture | 10.242 seconds; 33 objects; 50,749 bytes |
| Deliberate source/root loss | 3.156 seconds |
| Isolated S3 restore | 8.829 seconds |
| Initial verification | Stopped after 9.919 seconds on the documented wrong audit-count assertion |
| Explicit fresh-root continuation | 55.718 seconds; passed |
| RPO | Zero acknowledged captured objects lost; alpha prefix 2 and beta prefix 1 reproduced |
| RTO | **275.748 seconds (4 minutes 35.748 seconds)**, including the original failed verification, diagnosis, correction, rebuild, and continuation; below the agreed 60-minute lab target |

Alpha's restored strike is NAV **60,000 minor units**, prefix **2**, digest
`99f50131b5f23c1c52862dbb9013e26bc935c3a9044be3889eb902c7eb1349de`.
Beta's is NAV **7,000 minor units**, prefix **1**, digest
`ed32ca850fb5c1c13a2f188d66cab3b3123b81eb49ab9a80f281c6eca389afdb`.
The subsequent next-period alpha append is an additional readiness probe, not
a change to either pinned strike's captured prefix.

`evidence/attempt-2.zip` in the retained backup bucket includes the original
failure and continuation protobufs/logs. Its readback reproduced SHA-256
`df674bd4af0e422bd6a41034fc61a09fd586b470a563215b4c180df7facbd029`;
readback also confirmed AES-256 encryption and GOVERNANCE Object Lock.
Initial helper source is `ac101830b01f05e39d8feac4f0c3a42bfa7843c0`;
the inspected audit correction/continuation is
`a730bf91f8bebc146426c7cbe4b798352b0c2720`.

Matt Marshall's independent acceptance is **pending**. This result measures
fresh-process Console read/write readiness, not deployment of a public restore
endpoint, customer guarantees, provider OAuth resumption, or cross-account
disaster recovery. Supplemental real CLI, HTTP, and stdio MCP writer probes run
on another snapshot copy under `writers/`, outside the measured recovery clock.

### Configured HTTP writer failure

The supplemental writer run used a separate restored snapshot under `writers/`.
CLI posting and striking and stdio MCP posting and draft creation succeeded.
The HTTP server then panicked before binding: the synchronous S3 adapter called
`Runtime::block_on` from inside the HTTP server's Tokio runtime. This is a real
execution-context defect, not an inventory mismatch. No HTTP post was attempted.
The failed attempt remains in `evidence/writer-attempt-1.zip`, SHA-256
`e8a5883e30b7e4bcf8331f30df92743cbbfb39094a8a02c13b2bcde83557ff57`, with
AES256 encryption and seven-day governance retention.

The adapter now leaves an existing runtime before driving its own single
runtime and shuts it down without blocking an async handler's drop. Regression
checks cover ordinary synchronous execution and both Tokio runtime flavors.
An explicit continuation resumes at HTTP without repeating successful posts.
This supplemental failure does not alter the original recovery cut or measured
RTO; #300 remains incomplete until the continuation and verifier review pass.

After the runtime fix, the HTTP post and unbalanced-post refusal passed. The
next harness assertion incorrectly demanded `NoSuchBucket` from `init`, which
instead correctly refuses unsupported durable book publication before local
fallback. Attempt 2 is retained at `evidence/writer-attempt-2.zip`, SHA-256
`32495fb96087c716a69e3d5a585ef13f8fc8239a8253bac46c8a8a642acf691f`.
The continuation accepts that precise init refusal, still requires the live AWS
failure for the existing-book writers, and does not repeat the HTTP post.

The same publication-marker refusal protects existing published books too:
`FileBook::open_with_seed` refuses a missing durable publication before touching
local bytes. Attempt 3 records the overly narrow diagnostic assertion at `post`:
`evidence/writer-attempt-3.zip`, SHA-256
`e95ac30e26951bda82567414b1c26565a746cdd5a4ccc57121e4aba2324f45d4`.
The harness now accepts either that exact fail-closed diagnostic or the explicit
AWS `NoSuchBucket` error, always requiring failure and unchanged local bytes.

### Final supplemental writer results

The explicit continuation passed all unavailable-backend refusals for `init`,
`post`, `strike`, `close`, `approve`, `accept`, and stdio MCP, with unchanged
local book bytes. After deleting the supplemental serving root, new CLI and
MCP processes cold-loaded the books successfully. Live S3 readback reproduced
alpha's exact three journal IDs (including the CLI post), beta's exact three
IDs (including MCP and HTTP posts), the CLI NAV strike of 62,500 at prefix 3,
and the unapproved MCP draft. The HTTP unbalanced post was refused.

Final writer source: `b612799711e5bbc2e4728d052825bf6f2c27c2e2`.
Evidence: backup `evidence/writer-probes-refusal-continuation.zip`, SHA-256
`4d4b7ac3f00f2dcaae2553b7a14683dee9c1b4ddc7033d7ccefe860f3ca1c67e`;
the uploaded archive was read back and matched. Earlier failed phases remain
in the protobuf record and immutable attempt archives. The four affected Bazel
suites (CLI, API, configured-store refusal, PLAN) passed, in addition to the
three runtime-context regression cases.

**Independent verifier acceptance remains pending.** Matt Marshall must review
the figures, RTO, inventories, failures, retained evidence, and stated scope
limitations before either recovery issue can be treated as complete. Live
Plaid/Google resumption, customer deployment objectives, physical iOS signing,
and provider conformance are not proved by these results.
