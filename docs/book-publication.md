# Atomic publication of a new book

Related: [#297](https://github.com/mattmarshall/ratio/issues/297),
parent [#291](https://github.com/mattmarshall/ratio/issues/291), and recovery
[#264](https://github.com/mattmarshall/ratio/issues/264).

This document records the model and the initial Rust implementation (#298).
The model defines the publication boundary; the implementation and fresh-process
checks below demonstrate it for created books. Broader durability and recovery
work remains under #291 and #264.

## One publication makes the complete bootstrap discoverable

Two creators may prepare different initial generations for the same book ID.
They stage immutable content objects first. A single conditional publication
claim then selects one complete generation. No staged object is discoverable
as a book or grants anyone access before that claim succeeds.

The publication record commits to all of the following:

| Part | Required committed material |
|---|---|
| Identity | Validated book ID, supported BookKind, display name, creation attribution, and optional fund/organization fields. CreateBook keeps fund and organization absent. Neither identity attribution nor an organization field grants access. |
| Chart | Exact chart bytes and content address. Recovery must not regenerate a potentially different chart from the running binary's current template. |
| Opening configuration | Exact configuration bytes and their content address, including all sections and elections. The model does not allow an absent blob to mean empty rules. |
| ACTIVE | The content address of that opening configuration. |
| HISTORY | One initial promotion naming the same opening configuration. Preserve its order; subsequent promotions belong to a later control-plane protocol. |
| Creator grant | An explicit authorization record pairing the book ID with the authenticated creator's stable subject. It is bound into this bootstrap independently of the identity record. |

The concrete schema must be protobuf, versioned, and validated before staging.
Each stored part and every reference must be bound to its exact bytes; the
publication must identify a single consistent initial generation. The model's
`Address(creator, part)` is an abstract content address, not a proposed key
format or a proof of SHA-256. Hash collision resistance, canonical encoding,
unsupported-version refusal, and byte-preserving codecs are implementation
obligations.

An implementation may place these parts in one immutable bootstrap object or
several content-addressed objects plus a manifest. In either representation,
the book-ID publication claim is the only visibility boundary. It must not
publish a pointer to a chart/configuration/grant that is still being written.
The immutable staged objects are not a second journal and must never be
interpreted as a directory of independently effective memberships.

## State transitions and failure behavior

[BookPublication.tla](../tla/BookPublication.tla) models two creators, one
book ID, and a reader. Each creator stages six parts in a fixed order. The
interleavings include every partial prefix and either creator finishing first.
Publication checks the full set of references, rather than trusting the local
staging cursor. Other staging orders refine the same boundary if they retain
that full-reference check; their enumeration is not claimed by this model.

1. **Stage.** Store complete immutable objects at their content addresses.
   Staging does not create a discoverable book or an effective grant.
2. **Publish.** Verify that the candidate references a complete, mutually
   consistent bootstrap, then conditionally create the publication for the
   book ID. Exactly one initial generation wins. A competing create receives
   a conflict and cannot replace the winner or obtain membership.
3. **Acknowledge.** Report success only after the conditional publication has
   committed. A response is separate from a committed publication.
4. **Read.** Discover through publication, fetch its references, and verify
   their bytes and relationships before exposing a book or its grants. Missing
   and corrupt objects produce refusal, with no seeded/default replacements.

A failure while staging or before the conditional publication leaves only
unreferenced objects. It must not reserve the book ID indefinitely through a
partial catalog row. Garbage collection of such objects is outside this
issue; it must never delete something referenced by a winning publication.

A crash after publication but before the success response leaves a complete,
discoverable book. The caller's outcome is uncertain, not rolled back. A retry
must read the existing publication and either recognize an agreed idempotency
key or return a conflict; it must not overwrite the record or grant a second
subject access. Request-id/idempotency-key design is left to the implementation
child. The model does not claim liveness or completion after unlimited failures.

A process crash removes local staging progress. A restarted reader has no
cache and must obtain the complete bootstrap from durable storage. The model
keeps publication and immutable content through those crashes. It separately
injects missing and corrupt object responses: a reader refuses while either
prevents verification. Those faults model a storage/decoding failure at read
time; they do not claim that backups can recover deleted content. A prior
verified read remains evidence of its own snapshot, not a promise that a
later read will succeed.

The invariants checked by the normal model are:

- `PublishedBookComplete`: a publication references only fully staged objects.
- `PublishedGenerationAgrees`: identity, chart, configuration, ACTIVE/HISTORY,
  and the explicit grant belong to the same initial generation.
- `OneWinningGeneration`: one book ID has at most one successful initial claim,
  including claims whose response was lost.
- `AcknowledgedPublicationSurvives`: every successful reply still names the
  same complete durable publication after crashes or competing creates.
- `ReaderHasVerifiedSnapshot`: a reader exposes a book only after all referenced
  objects and their relationships verify.
- `ReaderGrantMatchesSnapshot`: a successful initial read derives exactly the
  published creator grant; refusal exposes no grant.
- `LosingCreatorHasNoMembership`: a competing creator who loses the claim gains
  no membership from their staged grant or identity.

## Checks and deliberate failures

Run from the repository root:

```sh
bazel test //tla:book_publication_check //tla:probes_test --test_output=errors
tla/probes.sh
```

The five new manual probes each flip one dial. Their configuration names the
invariant that must fail, and the probe sweep verifies TLC reports that reason.

| Target | Sabotage | Required failure |
|---|---|---|
| `//tla:book_publication_overwrite_check` | Disable the conditional publication claim | `AcknowledgedPublicationSurvives` |
| `//tla:book_publication_incomplete_check` | Publish before all referenced objects exist | `PublishedBookComplete` |
| `//tla:book_publication_mixed_check` | Borrow the other creator's ACTIVE object | `PublishedGenerationAgrees` |
| `//tla:book_publication_defaults_check` | Accept missing/corrupt objects instead of refusing | `ReaderHasVerifiedSnapshot` |
| `//tla:book_publication_staged_grant_check` | Treat staged grant objects as effective memberships | `ReaderGrantMatchesSnapshot` |

These failures are expected evidence. A parser error, missing constant,
timeout, or green probe is not a successful sabotage check.

## Implementation boundaries and remaining work

The next child must test the real S3 and local adapters, concurrent creators,
lost responses, and fresh serving processes. At the reviewed `cde2d69` baseline,
the S3 ObjectStore adapter only accepts UTF-8 and uses lossy decoding on GET;
a protobuf snapshot requires byte-preserving storage. The local DirStore
publishes an empty file before writing its body, which is not an atomic
complete-object publication. The process-global store uses `OnceLock`; a
test cannot simulate a fresh backend by calling its installer a second time.
Those are implementation obligations, not behavior repaired by this model.

The reader is the internal bootstrap loader and membership derivation, not an
HTTP authentication model. The request layer must still validate its caller
and apply the resulting membership to both lists and direct book operations.

Publication keys and journal book keys must share a deliberate book-ID
namespace. A changed local parent directory is not isolation: journal keys
currently use the book basename. Preserve legacy seed compatibility explicitly;
do not promote a seed, synthesize defaults, or grant all demo members merely
because an incomplete durable book is found.

This bootstrap contains the **initial** ACTIVE/HISTORY and creator membership.
Later configuration promotion, grant/revocation updates, and their concurrency
and audit history require separate modeled mutation protocols. Re-reading this
immutable bootstrap must never reset newer ACTIVE/HISTORY or resurrect a
revoked creator grant. Operational evidence (NAVs, reports, proposals, audit
events), original delivery custody, full recovery checkpoints, and customer
RPO/RTO remain on #291/#264 and their implementation children.

## Implemented boundary and recovery procedure (#298)

`ratio.storage.v1.BookBootstrap` stores the exact chart JSON and rules TOML
bytes, their SHA-256 addresses, identity and creation attribution, initial
ACTIVE/HISTORY, and a separate creator grant. Version 1 supports independent
books only: there is no implicit fund or organization. The reserved
`_bootstrap/blobs/<sha256>` key holds the immutable protobuf. A conditional PUT
at `_bootstrap/publications/<book-id>` holds `BookPublication`, the sole
catalog entry. A failed create can leave an unreachable blob; it cannot leave
a catalog entry pointing at a partially written bootstrap. Retries after a
committed publication return a conflict, including when the first response
was lost. Request idempotency keys are not implemented.

`ObjectStore` carries bytes without a UTF-8 conversion. S3 uses
`If-None-Match: *`, and returns false for an occupied key. Other SDK errors
propagate. DirStore writes and syncs a private temporary file before an atomic
no-replace hard link exposes the complete object. Its private staging files
are excluded from discovery. This is a local filesystem adapter test, not a
claim about a production bucket's retention or disaster recovery objectives.

The console unions legacy local directories with verified published IDs.
For a published book, AuthKit access requires the exact stable subject in its
explicit creator grant, even in an open demo. An email, organization, legacy
MEMBERSHIP.tsv row, or Connect token does not inherit that grant. Local CLI
inspection remains unrestricted. Durable CreateBook requires an authenticated
AuthKit creator; local and Connect creation refuse until there is an explicit
corresponding grant protocol. No global membership file is rewritten by
creation, so simultaneous creates of different books cannot lose each other's
grants.

To exercise a cold start safely:

1. Use an isolated local object-store directory and a disposable serving root.
   Do not point the drill at a production backend.
2. Create a book through a scoped console. Post an entry and retain the
   published identity, exact chart/config bytes, initial HISTORY, and journal
   entry/prefix as expectations outside the serving root.
3. Stop the serving process and remove its entire disposable serving root,
   including MEMBERSHIP.tsv, book.toml, accounts.json, and config/.
4. Start a new process with the same explicit object backend and an absent
   serving root. List books as the creator, open the recovered book, and compare
   identity, chart, ACTIVE/HISTORY/config, journal entry count, and entries.
   Verify that a different subject cannot list or open it.
5. Keep the object backend intact. Missing/corrupt content or unsupported
   versions must refuse; copying in a seed or accepting empty rules is not
   recovery. An existing local cache that differs also refuses. Investigate
   it before removing anything; never reset a potentially newer local state
   to the initial bootstrap.

The automated equivalent uses fresh child processes so the process-global
store's first-install-wins behavior cannot hide use of the wrong backend:

```sh
bazel test //crates/ratio-console:ratio-console_test --test_filter=bootstrap_tests --test_output=errors
bazel test //crates/ratio-store:ratio-store_test //crates/ratio-project:ratio-project_test //crates/ratio:ratio_test //proto:ratio_aip_lint //proto:mirrors_test --test_output=errors
```

Published books materialize a verified cache using an atomic directory rename,
then read journal/append-only planes from the configured object store. Discovery
excludes the reserved local staging namespace, including crash debris. Both
in-memory projection and Stage E pin/replay use the authorized book's explicit
backend, so a second process or store cannot silently supply a different prefix. They
never hydrate local seed JSONL into that journal. A baked legacy book is not
auto-published, and create refuses an existing local book or existing remote
book-prefix data. Legacy book operation keeps its prior storage behavior.
Legacy-to-published migration must fence writers first; the legacy prefix
check is not a migration lock against an old binary concurrently hydrating a
seed with the same ID. No production migration is part of this change.

Subsequent configuration, chart, and identity mutation on published books explicitly
refuses: acknowledging a local-only promotion would make the next container
recover an obsolete opening generation. Additional grants and revocations,
including Connect delegation, need a separate durable control-plane protocol;
MEMBERSHIP.tsv is still the legacy-book authority and is not that protocol.
Later NAV/report/audit evidence, complete backups, cross-region recovery,
retention, deletion protection, and garbage collection remain under #291 and
#264. This implementation does not establish an RPO or RTO, assign an incident
owner, or claim a production disaster-recovery drill.


The implementation sabotage checks demonstrate two distinct boundaries:
ignoring a failed conditional publication makes the racing-creator test fail
because both creators report success; omitting HISTORY validation makes the
required-plane test fail because an absent promotion history becomes a visible
book. Both checks were restored before the final passing suite. The S3 adapter
check uses a local HTTP endpoint and real SDK requests to verify binary bytes,
`If-None-Match`, occupied-key refusal, missing-key behavior, and other-error
propagation. It does not contact S3 or a production account.
