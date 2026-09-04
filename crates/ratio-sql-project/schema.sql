-- Stage E: derived lots, positions, and aggregates.
--
-- ⭐ journal.jsonl STAYS THE SYSTEM OF RECORD. These tables are a snapshot of
-- one journal prefix, content-addressed by `ratio_nav::prefix_digest`. Replay
-- rebuilds them. A figure that cannot name the digest it was folded from is
-- decoration, and `ratio replay` would disagree.
--
-- ⛔ ONE WATERMARK, NOT ONE PER TABLE. `//tla:sql_projection_check` is why:
-- ApplyAtomically false lets lots reach prefix 105 while positions stay at
-- 100; SnapshotRead false lets the projector commit between two statements of
-- the same figure. Both produce a NAV that never existed at any instant of
-- the journal, and both still tie. There is no per-table `at`. The prefix
-- is a property of the snapshot, not a label you can take the min of.
--
-- ⛔ ACQUIRED NULL IS UNSET, NOT A DEFAULT. A lot opened by an entry with no
-- trade date cannot be classified short- or long-term. The epoch and today
-- are wrong in opposite directions. Holding-period methods refuse.
--
-- ⛔ ORDER BY seq IS NOT FIFO RELIEF. Seq is the acquisition ordinal the
-- proved engine sorts by when the fund elected FIFO. A planner-style scan
-- that takes the head of that index under a HIFO / LIFO / LOFO book is the
-- silent SQL FIFO `//tla:stale_method_relief_check` exists to catch.
-- Relief reads load the rows and call `ratio_project::relief::relieve_by`
-- under the method the entry's own configuration named. MinTax, SpecID,
-- average cost, and wash are elections with their own shape — not a
-- Method / Order / relief-method variant, and not a SQL ORDER BY.
--
-- ⚠ A DATABASE DOES NOT CHANGE `Ratio.Exec`. IO adds up across partitions
-- and only CPU divides; no plan beats the IO floor. These tables make a
-- prefix addressable. They do not invent a cheaper fold.
--
-- Planner pushdown vs Pg.Rel.Semantics is `src/plan.rs` / Ratio.Sql.Pushdown.
-- Leftover on #159: the measured 20M-lot claim. Applying this file to a
-- live engine is `PgProjection` (psql). Console / API reads go through
-- `ProjectionReads` when RATIO_PG_URL is set — the journal stays the
-- system of record.
--
-- ⛔ PRIMARY KEY CANNOT HOLD A NULL. Postgres makes every PK column NOT NULL,
-- so `instrument` / `currency` as a PK would refuse the rest map and an
-- unset currency — both are NULL-as-unset, same as `acquired`. UNIQUE
-- NULLS NOT DISTINCT is the uniqueness the denotational store already has.

CREATE TABLE projection_watermark (
    book_id         text PRIMARY KEY,
    journal_prefix  bigint NOT NULL CHECK (journal_prefix >= 0),
    journal_digest  text NOT NULL
);

CREATE TABLE lots (
    book_id     text NOT NULL REFERENCES projection_watermark (book_id),
    view_id     text NOT NULL,
    dim         bigint NOT NULL,
    instrument  text NOT NULL,
    seq         bigint NOT NULL,
    units       bigint NOT NULL,
    cost        bigint NOT NULL,
    acquired    integer,
    PRIMARY KEY (book_id, view_id, dim, instrument, seq)
);

CREATE TABLE positions (
    book_id     text NOT NULL REFERENCES projection_watermark (book_id),
    view_id     text NOT NULL,
    dim         bigint NOT NULL,
    instrument  text,
    cost        bigint NOT NULL,
    quantity    bigint NOT NULL,
    UNIQUE NULLS NOT DISTINCT (book_id, view_id, dim, instrument)
);

-- debit / credit are the fold's i128 accumulators, stored as text so a
-- wrapped bigint cannot look like a NAV. `Ratio.Bounded` is why.
CREATE TABLE aggregates (
    book_id     text NOT NULL REFERENCES projection_watermark (book_id),
    view_id     text NOT NULL,
    dim         bigint NOT NULL,
    currency    text,
    debit       text NOT NULL,
    credit      text NOT NULL,
    postings    bigint NOT NULL,
    UNIQUE NULLS NOT DISTINCT (book_id, view_id, dim, currency)
);
