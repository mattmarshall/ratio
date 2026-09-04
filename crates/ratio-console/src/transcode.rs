//! HTTP transcoding for `ratio.v1.Console`.
//!
//! Turns the `google.api.http` rules on the service into routes. The table is
//! written by hand — there is no Rust equivalent of grpc-gateway worth taking a
//! dependency on, and Envoy's transcoder would mean putting a proxy in front of
//! a Lambda that costs nothing precisely because there is nothing in front of it.
//!
//! ⛔ **The table is CHECKED against the contract rather than trusted.**
//! `transcode_test` reads the descriptor set `//proto:ratio_proto` compiles to,
//! extracts every method's `google.api.http` rule, and asserts this file routes
//! exactly those patterns and no others. Hand-written and unchecked would be a
//! 404 discovered by a customer; hand-written and checked is a failing build.

use anyhow::{bail, Context, Result};

use crate::Console;

/// One route: the HTTP method, the template from the proto, and how to serve it.
///
/// `template` is stored verbatim — `"/v1/{parent=funds/*}/breaks"` — because
/// that string is what the test compares against the descriptor. Storing a
/// pre-parsed form would compare the parse to itself.
pub struct Route {
    pub method: &'static str,
    pub template: &'static str,
}

/// Every route this transcoder serves, in match order.
///
/// Order matters: `/v1/{name=funds/*}` would swallow `/v1/funds` if it came
/// first, so literals precede patterns. The test asserts the SET matches the
/// contract; this ordering is the implementation's own business.
pub const ROUTES: &[Route] = &[
    // Literals before patterns: `/v1/{name=books/*}` would swallow `/v1/books`.
    Route { method: "GET", template: "/v1/books" },
    Route { method: "GET", template: "/v1/{name=books/*}" },
    Route { method: "POST", template: "/v1/books" },
    Route { method: "GET", template: "/v1/funds" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/views" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*}:reconcile" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*}:projectProgress" },
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*}/breaks" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/changeLogEntries" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/breaks/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*/changeLogEntries/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/configVersions" },
    Route { method: "GET", template: "/v1/{name=funds/*/configVersions/*}:diff" },
    Route { method: "GET", template: "/v1/{name=funds/*/configVersions/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/deliveries" },
    Route { method: "GET", template: "/v1/{name=funds/*/deliveries/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/pendingFacts" },
    Route { method: "GET", template: "/v1/{name=funds/*/pendingFacts/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/facts" },
    Route { method: "GET", template: "/v1/{name=funds/*/facts/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*}/accounts" },
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*/accounts/*}/postings" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/accounts/*/postings/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/accounts/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/corporateActions" },
    Route { method: "GET", template: "/v1/{name=funds/*/corporateActions/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*}/navStrikes" },
    // A custom method (AIP-136) on GET, because replaying is safe and
    // idempotent — it folds a journal prefix and writes nothing.
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/navStrikes/*}:replay" },
    // The same, and for the same reason: explaining a strike reads.
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/navStrikes/*}:explain" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/navStrikes/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*}/periodCloses" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/periodCloses/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*}/positions" },
    // ⛔ BEFORE the bare position, and before the position pattern can swallow
    // it. `funds/f/positions/p/lots` has more segments than `funds/*/positions/*`
    // matches, but the ordering here is the documented contract and a reader
    // should not have to work that out.
    Route { method: "GET", template: "/v1/{parent=funds/*/views/*/positions/*}/lots" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/positions/*/lots/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*/positions/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/templates" },
    Route { method: "GET", template: "/v1/{name=funds/*/templates/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/rules" },
    Route { method: "GET", template: "/v1/{name=funds/*/rules/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/entries" },
    Route { method: "GET", template: "/v1/{name=funds/*/entries/*}" },
    // The one write. A custom method (AIP-136) because recording an event is a
    // domain operation with a computed result, not a Create of the thing the
    // caller sent.
    Route { method: "POST", template: "/v1/{parent=funds/*}:applyEvent" },
    Route { method: "POST", template: "/v1/{parent=funds/*}:ingest" },
    Route { method: "POST", template: "/v1/{parent=funds/*}:admit" },
    Route { method: "POST", template: "/v1/{parent=funds/*}:mark" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*}:operatingAging" },
    Route { method: "GET", template: "/v1/{name=funds/*/views/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*}" },
];

/// Serve one request. `path` excludes the query; `query` is the raw string.
///
/// Returns the JSON body. An unroutable path is an error, not an empty result —
/// a console that silently receives `{}` from a mistyped URL debugs badly.
pub fn serve(
    console: &Console,
    method: &str,
    path: &str,
    query: &str,
    body: &str,
) -> Result<String> {
    let rest = path
        .strip_prefix("/v1/")
        .ok_or_else(|| anyhow::anyhow!("{path:?} is not a /v1 path"))?;

    // Exactly one route writes, and it is named here rather than inferred, so
    // adding a method cannot quietly widen what POST reaches.
    if method == "POST" {
        // Each write is named here rather than inferred, so adding a method
        // cannot quietly widen what POST reaches.
        if rest == "books" {
            return to_json(&console.create_book(create_book_request(query, body)?)?);
        }
        let fund = |suffix: &str| -> Option<String> {
            rest.strip_suffix(suffix)?
                .strip_prefix("funds/")
                .filter(|s| !s.is_empty() && !s.contains('/'))
                .map(|s| format!("funds/{s}"))
        };
        if let Some(parent) = fund(":applyEvent") {
            return to_json(&console.apply_event(&apply_event_request(&parent, body)?)?);
        }
        if let Some(parent) = fund(":ingest") {
            return to_json(&console.ingest_delivery(&ingest_delivery_request(&parent, body)?)?);
        }
        if let Some(parent) = fund(":mark") {
            return to_json(&console.mark_positions(&mark_positions_request(&parent, body)?)?);
        }
        if let Some(parent) = fund(":admit") {
            return to_json(&console.admit_facts(&admit_facts_request(&parent, body)?)?);
        }
        bail!("{path:?} does not accept POST");
    }
    if method != "GET" {
        bail!("the console API is read-only apart from CreateBook and the :custom writes; {method} is not accepted");
    }
    let rest = path
        .strip_prefix("/v1/")
        .ok_or_else(|| anyhow::anyhow!("{path:?} is not a /v1 path"))?;
    let seg: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();

    // The match arms are the ROUTES table above, in the same order. Written as
    // a slice pattern so an unhandled shape is a compile-time hole rather than
    // a runtime fallthrough.
    let json = match seg.as_slice() {
        ["books"] => to_json(&console.list_books()?)?,
        ["books", id] => to_json(&console.get_book(&format!("books/{id}"))?)?,
        ["funds"] => to_json(&console.list_funds()?)?,
        ["funds", id, "views"] => to_json(&console.list_views(&format!("funds/{id}"))?)?,
        ["funds", id, "views", v] if v.ends_with(":reconcile") => {
            let view = v.trim_end_matches(":reconcile");
            to_json(&console.reconcile_views(
                &format!("funds/{id}/views/{view}"),
                param_of(query, "against"),
            )?)?
        }
        ["funds", id, "views", v] if v.ends_with(":projectProgress") => {
            let view = v.trim_end_matches(":projectProgress");
            to_json(&console.project_progress(&format!("funds/{id}/views/{view}"))?)?
        }
        ["funds", id, "views", v] if v.ends_with(":operatingAging") => {
            let view = v.trim_end_matches(":operatingAging");
            to_json(&console.operating_aging(
                &format!("funds/{id}/views/{view}"),
                &param_of(query, "filter"),
            )?)?
        }
        ["funds", id, "views", v] => {
            to_json(&console.get_view(&format!("funds/{id}/views/{v}"))?)?
        }
        ["funds", id, "views", v, "breaks"] => {
            to_json(&console.list_breaks(&format!("funds/{id}/views/{v}"), filter_of(query))?)?
        }
        ["funds", id, "changeLogEntries"] => {
            to_json(&console.list_change_log_entries(&format!("funds/{id}"))?)?
        }
        ["funds", id, "configVersions"] => {
            to_json(&console.list_config_versions(&format!("funds/{id}"))?)?
        }
        ["funds", id, "configVersions", v] if v.ends_with(":diff") => {
            let digest = v.trim_end_matches(":diff");
            to_json(&console.diff_config_versions(
                &format!("funds/{id}/configVersions/{digest}"),
                param_of(query, "base"),
            )?)?
        }
        ["funds", id, "configVersions", v] => {
            to_json(&console.get_config_version(&format!("funds/{id}/configVersions/{v}"))?)?
        }
        ["funds", id, "views", v, "positions"] => {
            to_json(&console.list_positions(&format!("funds/{id}/views/{v}"))?)?
        }
        ["funds", id, "views", v, "positions", p, "lots"] => {
            to_json(&console.list_lots(&format!("funds/{id}/views/{v}/positions/{p}"))?)?
        }
        ["funds", id, "views", v, "positions", p, "lots", l] => {
            to_json(&console.get_lot(&format!("funds/{id}/views/{v}/positions/{p}/lots/{l}"))?)?
        }
        ["funds", id, "views", v, "positions", p] => {
            to_json(&console.get_position(&format!("funds/{id}/views/{v}/positions/{p}"))?)?
        }
        ["funds", id, "templates"] => {
            to_json(&console.list_templates(&format!("funds/{id}"))?)?
        }
        ["funds", id, "templates", t] => {
            to_json(&console.get_template(&format!("funds/{id}/templates/{t}"))?)?
        }
        ["funds", id, "rules"] => to_json(&console.list_rules(&format!("funds/{id}"))?)?,
        ["funds", id, "rules", r] => to_json(&console.get_rule(&format!("funds/{id}/rules/{r}"))?)?,
        ["funds", id, "entries"] => to_json(&console.list_entries(&format!("funds/{id}"))?)?,
        ["funds", id, "entries", e] => {
            to_json(&console.get_entry(&format!("funds/{id}/entries/{e}"))?)?
        }
        ["funds", id, "deliveries"] => {
            to_json(&console.list_deliveries(&format!("funds/{id}"))?)?
        }
        ["funds", id, "deliveries", d] => {
            to_json(&console.get_delivery(&format!("funds/{id}/deliveries/{d}"))?)?
        }
        ["funds", id, "pendingFacts"] => {
            to_json(&console.list_pending_facts(&format!("funds/{id}"))?)?
        }
        ["funds", id, "pendingFacts", f] => {
            to_json(&console.get_pending_fact(&format!("funds/{id}/pendingFacts/{f}"))?)?
        }
        ["funds", id, "facts"] => {
            to_json(&console.list_facts(&format!("funds/{id}"), filter_of(query))?)?
        }
        ["funds", id, "facts", f] => {
            to_json(&console.get_fact(&format!("funds/{id}/facts/{f}"))?)?
        }
        ["funds", id, "views", v, "accounts"] => {
            to_json(&console.list_accounts(
                &format!("funds/{id}/views/{v}"),
                filter_of(query),
            )?)?
        }
        ["funds", id, "views", v, "accounts", a, "postings"] => {
            to_json(&console.list_postings(&format!("funds/{id}/views/{v}/accounts/{a}"))?)?
        }
        ["funds", id, "views", v, "accounts", a, "postings", p] => {
            to_json(&console.get_posting(&format!("funds/{id}/views/{v}/accounts/{a}/postings/{p}"))?)?
        }
        ["funds", id, "views", v, "accounts", a] => {
            to_json(&console.get_account(&format!("funds/{id}/views/{v}/accounts/{a}"))?)?
        }
        ["funds", id, "corporateActions"] => {
            to_json(&console.list_corporate_actions(&format!("funds/{id}"))?)?
        }
        ["funds", id, "corporateActions", a] => {
            to_json(&console.get_corporate_action(&format!("funds/{id}/corporateActions/{a}"))?)?
        }
        ["funds", id, "views", v, "navStrikes"] => {
            to_json(&console.list_nav_strikes(&format!("funds/{id}/views/{v}"))?)?
        }
        // The custom method is matched on the LAST segment before the plain
        // Get, so `x:replay` never falls through to a lookup for a strike
        // literally named `x:replay`.
        ["funds", id, "views", v, "navStrikes", s] if s.ends_with(":replay") => {
            let strike = s.trim_end_matches(":replay");
            to_json(
                &console.replay_nav_strike(&format!("funds/{id}/views/{v}/navStrikes/{strike}"))?,
            )?
        }
        ["funds", id, "views", v, "navStrikes", s] if s.ends_with(":explain") => {
            let strike = s.trim_end_matches(":explain");
            // ⛔ ANY VALUE BUT `true` IS `false`, INCLUDING `1` AND `yes`. The
            // parameter decides whether this request folds a journal, so a
            // typo must fall to the cheap answer rather than to the expensive
            // one — a permissive parse here is a way to spend a period end by
            // accident.
            let analyze = param_of(query, "analyze") == "true";
            to_json(&console.explain_nav_strike(
                &format!("funds/{id}/views/{v}/navStrikes/{strike}"),
                analyze,
            )?)?
        }
        ["funds", id, "views", v, "navStrikes", s] => {
            to_json(&console.get_nav_strike(&format!("funds/{id}/views/{v}/navStrikes/{s}"))?)?
        }
        ["funds", id, "views", v, "periodCloses"] => {
            to_json(&console.list_period_closes(&format!("funds/{id}/views/{v}"))?)?
        }
        ["funds", id, "views", v, "periodCloses", c] => {
            to_json(&console.get_period_close(&format!("funds/{id}/views/{v}/periodCloses/{c}"))?)?
        }
        ["funds", id, "views", v, "breaks", b] => {
            to_json(&console.get_break(&format!("funds/{id}/views/{v}/breaks/{b}"))?)?
        }
        ["funds", id, "changeLogEntries", e] => {
            to_json(&console.get_change_log_entry(&format!("funds/{id}/changeLogEntries/{e}"))?)?
        }
        ["funds", id] => to_json(&console.get_fund(&format!("funds/{id}"))?)?,
        _ => bail!("no route for {path:?}"),
    };
    Ok(json)
}

/// The request body, read field by field.
///
/// ⛔ A HAND-WRITTEN MIRROR, like the encoders below and checked with them by
/// `//proto:mirrors_test`. `parent` is bound by the path template rather than
/// carried in the body, which is what `body: "*"` means.
///
/// Everything is read as a string and validated downstream, so a caller
/// sending a JSON NUMBER for an amount is refused rather than silently run
/// through a double.
fn apply_event_request(parent: &str, body: &str) -> Result<pb::ApplyEventRequest> {
    let v: serde_json::Value =
        serde_json::from_str(body).context("the request body is not JSON")?;
    let text = |k: &str| -> Result<String> {
        match v.get(k) {
            None | Some(serde_json::Value::Null) => Ok(String::new()),
            Some(serde_json::Value::String(s)) => Ok(s.clone()),
            Some(other) => bail!("{k} must be a string, not {other}"),
        }
    };
    Ok(pb::ApplyEventRequest {
        parent: parent.to_string(),
        rule_id: text("ruleId")?,
        event_id: text("eventId")?,
        amount: text("amount")?,
        days: text("days")?,
        // ⛔ THE THREE THAT MAKE AN EVENT A TRADE. Absent, an event moves value
        // and opens no lot — see the fields' own comments in console.proto. A
        // transcoder that read the contract's other fields and quietly dropped
        // these would put the defect back with the contract looking correct,
        // which is the failure `transcode_test` exists to catch.
        instrument: text("instrument")?,
        quantity: text("quantity")?,
        // ⚠ `None` when the key is absent, NOT a zeroed Date. `0000-00-00`
        // would parse as a trade date nobody supplied, and the holding-period
        // methods would classify against it rather than refusing.
        trade_date: v.get("tradeDate").filter(|d| !d.is_null()).and_then(date_from_json),
        due_date: v.get("dueDate").filter(|d| !d.is_null()).and_then(date_from_json),
        application: text("application")?,
        validate_only: matches!(v.get("validateOnly"), Some(serde_json::Value::Bool(true))),
    })
}

fn ingest_delivery_request(parent: &str, body: &str) -> Result<pb::IngestDeliveryRequest> {
    let v: serde_json::Value =
        serde_json::from_str(body).context("the request body is not JSON")?;
    let text = |k: &str| -> Result<String> {
        match v.get(k) {
            None | Some(serde_json::Value::Null) => Ok(String::new()),
            Some(serde_json::Value::String(s)) => Ok(s.clone()),
            Some(other) => bail!("{k} must be a string, not {other}"),
        }
    };
    Ok(pb::IngestDeliveryRequest {
        parent: parent.to_string(),
        template_id: text("templateId")?,
        content: text("content")?,
        origin: text("origin")?,
        validate_only: matches!(v.get("validateOnly"), Some(serde_json::Value::Bool(true))),
    })
}

fn mark_positions_request(parent: &str, body: &str) -> Result<pb::MarkPositionsRequest> {
    let v: serde_json::Value =
        serde_json::from_str(body).context("the request body is not JSON")?;
    // proto3 canonical JSON renders a `google.type.Date` as an object of three
    // numbers, not the ISO string a person would write. Accepting only the
    // canonical form keeps one encoding rather than two.
    Ok(pb::MarkPositionsRequest {
        parent: parent.to_string(),
        valuation_date: date_from_json(
            v.get("valuationDate").context("valuationDate is required")?,
        ),
        validate_only: matches!(v.get("validateOnly"), Some(serde_json::Value::Bool(true))),
    })
}

/// A `google.type.Date` from its canonical JSON: three numbers, not a string.
///
/// A free function rather than a closure inside the decoder above, because
/// `//proto:mirrors_test` reads the field names a `*_request` decoder mentions
/// and would take `year`, `month` and `day` for fields of the REQUEST. Keeping
/// nested messages in their own decoder keeps that check honest.
fn date_from_json(v: &serde_json::Value) -> Option<ratio_proto::date_proto::google::r#type::Date> {
    let num = |k: &str| -> i32 { v.get(k).and_then(serde_json::Value::as_i64).unwrap_or(0) as i32 };
    Some(ratio_proto::date_proto::google::r#type::Date {
        year: num("year"),
        month: num("month"),
        day: num("day"),
    })
}

/// The HTTP body is the `book` field (`body: "book"`); `bookId` is the query.
///
/// ⚠ Kind and displayName are parsed in `book_from_json` so this function's
/// string literals stay the request's own fields — `//proto:mirrors_test`
/// reads every `"word"` here as a CreateBookRequest field.
fn create_book_request(query: &str, body: &str) -> Result<pb::CreateBookRequest> {
    let v: serde_json::Value =
        serde_json::from_str(if body.trim().is_empty() { "{}" } else { body })
            .context("the request body is not JSON")?;
    let book_v = v.get("book").unwrap_or(&v);
    Ok(pb::CreateBookRequest {
        book: Some(book_from_json(book_v)?),
        book_id: param_of(query, "bookId").to_string(),
    })
}

fn book_from_json(v: &serde_json::Value) -> Result<pb::Book> {
    let display_name = match v.get("displayName") {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => bail!("displayName must be a string, not {other}"),
    };
    let kind = match v.get("kind") {
        None | Some(serde_json::Value::Null) => 0,
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "PERSONAL" | "KIND_PERSONAL" => 1,
            "INVESTMENT" | "KIND_INVESTMENT" => 2,
            "PROJECT" | "KIND_PROJECT" => 3,
            "OPERATING" | "KIND_OPERATING" => 4,
            _ => 0,
        },
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(0) as i32,
        Some(other) => bail!("kind must be a string or number, not {other}"),
    };
    Ok(pb::Book {
        display_name,
        kind,
        ..Default::default()
    })
}

fn admit_facts_request(parent: &str, body: &str) -> Result<pb::AdmitFactsRequest> {
    let v: serde_json::Value =
        serde_json::from_str(body).context("the request body is not JSON")?;
    Ok(pb::AdmitFactsRequest {
        parent: parent.to_string(),
        validate_only: matches!(v.get("validateOnly"), Some(serde_json::Value::Bool(true))),
    })
}

/// `?filter=blocking` → `"blocking"`.
fn filter_of(query: &str) -> &str {
    param_of(query, "filter")
}

/// One query parameter, unescaped only insofar as it needs to be.
///
/// Values here are digests and short keywords, so there is nothing to decode —
/// and a hand-rolled percent-decoder on a public endpoint would be a liability
/// out of proportion to what it buys. A value containing `%` simply will not
/// match a digest, which is the correct outcome.
fn param_of<'a>(query: &'a str, key: &str) -> &'a str {
    let prefix = format!("{key}=");
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(prefix.as_str()))
        .unwrap_or("")
}

/// Serialize a prost message as JSON.
///
/// Hand-written rather than via prost's reflection because the console's
/// messages are a closed set and the property that matters — every `int64`
/// crossing as a STRING — is easier to guarantee by writing it than by
/// configuring it.
///
/// EVERY int64, not just the money. proto3's canonical JSON mapping says so,
/// and following it means a generated TypeScript client is correct against this
/// wire form without a translation layer. It is also the easier rule to keep: a
/// per-field judgment about which integers are "safe" as JSON numbers is a
/// judgment somebody eventually makes wrong, and 2^53 is not a boundary anyone
/// notices until a fund is large enough to cross it.
fn to_json<T: JsonView>(m: &T) -> Result<String> {
    Ok(m.to_json())
}

/// How a console message renders. Implemented per message rather than derived,
/// so the field names in the wire form are a deliberate contract with the
/// TypeScript client rather than whatever a derive happened to produce.
pub trait JsonView {
    fn to_json(&self) -> String;
}

pub(crate) fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ── the wire form ─────────────────────────────────────────────────────────
//
// Money is a STRING in every one of these, matching console.proto. A JSON
// number would be parsed as a double by the browser, which is the exact failure
// the integer kernel exists to prevent — and it would happen silently, in the
// last hop, after the arithmetic had been exact the whole way.

use ratio_proto::ratio::console::v1 as pb;

fn state_name(v: i32) -> &'static str {
    match pb::fund::State::try_from(v) {
        Ok(pb::fund::State::AwaitingPrices) => "AWAITING_PRICES",
        Ok(pb::fund::State::Blocked) => "BLOCKED",
        Ok(pb::fund::State::InReview) => "IN_REVIEW",
        Ok(pb::fund::State::Struck) => "STRUCK",
        _ => "UNSPECIFIED",
    }
}

fn severity_name(v: i32) -> &'static str {
    match pb::Severity::try_from(v) {
        Ok(pb::Severity::Low) => "LOW",
        Ok(pb::Severity::Medium) => "MEDIUM",
        Ok(pb::Severity::High) => "HIGH",
        _ => "UNSPECIFIED",
    }
}

fn actor_kind_name(v: i32) -> &'static str {
    match pb::ActorKind::try_from(v) {
        Ok(pb::ActorKind::Person) => "PERSON",
        Ok(pb::ActorKind::Model) => "MODEL",
        _ => "UNSPECIFIED",
    }
}

impl JsonView for pb::Fund {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"displayName\":{},\"currencyCode\":{},\"state\":{},\
             \"trialBalanceDifference\":{},\
             \"entryCount\":{},\"openBreakCount\":{},\"pendingFactCount\":{},\
             \"configDigest\":{},\"defaultView\":{},\"viewCount\":{},\
             \"lotMethod\":{},\"lotMethodDeclared\":{},\"longTermDays\":{},\
             \"washWindowDays\":{},\"washWindowDeclared\":{},\
             \"washKeepHoldingPeriod\":{},\
             \"minTaxShortWeight\":{},\"minTaxDeclared\":{},\
             \"averageCost\":{}}}",
            q(&self.name), q(&self.display_name), q(&self.currency_code),
            q(state_name(self.state)),
            q(&self.trial_balance_difference),
            q(&self.entry_count.to_string()), q(&self.open_break_count.to_string()),
            q(&self.pending_fact_count), q(&self.config_digest),
            q(&self.default_view), q(&self.view_count.to_string()),
            q(&self.lot_method), self.lot_method_declared,
            q(&self.long_term_days.to_string()),
            q(&self.wash_window_days.to_string()), self.wash_window_declared,
            self.wash_keep_holding_period,
            q(&self.min_tax_short_weight.to_string()), self.min_tax_declared,
            self.average_cost
        )
    }
}

/// ⛔ THE FIGURES THAT DEPEND ON WHICH ENTRIES ARE RECOGNISED LIVE HERE, NOT ON
/// `Fund`. A NAV that does not say which view produced it is the row already in
/// HANDOFF.md's failure table — the console and the CLI reporting different
/// NAVs for one book, neither saying which — with a second convention added to
/// make it easier rather than harder to hit.
impl JsonView for pb::View {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"displayName\":{},\"basis\":{},\"settlementOpenDays\":{},\
             \"calendar\":{},\"holidayCount\":{},\"declared\":{},\
             \"recognisedThrough\":{},\"unplaceableEntryCount\":{},\
             \"netAssetValue\":{},\"totalDebit\":{},\"totalCredit\":{},\
             \"openDifference\":{},\"openBreakCount\":{},\"state\":{},\
             \"realizedGain\":{},\"basisRelieved\":{},\"shortTermGain\":{},\
             \"longTermGain\":{},\"unclassifiedGain\":{},\"openLotCount\":{},\
             \"positionCount\":{},\"navStrike\":{},\"journalPosition\":{}}}",
            q(&self.name), q(&self.display_name), q(basis_name(self.basis)),
            q(&self.settlement_open_days.to_string()), q(&self.calendar),
            q(&self.holiday_count.to_string()), self.declared,
            date_json(&self.recognised_through),
            q(&self.unplaceable_entry_count.to_string()),
            q(&self.net_asset_value), q(&self.total_debit), q(&self.total_credit),
            q(&self.open_difference), q(&self.open_break_count.to_string()),
            q(state_name(self.state)),
            q(&self.realized_gain), q(&self.basis_relieved),
            q(&self.short_term_gain), q(&self.long_term_gain),
            q(&self.unclassified_gain), q(&self.open_lot_count.to_string()),
            q(&self.position_count.to_string()), duration_json(&self.nav_strike),
            q(&self.journal_position.to_string())
        )
    }
}

impl JsonView for pb::ListViewsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"views\":[{}],\"nextPageToken\":{}}}",
            self.views.iter().map(|v| v.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::RecognitionDifference {
    fn to_json(&self) -> String {
        format!(
            "{{\"entryId\":{},\"memo\":{},\"tradeDate\":{},\
             \"recognisedHere\":{},\"recognisedThere\":{},\
             \"netAssetValueEffect\":{}}}",
            q(&self.entry_id), q(&self.memo), date_json(&self.trade_date),
            date_json(&self.recognised_here), date_json(&self.recognised_there),
            q(&self.net_asset_value_effect)
        )
    }
}

fn plan_group_name(v: i32) -> &'static str {
    match pb::plan_node::Group::try_from(v) {
        Ok(pb::plan_node::Group::Recorded) => "RECORDED",
        Ok(pb::plan_node::Group::Maintained) => "MAINTAINED",
        _ => "UNSPECIFIED",
    }
}

/// ⛔ `UNREAD` IS NOT `REJECTED`, AND A CLIENT COLLAPSING THEM LOSES THE
/// THEOREM. A rejected step is a plan that would have worked and cost more; an
/// unread one is never touched at all, which is what
/// `Ratio.Closure.factored_nav_never_reads_the_lots` says about twenty million
/// tax lots.
fn plan_role_name(v: i32) -> &'static str {
    match pb::plan_node::Role::try_from(v) {
        Ok(pb::plan_node::Role::Chosen) => "CHOSEN",
        Ok(pb::plan_node::Role::Rejected) => "REJECTED",
        Ok(pb::plan_node::Role::Refusal) => "REFUSAL",
        Ok(pb::plan_node::Role::Unread) => "UNREAD",
        _ => "UNSPECIFIED",
    }
}

fn plan_edge_kind_name(v: i32) -> &'static str {
    match pb::plan_edge::Kind::try_from(v) {
        Ok(pb::plan_edge::Kind::Flow) => "FLOW",
        Ok(pb::plan_edge::Kind::Refusal) => "REFUSAL",
        Ok(pb::plan_edge::Kind::Unread) => "UNREAD",
        _ => "UNSPECIFIED",
    }
}

impl JsonView for pb::PlanNode {
    fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"operator\":{},\"detail\":{},\"group\":{},\"role\":{},\
             \"cites\":{},\"note\":[{}],\"estimatedReads\":{},\
             \"estimatedDuration\":{},\"actualRows\":{},\"actualDuration\":{}}}",
            q(&self.id), q(&self.operator), q(&self.detail),
            q(plan_group_name(self.group)), q(plan_role_name(self.role)),
            q(&self.cites), strings(&self.note),
            // ⛔ THE EMPTY STRING IS "NO FIGURE", NOT ZERO, and it reaches here
            // as one because `ratio_nav::explain` carries these as `Option`.
            // Rendering an unmeasured step as `0` would read as "instant" and a
            // step nothing costs as "free"; both are claims nobody made.
            q(&self.estimated_reads), duration_json(&self.estimated_duration),
            q(&self.actual_rows), duration_json(&self.actual_duration)
        )
    }
}

impl JsonView for pb::PlanEdge {
    fn to_json(&self) -> String {
        format!(
            "{{\"source\":{},\"target\":{},\"kind\":{},\"rows\":{}}}",
            q(&self.source), q(&self.target), q(plan_edge_kind_name(self.kind)),
            q(&self.rows)
        )
    }
}

impl JsonView for pb::PlanDials {
    fn to_json(&self) -> String {
        format!(
            "{{\"securities\":{},\"currencies\":{},\"lotsPer\":{},\
             \"openActions\":{},\"accounts\":{},\"totalRows\":{},\"openLots\":{}}}",
            q(&self.securities), q(&self.currencies), q(&self.lots_per),
            q(&self.open_actions), q(&self.accounts), q(&self.total_rows),
            q(&self.open_lots)
        )
    }
}

impl JsonView for pb::ExplainNavStrikeResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"view\":{},\"nodes\":[{}],\"edges\":[{}],\"dials\":{},\
             \"estimateRefusal\":{},\"analyzed\":{},\"nanosPerRead\":{},\
             \"provenance\":{},\"chosenReads\":{},\"rewriteReads\":{},\
             \"scanReads\":{}}}",
            q(&self.name), q(&self.view),
            self.nodes.iter().map(|n| n.to_json()).collect::<Vec<_>>().join(","),
            self.edges.iter().map(|e| e.to_json()).collect::<Vec<_>>().join(","),
            match &self.dials {
                Some(d) => d.to_json(),
                None => "null".into(),
            },
            q(&self.estimate_refusal), self.analyzed, q(&self.nanos_per_read),
            q(&self.provenance), q(&self.chosen_reads), q(&self.rewrite_reads),
            q(&self.scan_reads)
        )
    }
}

impl JsonView for pb::ReconcileViewsResponse {
    fn to_json(&self) -> String {
        let list = |rows: &[pb::RecognitionDifference]| {
            rows.iter().map(|r| r.to_json()).collect::<Vec<_>>().join(",")
        };
        format!(
            "{{\"name\":{},\"against\":{},\"netAssetValue\":{},\
             \"againstNetAssetValue\":{},\"difference\":{},\
             \"recognisedHere\":[{}],\"recognisedThere\":[{}],\
             \"unplaceable\":[{}],\"journalPosition\":{}}}",
            q(&self.name), q(&self.against), q(&self.net_asset_value),
            q(&self.against_net_asset_value), q(&self.difference),
            list(&self.recognised_here), list(&self.recognised_there),
            list(&self.unplaceable), q(&self.journal_position.to_string())
        )
    }
}

/// How a view's basis is written on the wire.
///
/// ⛔ `BASIS_RECORDED` IS NOT A SETTLEMENT CONVENTION and must never be rendered
/// as "T+0". It is the absence of an election — the journal's own order — and a
/// client that prints it as an agreed basis asserts a decision nobody made.
/// `View.declared` is the field that carries the distinction.
fn basis_name(v: i32) -> &'static str {
    match pb::view::Basis::try_from(v) {
        Ok(pb::view::Basis::Recorded) => "RECORDED",
        Ok(pb::view::Basis::Trade) => "TRADE",
        Ok(pb::view::Basis::Settlement) => "SETTLEMENT",
        _ => "UNSPECIFIED",
    }
}

impl JsonView for pb::ListFundsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"funds\":[{}],\"nextPageToken\":{}}}",
            self.funds.iter().map(|f| f.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

fn book_kind_name(v: i32) -> &'static str {
    match v {
        1 => "PERSONAL",
        2 => "INVESTMENT",
        3 => "PROJECT",
        4 => "OPERATING",
        _ => "UNSPECIFIED",
    }
}

impl JsonView for pb::Book {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"displayName\":{},\"kind\":{},\"currencyCode\":{},\
             \"fund\":{},\"organization\":{},\"defaultView\":{},\
             \"entryCount\":{},\"configDigest\":{},\"trialBalanceDifference\":{},\
             \"budget\":{},\"envelopes\":[{}],\"loans\":[{}],\
             \"partnerCut\":[{}],\"specialAllocations\":[{}],\
             \"feeReceivable\":{}}}",
            q(&self.name),
            q(&self.display_name),
            q(book_kind_name(self.kind)),
            q(&self.currency_code),
            q(&self.fund),
            q(&self.organization),
            q(&self.default_view),
            q(&self.entry_count.to_string()),
            q(&self.config_digest),
            q(&self.trial_balance_difference),
            q(&self.budget),
            self.envelopes.iter().map(|e| e.to_json()).collect::<Vec<_>>().join(","),
            self.loans.iter().map(|l| l.to_json()).collect::<Vec<_>>().join(","),
            self.partner_cut.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            self.special_allocations.iter().map(|s| s.to_json()).collect::<Vec<_>>().join(","),
            q(&self.fee_receivable)
        )
    }
}

impl JsonView for pb::PartnerShare {
    fn to_json(&self) -> String {
        format!(
            "{{\"partner\":{},\"weight\":{}}}",
            q(&self.partner),
            q(&self.weight.to_string())
        )
    }
}

impl JsonView for pb::SpecialAllocation {
    fn to_json(&self) -> String {
        format!(
            "{{\"partner\":{},\"kind\":{},\"weight\":{}}}",
            q(&self.partner),
            q(&self.kind),
            q(&self.weight.to_string())
        )
    }
}

impl JsonView for pb::HouseholdEnvelope {
    fn to_json(&self) -> String {
        format!(
            "{{\"dimension\":{},\"budget\":{}}}",
            q(&self.dimension),
            q(&self.budget)
        )
    }
}

impl JsonView for pb::LoanSchedule {
    fn to_json(&self) -> String {
        format!(
            "{{\"dimension\":{},\"interest\":{}}}",
            q(&self.dimension),
            q(&self.interest)
        )
    }
}

impl JsonView for pb::ListBooksResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"books\":[{}],\"nextPageToken\":{}}}",
            self.books.iter().map(|b| b.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

fn account_type_name(v: i32) -> &'static str {
    match pb::account::Type::try_from(v) {
        Ok(pb::account::Type::Asset) => "ASSET",
        Ok(pb::account::Type::Liability) => "LIABILITY",
        Ok(pb::account::Type::Equity) => "EQUITY",
        Ok(pb::account::Type::Revenue) => "REVENUE",
        Ok(pb::account::Type::Expense) => "EXPENSE",
        _ => "UNSPECIFIED",
    }
}

fn rule_kind_name(v: i32) -> &'static str {
    match pb::rule::Kind::try_from(v) {
        Ok(pb::rule::Kind::Trade) => "TRADE",
        Ok(pb::rule::Kind::Dividend) => "DIVIDEND",
        Ok(pb::rule::Kind::Accrual) => "ACCRUAL",
        Ok(pb::rule::Kind::Mark) => "MARK",
        _ => "UNSPECIFIED",
    }
}

fn strings(v: &[String]) -> String {
    v.iter().map(|s| q(s)).collect::<Vec<_>>().join(",")
}

/// proto3 JSON: every int64 is a string, including a repeated field.
fn int64s(v: &[i64]) -> String {
    v.iter().map(|n| q(&n.to_string())).collect::<Vec<_>>().join(",")
}

impl JsonView for pb::Rule {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"ruleId\":{},\"kind\":{},\"description\":{},\
             \"form\":{},\"accounts\":[{}],\"measured\":{}}}",
            q(&self.name), q(&self.rule_id), q(rule_kind_name(self.kind)),
            q(&self.description), q(&self.form), strings(&self.accounts),
            self.measured
        )
    }
}

impl JsonView for pb::ListRulesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"rules\":[{}],\"nextPageToken\":{}}}",
            self.rules.iter().map(|r| r.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::EntryPosting {
    fn to_json(&self) -> String {
        format!(
            "{{\"account\":{},\"displayName\":{},\"amount\":{}}}",
            q(&self.account), q(&self.display_name), q(&self.amount)
        )
    }
}

impl JsonView for pb::ListEntriesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"entries\":[{}],\"nextPageToken\":{}}}",
            self.entries.iter().map(|e| e.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::Entry {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"entryId\":{},\"memo\":{},\"configDigest\":{},\
             \"postings\":[{}],\"identifiedLots\":[{}],\
             \"identifiedLotsDeclared\":{}}}",
            q(&self.name), q(&self.entry_id), q(&self.memo), q(&self.config_digest),
            self.postings.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            int64s(&self.identified_lots), self.identified_lots_declared
        )
    }
}

impl JsonView for pb::ApplyEventResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"entry\":{},\"validateOnly\":{},\"netAssetValue\":{},\
             \"previousNetAssetValue\":{}}}",
            self.entry.as_ref().map(|e| e.to_json()).unwrap_or_else(|| "null".into()),
            self.validate_only, q(&self.net_asset_value), q(&self.previous_net_asset_value)
        )
    }
}

fn blocker_name(v: i32) -> &'static str {
    match pb::pending_fact::Blocker::try_from(v) {
        Ok(pb::pending_fact::Blocker::Absent) => "ABSENT",
        Ok(pb::pending_fact::Blocker::Ambiguous) => "AMBIGUOUS",
        _ => "UNSPECIFIED",
    }
}

impl JsonView for pb::Position {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"account\":{},\"accountLabel\":{},\"instrument\":{},\
             \"instrumentLabel\":{},\"quantity\":{},\"value\":{},\"openLotCount\":{},\
             \"markDate\":{},\"priceFact\":{},\"deliveryDigest\":{},\"configDigest\":{}}}",
            q(&self.name), q(&self.account), q(&self.account_label), q(&self.instrument),
            q(&self.instrument_label), q(&self.quantity), q(&self.value),
            q(&self.open_lot_count.to_string()),
            date_json(&self.mark_date),
            q(&self.price_fact), q(&self.delivery_digest), q(&self.config_digest)
        )
    }
}

impl JsonView for pb::ListPositionsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"positions\":[{}],\"nextPageToken\":{}}}",
            self.positions.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::Lot {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"sequence\":{},\"units\":{},\"cost\":{},\"acquired\":{}}}",
            q(&self.name),
            q(&self.sequence.to_string()),
            q(&self.units),
            q(&self.cost),
            date_json(&self.acquired)
        )
    }
}

impl JsonView for pb::ListLotsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"lots\":[{}],\"nextPageToken\":{}}}",
            self.lots.iter().map(|l| l.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::Template {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"templateId\":{},\"factKind\":{},\"form\":{},\"posts\":{}}}",
            q(&self.name), q(&self.template_id), q(&self.fact_kind), q(&self.form), self.posts
        )
    }
}

impl JsonView for pb::ListTemplatesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"templates\":[{}],\"nextPageToken\":{}}}",
            self.templates.iter().map(|t| t.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::RejectedRow {
    fn to_json(&self) -> String {
        format!("{{\"row\":{},\"reason\":{}}}", q(&self.row), q(&self.reason))
    }
}

impl JsonView for pb::IngestDeliveryResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"deliveryDigest\":{},\"rowCount\":{},\"factCount\":{},\
             \"newFactCount\":{},\"readyCount\":{},\"rejected\":[{}],\
             \"pending\":[{}],\"validateOnly\":{}}}",
            q(&self.delivery_digest), q(&self.row_count), q(&self.fact_count),
            q(&self.new_fact_count), q(&self.ready_count),
            self.rejected.iter().map(|r| r.to_json()).collect::<Vec<_>>().join(","),
            self.pending.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            self.validate_only
        )
    }
}

/// proto3 canonical JSON renders a Duration as seconds with an `s` suffix and
/// up to nine fractional digits — `"0.000005291s"`.
///
/// ⚠ FORMATTED, NOT DIVIDED. The seconds and the nanos are printed as digits and
/// concatenated; going through a float to build the string would lose precision
/// at exactly the scale this field is interesting at.
fn duration_json(d: &Option<ratio_proto::duration_proto::google::protobuf::Duration>) -> String {
    match d {
        Some(d) => format!("\"{}.{:09}s\"", d.seconds, d.nanos.unsigned_abs()),
        None => "null".into(),
    }
}

fn date_json(d: &Option<ratio_proto::date_proto::google::r#type::Date>) -> String {
    match d {
        Some(d) => format!(
            "{{\"year\":{},\"month\":{},\"day\":{}}}",
            d.year, d.month, d.day
        ),
        None => "null".into(),
    }
}

impl JsonView for pb::Mark {
    fn to_json(&self) -> String {
        format!(
            "{{\"instrument\":{},\"instrumentLabel\":{},\"quantity\":{},\
             \"carrying\":{},\"market\":{},\"movement\":{},\"price\":{},\
             \"priceDate\":{},\"priceFact\":{},\"deliveryDigest\":{},\
             \"configDigest\":{}}}",
            q(&self.instrument), q(&self.instrument_label), q(&self.quantity),
            q(&self.carrying), q(&self.market), q(&self.movement), q(&self.price),
            date_json(&self.price_date),
            q(&self.price_fact), q(&self.delivery_digest), q(&self.config_digest)
        )
    }
}

impl JsonView for pb::MarkPositionsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"marks\":[{}],\"unpriced\":[{}],\"inexact\":[{}],\
             \"postedCount\":{},\"netAssetValue\":{},\"previousNetAssetValue\":{},\
             \"validateOnly\":{}}}",
            self.marks.iter().map(|m| m.to_json()).collect::<Vec<_>>().join(","),
            self.unpriced.iter().map(|m| m.to_json()).collect::<Vec<_>>().join(","),
            strings(&self.inexact),
            q(&self.posted_count), q(&self.net_asset_value),
            q(&self.previous_net_asset_value), self.validate_only
        )
    }
}

impl JsonView for pb::AdmitFactsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"postedCount\":{},\"recordedCount\":{},\"pendingCount\":{},\
             \"refused\":[{}],\"netAssetValue\":{},\"previousNetAssetValue\":{},\
             \"validateOnly\":{}}}",
            q(&self.posted_count), q(&self.recorded_count), q(&self.pending_count),
            strings(&self.refused), q(&self.net_asset_value),
            q(&self.previous_net_asset_value), self.validate_only
        )
    }
}

impl JsonView for pb::Delivery {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"digest\":{},\"origin\":{},\"receiveTime\":{},\
             \"byteCount\":{},\"factCount\":{},\"pendingFactCount\":{}}}",
            q(&self.name), q(&self.digest), q(&self.origin), // proto3 canonical JSON renders a Timestamp as an RFC 3339 string.
            q(&self
                .receive_time
                .as_ref()
                .map(|t| ratio_nav::rfc3339(t.seconds))
                .unwrap_or_default()),
            q(&self.byte_count), q(&self.fact_count), q(&self.pending_fact_count)
        )
    }
}

impl JsonView for pb::ListDeliveriesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"deliveries\":[{}],\"nextPageToken\":{}}}",
            self.deliveries.iter().map(|d| d.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::PendingFact {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"reference\":{},\"kind\":{},\"blocker\":{},\
             \"detail\":{},\"deliveryDigest\":{},\"row\":{},\"templateId\":{}}}",
            q(&self.name), q(&self.reference), q(&self.kind), q(blocker_name(self.blocker)),
            q(&self.detail), q(&self.delivery_digest), q(&self.row), q(&self.template_id)
        )
    }
}

impl JsonView for pb::ListPendingFactsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"pendingFacts\":[{}],\"nextPageToken\":{}}}",
            self.pending_facts.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::Account {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"displayName\":{},\"dimension\":{},\"type\":{},\
             \"debit\":{},\"credit\":{},\"balance\":{},\"abnormal\":{},\
             \"postingCount\":{},\"currencyTotals\":[{}],\"units\":{},\
             \"unitsIssued\":{},\"unitsRedeemed\":{}}}",
            q(&self.name), q(&self.display_name), q(&self.dimension),
            q(account_type_name(self.r#type)), q(&self.debit), q(&self.credit),
            q(&self.balance), self.abnormal, q(&self.posting_count),
            self.currency_totals.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(","),
            q(&self.units), q(&self.units_issued), q(&self.units_redeemed)
        )
    }
}

impl JsonView for pb::CurrencyTotal {
    fn to_json(&self) -> String {
        format!(
            "{{\"currencyCode\":{},\"debit\":{},\"credit\":{},\"balance\":{},\
             \"rate\":{},\"rateFact\":{},\"deliveryDigest\":{},\"configDigest\":{}}}",
            q(&self.currency_code), q(&self.debit), q(&self.credit),
            q(&self.balance), q(&self.rate), q(&self.rate_fact),
            q(&self.delivery_digest), q(&self.config_digest)
        )
    }
}

impl JsonView for pb::Fact {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"kind\":{},\"reference\":{},\"assertion\":{},\
             \"deliveryDigest\":{},\"row\":{},\"templateId\":{},\
             \"configDigest\":{},\"superseded\":{}}}",
            q(&self.name), q(&self.kind), q(&self.reference), q(&self.assertion),
            q(&self.delivery_digest), q(&self.row), q(&self.template_id),
            q(&self.config_digest), self.superseded
        )
    }
}

impl JsonView for pb::ListFactsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"facts\":[{}],\"nextPageToken\":{}}}",
            self.facts.iter().map(|f| f.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::ListAccountsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"accounts\":[{}],\"nextPageToken\":{}}}",
            self.accounts.iter().map(|a| a.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::Posting {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"entryId\":{},\"memo\":{},\"amount\":{},\
             \"runningBalance\":{},\"configDigest\":{}}}",
            q(&self.name), q(&self.entry_id), q(&self.memo), q(&self.amount),
            q(&self.running_balance), q(&self.config_digest)
        )
    }
}

impl JsonView for pb::ListPostingsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"postings\":[{}],\"nextPageToken\":{}}}",
            self.postings.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::BreakPosting {
    fn to_json(&self) -> String {
        format!(
            "{{\"entryId\":{},\"memo\":{},\"amount\":{},\"configDigest\":{}}}",
            q(&self.entry_id), q(&self.memo), q(&self.amount), q(&self.config_digest)
        )
    }
}

impl JsonView for pb::Tolerance {
    fn to_json(&self) -> String {
        format!(
            "{{\"belowNotice\":{},\"blocksNav\":{},\"declared\":{}}}",
            q(&self.below_notice), q(&self.blocks_nav), self.declared
        )
    }
}

impl JsonView for pb::BreakExplanation {
    fn to_json(&self) -> String {
        format!(
            "{{\"text\":{},\"actor\":{},\"acceptTime\":{},\"difference\":{},\
             \"configDigest\":{},\"journalPosition\":{},\"journalDigest\":{},\
             \"qualification\":[{}]}}",
            q(&self.text), q(&self.actor),
            q(&ratio_nav::rfc3339(self.accept_time.as_ref().map(|t| t.seconds).unwrap_or(0))),
            q(&self.difference), q(&self.config_digest),
            q(&self.journal_position.to_string()), q(&self.journal_digest),
            self.qualification.iter().map(|s| q(s)).collect::<Vec<_>>().join(",")
        )
    }
}

impl JsonView for pb::Break {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"account\":{},\"accountDimension\":{},\"severity\":{},\
             \"explained\":{},\"cause\":{},\"ratioAmount\":{},\"reportedAmount\":{},\
             \"difference\":{},\"postings\":[{}],\"configDigest\":{},\"tolerance\":{},\
             \"explanation\":{}}}",
            q(&self.name), q(&self.account), q(&self.account_dimension.to_string()),
            q(severity_name(self.severity)), self.explained, q(&self.cause),
            q(&self.ratio_amount), q(&self.reported_amount), q(&self.difference),
            self.postings.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            q(&self.config_digest),
            self.tolerance.as_ref().map(|t| t.to_json()).unwrap_or_else(|| "null".into()),
            self.explanation.as_ref().map(|e| e.to_json()).unwrap_or_else(|| "null".into())
        )
    }
}

impl JsonView for pb::ListBreaksResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"breaks\":[{}],\"nextPageToken\":{}}}",
            self.breaks.iter().map(|b| b.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::ChangeLogEntry {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"actor\":{},\"actorKind\":{},\"action\":{},\
             \"subject\":{},\"detail\":{},\"configDigest\":{}}}",
            q(&self.name), q(&self.actor), q(actor_kind_name(self.actor_kind)),
            q(&self.action), q(&self.subject), q(&self.detail), q(&self.config_digest)
        )
    }
}

fn change_kind(v: i32) -> &'static str {
    match pb::rule_change::Kind::try_from(v) {
        Ok(pb::rule_change::Kind::Added) => "ADDED",
        Ok(pb::rule_change::Kind::Removed) => "REMOVED",
        Ok(pb::rule_change::Kind::Changed) => "CHANGED",
        _ => "UNSPECIFIED",
    }
}

impl JsonView for pb::ConfigVersion {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"digest\":{},\"sequence\":{},\"active\":{},\
             \"actor\":{},\"approveTime\":{},\"subject\":{},\"rules\":[{}]}}",
            q(&self.name), q(&self.digest), q(&self.sequence.to_string()), self.active,
            q(&self.actor),
            q(&self
                .approve_time
                .as_ref()
                .map(|t| ratio_nav::rfc3339(t.seconds))
                .unwrap_or_default()),
            q(&self.subject),
            self.rules.iter().map(|r| q(r)).collect::<Vec<_>>().join(",")
        )
    }
}

impl JsonView for pb::ListConfigVersionsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"configVersions\":[{}],\"nextPageToken\":{}}}",
            self.config_versions.iter().map(|v| v.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::RuleChange {
    fn to_json(&self) -> String {
        format!(
            "{{\"ruleId\":{},\"kind\":{},\"baseForm\":{},\"form\":{}}}",
            q(&self.rule_id), q(change_kind(self.kind)), q(&self.base_form), q(&self.form)
        )
    }
}

impl JsonView for pb::DiffConfigVersionsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"baseDigest\":{},\"digest\":{},\"changes\":[{}]}}",
            q(&self.base_digest), q(&self.digest),
            self.changes.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(",")
        )
    }
}

impl JsonView for pb::NavStrike {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"view\":{},\"valuationTime\":{},\"actor\":{},\
             \"journalPosition\":{},\
             \"journalDigest\":{},\"netAssetValue\":{},\"trialBalanceDifference\":{},\
             \"configDigest\":{},\"qualification\":[{}]}}",
            q(&self.name),
            q(&self.view),
            // proto3 canonical JSON renders a Timestamp as an RFC 3339 string.
            q(&self
                .valuation_time
                .as_ref()
                .map(|t| ratio_nav::rfc3339(t.seconds))
                .unwrap_or_default()),
            q(&self.actor),
            q(&self.journal_position.to_string()),
            q(&self.journal_digest),
            q(&self.net_asset_value),
            q(&self.trial_balance_difference),
            q(&self.config_digest),
            strings(&self.qualification)
        )
    }
}

impl JsonView for pb::CorporateAction {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"instrument\":{},\"numerator\":{},\"denominator\":{},\
             \"form\":{},\"exDate\":{},\"announceTime\":{},\"applied\":{},\
             \"journalPosition\":{},\"announcePosition\":{},\
             \"qualifiedNavStrikes\":[{}]}}",
            q(&self.name),
            q(&self.instrument),
            q(&self.numerator),
            q(&self.denominator),
            q(&self.form),
            date_json(&self.ex_date),
            q(&self
                .announce_time
                .as_ref()
                .map(|t| ratio_nav::rfc3339(t.seconds))
                .unwrap_or_default()),
            self.applied,
            q(&self.journal_position.to_string()),
            q(&self.announce_position.to_string()),
            strings(&self.qualified_nav_strikes)
        )
    }
}

impl JsonView for pb::ListCorporateActionsResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"corporateActions\":[{}],\"nextPageToken\":{}}}",
            self.corporate_actions.iter().map(|a| a.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::PeriodClose {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"view\":{},\"closedDate\":{},\"actor\":{},\
             \"journalPosition\":{},\"journalDigest\":{},\"configDigest\":{},\
             \"closingEntry\":{},\"createTime\":{},\"equityDestination\":{},\
             \"surplus\":{}}}",
            q(&self.name),
            q(&self.view),
            date_json(&self.closed_date),
            q(&self.actor),
            q(&self.journal_position.to_string()),
            q(&self.journal_digest),
            q(&self.config_digest),
            q(&self.closing_entry),
            q(&self
                .create_time
                .as_ref()
                .map(|t| ratio_nav::rfc3339(t.seconds))
                .unwrap_or_default()),
            q(&self.equity_destination),
            q(&self.surplus)
        )
    }
}

impl JsonView for pb::ListPeriodClosesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"periodCloses\":[{}],\"nextPageToken\":{}}}",
            self.period_closes.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::ListNavStrikesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"navStrikes\":[{}],\"nextPageToken\":{}}}",
            self.nav_strikes.iter().map(|s| s.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::ReplayNavStrikeResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"historyIntact\":{},\"reproduced\":{},\
             \"netAssetValue\":{},\"journalDigest\":{}}}",
            q(&self.name), self.history_intact, self.reproduced,
            q(&self.net_asset_value), q(&self.journal_digest)
        )
    }
}

impl JsonView for pb::ListChangeLogEntriesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"changeLogEntries\":[{}],\"nextPageToken\":{}}}",
            self.change_log_entries.iter().map(|e| e.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}

impl JsonView for pb::PhaseCost {
    fn to_json(&self) -> String {
        format!(
            "{{\"account\":{},\"displayName\":{},\"cost\":{},\"budget\":{}}}",
            q(&self.account),
            q(&self.display_name),
            q(&self.cost),
            q(&self.budget)
        )
    }
}

impl JsonView for pb::ProjectProgressResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"billed\":{},\"earned\":{},\"billedMinusEarned\":{},\
             \"retainageReceivable\":{},\"retainagePayable\":{},\"phases\":[{}]}}",
            q(&self.name),
            q(&self.billed),
            q(&self.earned),
            q(&self.billed_minus_earned),
            q(&self.retainage_receivable),
            q(&self.retainage_payable),
            self.phases.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(",")
        )
    }
}

impl JsonView for pb::AgingSchedule {
    fn to_json(&self) -> String {
        format!(
            "{{\"current\":{},\"daysThirty\":{},\"daysSixty\":{},\"daysNinety\":{},\
             \"daysOlder\":{},\"undated\":{},\"control\":{}}}",
            q(&self.current),
            q(&self.days_thirty),
            q(&self.days_sixty),
            q(&self.days_ninety),
            q(&self.days_older),
            q(&self.undated),
            q(&self.control)
        )
    }
}

impl JsonView for pb::OperatingAgingResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"receivable\":{},\"payable\":{},\"journalPosition\":{}}}",
            q(&self.name),
            self.receivable
                .as_ref()
                .map(|s| s.to_json())
                .unwrap_or_else(|| "null".into()),
            self.payable
                .as_ref()
                .map(|s| s.to_json())
                .unwrap_or_else(|| "null".into()),
            q(&self.journal_position.to_string())
        )
    }
}
