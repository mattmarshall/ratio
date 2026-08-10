//! ratio-gen — a fund with realistic shape, generated the same way every time.
//!
//! `Ratio.Closure.Dials` names what a period end's cost turns on: securities,
//! currencies, lots per security, open corporate actions, capital transactions.
//! This builds a book with those dials set, so the model can be checked against
//! a thing that runs rather than argued about.
//!
//! # ⛔ Deterministic, and not by a random seed that happens to be fixed
//!
//! There is no RNG crate here. Every quantity is a pure function of the dials
//! and a seed, through one integer hash, so the same dials give byte-identical
//! books on any machine and in CI. A generator whose output drifted would make
//! every measurement taken against it unreproducible — which is a strange thing
//! to accept in a system whose entire claim is reproducibility.
//!
//! # ⛔ Open lots reach a STEADY STATE. The journal does not.
//!
//! The first version of this bought and never sold, so open lots grew without
//! bound and "twenty years of fragmentation" meant twenty years of accumulation.
//! That is not how a fund behaves. Positions turn over: new lots open, old ones
//! are relieved, and the OPEN count settles at roughly turnover × holding
//! period rather than climbing forever.
//!
//! Which separates three quantities this crate had collapsed into one:
//!
//!   journal entries   MONOTONIC. Every buy and every sale, forever — an
//!                     append-only log does not forget a closed lot. This is
//!                     what a cold projection build costs, and it is O(history).
//!   open lots         STEADY STATE. What a tax report scans and what
//!                     `Ratio.Closure.lots` counts. Bounded by turnover, not by
//!                     age.
//!   the NAV           `Ratio.Closure.navCost` — neither of the above. Prices,
//!                     rates, and the maintained totals.
//!
//! Conflating the first two is the easy overclaim: "twenty million lots are
//! free" is true of the NAV and false of a cold rebuild, and the demo has to
//! show both curves.
//!
//! # ⚠ What is realistic here and what is not
//!
//! REALISTIC: buys and sells reaching a steady state of open lots; lot counts
//! and sizes drawn from a spread; several currencies with rates; a price per
//! security; corporate actions announced and left outstanding.
//!
//! NOT REALISTIC, and worth saying plainly: prices are a single observation per
//! security rather than a series, and the sale schedule is regular rather than
//! driven by anything. Those change what a NAV is WORTH. They do not change
//! what it COSTS to strike, which is what this exists to measure.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use ratio_store::{
    Account, AccountTypeRecord, AnnouncementRecord, ConfigStore, FileBook, Journal, JournalEntry,
    PostingRecord,
};

/// The shape of the fund to build. Mirrors `Ratio.Closure.Dials`.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub securities: i64,
    /// Currencies the chart is denominated in. `Ratio.Closure.fxCost` is one
    /// rate per CURRENCY, not per position — three currencies is three
    /// translations at five names or five hundred.
    pub currencies: i64,
    /// Open tax lots per security AT STEADY STATE — what remains after
    /// relieving, not what was ever bought.
    pub lots_per: i64,
    /// How many lots are opened for each one that stays open.
    ///
    /// ⛔ THE DIAL THAT SEPARATES THE JOURNAL FROM THE LOTS. At 1 nothing is
    /// ever sold and the two grow together, which is the unrealistic case the
    /// first version of this crate had. At 4, three lots in four are relieved:
    /// open lots stay flat while the journal grows four times as fast.
    pub turnover: i64,
    /// Corporate actions announced and left OUTSTANDING — never rewritten, so
    /// the factor read path carries them.
    pub open_actions: i64,
    pub capital_txns: i64,
    pub seed: u64,
}

impl Default for Shape {
    /// An S&P tracker in its twentieth year.
    fn default() -> Self {
        Self {
            securities: 500,
            currencies: 3,
            lots_per: 40,
            turnover: 4,
            open_actions: 0,
            capital_txns: 4,
            seed: 1,
        }
    }
}

impl Shape {
    pub fn lots(&self) -> i64 {
        self.securities * self.lots_per
    }
}

/// One integer hash, and the only source of variation in this crate.
///
/// splitmix64. ⛔ NOT `rand`: a dependency whose version could change the bytes
/// this produces would make every measurement taken against a generated book
/// unreproducible.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// A value in `lo..=hi`, determined by the inputs.
fn between(seed: u64, a: u64, b: u64, lo: i64, hi: i64) -> i64 {
    if hi <= lo {
        return lo;
    }
    let span = (hi - lo + 1) as u64;
    lo + (mix(seed ^ mix(a).wrapping_add(b)) % span) as i64
}

/// Ticker for security `i`. Deterministic, and shaped like a real one.
pub fn ticker(i: i64) -> String {
    let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut s = String::new();
    let mut n = i as usize;
    for _ in 0..3 {
        s.push(letters[n % 26] as char);
        n /= 26;
    }
    s
}

/// Build the book. Returns how many journal entries were written.
///
/// ⚠ EVERY LOT IS A JOURNAL ENTRY, which is what makes the cold-build cost
/// O(lots) and is the honest shape of the problem. `Ratio.Closure` proves a NAV
/// does not READ the lots; it says nothing about the one-time cost of folding a
/// journal that contains them, and conflating the two would be the easiest
/// overclaim available here.
pub fn generate(path: &std::path::Path, shape: Shape) -> Result<usize> {
    let _ = std::fs::remove_dir_all(path);
    let mut b = FileBook::open(path).context("creating the book")?;

    b.put_accounts(&[
        Account { dim: 1, display_name: "Investments at fair value".into(), account_type: AccountTypeRecord::Asset },
        Account { dim: 2, display_name: "Cash and equivalents".into(), account_type: AccountTypeRecord::Asset },
        Account { dim: 20, display_name: "Capital".into(), account_type: AccountTypeRecord::Equity },
    ])?;
    let cfg = b.put(b"rules = []\n")?;
    b.set_active(&cfg)?;

    let mut written = 0usize;

    // Capital first: the fund has to be funded before it can buy anything.
    for k in 0..shape.capital_txns {
        let amount = between(shape.seed, 7, k as u64, 10_000_000_00, 50_000_000_00);
        b.append(&JournalEntry {
            id: format!("cap-{k}"),
            memo: "subscription".into(),
            config: cfg.clone(),
            postings: vec![PostingRecord::new(2, amount), PostingRecord::new(20, -amount)],
            trade_date: None,
            announcement: None,
        })?;
        written += 1;
    }

    // Then trading, to a STEADY STATE.
    //
    // ⛔ `turnover` LOTS ARE OPENED FOR EACH ONE LEFT OPEN, and the rest are
    // relieved. The journal grows by all of them; the open count settles at
    // `lots_per`. That separation is the whole point — a cold projection build
    // is O(journal) and a tax scan is O(open lots), and they are different
    // numbers by a factor of `turnover`.
    //
    // ⚠ Counts VARY per security. Uniform lots would make every partition the
    // same size and hide `Ratio.Exec.the_slowest_partition_sets_the_pace`,
    // which is the tail a planner has to survive.
    // ⛔ BATCHED. `FileBook::append` opens and closes the journal every call —
    // ~4 ms an entry, 549 seconds for 140,000, twenty-one hours for the twenty
    // million this system's central claim is about. Measured, not assumed.
    // `append_all` runs the same per-entry checks and shares one file handle.
    let mut batch: Vec<JournalEntry> = Vec::new();
    let mut memo = String::new();
    for i in 0..shape.securities {
        let t = ticker(i);
        let keep =
            between(shape.seed, 1, i as u64, (shape.lots_per / 2).max(1), shape.lots_per * 3 / 2);
        let opened = keep * shape.turnover.max(1);
        // A ring of open lots: buy, and once there are more than `keep`, sell
        // the oldest. `Ratio.Lots.relief_touches_only_what_it_takes` is the FIFO
        // rule; here it is only the shape that matters.
        let mut open: std::collections::VecDeque<(i64, i64)> = Default::default();
        for l in 0..opened {
            let units = between(shape.seed, 2, (i * 4096 + l) as u64, 2, 500) * 2;
            let cost = units * between(shape.seed, 3, (i * 4096 + l) as u64, 10_00, 400_00);
            memo.clear();
            let _ = write!(memo, "buy {t}");
            batch.push(JournalEntry {
                id: format!("t-{t}-{l}"),
                memo: memo.clone(),
                config: cfg.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: cost,
                        currency: None,
                        instrument: Some(t.clone()),
                        quantity: Some(units),
                    },
                    PostingRecord::new(2, -cost),
                ],
                trade_date: None,
                announcement: None,
            });
            written += 1;
            open.push_back((units, cost));

            if open.len() as i64 > keep {
                let (u, c) = open.pop_front().expect("just checked");
                memo.clear();
                let _ = write!(memo, "sell {t}");
                batch.push(JournalEntry {
                    id: format!("s-{t}-{l}"),
                    memo: memo.clone(),
                    config: cfg.clone(),
                    postings: vec![
                        PostingRecord {
                            dim: 1,
                            amount: -c,
                            currency: None,
                            instrument: Some(t.clone()),
                            quantity: Some(-u),
                        },
                        PostingRecord::new(2, c),
                    ],
                    trade_date: None,
                    announcement: None,
                });
                written += 1;
            }
        }
    }

    b.append_all(&batch)?;
    batch.clear();

    // Corporate actions, announced and left OUTSTANDING.
    //
    // ⛔ ANNOUNCED ONLY — no `action-{id}` entry, so nothing is rewritten and
    // the factor read path carries them. That is the case the whole redesign is
    // about: `Ratio.Closure.an_open_action_makes_the_nav_read_the_lots` is what
    // a rewrite would cost here, and it is 40,000 reads per action.
    //
    // ⛔ N-FOR-1 ONLY, AND A TEST CAUGHT ME GETTING THIS WRONG. The first
    // version used 2-for-1 and 1-for-2, on the reasoning that both are "clean"
    // ratios. A 1-for-2 REVERSE split halves the holding, and half of an odd
    // number of units is not a number of units — `every_generated_holding_can_
    // actually_be_valued` failed on `CAA` at 1,853 units.
    //
    // That refusal is correct: the holder is paid cash for the fraction, which
    // realizes a gain and is a posting no configuration here declares. But a
    // generator that produces books which cannot be valued measures nothing, so
    // this only emits ratios that always divide. Reverse splits are a real case
    // and belong in a book that declares how it handles cash in lieu.
    for k in 0..shape.open_actions {
        let i = between(shape.seed, 5, k as u64, 0, (shape.securities - 1).max(0));
        let (num, den) = if k % 2 == 0 { (2, 1) } else { (3, 1) };
        b.append(&JournalEntry {
            id: format!("announce-ca-{k}"),
            memo: format!("announced ca-{k}"),
            config: cfg.clone(),
            postings: Vec::new(),
            trade_date: None,
            announcement: Some(AnnouncementRecord {
                id: format!("ca-{k}"),
                instrument: ticker(i),
                numerator: num,
                denominator: den,
                ex_date: "2026-01-15".into(),
                announced: 1_767_225_600,
            }),
        })?;
        written += 1;
    }

    // ⛔ AND THE THINGS A NAV ACTUALLY READS. `Ratio.Closure.navCost` is
    // `markCost + fxCost + actionCost + capitalCost` — one price per SECURITY
    // and one rate per CURRENCY. A book with lots and no prices can be folded
    // into a trial balance and cannot be VALUED, so a benchmark against one
    // would measure the wrong thing entirely.
    let master: Vec<ratio_ingest::Entity> = (0..shape.securities)
        .map(|i| ratio_ingest::Entity {
            id: ticker(i),
            kind: ratio_ingest::EntityKind::Instrument,
            display_name: format!("{} Corp", ticker(i)),
            attributes: [
                ("ticker".to_string(), ticker(i)),
                ("currency".to_string(), currency_of(i, shape.currencies).to_string()),
            ]
            .into_iter()
            .collect(),
        })
        .collect();
    for e in &master {
        b.append_record(ratio_store::Plane::Entities, e)?;
    }

    // One price per security, and one rate per currency other than the base.
    // `fx_does_not_grow_with_the_chart`: three currencies is three rows at five
    // names or five hundred.
    for i in 0..shape.securities {
        let t = ticker(i);
        let px = between(shape.seed, 11, i as u64, 5_00, 800_00);
        b.append_record(ratio_store::Plane::Facts, &price_fact(&t, px, shape))?;
    }
    for c in 1..shape.currencies {
        let code = currency_code(c);
        let rate = between(shape.seed, 12, c as u64, 70_00, 150_00);
        b.append_record(ratio_store::Plane::Facts, &rate_fact(code, rate, shape))?;
    }

    Ok(written)
}

/// Which currency security `i` is denominated in.
fn currency_of(i: i64, currencies: i64) -> &'static str {
    currency_code(if currencies <= 1 { 0 } else { i % currencies })
}

fn currency_code(c: i64) -> &'static str {
    match c {
        0 => "USD",
        1 => "EUR",
        2 => "GBP",
        3 => "JPY",
        _ => "CHF",
    }
}

fn provenance(shape: Shape) -> ratio_ingest::Provenance {
    ratio_ingest::Provenance {
        delivery: format!("generated-{}", shape.seed),
        row: 1,
        template: "generated".into(),
        template_id: "ratio-gen".into(),
        received: 1_767_225_600,
    }
}

fn price_fact(ticker: &str, minor: i64, shape: Shape) -> ratio_ingest::Fact {
    ratio_ingest::Fact {
        id: format!("px-{ticker}"),
        kind: "price".into(),
        reference: ticker.into(),
        entities: [(
            "instrument".to_string(),
            ratio_ingest::EntityRef {
                kind: ratio_ingest::EntityKind::Instrument,
                // One rung: match on the ticker. `Ratio.Ingest.empty_rung_
                // matches_nothing` — a rung with no claims would match EVERY
                // instrument in the master and book against an arbitrary one.
                rungs: vec![vec![ratio_ingest::Claim {
                    attr: "ticker".into(),
                    value: ticker.to_string(),
                }]],
            },
        )]
        .into_iter()
        .collect(),
        values: [
            ("asOf".to_string(), ratio_ingest::Value::Date { iso: "2026-06-30".into() }),
            (
                "price".to_string(),
                ratio_ingest::Value::Money { minor, currency: "USD".into() },
            ),
        ]
        .into_iter()
        .collect(),
        provenance: provenance(shape),
    }
}

fn rate_fact(code: &str, minor: i64, shape: Shape) -> ratio_ingest::Fact {
    ratio_ingest::Fact {
        id: format!("fx-{code}"),
        kind: "rate".into(),
        reference: code.into(),
        entities: Default::default(),
        values: [
            ("asOf".to_string(), ratio_ingest::Value::Date { iso: "2026-06-30".into() }),
            ("currency".to_string(), ratio_ingest::Value::Text { text: code.into() }),
            ("rate".to_string(), ratio_ingest::Value::Decimal { minor }),
        ]
        .into_iter()
        .collect(),
        provenance: provenance(shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("ratio-gen-{name}"))
    }

    #[test]
    fn the_same_shape_gives_the_same_book_byte_for_byte() {
        // ⛔ The property every measurement taken against this depends on. A
        // generator that drifted would make its own benchmarks unreproducible.
        let s = Shape { securities: 12, currencies: 2, turnover: 3, lots_per: 6, open_actions: 2, capital_txns: 2, seed: 7 };
        let a = tmp("det-a");
        let b = tmp("det-b");
        generate(&a, s).unwrap();
        generate(&b, s).unwrap();
        assert_eq!(
            std::fs::read(a.join("journal.jsonl")).unwrap(),
            std::fs::read(b.join("journal.jsonl")).unwrap(),
        );
    }

    #[test]
    fn a_different_seed_gives_a_different_book() {
        // Otherwise the previous test passes on a generator that ignores its
        // inputs entirely.
        let a = tmp("seed-a");
        let b = tmp("seed-b");
        generate(&a, Shape { securities: 8, lots_per: 4, seed: 1, ..Shape::default() }).unwrap();
        generate(&b, Shape { securities: 8, lots_per: 4, seed: 2, ..Shape::default() }).unwrap();
        assert_ne!(
            std::fs::read(a.join("journal.jsonl")).unwrap(),
            std::fs::read(b.join("journal.jsonl")).unwrap(),
        );
    }

    #[test]
    fn the_book_it_writes_balances() {
        // Every entry passed `FileBook::append`'s check at the door, so this is
        // really asserting that the generator produced entries at all — but a
        // generator that wrote nothing would also produce a balanced book, so:
        let d = tmp("balanced");
        let n = generate(&d, Shape { securities: 5, currencies: 2, turnover: 3, lots_per: 4, open_actions: 1, capital_txns: 2, seed: 3 }).unwrap();
        let b = FileBook::open(&d).unwrap();
        let entries = b.entries().unwrap();
        assert_eq!(entries.len(), n);
        assert!(n > 20, "only {n} entries");
        assert!(entries.iter().all(|e| e.is_balanced()));
    }

    #[test]
    fn lot_counts_vary_between_securities() {
        // ⚠ Uniform lot counts would make every partition the same size and
        // hide `Ratio.Exec.the_slowest_partition_sets_the_pace` — the tail a
        // real planner has to survive.
        let d = tmp("varies");
        generate(&d, Shape { securities: 30, currencies: 2, turnover: 3, lots_per: 20, open_actions: 0, capital_txns: 1, seed: 4 })
            .unwrap();
        let b = FileBook::open(&d).unwrap();
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for e in b.entries().unwrap() {
            for p in e.postings {
                if let Some(i) = p.instrument {
                    *counts.entry(i).or_default() += 1;
                }
            }
        }
        let lo = counts.values().min().copied().unwrap();
        let hi = counts.values().max().copied().unwrap();
        assert!(hi > lo, "every security got {lo} lots — the spread is not being applied");
    }

    #[test]
    fn open_actions_are_announced_and_never_rewritten() {
        // ⛔ The case the redesign is about. An `action-{id}` entry here would
        // mean the lots had been walked, which is the cost being avoided.
        let d = tmp("open");
        generate(&d, Shape { securities: 6, currencies: 2, turnover: 3, lots_per: 4, open_actions: 3, capital_txns: 1, seed: 5 })
            .unwrap();
        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        assert_eq!(entries.iter().filter(|e| e.announcement.is_some()).count(), 3);
        assert!(
            !entries.iter().any(|e| e.id.starts_with("action-")),
            "nothing was rewritten"
        );
    }

    #[test]
    fn every_generated_holding_can_actually_be_valued() {
        // ⚠ THE TEST THAT KEEPS THE GENERATOR HONEST. A 3-for-2 against a lot
        // holding an odd number of units refuses — cash in lieu, which realizes
        // a gain and is a posting no configuration here declares. Generating
        // books that cannot be valued would measure nothing at all.
        let d = tmp("valuable");
        generate(&d, Shape { securities: 20, currencies: 2, turnover: 3, lots_per: 10, open_actions: 6, capital_txns: 2, seed: 9 })
            .unwrap();
        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        let p = ratio_project::Projection::rebuild(&entries);
        for i in 0..20i64 {
            p.units_as_of(1, &ticker(i), "2026-06-30")
                .unwrap_or_else(|e| panic!("{} cannot be valued: {e:#}", ticker(i)));
        }
    }
}
