//! Live Postgres walk for the Stage E projection schema.
//!
//! ⛔ NOT A TLA PROBE. `tags = ["manual"]` keeps this out of `bazel test //...`
//! so a machine without a server stays green. CI runs the target by name
//! against a service container and it must PASS. RATIO_PG_URL unset is a
//! refuse, not a skip — a skip here is the green suite that tests nothing.

use std::collections::BTreeMap;
use std::path::Path;

use ratio_project::{relief, Projection};
use ratio_rules::UNDECLARED_VIEW as B;
use ratio_sql_project::{
    fold, Geometry, JournalPin, PgProjection, ProjectionReads, StoreConfig, Watermark,
};
use ratio_store::{
    Account, AccountTypeRecord as A, ConfigStore, FileBook, Journal, JournalEntry,
    PostingRecord,
};

fn pg_url() -> String {
    std::env::var("RATIO_PG_URL").expect(
        "RATIO_PG_URL unset. This target requires a live Postgres. CI sets it; \
         locally: see DEVELOPING.md",
    )
}

fn tmp_root() -> std::path::PathBuf {
    match std::env::var_os("TEST_TMPDIR") {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::temp_dir(),
    }
}

fn book_path(name: &str) -> std::path::PathBuf {
    let d = tmp_root().join(format!("ratio-sql-pg-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn seed(
    name: &str,
    rules: &[u8],
    trades: &[(&str, i64, i64)],
) -> (std::path::PathBuf, ratio_store::Digest) {
    let d = book_path(name);
    let mut b = FileBook::open(&d).unwrap();
    b.put_accounts(&[
        Account {
            dim: 1,
            display_name: "Investments".into(),
            account_type: A::Asset,
        },
        Account {
            dim: 2,
            display_name: "Cash".into(),
            account_type: A::Asset,
        },
    ])
    .unwrap();
    let c = b.put(rules).unwrap();
    b.set_active(&c).unwrap();
    for (n, (inst, cost, qty)) in trades.iter().enumerate() {
        b.append(&JournalEntry {
            id: format!("t{n}"),
            memo: "buy".into(),
            config: c.clone(),
            postings: vec![
                PostingRecord::of(1, *cost, inst, Some(*qty)),
                PostingRecord::new(2, -*cost),
            ],
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
            kind: None,
        })
        .unwrap();
    }
    (d, c)
}

fn pin_of(path: &Path) -> JournalPin {
    let entries = FileBook::open(path).unwrap().entries().unwrap();
    JournalPin::of(&entries).unwrap()
}

fn append_buy(
    path: &Path,
    config: &ratio_store::Digest,
    id: &str,
    inst: &str,
    cost: i64,
    qty: i64,
) {
    let mut b = FileBook::open(path).unwrap();
    b.append(&JournalEntry {
        id: id.into(),
        memo: "buy".into(),
        config: config.clone(),
        postings: vec![
            PostingRecord::of(1, cost, inst, Some(qty)),
            PostingRecord::new(2, -cost),
        ],
        trade_date: None,
        announcement: None,
        due_date: None,
        application: None,
        identified_lots: None,
        special_allocations: None,
        kind: None,
    })
    .unwrap();
}

fn live(name: &str) -> PgProjection {
    let schema = format!("ratio_pg_{}", name.replace('-', "_"));
    let pg = PgProjection::connect(&pg_url(), &schema).unwrap();
    pg.drop_schema().unwrap();
    pg.apply_schema().unwrap();
    pg
}

#[test]
fn the_schema_applies_and_holds_a_null_instrument_rest_row() {
    // ⭐ THE CLAIM #153 COULD NOT MAKE. PRIMARY KEY (instrument) makes the
    // column NOT NULL on a live engine; the rest map is NULL-as-unset. A
    // schema that only passed the denotational store hid that.
    let (d, _) = seed("rest", b"rules = []\n", &[("vti", 10_000, 10)]);
    let pg = live("rest");
    pg.replay_book("rest", &d).unwrap();
    let pin = pin_of(&d);
    let pos = pg.positions("rest", B, &pin).unwrap();
    let rest = pos
        .value
        .iter()
        .filter(|r| r.instrument.is_none())
        .collect::<Vec<_>>();
    assert_eq!(rest.len(), 1, "cash is the rest map, instrument NULL");
    assert_eq!(rest[0].dim, 2);
    assert_eq!(rest[0].cost, -10_000);
}

#[test]
fn replay_from_the_journal_digest_matches_the_in_memory_fold() {
    let (d, _) = seed(
        "agrees",
        b"rules = []\n",
        &[("vti", 25_000, 100), ("voo", 10_000, 40), ("vti", 5_000, 20)],
    );
    let pin = pin_of(&d);
    let memory = Projection::of_book(&d).unwrap();
    let pg = live("agrees");
    let mark = pg.replay_book("agrees", &d).unwrap();
    assert_eq!(mark.prefix, pin.prefix);
    assert_eq!(mark.digest, pin.digest);
    assert_eq!(mark.prefix, 3);

    let lots = pg.lots_of("agrees", B, 1, "vti", &pin).unwrap();
    assert_eq!(lots.prefix, pin.prefix);
    assert_eq!(lots.value, memory.lots_of(B, 1, "vti").unwrap().value);

    let pos = pg.positions("agrees", B, &pin).unwrap();
    let held: BTreeMap<(i64, String), (i64, i64)> = memory
        .positions(B)
        .unwrap()
        .value
        .held
        .iter()
        .map(|((dim, inst), v)| ((*dim, inst.to_string()), *v))
        .collect();
    let projected: BTreeMap<(i64, String), (i64, i64)> = pos
        .value
        .iter()
        .filter_map(|r| {
            r.instrument
                .as_ref()
                .map(|i| ((r.dim, i.clone()), (r.cost, r.quantity)))
        })
        .collect();
    assert_eq!(projected, held);

    let agg = pg.aggregates("agrees", B, &pin).unwrap();
    let totals = memory.totals(B).unwrap();
    assert_eq!(agg.value.len(), totals.value.by_dim.len());
    assert_eq!(agg.prefix, totals.prefix);
}

#[test]
fn a_second_replay_replaces_the_snapshot_and_does_not_double_count() {
    let (d, c) = seed("twice", b"rules = []\n", &[("vti", 10_000, 10)]);
    let pg = live("twice");
    pg.replay_book("twice", &d).unwrap();
    append_buy(&d, &c, "t1", "vti", 20_000, 10);
    let mark = pg.replay_book("twice", &d).unwrap();
    let pin = pin_of(&d);
    assert_eq!(
        mark,
        Watermark {
            book_id: "twice".into(),
            prefix: pin.prefix,
            digest: pin.digest.clone()
        }
    );
    let lots = pg.lots_of("twice", B, 1, "vti", &pin).unwrap();
    assert_eq!(
        lots.value.len(),
        2,
        "a rebuild that appended onto itself would hold four"
    );
    assert_eq!(lots.value.iter().map(|l| l.units).sum::<i64>(), 20);
}

#[test]
fn a_stale_projection_refuses_rather_than_answering() {
    let (d, c) = seed("lag", b"rules = []\n", &[("vti", 10_000, 10)]);
    let pg = live("lag");
    pg.replay_book("lag", &d).unwrap();
    append_buy(&d, &c, "t1", "vti", 5_000, 5);
    let head = pin_of(&d);
    assert_eq!(head.prefix, 2);
    assert_eq!(pg.watermark("lag").unwrap().unwrap().prefix, 1);

    let err = pg.lots_of("lag", B, 1, "vti", &head).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("lags or leads"), "{msg}");
    assert!(msg.contains("must refuse"), "{msg}");

    let err = pg
        .relieve("lag", B, 1, "vti", relief::Method::Fifo, 5, &head)
        .unwrap_err();
    assert!(format!("{err:#}").contains("lags or leads"));
}

#[test]
fn elected_hifo_is_not_silently_replaced_by_sql_fifo() {
    // ⭐ TWO LOTS, CHEAP THEN DEAR. Physical storage is seq order. HIFO
    // must take the dear one. Relief loads the rows and calls relieve_by.
    let (d, _) = seed(
        "hifo",
        b"lot_method = \"hifo\"\nrules = []\n",
        &[("vti", 1_000, 10), ("vti", 10_000, 10)],
    );
    let pg = live("hifo");
    pg.replay_book("hifo", &d).unwrap();
    let pin = pin_of(&d);
    let lots = pg.lots_of("hifo", B, 1, "vti", &pin).unwrap();
    assert_eq!(lots.value.len(), 2);
    assert_eq!(lots.value[0].seq, 0);
    assert_eq!(lots.value[0].cost, 1_000);
    assert_eq!(lots.value[1].cost, 10_000);

    let got = pg
        .relieve("hifo", B, 1, "vti", relief::Method::Hifo, 10, &pin)
        .unwrap();
    assert_eq!(got.prefix, pin.prefix);
    assert_eq!(got.value.cost, 10_000, "HIFO takes the dear lot");
    assert_eq!(got.value.taken[0].seq, 1);

    let memory = relief::relieve_by(relief::Method::Hifo, &lots.value, 10).unwrap();
    assert_eq!(got.value, memory);
}

#[test]
fn console_shaped_reads_pin_the_journal_and_refuse_an_empty_store() {
    // ⭐ THE API READ PATH AGAINST A LIVE ENGINE. ProjectionReads is what
    // the console holds. An empty store refuses; catch_up then serves a
    // figure that carries the journal pin. Unset acquired stays unset.
    let reads = ProjectionReads::in_process();
    let empty = JournalPin::of(&[]).unwrap();
    let err = reads
        .lots_of("api", B, 1, "vti", &empty)
        .unwrap_err();
    assert!(format!("{err:#}").contains("has not been replayed"));

    let (d, _) = seed("api", b"rules = []\n", &[("vti", 10_000, 10)]);
    let pg = live("api");
    let reads = ProjectionReads::from_pg(pg);
    assert!(reads.watermark("api").unwrap().is_none());

    let pin = reads.catch_up("api", &d).unwrap();
    assert_eq!(pin, pin_of(&d));
    let lots = reads.lots_of("api", B, 1, "vti", &pin).unwrap();
    assert_eq!(lots.prefix, pin.prefix);
    assert_eq!(lots.value.len(), 1);
    assert!(lots.value[0].acquired.is_none(), "unset acquired stays unset");

    let pos = reads.positions("api", B, &pin).unwrap();
    assert_eq!(pos.prefix, pin.prefix);
    let rest = pos.value.iter().filter(|r| r.instrument.is_none()).count();
    assert_eq!(rest, 1);

    let agg = reads.aggregates("api", B, &pin).unwrap();
    assert_eq!(agg.prefix, pin.prefix);
    assert!(!agg.value.is_empty());
}

#[test]
fn projection_reads_connect_applies_a_missing_schema() {
    // ⭐ THE CONSOLE CONSTRUCTOR. `with_stage_e_from_env` calls
    // ProjectionReads::connect. A missing schema is first-use apply;
    // a second connect on the same schema must not try apply_schema
    // again (that would fail on existing tables).
    let url = pg_url();
    let schema = "ratio_pg_connect";
    let probe = PgProjection::connect(&url, schema).unwrap();
    probe.drop_schema().unwrap();

    let cfg = StoreConfig {
        url: url.clone(),
        schema: schema.into(),
    };
    let reads = ProjectionReads::connect(&cfg).unwrap();
    let (d, _) = seed("connect", b"rules = []\n", &[("vti", 10_000, 10)]);
    let pin = reads.catch_up("connect", &d).unwrap();
    let lots = reads.lots_of("connect", B, 1, "vti", &pin).unwrap();
    assert_eq!(lots.prefix, pin.prefix);
    assert_eq!(lots.value.len(), 1);

    let again = ProjectionReads::connect(&cfg).unwrap();
    let lots = again.lots_of("connect", B, 1, "vti", &pin).unwrap();
    assert_eq!(lots.value.len(), 1, "reconnect must see the first replay");
}

#[test]
fn a_small_handoff_shaped_load_relieves_through_relieve_by() {
    // ⭐ SAME GENERATOR AS THE 20M FOLD, ON A LIVE ENGINE. The HANDOFF
    // row refuses load_scale (it would emit 20M INSERTs). This is the
    // table path at a size the server already has; the measured 20M
    // claim is `//crates/ratio-sql-project:fold_scale_test`.
    let g = Geometry {
        securities: 5,
        lots_per: 8,
    };
    let report = fold(g).unwrap();
    let pg = live("scale");
    let mark = pg.load_scale("scale", g).unwrap();
    assert_eq!(mark.digest, report.digest);
    let pin = JournalPin {
        prefix: mark.prefix,
        digest: mark.digest.clone(),
    };
    let inst = g.instrument(0);
    let got = pg
        .relieve(
            "scale",
            B,
            1,
            &inst,
            relief::Method::Hifo,
            10,
            &pin,
        )
        .unwrap();
    assert_eq!(got.value.cost, report.hifo_cost, "HIFO takes the dear lot");
    assert_ne!(got.value.cost, report.seq_scan_cost);
    let lots = pg.lots_of("scale", B, 1, &inst, &pin).unwrap();
    assert!(lots.value.iter().all(|l| l.acquired.is_none()), "unset stays unset");
}

#[test]
fn store_config_empty_url_is_unset_on_the_live_target_too() {
    // Same parse the console uses. A live job that forgot to export
    // RATIO_PG_URL must not invent a default server.
    assert!(StoreConfig::from_url(None, None).is_none());
    assert!(StoreConfig::from_url(Some(""), None).is_none());
    let cfg = StoreConfig::from_url(Some(&pg_url()), None).unwrap();
    assert_eq!(cfg.schema, StoreConfig::DEFAULT_SCHEMA);
}

#[test]
fn a_replaced_journal_at_the_same_height_refuses_replay_onto_the_old_snapshot() {
    let (d, _) = seed("swap", b"rules = []\n", &[("vti", 10_000, 10)]);
    let pg = live("swap");
    pg.replay_book("swap", &d).unwrap();
    let (other, _) = seed("swap-other", b"rules = []\n", &[("voo", 99_000, 3)]);
    let err = pg.replay_book("swap", &other).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("digest"), "{msg}");
    assert!(msg.contains("replaced"), "{msg}");
}
