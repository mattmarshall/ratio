# Book recovery: inventory and local drill

Related: [#264](https://github.com/mattmarshall/ratio/issues/264).
Durable follow-on work: [#299](https://github.com/mattmarshall/ratio/issues/299)
and [#300](https://github.com/mattmarshall/ratio/issues/300).
Source review refreshed after #302, September 9, 2026.

Ratio does not yet have a demonstrated whole-book disaster recovery procedure
for the deployed service. The journal's conditional S3 writes protect one part
of a book. A book also needs its chart, configuration, identity, access grants,
and the evidence attached to its figures. Several of those still live only in
the serving container's filesystem.

The first executable evidence is a disposable **local directory restore**. It
checks a journal prefix and digest, two local configuration versions, a NAV
strike and replay, book kinds, and membership isolation. A second probe records
the legacy gap when a local book is created before an object store is attached.
Published CreateBook recovery is covered separately by the bootstrap tests.
Neither is an S3 recovery drill or a customer recovery commitment.

## Recovery objectives and ownership

| Decision | Current state | Accountable role to assign before a pilot |
|---|---|---|
| RPO: maximum acceptable loss of acknowledged writes | **Not agreed**; no number is implied by append-only storage | Customer book owner and Ratio service owner |
| RTO: time to restore verified service | **Not agreed**; local test duration is not an RTO | Customer book owner and Ratio service owner |
| Backup frequency, retention, copies, region/account separation | **Not agreed**; no complete backup job is defined here | Ratio service owner |
| Restore execution and access to backup material | Named operator and alternate **not assigned** | Ratio service owner |
| Acceptance of figures, closes, and access after a restore | Named verifier **not assigned**; keep separate from the executor where possible | Customer book owner |
| Drill cadence and evidence retention | **Not agreed** | Ratio service owner |

The Platform & Connect track owns the implementation queue. That queue does
not appoint an incident responder or make a recovery guarantee. Record the
named owners and agreed objectives on #264 before a customer drill.

## Authoritative material

Paths below are relative to a book directory unless marked as a root path.
The deployed object key begins with `RATIO_JOURNAL_PREFIX` (default
`journals/`), then the **book directory basename**. A sequence is one-based,
zero-padded to 20 digits. `RATIO_JOURNAL_BUCKET` selects S3;
`RATIO_JOURNAL_LOCAL` selects a directory object store for local `ratio watch`.
With neither installed, FileBook uses local JSONL files.

| Material | Local path | With the object store installed | Recovery significance |
|---|---|---|---|
| Journal | `journal.jsonl` | `<book>/journal/<sequence>` | Preserve exact order, every entry, and all cited configuration digests. A balanced shortened journal can still be wrong. |
| Baked-seed publication marker | Baked JSONL planes in the deployment image | `_seed/publications/<book-id>` under `RATIO_JOURNAL_PREFIX` | Format version 1, whole-seed digest, and per-plane lengths prove which baked prefix deployment published. Preserve it with the journal; deleting it to clear a mismatch removes the deployment fence. |
| Published bootstrap | `BOOTSTRAP.pb` and its materialized files | `_bootstrap/publications/<book-id>` and referenced `_bootstrap/blobs/<digest>` | The immutable, content-addressed bootstrap preserves chart, identity, kind, opening configuration, and creator grant. Capture both the publication pointer and its exact referenced blob. |
| Configurations and promotion state | `config/<digest>`, `config/ACTIVE`, `config/HISTORY` | Opening state is in the published bootstrap; later promotions are **still local** | Preserve every later referenced blob, promotion history, and the actual active pointer until #304 supplies durable transitions. |
| Chart | `accounts.json` | In the published bootstrap for new books; legacy local books have no publication | Names and types the dimensions. Never infer a missing legacy chart from defaults. |
| Book identity and kind | `book.toml` | In the published bootstrap for new books; legacy local books have no publication | Carries kind, display name, optional fund and organization. Missing legacy metadata can fall back to Investment semantics. |
| Membership | `<funds-root>/MEMBERSHIP.tsv` | The creator grant is in the published bootstrap; later grants and revocations are **still local** | Preserve exact post-create grants. An organization claim and a Connect client/template grant cannot substitute for current book membership. |
| Deliveries | `deliveries.jsonl` | `<book>/deliveries/<sequence>` | Delivery metadata contains digest, origin, receipt time, and byte count; it does **not** contain the original delivered bytes. |
| Entity master | `entities.jsonl` | `<book>/entities/<sequence>` | Preserve resolution history, including corrections. |
| Facts | `facts.jsonl` | `<book>/facts/<sequence>` | Preserve provenance and order. Prices and FX are evidence a figure cites. |
| Legacy action announcements | `actions.jsonl` | `<book>/actions/<sequence>` | Preserve for legacy books. New announcement/application evidence also resides in journal entries; do not discard the legacy plane. |
| Break explanations | `explanations.jsonl` | `<book>/explanations/<sequence>` | Records actor, reason, accepted difference, configuration, and journal citation. Rebuilding balances cannot recreate a person's decision. |
| Period closes | `closes.jsonl` | `<book>/closes/<sequence>` | Records the cited boundary and prevents posting back into a closed period. Closing postings are separate journal entries. |
| NAV strikes | `NAVS` | **Still local** | Records `(view, valuation point)`, actor, prefix, digest, and figure. Recomputing today's NAV does not restore the signed strike. |
| Reconciliation reports | `reports/*.pb` | **Still local** | Preserve report bytes, names, and modification times: `newest_report` selects by mtime. A recomputed report is a new artifact, not the one previously accepted. |
| Proposals and audit trail | `proposals/`, `CHANGELOG` | **Still local** | Preserve pending/accepted proposal artifacts and who approved or acted under a configuration. |
| Original delivery files and external app evidence | External source locations; no general retained-blob path in the ingest code | **No complete retention contract here** | Inventory the actual upstream archive and Connect app stores. A delivery digest alone cannot recover the original file. |

Do not restore into a different parent directory in the same object store and
assume isolation: `book_key` uses only the basename. Use a separate empty object
store namespace/account for a recovery environment, retaining the original book
IDs inside it. Reusing a live prefix would put recovery writes onto live books.

The implementation supporting this inventory is:

- [FileBook, planes, hydration, and bootstrap materialization](../crates/ratio-store/src/lib.rs).
  `hydrate_objects` copies legacy local JSONL into the object store. Published
  books are discovered through their publication records and materialize a
  verified immutable bootstrap; later control transitions remain #304.
- [ObjectStore and SeqLog](../crates/ratio-store/src/objects.rs), plus the
  [S3 adapter](../crates/ratio/src/scale.rs). `put_if_absent` protects sequence
  slots; that is not a backup, a cross-plane checkpoint, or a metadata store.
- [Book initialization and grants](../crates/ratio-console/src/book.rs) and
  [CreateBook, CHANGELOG, reports, and book discovery](../crates/ratio-console/src/lib.rs).
  CreateBook publishes complete bootstrap state before returning success.
- [NAV persistence](../crates/ratio-nav/src/lib.rs) and
  [ingest delivery schema](../crates/ratio-ingest/src/lib.rs).
- [Startup](../deploy/entrypoint.sh) and
  [store installation](../crates/ratio/src/watch.rs). The Platform deployment
  owner conditionally publishes and validates baked JSONL before traffic.
  Startup copies the local chart/config cache into `/tmp`, verifies every
  `_seed/publications/<book-id>` marker, and attaches without seed PUTs. It can
  regenerate demo memberships from `RATIO_DEMO_MEMBER`. Published CreateBook
  books recover independently of the baked seeds.

## Rebuildable material

In-memory projections and the optional Postgres lots/positions/Current tables
are derived from the journal. They are not a replacement backup for it.
See [the console store door](../crates/ratio-console/src/store.rs) and
[projection storage](../crates/ratio-sql-project/src/lib.rs).

Start a recovery environment with fresh projection state and rebuild at the
restored journal prefix. Compare its watermark prefix and journal digest with
the captured evidence before serving figures. An old projection can be ahead
of a restored journal or carry a different digest at the same height; catch-up
correctly refuses that case. Do not erase the live projection to make a test
restore work, or disable its refusal. A production Postgres rebuild/timing drill
remains part of #264.

## Capture and restore procedure

This is the operator procedure to complete and demonstrate in an isolated
environment. It is not currently an automated production backup command.

1. **Identify the boundary.** Record the deployed commit/image, book IDs,
   source bucket and exact prefix, local serving roots, projection mode, and
   separately controlled identity/Connect configuration. Identify where every
   row in the inventory is currently available. If local authoritative files
   have already vanished, label that loss; do not recreate their history from
   defaults and call it restored.
2. **Quiesce all writers.** Stop new posts, ingest, promotions, closes,
   explanations, NAV strikes, and membership changes across every writer for
   the selected books. Wait for acknowledged operations to finish. There is
   no atomic cross-plane snapshot mechanism here; copying a running container
   and independently listing S3 is not an agreed checkpoint.
3. **Capture evidence while quiescent.** Retain complete book directories and
   the root membership file, preserving bytes and report mtimes. Capture every
   object in each book's seven sequence prefixes, plus every
   `_bootstrap/publications/<book-id>` record and its referenced
   `_bootstrap/blobs/<digest>`. Record sequence heights and content hashes and
   every `_seed/publications/<book-id>` marker. Check each marker's format
   version, digest, and plane lengths against the captured baked source; do not
   synthesize or remove a marker during restore. Check that each sequence is
   contiguous. Record
   the journal prefix/digest, ACTIVE, HISTORY, hashes of every configuration
   blob, NAV strike records and their replay results, closes, explanations,
   and the expected authorized/unauthorized subjects. Keep identity material
   in the restricted backup evidence, not a public issue or CI log.
4. **Validate the capture.** Hash the configuration bytes against their names
   and resolve all digests cited by journal entries, facts, closes, reports,
   and strikes. Check source/backup counts and bytes independently. The
   existing `DirectoryConfigStore::get` reads the blob but does not verify
   its content address for you. Preserve all versions; selecting only ACTIVE
   makes old figures unreplayable. Record any missing source deliveries.
5. **Restore into an empty isolated destination.** Use a separate storage
   namespace and serving root, with no production traffic or outbound Connect
   writes. Restore exact keys/order plus local metadata and membership. Do not
   seed defaults over missing evidence. `hydrate_jsonl` skips already-occupied
   slots by height, so reopening on a nonempty target does not establish that
   it contains your intended bytes. Do not rename book IDs as an isolation
   mechanism.
6. **Verify the restored book.** Compare all captured counts/digests, exact
   active/history state, chart and BookKind, recorded closes and explanations,
   reports and NAV records. Rebuild projections; replay each selected strike
   against its own pinned prefix and configuration. Check an independently
   known figure as well as conservation. Exercise membership with a granted
   subject and a subject granted another book, including direct reads and
   attempted writes. Check that closed-period refusal still holds.
7. **Exercise failure and finish the drill.** On disposable copies, remove a
   plane/configuration and change an entry without unbalancing it; verification
   must reject the copy. Record start/end times, bytes, book size, verification
   evidence, failures, and the named verifier. Compare measured recovery with
   the agreed objectives only after those objectives exist. Reopening live
   service is a separate operator decision after acceptance, not a test step.

No step asks an agent to accept a break, approve policy, or certify customer
figures. Those decisions remain with the authorized book owner.

## Run the local evidence

From the repository root:

```sh
bazel test //crates/ratio-console:recovery_test --test_output=all
```

The [test](../crates/ratio-console/tests/recovery.rs) creates synthetic books
under Bazel's test temporary directory. It never opens AWS, a production root,
a network listener, or a user's session. Its complete-directory drill:

- Creates Investment and Personal books for different synthetic subjects.
- Posts two capital entries under two promoted configurations and records a
  NAV of 60,000 minor units at prefix 2.
- Captures the files, removes the source, restores into a fresh directory,
  checks every file's bytes, validates both config content addresses, and
  replays the recorded strike.
- Verifies both users retain exactly their own book and an outsider sees none.
- Changes only a journal memo: the NAV stays 60,000 and the prefix stays 2,
  but the digest check rejects the corrupted recovery. A second sabotage
  overwrites a configuration blob under its old name and is also rejected.

The second test uses a real directory-backed object store but deliberately
creates a legacy local book before attaching that store. After the serving root
is removed, its journal survives while its un-published control plane does not.
This characterizes legacy compatibility; it does not describe CreateBook after
#302. `ratio-store` bootstrap tests cover successful published-book discovery,
verification, and materialization after local-root loss.

The local copy helper checks bytes; it does not implement archival metadata
preservation. This small fixture has no reports, facts, entity corrections,
accepted explanations, or period close and does not validate those lifecycles.
Their full preservation, report selection by mtime, S3 capture/restore, gateway
authentication, Postgres rebuild, concurrent writers, operator procedures, and
customer-scale timing remain required external drill coverage.

## Gaps that block a production recovery claim

- [#299](https://github.com/mattmarshall/ratio/issues/299): post-create config
  promotions and membership changes still need durable transitions. [#300](https://github.com/mattmarshall/ratio/issues/300)
  covers NAVs, reports/proposals, audit logs, and the remaining operational
  evidence and writer-storage consistency.
- No whole-book backup scheduler, consistent checkpoint/export command,
  restore command, retention policy, or independent backup copy is established
  by this inventory. The S3 template configures encryption and prevents public
  access; it does not declare versioning or Object Lock on ScaleBucket.
  Live settings were not inspected and are not inferred from the template.
- Original delivery bytes and Connect app state need explicit custody and
  recovery owners. Journal/fact citations cannot replace missing source bytes.
- Customer RPO/RTO, named operators, the external drill and signed evidence
  remain open on #264.
