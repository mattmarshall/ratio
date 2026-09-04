//! Live Postgres adapter for the Stage E projection schema.
//!
//! ⭐ THE FOLD IS STILL `Projection`. This module applies [`crate::SCHEMA_SQL`]
//! and writes the same snapshot [`crate::SqlProjection`] holds, in one
//! transaction. Relief still calls [`ratio_project::relief::relieve_by`]. The
//! journal stays the system of record.
//!
//! ⚠ TALKS TO THE SERVER THROUGH `psql`. A rust-postgres crate would be a
//! crate_universe member this package is not — the crate stays off the Cargo
//! workspace members list (same as #229). The client is the one CI and a local
//! install already ship. Planner pushdown vs `Pg.Rel.Semantics` stays leftover.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use ratio_project::{relief, AsOf};

use crate::{
    fold_book_snapshot, refuse_replay_onto, AggregateRow, JournalPin, PositionRow, Snapshot,
    Watermark, SCHEMA_SQL,
};

/// A Postgres that holds one book's snapshot under one watermark.
///
/// ⛔ NOT THE SYSTEM OF RECORD. Replay rebuilds the tables from `journal.jsonl`.
/// ⚠ ONE SCHEMA PER CONNECTION so tests do not share a search_path, and so a
/// leftover row from another book cannot look like this one.
pub struct PgProjection {
    url: String,
    schema: String,
}

impl PgProjection {
    /// Probe `psql` and the server. The named schema is created by
    /// [`Self::apply_schema`], not here — applying the contract is a
    /// deliberate step, not a connect side-effect.
    pub fn connect(url: &str, schema: &str) -> Result<Self> {
        if schema.is_empty()
            || !schema
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            bail!(
                "schema {schema:?} must be [a-z0-9_]+ — a quoted identifier would be a second \
                 way to name the same tables"
            );
        }
        let this = Self {
            url: url.to_string(),
            schema: schema.to_string(),
        };
        this.exec_raw("SELECT 1")?;
        Ok(this)
    }

    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Apply [`crate::SCHEMA_SQL`] when `schema` is missing.
    ///
    /// ⚠ NOT A SECOND APPLY. [`Self::apply_schema`] still fails if the
    /// tables already exist — leftover rows from a previous shape must not
    /// look like this book's snapshot. First-use on an empty search_path is
    /// the one case a console process may do for itself.
    pub fn ensure_schema(&self) -> Result<()> {
        let out = self.exec_raw(&format!(
            "SELECT 1 FROM information_schema.schemata WHERE schema_name = {}",
            lit(&self.schema)
        ))?;
        if first_line(&out).is_none() {
            self.apply_schema()?;
        }
        Ok(())
    }

    /// Apply [`crate::SCHEMA_SQL`] into `schema`.
    ///
    /// ⛔ NOT IF-NOT-EXISTS ON THE TABLES. A second apply on the same schema
    /// must fail rather than silently keep leftover rows from a previous shape.
    pub fn apply_schema(&self) -> Result<()> {
        self.exec_raw(&format!(
            "CREATE SCHEMA {};\nSET search_path TO {}, public;\n{}",
            self.schema, self.schema, SCHEMA_SQL
        ))?;
        Ok(())
    }

    /// Drop the schema. Tests only, so a rerun does not see yesterday's rows.
    pub fn drop_schema(&self) -> Result<()> {
        self.exec_raw(&format!(
            "DROP SCHEMA IF EXISTS {} CASCADE",
            self.schema
        ))?;
        Ok(())
    }

    pub fn watermark(&self, book_id: &str) -> Result<Option<Watermark>> {
        let out = self.exec(&format!(
            "SELECT journal_prefix, journal_digest FROM projection_watermark WHERE book_id = {}",
            lit(book_id)
        ))?;
        let Some(line) = first_line(&out) else {
            return Ok(None);
        };
        let (prefix, digest) = split2(&line)?;
        Ok(Some(Watermark {
            book_id: book_id.to_string(),
            prefix: parse_usize(&prefix)?,
            digest,
        }))
    }

    /// Fold `path`'s journal through the proved projection and replace every
    /// table for `book_id` in one transaction.
    pub fn replay_book(&self, book_id: &str, path: &Path) -> Result<Watermark> {
        let snap = fold_book_snapshot(book_id, path)?;
        let pin = JournalPin {
            prefix: snap.watermark.prefix,
            digest: snap.watermark.digest.clone(),
        };
        let have = self.watermark(book_id)?;
        refuse_replay_onto(have.as_ref(), &pin, book_id)?;
        self.commit(&snap)
    }

    pub fn require_caught_up(&self, book_id: &str, pin: &JournalPin) -> Result<Watermark> {
        let have = self.watermark(book_id)?.ok_or_else(|| {
            anyhow::anyhow!(
                "projection {book_id} has not been replayed; refusing a figure from an \
                 empty snapshot rather than answering with zeros that look like a fund"
            )
        })?;
        if have.prefix != pin.prefix || have.digest != pin.digest {
            bail!(
                "projection {book_id} lags or leads the journal: snapshot prefix {} \
                 digest {}, journal prefix {} digest {}. A stale projection must refuse, \
                 not answer with a figure from a different prefix. \
                 `//tla:unpinned_projection_check`",
                have.prefix,
                have.digest,
                pin.prefix,
                pin.digest
            );
        }
        Ok(have)
    }

    pub fn lots_of(
        &self,
        book_id: &str,
        view: &str,
        dim: i64,
        instrument: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<relief::Lot>>> {
        let mark = self.require_caught_up(book_id, pin)?;
        // ⚠ PSQL -A DROPS A TRAILING NULL, so `acquired` last would look like
        // a 3-column row and invent a default. Empty string is unset.
        let out = self.exec(&format!(
            "SELECT seq, units, cost, COALESCE(acquired::text, '') FROM lots \
             WHERE book_id = {} AND view_id = {} AND dim = {dim} AND instrument = {} \
             ORDER BY seq",
            lit(book_id),
            lit(view),
            lit(instrument)
        ))?;
        let mut lots = Vec::new();
        for line in lines(&out) {
            let cols = split_tabs(&line);
            // ⚠ PSQL -A ALSO DROPS A TRAILING EMPTY FIELD, so COALESCE to ''
            // still arrives as three columns. That is unset, not a default day.
            let acquired = match cols.len() {
                3 => None,
                4 => parse_acquired(&cols[3])?,
                n => bail!("lots row had {n} columns, not 3 or 4: {line:?}"),
            };
            lots.push(relief::Lot {
                seq: parse_u64(&cols[0])?,
                units: parse_i64(&cols[1])?,
                cost: parse_i64(&cols[2])?,
                acquired,
            });
        }
        Ok(AsOf {
            value: lots,
            prefix: mark.prefix,
            view: view.to_string(),
            through: None,
        })
    }

    pub fn positions(
        &self,
        book_id: &str,
        view: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<PositionRow>>> {
        let mark = self.require_caught_up(book_id, pin)?;
        let out = self.exec(&format!(
            "SELECT dim, instrument, cost, quantity FROM positions \
             WHERE book_id = {} AND view_id = {}",
            lit(book_id),
            lit(view)
        ))?;
        let mut rows = Vec::new();
        for line in lines(&out) {
            let cols = split_tabs(&line);
            if cols.len() != 4 {
                bail!("positions row had {} columns, not 4: {line:?}", cols.len());
            }
            rows.push(PositionRow {
                view: view.to_string(),
                dim: parse_i64(&cols[0])?,
                instrument: empty_is_none(&cols[1]),
                cost: parse_i64(&cols[2])?,
                quantity: parse_i64(&cols[3])?,
            });
        }
        Ok(AsOf {
            value: rows,
            prefix: mark.prefix,
            view: view.to_string(),
            through: None,
        })
    }

    pub fn aggregates(
        &self,
        book_id: &str,
        view: &str,
        pin: &JournalPin,
    ) -> Result<AsOf<Vec<AggregateRow>>> {
        let mark = self.require_caught_up(book_id, pin)?;
        let out = self.exec(&format!(
            "SELECT dim, currency, debit, credit, postings FROM aggregates \
             WHERE book_id = {} AND view_id = {}",
            lit(book_id),
            lit(view)
        ))?;
        let mut rows = Vec::new();
        for line in lines(&out) {
            let cols = split_tabs(&line);
            if cols.len() != 5 {
                bail!("aggregates row had {} columns, not 5: {line:?}", cols.len());
            }
            rows.push(AggregateRow {
                view: view.to_string(),
                dim: parse_i64(&cols[0])?,
                currency: empty_is_none(&cols[1]),
                debit: parse_i128(&cols[2])?,
                credit: parse_i128(&cols[3])?,
                postings: parse_i64(&cols[4])?,
            });
        }
        Ok(AsOf {
            value: rows,
            prefix: mark.prefix,
            view: view.to_string(),
            through: None,
        })
    }

    /// Relieve `want` units under the elected method, from the snapshot.
    ///
    /// ⛔ THE WALK IS `relieve_by`, NOT A SEQ SCAN. Same trap as
    /// [`crate::SqlProjection::relieve`]: physical storage is seq-keyed.
    pub fn relieve(
        &self,
        book_id: &str,
        view: &str,
        dim: i64,
        instrument: &str,
        method: relief::Method,
        want: i64,
        pin: &JournalPin,
    ) -> Result<AsOf<relief::Relieved>> {
        let lots = self.lots_of(book_id, view, dim, instrument, pin)?;
        let relieved = relief::relieve_by(method, &lots.value, want)?;
        Ok(AsOf {
            value: relieved,
            prefix: lots.prefix,
            view: lots.view,
            through: lots.through,
        })
    }

    fn commit(&self, snap: &Snapshot) -> Result<Watermark> {
        self.exec(&commit_sql(snap)?)?;
        Ok(snap.watermark.clone())
    }

    fn exec(&self, sql: &str) -> Result<String> {
        self.exec_raw(&format!(
            "SET search_path TO {}, public;\n{sql}",
            self.schema
        ))
    }

    fn exec_raw(&self, sql: &str) -> Result<String> {
        run_psql(&self.url, sql)
    }
}

/// The SQL a live engine applies for one snapshot. One transaction, replace
/// every table, then the watermark. Tests inspect this without a server.
///
/// ⛔ DELETE CHILDREN BEFORE THE WATERMARK. The FK is `REFERENCES
/// projection_watermark (book_id)`. Insert watermark before children.
pub(crate) fn commit_sql(snap: &Snapshot) -> Result<String> {
    let book = lit(&snap.watermark.book_id);
    let mut sql = String::from("BEGIN;\n");
    sql.push_str(&format!("DELETE FROM lots WHERE book_id = {book};\n"));
    sql.push_str(&format!("DELETE FROM positions WHERE book_id = {book};\n"));
    sql.push_str(&format!("DELETE FROM aggregates WHERE book_id = {book};\n"));
    sql.push_str(&format!(
        "DELETE FROM projection_watermark WHERE book_id = {book};\n"
    ));
    sql.push_str(&format!(
        "INSERT INTO projection_watermark (book_id, journal_prefix, journal_digest) \
         VALUES ({book}, {}, {});\n",
        snap.watermark.prefix,
        lit(&snap.watermark.digest)
    ));
    for ((_, view, dim, inst, seq), lot) in &snap.lots {
        if *seq > i64::MAX as u64 {
            bail!("lot seq {seq} does not fit bigint");
        }
        let acquired = match lot.acquired {
            None => "NULL".to_string(),
            Some(d) => d.to_string(),
        };
        sql.push_str(&format!(
            "INSERT INTO lots (book_id, view_id, dim, instrument, seq, units, cost, acquired) \
             VALUES ({book}, {}, {dim}, {}, {seq}, {}, {}, {acquired});\n",
            lit(view),
            lit(inst),
            lot.units,
            lot.cost
        ));
    }
    for ((_, view, dim, inst), (cost, qty)) in &snap.positions {
        sql.push_str(&format!(
            "INSERT INTO positions (book_id, view_id, dim, instrument, cost, quantity) \
             VALUES ({book}, {}, {dim}, {}, {cost}, {qty});\n",
            lit(view),
            lit_opt(inst.as_deref())
        ));
    }
    for ((_, view, dim, cur), (debit, credit, postings)) in &snap.aggregates {
        sql.push_str(&format!(
            "INSERT INTO aggregates (book_id, view_id, dim, currency, debit, credit, postings) \
             VALUES ({book}, {}, {dim}, {}, {}, {}, {postings});\n",
            lit(view),
            lit_opt(cur.as_deref()),
            lit(&debit.to_string()),
            lit(&credit.to_string())
        ));
    }
    sql.push_str("COMMIT;\n");
    Ok(sql)
}

fn run_psql(url: &str, sql: &str) -> Result<String> {
    let output = Command::new("psql")
        .args([
            "-v",
            "ON_ERROR_STOP=1",
            "-X",
            "-q",
            "-A",
            "-t",
            "-F",
            "\t",
            "--no-psqlrc",
            url,
            "-c",
            sql,
        ])
        .output()
        .map_err(|e| {
            anyhow::anyhow!(
                "psql is not runnable ({e}). Install postgresql-client and point \
                 RATIO_PG_URL at a server — DEVELOPING.md"
            )
        })?;
    if !output.status.success() {
        bail!(
            "psql refused:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn lit_opt(s: Option<&str>) -> String {
    match s {
        None => "NULL".to_string(),
        Some(v) => lit(v),
    }
}

fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn first_line(s: &str) -> Option<String> {
    lines(s).into_iter().next()
}

fn split_tabs(s: &str) -> Vec<String> {
    s.split('\t').map(|c| c.to_string()).collect()
}

fn split2(s: &str) -> Result<(String, String)> {
    let cols = split_tabs(s);
    if cols.len() != 2 {
        bail!("expected 2 columns, got {}: {s:?}", cols.len());
    }
    Ok((cols[0].clone(), cols[1].clone()))
}

fn empty_is_none(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn parse_acquired(s: &str) -> Result<Option<relief::Day>> {
    if s.is_empty() {
        return Ok(None);
    }
    Ok(Some(s.parse::<relief::Day>().map_err(|e| {
        anyhow::anyhow!("acquired {s:?} is not a day: {e}")
    })?))
}

fn parse_usize(s: &str) -> Result<usize> {
    s.parse().map_err(|e| anyhow::anyhow!("not a prefix {s:?}: {e}"))
}

fn parse_u64(s: &str) -> Result<u64> {
    s.parse().map_err(|e| anyhow::anyhow!("not a u64 {s:?}: {e}"))
}

fn parse_i64(s: &str) -> Result<i64> {
    s.parse().map_err(|e| anyhow::anyhow!("not an i64 {s:?}: {e}"))
}

fn parse_i128(s: &str) -> Result<i128> {
    s.parse().map_err(|e| anyhow::anyhow!("not an i128 {s:?}: {e}"))
}
