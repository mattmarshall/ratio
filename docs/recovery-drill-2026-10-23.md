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
