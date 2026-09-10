# Durable configuration and membership transitions

Related: [#303](https://github.com/mattmarshall/ratio/issues/303), implementation
[#304](https://github.com/mattmarshall/ratio/issues/304), parent
[#299](https://github.com/mattmarshall/ratio/issues/299), and durability parent
[#291](https://github.com/mattmarshall/ratio/issues/291).

This is the implemented contract for post-create control state. It extends
[atomic book publication](book-publication.md): the immutable bootstrap is
revision zero, including its exact opening configuration and explicit creator
grant. Later ACTIVE, promotion history, and membership come from one verified
immutable transition stream for that book. `ratio-store::control` implements
the protobuf boundary and fold over the same `ObjectStore` used by the server's
durable journal.

## One predecessor, one conditional publication

A control operation contains its book and bootstrap identity, expected revision
and predecessor digest, operation ID, verified actor, and either a configuration
promotion or one explicit membership grant/revocation. Configuration and
membership share the same per-book ordering. They cannot overwrite separate
whole-file snapshots from different predecessors.

1. Resolve the configured backend and read a complete verified control prefix.
   Authorize the stable actor against that fresh prefix. A missing object that
   the prefix references, malformed record, invalid version, invalid principal,
   unsupported action, gap, wrong book/bootstrap, or digest mismatch refuses.
2. Persist and verify exact configuration bytes at their SHA-256 address before
   preparing a promotion. Saving bytes alone is not a promotion. An unused
   staged blob grants no access and changes no ACTIVE pointer.
3. Look for the operation ID within this book's committed stream. The exact
   same operation bytes may return its original receipt. A different actor,
   predecessor, payload, or other identity field with that ID refuses.
4. For a new operation, compare both expected revision and expected predecessor
   digest with the verified head. A stale operation refuses. Choose exactly
   the expected next revision; never substitute a later head on its behalf.
5. Conditionally publish the complete immutable transition at that revision.
   Preparing/checking and publishing are separate operations. Two writers can
   pass the same check; only the conditional claim resolves that race. A lost
   claim requires reread/review. It is not permission to append old intent at
   the next available slot.
6. Acknowledge only after publication succeeds. A crash or lost response after
   commit leaves an uncertain client result and a recoverable durable record.
   A retry uses the original request bytes and operation ID. A reviewed new
   intent uses a new ID and explicitly names its new predecessor.

There is no global revision or lock. An operation on book B does not consume a
revision in book A or block A's unused slot. This is a safety and enabledness
claim; unlimited failures or an unfair scheduler can prevent progress.

## Protobuf boundary

`proto/ratio/storage/v1/control.proto` defines these messages. The following
field table is the persisted contract, not an alternate JSON schema. Published
version-one bytes must remain interpretable.

| Message | Required fields and validation |
|---|---|
| `ControlOperation` | Supported `format_version`; validated `book_id`; immutable `bootstrap_digest`; positive or zero `expected_revision`; `expected_predecessor_digest`; nonempty `operation_id`; stable verified `actor_subject` and stable actor provenance (trusted authority/source, never a bearer token); a `oneof` containing exactly one supported change. |
| `ConfigPromotion` | `config_digest`, the SHA-256 of exact saved configuration bytes. The rules must parse and check against the book's exact chart. This contains a reference, not a regenerated template or an implicit default. |
| `MembershipRevision` | Explicit `GRANT` or `REVOKE` enum, with zero/unknown refused; a typed `oneof` of exact AuthKit subject or explicit organization ID. Empty principals refuse. Organization metadata on a book is not a grant. |
| `ControlTransition` | Supported `format_version`; `revision`; complete `ControlOperation`; `operation_digest` binding its exact canonical protobuf bytes. The full transition's content address binds the successor's expected predecessor. The record's book and revision must match its storage key. |
| `ControlReceipt` | Book/bootstrap identity, operation ID/digest, committed revision, and transition digest. Exact retries return that original result rather than a receipt for current state. |

At revision one, `expected_predecessor_digest` equals the immutable bootstrap
blob digest in #298's `BookPublication.bootstrap_digest`; it is not an empty
sentinel or the hash of a different wrapper. At later revisions it names the
exact previous committed transition. Bind and validate the same choice
everywhere. Unsupported protobuf versions, unknown changes,
missing required fields, noncanonical encodings, and digest mismatches refuse;
protobuf's scalar defaults do not establish a valid transition.

The server resolves actor identity/provenance from its trusted authorization
context; a request body cannot choose its attributed actor. Do not put changing
session-token bytes or retry-time timestamps into operation identity.

Operation IDs are unique within a book. The same ID on two independent books
is allowed. Digest equality must identify the same exact operation bytes,
including its actor and predecessor; an ID-only index is insufficient. If #304
adds an acceleration index, the verified transition stream remains authority.
A current membership check still applies to a retry. A revoked actor cannot
use an old receipt or old operation ID as authority for a new mutation; an
otherwise exact retry can refuse if its caller no longer has access.

The transition authorizes only the explicitly named membership principal.
Connect client/template grants remain separate under #260. The implementation
must preserve the conjunction of verified client identity, catalog scope,
BookKind/template permission, and current book membership. Connect does not
inherit an AuthKit creator's grant or an organization grant through this model.
Gateway authentication, administrative policy, and delegation are not modeled
by replacing principals with arbitrary strings.

## Derived state, recovery, and authorization

Fold transitions in ascending committed revision order from the bootstrap.
A promotion prepends its digest to **newest-first HISTORY**, and ACTIVE equals
HISTORY's first entry. Membership revisions leave that history intact;
promotions leave membership intact. All configuration references must resolve
to stored, hash-verified exact bytes. Derived files/caches are disposable views,
not additional authoritative mutable ACTIVE or MEMBERSHIP records.

A fresh process verifies the bootstrap and every transition in its observed
prefix before materializing that view. Server commands and trusted CLI
configuration/membership commands select the same backend. An unreadable
configured backend cannot become local mode. Baked seeds and preexisting local
files cannot substitute for missing durable state, truncate later HISTORY, or
reintroduce a revoked creator. Legacy-to-published migration remains a separate
writer-fencing operation; this stream does not auto-import legacy seeds.

Each public operation begins with fresh membership resolution. If its start
follows an acknowledged transition, it observes that revision or a successor.
The model's read captures the current committed prefix, including committed
transitions whose success response was lost. An implementation must establish
that visibility contract for its backend; a process-lifetime membership cache
or an eventually consistent head read does not meet it.

An already authorized request may finish within its bounded authorization
window after a concurrent revocation. It retains the configuration digest and
membership snapshot captured at its own start. This does not permit a second
request to reuse that authorization. Expiration, retry, and process restart
require a new check. The network console gives an authenticated operation a
25-second authorization window, inside API Gateway's 30-second integration
ceiling. It resolves current membership when a public book boundary is entered
and checks the deadline again immediately before `ApplyEvent` appends. Expiry
refuses and requires a new request; the window is never renewed on one
`Console`. A concurrent revocation may therefore allow the already-authorized
operation to finish before that deadline, but the next operation resolves the
committed revocation. The posting keeps the configuration digest captured
before its append even if a promotion commits concurrently.

Control operations also respect that window, and their expected predecessor
provides an additional fence: an intervening revocation or promotion takes the
slot they expected. A conflict cannot move their captured intent to a new
revision. There is no modeled atomic transaction combining authorization,
control publication, and an ordinary journal append. Stronger revocation of
already running postings needs a separate protocol and proof.

The durable store assumption is immutable, complete, byte-preserving conditional
publication and reads that honor the acknowledged-prefix visibility contract.
A process crash does not delete committed objects. Missing listed/referenced
objects must refuse. Detecting deletion or rollback of an entire otherwise
valid suffix without a surviving head/receipt/retention anchor is a separate
storage protection obligation; this model does not claim to detect information
that no longer exists anywhere. Backup copies, retention, original evidence,
NAV/report/proposal persistence, production recovery drills, and RPO/RTO remain
outside #303/#304.

## Bounded model and checks

[DurableControl.tla](../tla/DurableControl.tla) has two writers, two books, at
most two transitions per book, and two logical clock ticks. Five fixed
workloads keep the check bounded while exploring scheduling, crash/retry,
and reader interleavings:

- Different configuration promotions racing at one predecessor, or starting
  sequentially so both promotions commit.
- Independent books using the same operation ID without a global conflict.
- A configuration promotion and explicit creator revocation.
- A guest grant followed by revocation, including the reverse/racing schedules.
- Two different payloads reusing an operation ID on the same book.

Recovery and ordinary-request observations are read-only and are checked in
separate execution families instead of multiplying their unrelated local
states. The ordinary-request family covers the grant and creator-revocation
workloads, including concurrent promotion. All five workloads check recovery
and writer authorization. No observer changes a stream or disables a writer.
Completed/conflicting/expired requests release their server-local state.

The model retains client request bytes outside a crashed server, but drops its
prepared slot, reply, and authorization. Retries may resolve an existing exact
operation; they do not rewrite that request. Recovery records its verified
source snapshot, so a later publication does not incorrectly invalidate a
previously verified view. The request snapshot likewise survives a later
revocation only within its original deadline.

The normal check covers durable acknowledgments, predecessor/hash-chain
binding, unique per-book operation IDs, exact retry receipts, configuration
references, fresh authorization, seed/recovery preservation, bounded request
completion, and independent-book publication enabledness. It is a bounded
exhaustive check, not an unbounded proof, cryptographic proof, real-clock
scheduler model, or implementation-level storage test.

```sh
bazel test //tla:durable_control_check //tla:probes_test --test_output=errors
tla/probes.sh
```

| Manual target | Sabotage | Required violated invariant |
|---|---|---|
| `durable_control_overwrite_check` | Overwrite a claimed revision | `AcknowledgedTransitionsSurvive` |
| `durable_control_predecessor_check` | Append stale intent at a newer head | `CommittedPredecessorMatches` |
| `durable_control_early_ack_check` | Acknowledge before publication | `AcknowledgedTransitionsSurvive` |
| `durable_control_retry_reuse_check` | Treat operation ID alone as an exact retry | `RetryResponseMatchesPayload` |
| `durable_control_stale_auth_check` | Reuse bootstrap authorization for a fresh request | `FreshAuthorizationAtRequestStart` |
| `durable_control_seed_reset_check` | Reset from a seed after creator revocation | `SeedCannotResurrectCreator` |

Every manual target must fail for its named invariant. A parser/configuration
error, timeout, or green probe is a failed check. The full probe sweep verifies
these names as well as every preexisting manual probe.

Verification on this bounded model: the normal run explored 1,032,189 generated
states and 186,198 distinct states to depth 18, with no invariant violation.
The six focused probes each failed for the invariant listed above; the complete
`tla/probes.sh` sweep reported all 41 probes red for their named reason.

## Running implementation

Configuration blobs are stored at `_control/config-blobs/{sha256}`. Transition
objects are conditionally created at
`_control/transitions/{book}/{revision:020}`. Readers verify canonical protobuf
bytes, bootstrap identity, gapless revisions, predecessor and operation
digests, every promoted blob, and unique operation IDs before deriving ACTIVE,
newest-first HISTORY, or membership. Local `config/ACTIVE`,
`config/HISTORY`, and `MEMBERSHIP.tsv` remain legacy inputs only for books
without a durable publication; they cannot override a published book.

Trusted `ratio config set`, `ratio approve`, and `ratio membership
grant|revoke` durable forms require `--operation`, `--expected-revision`, and
`--predecessor`. They select `RATIO_JOURNAL_BUCKET` /
`RATIO_JOURNAL_PREFIX`, or `RATIO_JOURNAL_LOCAL`, exactly as the server does.
An unusable configured store refuses instead of mutating local state. The
actor is the stable `RATIO_ACTOR` / OS user and provenance is `trusted-cli`;
the committed operation still requires that actor to be a current book member.
