# Storage backends and the distribution model

Status: **analysis and proposal.** Nothing here is a committed feature, a PLAN
amendment, or an authorization to implement. It maps the persistence layer as
built, states which alternative backends the existing seams can carry, and
names the blockers that stand between the current shape and a two-track
distribution model — on-device Personal, hosted SQL for funds and shared
workspaces.

Related decision gate: [#282](https://github.com/mattmarshall/ratio/issues/282)
(reviewed config storage). Related measurement: [#268](https://github.com/mattmarshall/ratio/issues/268).
Related durability work: [#264](https://github.com/mattmarshall/ratio/issues/264),
[#291](https://github.com/mattmarshall/ratio/issues/291),
[#293](https://github.com/mattmarshall/ratio/issues/293).

⚠ `specs/architecture/tech-stack.md` describes PostgreSQL as the primary data
store, with `sqlx` and `tui-rs`. AGENTS.md already declares that program
obsolete. It is not evidence of a current storage decision and must not be
cited as one.

---

## 1. What exists

### 1.1 Four planes, three seams

A book is not one store. It is four kinds of material with different
durability rules, and the code keeps them apart on purpose.

| Plane | What it holds | Seam | v1 implementation |
|---|---|---|---|
| Journal | The append-only record of what happened | `Journal` (`crates/ratio-store/src/lib.rs:1191`) | `journal.jsonl`, or one object per entry on an `ObjectStore` |
| Side planes | Deliveries, entities, facts, actions, explanations, closes | `Plane` (`lib.rs:1268`), `FactStore` (`lib.rs:1153`) | Same two shapes as the journal |
| Configuration | Content-addressed rule bytes, an ACTIVE pointer, a HISTORY | `ConfigStore` (`lib.rs:1011`) | `DirectoryConfigStore` on disk; `ControlStore` protobuf transitions on the object store for published books |
| Projection | Lots, positions, aggregates — derived, disposable | `Projection` / `ProjectionReads` | In-memory fold; optional Postgres snapshot |

⭐ **The journal/projection split is the load-bearing one.** The journal is the
book of record because replay and the prefix digest are the product. The
projection is a snapshot of one prefix under one watermark and can be thrown
away. Any backend proposal that collapses these two is a different product.

### 1.2 The `ObjectStore` seam

Three methods (`crates/ratio-store/src/objects.rs:32`):

```rust
fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool>;
fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
fn list(&self, prefix: &str) -> Result<Vec<String>>;
```

`put_if_absent` is the entire concurrency argument. `tla/S3Journal.tla` models
it; `MemoryStore::unconditional()` is the probe that shows a non-atomic store
losing an acked entry silently. Three implementations exist: `MemoryStore`,
`DirStore` (atomic no-replace hard link), and `S3` in `crates/ratio/src/scale.rs:346`
(`If-None-Match: *`, 412 is the expected refusal).

`SeqLog` (`objects.rs:232`) lays a log over it: keys are `{prefix}{seq:020}`,
zero-padded so a LIST is already in order. Append is LIST-for-height then
conditional PUT at `height + 1`, retrying on a lost claim.

The store is installed **once per process** through a `OnceLock`
(`objects.rs:72`), first call wins. `RATIO_JOURNAL_BUCKET` selects S3,
`RATIO_JOURNAL_LOCAL` selects a directory, unset is local JSONL.

### 1.3 The Postgres projection

`crates/ratio-sql-project` is a table snapshot, not a ledger. One watermark
(`book_id`, `journal_prefix`, `journal_digest`) covers every table and is
replaced in one commit — there is no per-table advance. A pin that does not
match refuses rather than answering from a lagging snapshot.
`RATIO_PG_URL` opts in; unset is the in-memory fold and never a default server.

⚠ **It talks to Postgres by spawning `psql`** (`pg.rs:8`). That was a
deliberate call to keep a `crate_universe` member out of the Bazel build. It
is also the single fact that decides most of section 3: a subprocess client
cannot be a library client, cannot run in wasm, and cannot talk to pglite.

Read SQL is emitted from `plan.rs`, not assembled as free strings — which is
what makes retargeting the dialect feasible at all.

### 1.4 Checkpoints

`crates/ratio-project/src/checkpoint.rs` captures a folded projection against a
verified prefix pin, so a cold read loads the newest valid pin and replays only
the tail. A corrupt, missing, or mismatched blob falls back to a full replay.

⛔ **The store is a local directory.** `DirectoryCheckpointStore`
(`checkpoint.rs:642`) is `fs::` only, and the console keeps it under
`.ratio-cache/projection-checkpoints` at the console root
(`ratio-console/src/lib.rs:492`). On Lambda that is container-local and dies
with the container. Every cold start is `CheckpointMiss::Missing` and pays a
full fold. **The acceleration exists in the library and does not reach the
deployed read path.** This is the highest-leverage unfixed thing in the
persistence layer, for either track.

### 1.5 Hosting

| Component | Where | Shape |
|---|---|---|
| API + three public screens + MCP | AWS Lambda, container image, Lambda Web Adapter | 512 MB, 60 s timeout (`deploy/app.yaml:174`) |
| Gateway | API Gateway HTTP API | 30 s integration cap — the real ceiling on any authenticated fold |
| Journal, bootstraps, control transitions | S3 (`ScaleBucket`), prefix `journals/` | One object per entry |
| Console | Next.js on Vercel | Server-side only; the browser never calls the API, and `authorization` is deliberately absent from `AllowHeaders` |
| Big folds | Fargate `ScaleTask` | 4 vCPU / 8 GB / 100 GiB ephemeral |
| Identity | WorkOS AuthKit | Two JWT authorizers, two issuers |

---

## 2. The cost and latency shape, as measured

These are recorded numbers from HANDOFF and `docs/durable-startup.md`, not
estimates.

| Measurement | Value |
|---|---|
| Stage E projection fold, 10,000 securities × 2,000 lots = 20M lots | 17.4 s on 4 vCPU / 15 GiB |
| Peak footprint, one view, 20M lots | ~640 MB per view; 1.00 GB peak |
| Fold of 1.77M entries after streaming landed | 50 MB resident (was 1.85 GB) |
| `ratio balance` | 9 MB (was 1.26 GB) |
| Console serving a book | 39 MB |
| Deploy `34505523090` publication | 41 min for ~32,000 sequential S3 body reads, cancelled at the 45-min job budget |

⛔ **The last row is the whole hosting problem in one line.** One object per
journal entry means a cold fold is N serial GETs. Publication hit it first and
was capped at 32 concurrent readers as mitigation; the *serving* read path has
no such ceiling raised, no batching, and — per §1.4 — no durable checkpoint.
A household book of 50,000 entries at ~10 ms per GET is roughly eight minutes
of cold fold against a 30 s gateway cap.

⚠ **And `SeqLog::height()` is O(n) on every append.** It calls
`store.list(prefix)` and scans every key (`objects.rs:254`). On S3 that is a
paginated `ListObjectsV2` over the whole book — 200 requests for a 200k-entry
book — and `append()` pays it once per retry. This is backend-independent and
is a present defect, not a property of any proposed backend.

So the economics do not currently favour hosting Personal books, and the work
that would fix them is largely the same work either track needs.

---

## 3. Backend-by-backend verdict

The question is not "can the trait be implemented" — it is three methods and
almost anything can. The question is whether the *claim* survives, whether the
build tolerates the dependency, and whether it makes a cost curve better.

### 3.1 S3 / R2 / any conditional-PUT object store — **built, keep**

Native `If-None-Match: *`. R2 and GCS offer the same precondition, so the
existing `S3` adapter generalizes to a cheaper egress profile with a config
change and a different SDK, not a redesign. Best fit for cold archival and for
the encrypted-blob backup in §4.1.

Obstacle: none architectural. The per-entry-object read cost in §2 is the
issue, and it is fixed by checkpoints and segmentation, not by changing store.

### 3.2 Postgres as the **journal** store — **plausible, and the strongest claim available**

`INSERT INTO journal (book_id, seq, body) VALUES (...) ON CONFLICT DO NOTHING`
with the rowcount as the boolean is a genuinely atomic claim — stronger than
S3's, because it composes with a transaction. `height` becomes
`SELECT max(seq)`, which kills the O(n) append in §2. A cold fold becomes one
ordered streaming scan instead of N round trips.

Obstacles:

1. **`ObjectStore` is the wrong shape for it.** `list(prefix) -> Vec<String>`
   forces materializing every key to learn the height. The trait needs a
   `height(prefix) -> u64` (or `max_key`) with a default implementation over
   `list`, so a backend that can answer cheaply does. Small change, wide blast
   radius, and it should land **before** any new backend, not after.
2. **A real Rust client is a new `crate_universe` member.** PLAN refused
   SQLite partly over a C dependency; `tokio-postgres` / `rust-postgres` is
   pure Rust, so that objection does not transfer. `CARGO_BAZEL_REPIN=1` is
   already verified working (PLAN, Stage 0 amendment), so this is a dependency
   edit rather than a research project — but it is the first time the core
   store crate would take a network dependency, and `ratio-store` is currently
   free of one on purpose.
3. **The TLA+ spec is about S3.** `tla/S3Journal.tla` models LIST-then-
   conditional-PUT. A Postgres journal has a different — stronger — model and
   needs its own spec before it decides an amount, not after.

### 3.3 Postgres as the **projection** — **built, extend**

Already there. Extending it to serve aggregation natively for funds and shared
workspaces is the least speculative item in this document: the schema exists,
the watermark discipline exists, the pushdown surface (`plan.rs`) exists, and
the stale-read refusal exists.

Obstacle: the `psql` subprocess. Per-request process spawn is not a serving
strategy for a multi-tenant workspace, and it precludes connection pooling,
prepared statements, and transactions spanning calls. `pg.rs` needs its
execution split behind a trait with (at least) a subprocess impl and a library
impl. 449 lines, and the SQL is already emitted rather than concatenated.

### 3.4 pglite — **plausible for the projection, pointless for the journal**

pglite is Postgres compiled to wasm. Used as the **on-device projection
engine** it buys something real: the same `schema.sql` and the same emitted
SQL run on-device and on the server, so a figure's query path is one path, not
two. That is worth more to this project than to most, because "two engines,
two answers" is exactly the class of failure the repository is built to
refuse.

Obstacles:

1. Same `psql` blocker as §3.3, and worse — pglite has no `psql` at all. The
   exec trait in §3.3 is a hard prerequisite.
2. pglite runs in JS; the kernel would be Rust-in-wasm. That is a wasm↔JS
   boundary on every projection query, with serialization on both sides.
3. It is ~3 MB compressed on top of the Rust wasm bundle. For a Personal book
   whose entire in-memory projection is single-digit MB (§2), **an embedded
   Postgres is not obviously earning its size.** The in-memory fold plus an
   OPFS checkpoint blob is simpler and already proven.

Verdict: do not adopt for v1 of on-device. Revisit if and only if on-device
books grow past what the in-memory fold handles, or if query-path unification
becomes a stated requirement.

### 3.5 OPFS — **the right on-device journal store, with one real caveat**

Origin Private File System, accessed through `createSyncAccessHandle` in a
worker, gives synchronous file IO in the browser — which matters, because
`ObjectStore` is a synchronous trait and making it async would touch every
caller.

⛔ **OPFS has no atomic create-if-absent.** `getFileHandle(name, {create:true})`
does not report whether it created. Probe-then-create is a TOCTOU race, which
is precisely the `LostWrite.cfg` dial.

This is not fatal, but it must be met head-on rather than papered over. The
honest resolution is that **on-device is a single-writer model by
construction** — one `SharedWorker` (or a Web Locks `navigator.locks` holder)
owns the journal, and every tab posts to it. `put_if_absent` then becomes
atomic because there is exactly one writer, not because the filesystem said so.

That is a *different* safety argument, and it needs its own TLA+ spec
(`tla/DeviceJournal.tla` or similar) establishing that single-writer exclusion
gives the same `NoWriteIsLost` the conditional PUT gives. Shipping the OPFS
adapter without it would be asserting the property the repository's whole
method exists to avoid asserting.

### 3.6 RocksDB — **recommend against**

Technically capable (`OptimisticTransactionDB` gives a real compare-and-set),
but it is a large C++ dependency inside a hermetic Bazel build maintained
part-time. That is the exact objection PLAN used to refuse SQLite, and nothing
about RocksDB weakens it.

If an embedded key-value store is wanted on desktop, **`redb` or `fjall` —
pure Rust, real transactions — deliver the same atomic claim with none of the
toolchain cost.** `redb`'s write transaction is a genuine `put_if_absent`.

### 3.7 SQLite — **same objection, better mitigations**

`libsqlite3-sys` wants a `cc` toolchain; `rusqlite`'s bundled feature makes it
worse, not better. Pure-Rust reimplementations exist but are not a foundation
for a financial book of record. If a single-file embedded store on desktop is
the goal, §3.6's pure-Rust options are the better trade.

### 3.8 IndexedDB — **not needed**

OPFS supersedes it for this use, and IndexedDB's async-only API would force
the `ObjectStore` trait async. Keep it as a fallback only for browsers where
OPFS sync access handles are unavailable, and accept the async cost there.

---

## 4. The proposed distribution model

### 4.1 Tier 1 — Personal, on-device

**Shape.** The kernel, `ratio-store`, `ratio-project`, `ratio-rules`,
`ratio-nav` and `ratio-console` compiled to `wasm32-unknown-unknown`, running
in the browser or in a desktop shell. Journal and side planes in OPFS behind a
single-writer worker. Configuration content-addressed in the same store.
Projection folded in memory, with a checkpoint blob in OPFS.

**Why this is plausible and not wishful:** the core crates depend only on
`anyhow`, `serde`, `serde_json`, `sha2`, `toml`, and `prost`. No C
dependencies. No `tokio`. The AWS SDK and the async runtime are confined to
`crates/ratio`, the binary. `ratio-console` — which is already the 40-RPC API
surface the Next.js console consumes — is pure Rust plus prost, so the same
RPC surface can be served in-process by the wasm module instead of over HTTP.
The client does not need rewriting so much as re-pointing.

**Our marginal cost per user:** storage of one opaque encrypted blob, if the
user opts into backup. No compute. No per-user database. That is the cost
structure being asked for.

**What cannot go on-device:** Plaid `Item` tokens and Google refresh tokens.
`connect/bank-feed/` and `connect/calendar-bills/` currently seal these
AES-256-GCM per membership in a local runner. A browser cannot hold a Plaid
Item token safely, and Plaid's exchange requires a server-side secret. The
minimum server-side residue for Tier 1 is a **stateless token broker** —
exchange, refresh, revoke — that never sees book content. That is a real,
irreducible piece of hosted infrastructure, and it should be scoped and priced
as such rather than discovered later.

**Licensing note.** Shipping a fully client-side wasm build is *conveying*
under AGPL-3.0, so the corresponding source must be offered — which it already
is. It also means the artifact is trivially self-hostable by anyone. Per
LICENSING.md that is the intended posture, but it is worth stating explicitly
for a track whose entire deliverable is a downloadable bundle.

### 4.2 Tier 2 — Shared workspaces and small funds, hosted SQL

**Shape.** Journal in Postgres (§3.2), projection in Postgres (§3.3), schema
per tenant. Aggregation native. The 30 s gateway cap stops being the binding
constraint because a cold fold is a streaming scan, not N round trips.

This is also where the multi-tenancy blocker in §5.3 must be resolved: one
process serving many books needs a store handle per book, not a process-global
`OnceLock`.

### 4.3 Tier 3 — Funds and enterprise

Tier 2's shape, dedicated or self-hosted, under the commercial license, with
the Fargate `ScaleTask` path for folds past what a request can carry. Nothing
architecturally new; it is Tier 2 with isolation and a different contract.

### 4.4 What stays identical across all three

The journal format, the prefix digest, the configuration digests, the kernel,
and the proofs. ⭐ **A Personal book folded on a phone and the same journal
folded on Fargate must produce the same digest, or the tiering is a fork
wearing one name.** A cross-tier replay-equivalence test — same journal, three
backends, one digest — is the acceptance gate for this entire programme and
should be written before the first adapter.

---

## 5. Blockers, ordered by what they block

### 5.1 Durable checkpoints — blocks *everything*, cheapest to fix

`DirectoryCheckpointStore` is `fs::` only and the console caches under a
container-local path. Move publication behind a trait with an `ObjectStore`
implementation (the blob-plus-HEAD-pointer shape already matches
`put_if_absent` + an atomic pointer) and the hosted cold-start fold collapses
from N GETs to one blob plus a tail. This is a prerequisite for hosting
Personal books *and* the single biggest serving-latency win for funds.

Severity: **high.** Effort: **low.** No new dependency, no new spec.

### 5.2 `SeqLog::height()` is O(n) per append

Add `height`/`max_key` to `ObjectStore` with a `list`-based default. Backend-
independent defect; blocks any backend from being fast. Do this before adding
a backend, not after.

Severity: **high.** Effort: **low.**

### 5.3 The process-global store install

`install_object_store` is a first-call-wins `OnceLock`. The tree carries
**225 `FileBook::open(` call sites against 33 `open_with`**. Many of the 225
are in test modules, but every production path goes through the global —
`ratio-console` (79 in `lib.rs`, 9 in `book.rs`), `ratio-project` (38),
`ratio/src/main.rs` (32), `ratio-mcp` (9), `ratio-nav` (8), `ratio-gen` (10).
One process therefore cannot serve tenant A on Postgres and tenant B on S3.
`open_with` proves the plumbing exists; threading a handle through is
mechanical but not small, and it is a hard prerequisite for Tier 2
multi-tenancy.

Severity: **high for Tier 2, none for Tier 1.** Effort: **medium, mechanical.**

### 5.4 `FileBook`'s residual `std::fs`

Even with an `ObjectStore` installed, `accounts.json` is local
(`lib.rs:1530`), `bootstrap::materialize` writes a local verified cache, and
unpublished books keep `config/` on disk via `DirectoryConfigStore`. Published
books already route configuration through `ControlStore` on the object store,
so the pattern is half-established. wasm32 has no filesystem; the remaining
`fs::` paths must route through the store or the target does not build.

Severity: **blocking for Tier 1.** Effort: **medium.**

### 5.5 The wasm build target

`rules_rust` supports wasm32, and the Lean-emitted Rust is ordinary Rust. But
the current build pulls tonic/prost for a real gRPC server, and MODULE.bazel
records that the sibling project meridian needed a "wasm-constrained
tonic-free override." A `ratio-core-wasm` target excluding tonic, the AWS SDK
and tokio is required. Precedent exists; the work does not.

Severity: **blocking for Tier 1.** Effort: **medium.**

### 5.6 wasm32 is 32-bit

~2 GB practical address space. Irrelevant for Personal — a household book's
projection is single-digit MB by §2. A hard ceiling for funds, where one view
at 20M lots is ~640 MB and nobody has measured two. ⭐ **This is not only a
constraint, it is a confirmation: the memory numbers already imply the split
between on-device Personal and hosted funds. The tiering follows the
measurements rather than the marketing.**

### 5.7 The OPFS atomicity gap needs a spec, not a shrug

Per §3.5. Single-writer exclusion is a sound argument and a *different* one
from conditional PUT. It needs its own TLA+ model before it is relied on.

Severity: **blocking for Tier 1 correctness claims.** Effort: **low code,
real thought.**

### 5.8 `psql` as the Postgres client

Blocks pooling, blocks pglite, blocks any serving story past the demo. Split
`pg.rs`'s exec behind a trait.

Severity: **high for Tier 2.** Effort: **low–medium.**

### 5.9 Connect token custody has no on-device answer

Per §4.1. Plaid and Google credentials cannot live in a browser. Scope the
stateless broker explicitly.

Severity: **blocking for the Personal product, not for the Personal
kernel.** Effort: **medium, and it is operational as much as it is code.**

### 5.10 Recovery objectives are unagreed

`docs/recovery.md` records that RPO, RTO, backup frequency, retention, and
named operators are **all unagreed**, and that NAV strikes, post-create
membership, and later configuration promotions are still local for legacy
books. Multiplying the number of backends before these are agreed multiplies
the number of restore procedures that do not exist. ⚠ **Do not add a backend
before #264 has answers; add the backend to the answer.**

Severity: **process, and the one most likely to be skipped.**

---

## 6. Suggested sequence

Nothing here is dispatchable until it is split into issues with acceptance
criteria, per the roadmap's own rules.

1. **Backend-independent fixes.** Durable checkpoint store (§5.1); `height` on
   `ObjectStore` (§5.2). Both improve the deployed product today regardless of
   which tier ever ships.
2. **The equivalence gate.** A test that folds one journal through every
   available backend and asserts one prefix digest (§4.4). Write it while there
   are two backends, so it is cheap; it becomes the contract every later
   adapter passes.
3. **Decide the tiering** against PLAN, as an amendment. §5.6 argues the
   measurements already point at it, but that is an argument, not a decision.
4. **Tier 2 first, not Tier 1.** Postgres journal + library client + per-book
   store handles (§5.2, §5.3, §5.8, §3.2). It serves the existing pilot
   customer path, it needs no wasm toolchain, and it retires the cold-start
   problem for every tier.
5. **Tier 1 afterwards.** wasm target, `fs::` removal, OPFS adapter with its
   spec, token broker (§5.4–§5.7, §5.9).

⛔ Taking these in the opposite order — wasm first, because it is the more
interesting problem — spends the toolchain budget before the storage seam is
ready to carry a second backend, and leaves the hosted read path broken for
whichever customer arrives first.
