//! Measured Stage E fold of the HANDOFF 20M-lot geometry.
//!
//! # Why this module exists
//!
//! PLAN's scale claim is a visitor who can RUN a twenty-million-lot fold,
//! not a recorded cold build of a 140-million-entry journal. The geometry
//! HANDOFF names is 10,000 securities × 2,000 lots = 20,000,000 rows
//! (~640 MB), not `ratio closure`'s 500 × 40,000 dial. This module
//! generates that projection, folds relief and aggregates under a
//! content-addressed digest, and publishes a report a console or CI
//! step can cite.
//!
//! # ⛔ This is not the 40GB journal
//!
//! `ratio-gen` at the full shape writes ~140 million entries (~40 GB).
//! That fold stays on Fargate ScaleTask / ScaleBucket — this process
//! does not have those secrets and must not pretend it ran that task.
//! The journal remains the system of record. This path loads the
//! **open-lot projection** HANDOFF already said the fold is bounded by.
//! `Ratio.Exec` still holds: a database does not change the IO floor.
//!
//! # ⛔ Relief is `relieve_by`, not `ORDER BY seq`
//!
//! Physical storage is seq-keyed. A silent SQL FIFO is
//! `//tla:stale_method_relief_check`. Every security is relieved under
//! HIFO through [`ratio_project::relief::relieve_by`]. The first
//! holding is also walked in seq order so the report can show the two
//! costs are different. Acquired stays unset — not a default day.
//! MinTax, SpecID, average cost, and wash stay elections.
//!
//! # ⛔ `load_scale` is not the 20M claim
//!
//! [`SqlProjection::load_scale`] and [`PgProjection::load_scale`] hold
//! every row. They refuse above [`Geometry::LOAD_ROW_CAP`] so a caller
//! cannot "measure 20M" by allocating a BTreeMap of string keys and
//! calling that a table. The 20M claim is [`fold`].

use std::time::Instant;

use anyhow::{anyhow, bail, Result};
use ratio_project::relief;
use ratio_rules::UNDECLARED_VIEW as VIEW;
use ratio_store::DigestBuilder;

use crate::{JournalPin, Snapshot, SqlProjection, Watermark};

/// HANDOFF's recorded twenty-million-lot shape: 10,000 × 2,000, not 500 × 40,000.
pub const HANDOFF_SECURITIES: u32 = 10_000;
pub const HANDOFF_LOTS_PER: u32 = 2_000;

/// Units on every generated lot. A whole-lot take divides exactly —
/// `Ratio.Lots.partial_relief_is_exactly_pro_rata` refuses a remainder,
/// and a scale bench that rounded would be measuring the wrong walk.
pub const LOT_UNITS: i64 = 10;

/// Base cost of seq 0. Each later lot is this much dearer, so HIFO and
/// a seq scan cannot agree.
pub const LOT_COST_STEP: i64 = 1_000;

/// The geometry a fold names. Dialled values must be the pair, not only
/// the product — 500 × 40,000 is also twenty million lots and is a
/// different fund (`//:scale_shapes_test`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub securities: u32,
    pub lots_per: u32,
}

impl Geometry {
    /// The HANDOFF row. The only shape the 20M claim may quote.
    pub const HANDOFF: Self = Self {
        securities: HANDOFF_SECURITIES,
        lots_per: HANDOFF_LOTS_PER,
    };

    /// Rows [`SqlProjection::load_scale`] / [`PgProjection::load_scale`]
    /// will hold. Above this, use [`fold`].
    pub const LOAD_ROW_CAP: u64 = 50_000;

    pub fn lots(self) -> u64 {
        u64::from(self.securities) * u64::from(self.lots_per)
    }

    pub fn instrument(self, security: u32) -> String {
        format!("s{security:05}")
    }
}

/// What a measured fold publishes. Timing is this machine, this run;
/// the digest is deterministic and is the citation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub securities: u32,
    pub lots_per: u32,
    pub lots: u64,
    pub digest: String,
    pub fold_ms: u128,
    pub relieve_ms: u128,
    pub hifo_cost: i64,
    pub fifo_cost: i64,
    pub seq_scan_cost: i64,
    pub aggregate_debit: i128,
    pub aggregate_units: i64,
}

impl Report {
    /// JSON a console or CI step can cite. Written by hand so this
    /// crate does not grow a serde member (it stays off the workspace).
    pub fn to_json(&self) -> String {
        format!(
            "{{\n\
             \x20\x20\"securities\": {},\n\
             \x20\x20\"lots_per\": {},\n\
             \x20\x20\"lots\": {},\n\
             \x20\x20\"geometry\": \"{} x {}\",\n\
             \x20\x20\"digest\": \"{}\",\n\
             \x20\x20\"fold_ms\": {},\n\
             \x20\x20\"relieve_ms\": {},\n\
             \x20\x20\"hifo_cost\": {},\n\
             \x20\x20\"fifo_cost\": {},\n\
             \x20\x20\"seq_scan_cost\": {},\n\
             \x20\x20\"aggregate_debit\": {},\n\
             \x20\x20\"aggregate_units\": {},\n\
             \x20\x20\"acquired\": \"unset\",\n\
             \x20\x20\"relief\": \"relieve_by\",\n\
             \x20\x20\"journal\": \"projection load of the HANDOFF geometry; \
not the 140M-entry / 40GB journal. journal.jsonl stays SoR\"\n\
             }}",
            self.securities,
            self.lots_per,
            self.lots,
            self.securities,
            self.lots_per,
            self.digest,
            self.fold_ms,
            self.relieve_ms,
            self.hifo_cost,
            self.fifo_cost,
            self.seq_scan_cost,
            self.aggregate_debit,
            self.aggregate_units,
        )
    }
}

/// One security's open lots. Cheap at seq 0, dear at the tail, units
/// identical so HIFO is per-unit and per-lot the same walk.
///
/// ⚠ `acquired` IS UNSET. A default day would classify every lot and
/// hide the election. Holding-period methods refuse this holding;
/// HIFO / FIFO do not read the date.
pub fn lots_for(security: u32, lots_per: u32) -> Vec<relief::Lot> {
    let mut lots = Vec::with_capacity(lots_per as usize);
    for seq in 0..lots_per {
        lots.push(relief::Lot {
            seq: u64::from(seq),
            units: LOT_UNITS,
            cost: lot_cost(security, seq),
            acquired: None,
        });
    }
    lots
}

fn lot_cost(security: u32, seq: u32) -> i64 {
    LOT_COST_STEP + i64::from(seq) * LOT_COST_STEP + i64::from(security)
}

/// Canonical bytes one lot contributes to the projection digest.
///
/// ⛔ SECURITY INDEX IS PART OF THE DIGEST. Two geometries with the same
/// lots-per and a different security count must not collide, and a
/// fold that hashed only `(seq, units, cost)` would.
fn absorb_lot(digest: &mut DigestBuilder, security: u32, lot: &relief::Lot) {
    digest.update(&security.to_le_bytes());
    digest.update(&lot.seq.to_le_bytes());
    digest.update(&lot.units.to_le_bytes());
    digest.update(&lot.cost.to_le_bytes());
    // 0xff = acquired unset. A default 0 would look like the epoch.
    digest.update(&[0xff]);
}

/// What a silent `ORDER BY seq` take would cost. Not a relief method.
///
/// ⛔ WHOLE LOTS ONLY. The proved walk refuses a remainder; this
/// stand-in is the bug, and it must not grow a rounding rule we then
/// have to defend.
fn seq_scan_take(lots: &[relief::Lot], want: i64) -> Result<i64> {
    let mut left = want;
    let mut cost = 0i64;
    let mut ordered = lots.to_vec();
    ordered.sort_by_key(|l| l.seq);
    for lot in ordered {
        if left == 0 {
            break;
        }
        if left < lot.units {
            bail!("seq scan wanted a partial of lot {} — the scale bench takes whole lots", lot.seq);
        }
        cost = cost
            .checked_add(lot.cost)
            .ok_or_else(|| anyhow::anyhow!("seq-scan cost overflow"))?;
        left -= lot.units;
    }
    if left != 0 {
        bail!("seq scan wanted {want} and the holding was short");
    }
    Ok(cost)
}

/// Fold `geometry`: digest every lot, relieve every holding under HIFO,
/// accumulate aggregates. Never holds more than one security's lots.
///
/// ⭐ THE 20M CLAIM. [`Geometry::HANDOFF`] is twenty million lots.
/// Timing is this run; the digest is the citation.
pub fn fold(geometry: Geometry) -> Result<Report> {
    if geometry.securities == 0 || geometry.lots_per == 0 {
        bail!("a fold of {} × {} is not a fold", geometry.securities, geometry.lots_per);
    }
    let t0 = Instant::now();
    let mut digest = DigestBuilder::new();
    digest.update(b"ratio-sql-project/fold_scale/v1\n");
    digest.update(format!("securities={}\n", geometry.securities).as_bytes());
    digest.update(format!("lots_per={}\n", geometry.lots_per).as_bytes());

    let mut debit = 0i128;
    let mut units = 0i64;
    let mut relieve_ns = 0u128;
    let mut first_hifo = None;
    let mut first_fifo = None;
    let mut first_seq = None;

    for security in 0..geometry.securities {
        let lots = lots_for(security, geometry.lots_per);
        for lot in &lots {
            absorb_lot(&mut digest, security, lot);
            debit += i128::from(lot.cost);
            units = units
                .checked_add(lot.units)
                .ok_or_else(|| anyhow::anyhow!("unit overflow"))?;
        }
        let want = LOT_UNITS;
        let tr = Instant::now();
        let hifo = relief::relieve_by(relief::Method::Hifo, &lots, want)?;
        relieve_ns += tr.elapsed().as_nanos();
        if security == 0 {
            let fifo = relief::relieve_by(relief::Method::Fifo, &lots, want)?;
            let seq = seq_scan_take(&lots, want)?;
            first_hifo = Some(hifo.cost);
            first_fifo = Some(fifo.cost);
            first_seq = Some(seq);
        }
    }

    let hifo_cost = first_hifo.expect("securities > 0");
    let fifo_cost = first_fifo.expect("securities > 0");
    let seq_scan_cost = first_seq.expect("securities > 0");
    if hifo_cost == seq_scan_cost {
        bail!(
            "HIFO cost {hifo_cost} matched the seq scan — the geometry is not \
             cheap-then-dear, so the bench cannot refuse a silent FIFO"
        );
    }
    if hifo_cost == fifo_cost {
        bail!("HIFO cost {hifo_cost} matched FIFO — the walk is not distinguishing methods");
    }

    Ok(Report {
        securities: geometry.securities,
        lots_per: geometry.lots_per,
        lots: geometry.lots(),
        digest: digest.finish().as_str().to_string(),
        fold_ms: t0.elapsed().as_millis(),
        relieve_ms: relieve_ns / 1_000_000,
        hifo_cost,
        fifo_cost,
        seq_scan_cost,
        aggregate_debit: debit,
        aggregate_units: units,
    })
}

/// Build the four tables for a geometry small enough to hold.
///
/// ⛔ NOT THE 20M PATH. The watermark digest is the same function
/// [`fold`] publishes, so a store that loaded these rows can be pinned
/// by a report. Prefix is the lot count — this is a projection load,
/// not a journal height.
pub(crate) fn snapshot(book_id: &str, geometry: Geometry) -> Result<Snapshot> {
    if geometry.lots() > Geometry::LOAD_ROW_CAP {
        bail!(
            "load_scale holds every row ({} lots); the 20M claim is fold(), \
             which never materializes the string-keyed store. Cap is {}",
            geometry.lots(),
            Geometry::LOAD_ROW_CAP
        );
    }
    let report = fold(geometry)?;
    let pin = JournalPin {
        prefix: geometry.lots() as usize,
        digest: report.digest.clone(),
    };
    let mut lots = std::collections::BTreeMap::new();
    let mut positions = std::collections::BTreeMap::new();
    let mut aggregates = std::collections::BTreeMap::new();
    let book = book_id.to_string();
    let view = VIEW.to_string();
    for security in 0..geometry.securities {
        let inst = geometry.instrument(security);
        let held = lots_for(security, geometry.lots_per);
        let mut cost = 0i64;
        let mut qty = 0i64;
        for lot in held {
            cost = cost
                .checked_add(lot.cost)
                .ok_or_else(|| anyhow::anyhow!("position cost overflow"))?;
            qty = qty
                .checked_add(lot.units)
                .ok_or_else(|| anyhow::anyhow!("position qty overflow"))?;
            lots.insert(
                (book.clone(), view.clone(), 1, inst.clone(), lot.seq),
                lot,
            );
        }
        positions.insert((book.clone(), view.clone(), 1, Some(inst)), (cost, qty));
    }
    let cash = i64::try_from(report.aggregate_debit)
        .map_err(|_| anyhow!("cash rest-map does not fit i64"))?;
    positions.insert((book.clone(), view.clone(), 2, None), (-cash, 0));
    aggregates.insert(
        (book.clone(), view.clone(), 1, None),
        (report.aggregate_debit, 0, i64::from(geometry.securities)),
    );
    aggregates.insert(
        (book.clone(), view.clone(), 2, None),
        (0, report.aggregate_debit, i64::from(geometry.securities)),
    );
    Ok(Snapshot::from_parts(
        Watermark {
            book_id: book,
            prefix: pin.prefix,
            digest: pin.digest,
        },
        lots,
        positions,
        aggregates,
    ))
}

impl SqlProjection {
    /// Load a small geometry into the denotational store. Refuses the
    /// HANDOFF row — that measurement is [`fold`].
    pub fn load_scale(&mut self, book_id: &str, geometry: Geometry) -> Result<Watermark> {
        let snap = snapshot(book_id, geometry)?;
        self.commit(snap)
    }
}

impl crate::PgProjection {
    /// Load a small geometry into a live engine. Same cap as
    /// [`SqlProjection::load_scale`] — the 20M claim is [`fold`].
    pub fn load_scale(&self, book_id: &str, geometry: Geometry) -> Result<Watermark> {
        let snap = snapshot(book_id, geometry)?;
        self.commit(&snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handoff_geometry_is_ten_thousand_by_two_thousand() {
        // ⛔ NOT 500 × 40,000. Same product, a twentieth of the mark
        // cost, both captioned "twenty million tax lots".
        assert_eq!(Geometry::HANDOFF.securities, 10_000);
        assert_eq!(Geometry::HANDOFF.lots_per, 2_000);
        assert_eq!(Geometry::HANDOFF.lots(), 20_000_000);
        assert_ne!(Geometry::HANDOFF.securities, 500);
    }

    #[test]
    fn a_tiny_fold_digests_relieves_hifo_and_leaves_acquired_unset() {
        let g = Geometry {
            securities: 4,
            lots_per: 8,
        };
        let report = fold(g).unwrap();
        assert_eq!(report.lots, 32);
        assert_eq!(report.digest.len(), 64);
        assert_eq!(report.hifo_cost, lot_cost(0, 7), "HIFO takes the dear lot");
        assert_eq!(report.fifo_cost, lot_cost(0, 0), "FIFO takes the cheap lot");
        assert_eq!(report.seq_scan_cost, report.fifo_cost);
        assert_ne!(report.hifo_cost, report.seq_scan_cost);
        assert_eq!(report.aggregate_units, 32 * LOT_UNITS);
        let lots = lots_for(0, 8);
        assert!(lots.iter().all(|l| l.acquired.is_none()), "unset stays unset");
    }

    #[test]
    fn the_digest_is_deterministic_and_moves_when_the_geometry_does() {
        let a = fold(Geometry {
            securities: 3,
            lots_per: 5,
        })
        .unwrap();
        let b = fold(Geometry {
            securities: 3,
            lots_per: 5,
        })
        .unwrap();
        assert_eq!(a.digest, b.digest);
        let c = fold(Geometry {
            securities: 3,
            lots_per: 6,
        })
        .unwrap();
        assert_ne!(a.digest, c.digest, "lots-per is in the digest");
        let d = fold(Geometry {
            securities: 4,
            lots_per: 5,
        })
        .unwrap();
        assert_ne!(a.digest, d.digest, "security count is in the digest");
    }

    #[test]
    fn load_scale_pins_the_fold_digest_and_relieves_through_relieve_by() {
        let g = Geometry {
            securities: 3,
            lots_per: 6,
        };
        let report = fold(g).unwrap();
        let mut store = SqlProjection::new();
        let mark = store.load_scale("tiny", g).unwrap();
        assert_eq!(mark.digest, report.digest);
        assert_eq!(mark.prefix, g.lots() as usize);
        let pin = JournalPin {
            prefix: mark.prefix,
            digest: mark.digest.clone(),
        };
        let inst = g.instrument(0);
        let lots = store.lots_of("tiny", VIEW, 1, &inst, &pin).unwrap();
        assert_eq!(lots.value.len(), 6);
        assert!(lots.value.iter().all(|l| l.acquired.is_none()));
        let got = store
            .relieve("tiny", VIEW, 1, &inst, relief::Method::Hifo, LOT_UNITS, &pin)
            .unwrap();
        assert_eq!(got.value.cost, report.hifo_cost);
        assert_eq!(got.prefix, pin.prefix);
        let agg = store.aggregates("tiny", VIEW, &pin).unwrap();
        let debit: i128 = agg.value.iter().map(|r| r.debit).sum();
        assert_eq!(debit, report.aggregate_debit);
    }

    #[test]
    fn load_scale_refuses_the_handoff_row() {
        let err = match snapshot("no", Geometry::HANDOFF) {
            Ok(_) => panic!("load_scale must refuse the HANDOFF row"),
            Err(e) => e,
        };
        let msg = format!("{err:#}");
        assert!(msg.contains("fold()"), "{msg}");
        assert!(msg.contains("20M") || msg.contains("Cap"), "{msg}");
    }

    #[test]
    fn a_json_report_names_relieve_by_and_not_a_journal_of_140m() {
        let report = fold(Geometry {
            securities: 2,
            lots_per: 3,
        })
        .unwrap();
        let json = report.to_json();
        assert!(json.contains("\"relief\": \"relieve_by\""));
        assert!(json.contains("journal.jsonl stays SoR"));
        assert!(json.contains("40GB"));
        assert!(!json.contains("lot_method"));
        assert!(json.contains(&report.digest));
    }
}
