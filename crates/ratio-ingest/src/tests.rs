//! The feasibility question, asked directly: does a file that HALF resolves
//! behave correctly, and does it finish resolving without being re-ingested?

use super::*;

const TRADES: &str = "\
TradeRef,ISIN,Symbol,Exch,Broker,B/S,Quantity,Price,Ccy,TradeDate
T-1,US0378331005,AAPL,XNAS,GSCO,B,100,189.50,USD,01/14/2026
T-2,,VOO,ARCX,GSCO,B,50,441.20,USD,01/15/2026
T-3,GB00B03MLX29,SHEL,XLON,GSCO,S,200,27.40,GBP,01/16/2026
";

fn template() -> Template {
    let toml = r#"
[[template]]
id = "gs_equity_trades"
reads = "csv"

  [[template.entity]]
  name = "security"
  kind = "instrument"
  absent = "pend"
  by = [
    { attribute = "isin", column = "ISIN" },
    { attribute = "ticker", column = "Symbol", within = { attribute = "exchange", column = "Exch" } },
  ]

  [[template.entity]]
  name = "broker"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "code", column = "Broker" }]

  [template.fact]
  kind = "trade"
  reference = "TradeRef"
  entities = { security = "security", broker = "broker" }

  [[template.fact.value]]
  field = "side"
  as = "enum"
  column = "B/S"
  map = { B = "buy", S = "sell" }

  [[template.fact.value]]
  field = "quantity"
  as = "decimal"
  column = "Quantity"

  [[template.fact.value]]
  field = "price"
  as = "money"
  column = "Price"
  currency = "Ccy"

  [[template.fact.value]]
  field = "traded"
  as = "date"
  column = "TradeDate"
  format = "MM/DD/YYYY"
"#;
    let set = TemplateSet::from_toml(toml).expect("the template should parse");
    set.template("gs_equity_trades").expect("by id").clone()
}

fn delivery() -> Delivery {
    Delivery {
        digest: "a".repeat(64),
        origin: "sftp://gs/outgoing/trades.csv".into(),
        received: 1_780_000_000,
        bytes: TRADES.len() as i64,
    }
}

fn instrument(id: &str, attrs: &[(&str, &str)]) -> Entity {
    Entity {
        id: id.into(),
        kind: EntityKind::Instrument,
        display_name: id.into(),
        attributes: attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    }
}

fn broker(id: &str, code: &str) -> Entity {
    Entity {
        id: id.into(),
        kind: EntityKind::Counterparty,
        display_name: id.into(),
        attributes: [("code".to_string(), code.to_string())].into_iter().collect(),
    }
}

fn facts() -> Vec<Fact> {
    let t = template();
    assert!(t.check().is_empty(), "the template should check clean: {:?}", t.check());
    let rows = extract_csv(TRADES).unwrap();
    let p = project(&t, &delivery(), &rows, "cfg-digest");
    assert!(p.rejected.is_empty(), "no row should be rejected: {:?}", p.rejected);
    assert_eq!(p.facts.len(), 3);
    p.facts
}

#[test]
fn a_row_becomes_a_fact_that_names_where_it_came_from() {
    let f = &facts()[0];
    assert_eq!(f.reference, "T-1");
    assert_eq!(f.kind, "trade");
    // The chain back to the bytes, unbroken.
    assert_eq!(f.provenance.row, 2, "1-based, counting the header");
    assert_eq!(f.provenance.delivery, "a".repeat(64));
    assert_eq!(f.provenance.template, "cfg-digest");
    assert_eq!(f.provenance.template_id, "gs_equity_trades");

    // Money never becomes a float on the way in.
    assert_eq!(
        f.values.get("price"),
        Some(&Value::Money { minor: 18_950, currency: "USD".into() }),
    );
    assert_eq!(f.values.get("quantity").and_then(Value::as_minor), Some(10_000));
    assert_eq!(f.values.get("side").and_then(Value::as_text), Some("buy"));
    assert_eq!(f.values.get("traded").and_then(Value::as_text), Some("2026-01-14"));
}

#[test]
fn a_blank_column_is_not_a_claim() {
    // ⛔ Row T-2 has no ISIN. If the empty cell became a claim of `isin = ""`,
    // it would match every instrument that also has no ISIN recorded — which is
    // how one security comes to answer for another. The rung is dropped and the
    // ladder falls through to the ticker instead.
    let f = &facts()[1];
    let sec = f.entities.get("security").unwrap();
    assert_eq!(sec.rungs.len(), 1, "only the ticker rung survives");
    assert_eq!(sec.rungs[0][0].attr, "ticker");
    // …and the qualifier came with it, because a ticker is unique only within
    // an exchange.
    assert_eq!(sec.rungs[0][1].attr, "exchange");
    assert_eq!(sec.rungs[0][1].value, "ARCX");
}

#[test]
fn the_missing_instrument_pends_and_then_clears_without_re_ingesting() {
    let facts = facts();

    // The master knows the broker and two of the three instruments.
    let mut master = vec![
        broker("cp-gs", "GSCO"),
        instrument("inst-aapl", &[("isin", "US0378331005")]),
        instrument("inst-voo", &[("ticker", "VOO"), ("exchange", "ARCX")]),
    ];

    let first = resolve_all(&facts, &master);
    let admitted: Vec<&Resolved> = first.iter().filter(|r| r.is_admissible()).collect();
    let pending: Vec<&Resolved> = first.iter().filter(|r| !r.is_admissible()).collect();
    assert_eq!(admitted.len(), 2, "the two known instruments clear");
    assert_eq!(pending.len(), 1, "the unknown one pends");

    // It pends for a reason an operator can act on, and the reason names the
    // claim rather than saying "unresolved".
    let blocked = pending[0];
    assert_eq!(blocked.fact.reference, "T-3");
    let why = blocked.blocker().unwrap();
    assert!(why.contains("no security matches"), "{why}");
    assert!(why.contains("GB00B03MLX29"), "the reason names what it looked for: {why}");

    // ⭐ THE WHOLE POINT. Someone adds the instrument. NOTHING is re-ingested,
    // no queue is drained, no callback fires — the same facts are folded over a
    // master that now has one more row in it.
    master.push(instrument("inst-shel", &[("isin", "GB00B03MLX29")]));
    let second = resolve_all(&facts, &master);

    assert!(second.iter().all(Resolved::is_admissible), "everything clears");
    assert_eq!(
        second.iter().filter(|r| r.is_admissible()).count(),
        3,
    );

    // And the two that had already cleared still point at the same entities —
    // `Ratio.Ingest.resolved_never_becomes_absent` says growing the master can
    // never take that away.
    for (before, after) in first.iter().zip(second.iter()) {
        if before.is_admissible() {
            assert_eq!(before.entity("security"), after.entity("security"));
            assert_eq!(before.entity("broker"), after.entity("broker"));
        }
    }
}

#[test]
fn two_instruments_matching_one_claim_is_ambiguous_and_blocks() {
    // ⛔ NOT the same as absent, and the difference matters: this is a data
    // quality problem in the master, and the remedy is to de-duplicate or
    // narrow the ladder — NOT to add a third record. A system that reported it
    // as "unknown security" would invite exactly the wrong fix.
    let facts = facts();
    let master = vec![
        broker("cp-gs", "GSCO"),
        instrument("inst-aapl", &[("isin", "US0378331005")]),
        instrument("inst-aapl-dup", &[("isin", "US0378331005")]),
    ];

    let r = &resolve_all(&facts, &master)[0];
    assert!(!r.is_admissible());
    match r.entities.get("security").unwrap() {
        Resolution::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 2),
        other => panic!("expected ambiguous, got {other:?}"),
    }
    let why = r.blocker().unwrap();
    assert!(why.contains("2 securitys match"), "{why}");
    assert!(why.contains("narrowing"), "the reason points at the right remedy: {why}");
}

#[test]
fn resolution_never_falls_through_an_ambiguous_rung() {
    // The first rung that matches ANYTHING decides. If an ambiguous ISIN fell
    // through to a ticker that happened to match uniquely, a data-quality
    // problem would silently become a confident wrong answer.
    let facts = facts();
    let master = vec![
        broker("cp-gs", "GSCO"),
        instrument("dup-a", &[("isin", "US0378331005")]),
        instrument("dup-b", &[("isin", "US0378331005")]),
        instrument("by-ticker", &[("ticker", "AAPL"), ("exchange", "XNAS")]),
    ];
    let r = &resolve_all(&facts, &master)[0];
    assert!(
        matches!(r.entities.get("security"), Some(Resolution::Ambiguous { .. })),
        "the ambiguous rung must decide, not the one below it",
    );
}

#[test]
fn silence_about_an_attribute_is_not_agreement() {
    // An instrument that records no ISIN does not match a row claiming one.
    let e = instrument("inst-voo", &[("ticker", "VOO")]);
    assert!(!e.satisfies(&[Claim { attr: "isin".into(), value: "US0378331005".into() }]));
    // The emitted `claim_holds` is where that lives, and it is `false && _`.
    assert!(!claim_holds(false, true));
}

#[test]
fn an_empty_rung_matches_nothing_rather_than_everything() {
    // ⛔ `iter().all(…)` on an empty slice is TRUE. Without the guard this
    // entity would satisfy an empty rung, and a template whose ladder read only
    // columns the file does not have would match every instrument in the
    // master. `Ratio.Ingest.empty_rung_matches_nothing` is the proof; removing
    // the guard makes it unprovable.
    let e = instrument("inst-aapl", &[("isin", "US0378331005")]);
    assert!(!e.satisfies(&[]));
    assert!(!rung_satisfied(true, true));
}

#[test]
fn the_emitted_decision_is_the_one_the_fold_uses() {
    // The bridge `Ratio.Ingest.kind_is_determined_by_count` proves the outcome
    // depends on nothing but the count. This checks the Rust establishes that
    // count and reads the emitted answer, rather than deciding for itself.
    assert_eq!(resolution_of_count(0), ResolutionKind::Absent);
    assert_eq!(resolution_of_count(1), ResolutionKind::Resolved);
    assert_eq!(resolution_of_count(2), ResolutionKind::Ambiguous);
    assert_eq!(resolution_of_count(97), ResolutionKind::Ambiguous);

    // …and the scale, where 0 stands for "not representable".
    assert_eq!(scale_for(0), 100);
    assert_eq!(scale_for(1), 10);
    assert_eq!(scale_for(2), 1);
    assert_eq!(scale_for(3), 0, "three decimal places have no exact scale");
    assert_eq!(minor_units(250, 50, 1), 25_050);
}

#[test]
fn a_row_that_cannot_be_mapped_rejects_alone() {
    // ⚠ PER ROW, never per file. One bad row rejecting a ten-thousand-row file
    // is the most common failure of this kind of system.
    let bad = "\
TradeRef,ISIN,Symbol,Exch,Broker,B/S,Quantity,Price,Ccy,TradeDate
T-1,US0378331005,AAPL,XNAS,GSCO,B,100,189.50,USD,01/14/2026
T-2,US0378331005,AAPL,XNAS,GSCO,X,100,189.50,USD,01/14/2026
T-3,US0378331005,AAPL,XNAS,GSCO,B,100,1.005,USD,01/14/2026
T-4,US0378331005,AAPL,XNAS,GSCO,B,100,189.50,USD,13/14/2026
";
    let rows = extract_csv(bad).unwrap();
    let p = project(&template(), &delivery(), &rows, "cfg");
    assert_eq!(p.facts.len(), 1, "the good row still lands");
    assert_eq!(p.rejected.len(), 3);

    // Each rejection says which row and what was wrong with it.
    assert_eq!(p.rejected[0].row, 3);
    assert!(p.rejected[0].reason.contains("\"X\""), "{:?}", p.rejected[0].reason);
    // ⛔ 1.005 is refused, not truncated — the case the whole money approach
    // exists for.
    assert!(p.rejected[1].reason.contains("two decimal places"), "{:?}", p.rejected[1].reason);
    assert!(p.rejected[2].reason.contains("not a date"), "{:?}", p.rejected[2].reason);
}

#[test]
fn a_date_is_read_in_the_format_the_template_declares() {
    // 03/04/2026 is March in one convention and April in the other, and getting
    // it wrong is a silent off-by-a-month. The template says which.
    assert_eq!(parse_date("03/04/2026", "MM/DD/YYYY").unwrap(), "2026-03-04");
    assert_eq!(parse_date("03/04/2026", "DD/MM/YYYY").unwrap(), "2026-04-03");
    assert_eq!(parse_date("2026-03-04", "YYYY-MM-DD").unwrap(), "2026-03-04");
    assert!(parse_date("03/04/2026", "MM-DD-YY").is_err(), "an unknown format is not a guess");
}

#[test]
fn a_template_checks_before_it_maps_anything() {
    let mut t = template();
    assert!(t.check().is_empty());

    t.fact.entities.insert("custodian".into(), "custodian".into());
    let problems = t.check();
    assert!(
        problems.iter().any(|p| p.contains("not declared")),
        "pointing at an undeclared entity is caught at approval, not on the first file: {problems:?}",
    );
}

#[test]
fn a_template_renders_the_way_it_reads() {
    let r = template().render();
    // The readable form is DERIVED from the TOML, so there is one authored
    // syntax and no second grammar to keep in step.
    for expected in [
        "template gs_equity_trades {",
        "reads      csv with header",
        "entity     security  (instrument)",
        "by     isin   from \"ISIN\"",
        "or     ticker from \"Symbol\" within exchange from \"Exch\"",
        "absent   pend",
    ] {
        assert!(r.contains(expected), "missing {expected:?} in:\n{r}");
    }
}
