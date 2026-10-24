# Postgres journal claim proof (#362)

The journal remains authoritative. A database row stores an immutable object;
projection tables remain disposable read models. A row is not a relational
ledger entry with independently decided accounting semantics.

`PostgresJournal.tla` models height observation, a transaction-private insert,
unique-key conflict resolution, commit, abort, and a committed-prefix reader.
It does not inherit the S3 LIST/conditional-PUT argument. Each writer owns one
payload. Namespace binding is fixed for one model instance; actual cross-book
binding requires backend conformance and isolation checks.

The positive two-writer/two-slot model checked 56 distinct states and passed
NoWriteIsLost, ContiguousNoHoles, NoEntryTwice, and
AcceptedReadsAreCommitted. The model uses safety properties, not a promise of
progress during database failure. Removing unique claims loses an acknowledged
write; acknowledging before commit also violates NoWriteIsLost; exposing
uncommitted rows violates AcceptedReadsAreCommitted. The executable probe
sweep checked all 45 probes and each failed for its named invariant.

Database assumptions are durable commit, a unique key that covers namespace
and object identity, and reads restricted to committed rows. Database roles,
WAL/backup configuration, service failure, and TLS deployment must be verified
operationally; this bounded model cannot establish them. An application error
or abort must never return successful append acknowledgement. Missing sequence
keys must refuse; a SQL scan must not skip them and cite a nonexistent prefix.

The proof was checked and committed before any native Postgres journal code.
At that proof commit, implementation, client repin, live conformance, and
equal-height measurement remained required. The execution-trait prerequisite
subsequently merged before native journal code was written.

Implementation follows the proof commit. The separate native crate uses
[postgres 0.19.14](https://docs.rs/postgres/0.19.14/postgres/struct.Client.html)
and its native Rust driver, not libpq or a subprocess. Repin command:
`CARGO_BAZEL_REPIN=1 bazel build //crates/ratio-postgres-store`. The committed
Cargo.lock and MODULE.bazel.lock record postgres 0.19.14, tokio-postgres 0.7.18,
and their transitive dependencies, including unified futures and wasm bindings.
The pinned Rust 1.85 toolchain built the client successfully.

The adapter enables synchronous commit and refuses fsync=off. A local-only
connector refuses remote plaintext; hosted integration must provide a
TLS-connected native client. The shared store does not gain a default database
or depend on this optional networking crate. A bounded row handoff keeps the
kernel callback on its caller thread. The repeatable-read scan observes one
committed snapshot; SeqLog additionally rejects missing, repeated, or truncated
sequences. A consumer refusal cancels the stream and remains the returned error.

The reusable live native fixture passed in 0.48 seconds on PostgreSQL 16.
It includes exact journal/all-six-plane equivalence, all prefix pins, a
concurrent unique claim, two namespaces, and no-hole refusal on all seven
planes. CI runs it explicitly on every build; the cheap registry requires that
workflow binding. Simulated backends remain a separate hermetic gate.

The complete native live gate passed 9/9 targets after implementation, and the
final native fixture passed rollback visibility, nested-runtime construction
and destruction, binary bytes, and schema identifier-length refusal. Three
fresh-process 5,396-entry missing-checkpoint samples measured 209/205/192 ms,
with the exact retained S3 journal digest and view balances. See the provider
report for placement limits and preserved evidence. Final PR CI remains.
