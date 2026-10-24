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
The implementation, pure Rust client repin, reusable live backend fixture, and
same-height S3 cold-fold comparison remain required. The execution-trait
prerequisite must merge before native journal implementation proceeds.
