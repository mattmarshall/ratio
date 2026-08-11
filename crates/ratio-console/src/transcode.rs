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
    Route { method: "GET", template: "/v1/funds" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/breaks" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/changeLogEntries" },
    Route { method: "GET", template: "/v1/{name=funds/*/breaks/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*/changeLogEntries/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/configVersions" },
    Route { method: "GET", template: "/v1/{name=funds/*/configVersions/*}:diff" },
    Route { method: "GET", template: "/v1/{name=funds/*/configVersions/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/deliveries" },
    Route { method: "GET", template: "/v1/{name=funds/*/deliveries/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/pendingFacts" },
    Route { method: "GET", template: "/v1/{name=funds/*/pendingFacts/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/accounts" },
    Route { method: "GET", template: "/v1/{parent=funds/*/accounts/*}/postings" },
    Route { method: "GET", template: "/v1/{name=funds/*/accounts/*/postings/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*/accounts/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/corporateActions" },
    Route { method: "GET", template: "/v1/{name=funds/*/corporateActions/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/navStrikes" },
    // A custom method (AIP-136) on GET, because replaying is safe and
    // idempotent — it folds a journal prefix and writes nothing.
    Route { method: "GET", template: "/v1/{name=funds/*/navStrikes/*}:replay" },
    Route { method: "GET", template: "/v1/{name=funds/*/navStrikes/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/positions" },
    // ⛔ BEFORE the bare position, and before the position pattern can swallow
    // it. `funds/f/positions/p/lots` has more segments than `funds/*/positions/*`
    // matches, but the ordering here is the documented contract and a reader
    // should not have to work that out.
    Route { method: "GET", template: "/v1/{parent=funds/*/positions/*}/lots" },
    Route { method: "GET", template: "/v1/{name=funds/*/positions/*/lots/*}" },
    Route { method: "GET", template: "/v1/{name=funds/*/positions/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/templates" },
    Route { method: "GET", template: "/v1/{name=funds/*/templates/*}" },
    Route { method: "GET", template: "/v1/{parent=funds/*}/rules" },
    Route { method: "GET", template: "/v1/{name=funds/*/rules/*}" },
    // The one write. A custom method (AIP-136) because recording an event is a
    // domain operation with a computed result, not a Create of the thing the
    // caller sent.
    Route { method: "POST", template: "/v1/{parent=funds/*}:applyEvent" },
    Route { method: "POST", template: "/v1/{parent=funds/*}:ingest" },
    Route { method: "POST", template: "/v1/{parent=funds/*}:admit" },
    Route { method: "POST", template: "/v1/{parent=funds/*}:mark" },
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
        bail!("the console API is read-only apart from :applyEvent; {method} is not accepted");
    }
    let rest = path
        .strip_prefix("/v1/")
        .ok_or_else(|| anyhow::anyhow!("{path:?} is not a /v1 path"))?;
    let seg: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();

    // The match arms are the ROUTES table above, in the same order. Written as
    // a slice pattern so an unhandled shape is a compile-time hole rather than
    // a runtime fallthrough.
    let json = match seg.as_slice() {
        ["funds"] => to_json(&console.list_funds()?)?,
        ["funds", id, "breaks"] => {
            to_json(&console.list_breaks(&format!("funds/{id}"), filter_of(query))?)?
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
        ["funds", id, "positions"] => {
            to_json(&console.list_positions(&format!("funds/{id}"))?)?
        }
        ["funds", id, "positions", p, "lots"] => {
            to_json(&console.list_lots(&format!("funds/{id}/positions/{p}"))?)?
        }
        ["funds", id, "positions", p, "lots", l] => {
            to_json(&console.get_lot(&format!("funds/{id}/positions/{p}/lots/{l}"))?)?
        }
        ["funds", id, "positions", p] => {
            to_json(&console.get_position(&format!("funds/{id}/positions/{p}"))?)?
        }
        ["funds", id, "templates"] => {
            to_json(&console.list_templates(&format!("funds/{id}"))?)?
        }
        ["funds", id, "templates", t] => {
            to_json(&console.get_template(&format!("funds/{id}/templates/{t}"))?)?
        }
        ["funds", id, "rules"] => to_json(&console.list_rules(&format!("funds/{id}"))?)?,
        ["funds", id, "rules", r] => to_json(&console.get_rule(&format!("funds/{id}/rules/{r}"))?)?,
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
        ["funds", id, "accounts"] => {
            to_json(&console.list_accounts(&format!("funds/{id}"), filter_of(query))?)?
        }
        ["funds", id, "accounts", a, "postings"] => {
            to_json(&console.list_postings(&format!("funds/{id}/accounts/{a}"))?)?
        }
        ["funds", id, "accounts", a, "postings", p] => {
            to_json(&console.get_posting(&format!("funds/{id}/accounts/{a}/postings/{p}"))?)?
        }
        ["funds", id, "accounts", a] => {
            to_json(&console.get_account(&format!("funds/{id}/accounts/{a}"))?)?
        }
        ["funds", id, "corporateActions"] => {
            to_json(&console.list_corporate_actions(&format!("funds/{id}"))?)?
        }
        ["funds", id, "corporateActions", a] => {
            to_json(&console.get_corporate_action(&format!("funds/{id}/corporateActions/{a}"))?)?
        }
        ["funds", id, "navStrikes"] => {
            to_json(&console.list_nav_strikes(&format!("funds/{id}"))?)?
        }
        // The custom method is matched on the LAST segment before the plain
        // Get, so `x:replay` never falls through to a lookup for a strike
        // literally named `x:replay`.
        ["funds", id, "navStrikes", s] if s.ends_with(":replay") => {
            let strike = s.trim_end_matches(":replay");
            to_json(&console.replay_nav_strike(&format!("funds/{id}/navStrikes/{strike}"))?)?
        }
        ["funds", id, "navStrikes", s] => {
            to_json(&console.get_nav_strike(&format!("funds/{id}/navStrikes/{s}"))?)?
        }
        ["funds", id, "breaks", b] => to_json(&console.get_break(&format!("funds/{id}/breaks/{b}"))?)?,
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
             \"netAssetValue\":{},\"totalDebit\":{},\"totalCredit\":{},\
             \"trialBalanceDifference\":{},\"openDifference\":{},\
             \"entryCount\":{},\"openBreakCount\":{},\"pendingFactCount\":{},\
             \"configDigest\":{},\"lotMethod\":{},\"lotMethodDeclared\":{},\"realizedGain\":{},\
             \"basisRelieved\":{},\"shortTermGain\":{},\"longTermGain\":{},\
             \"unclassifiedGain\":{},\"longTermDays\":{},\"openLotCount\":{},\
             \"positionCount\":{},\"navStrike\":{}}}",
            q(&self.name), q(&self.display_name), q(&self.currency_code),
            q(state_name(self.state)), q(&self.net_asset_value),
            q(&self.total_debit), q(&self.total_credit),
            q(&self.trial_balance_difference), q(&self.open_difference),
            q(&self.entry_count.to_string()), q(&self.open_break_count.to_string()),
            q(&self.pending_fact_count), q(&self.config_digest),
            q(&self.lot_method), self.lot_method_declared,
            q(&self.realized_gain), q(&self.basis_relieved),
            q(&self.short_term_gain), q(&self.long_term_gain),
            q(&self.unclassified_gain), q(&self.long_term_days.to_string()),
            q(&self.open_lot_count.to_string()), q(&self.position_count.to_string()),
            duration_json(&self.nav_strike)
        )
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

impl JsonView for pb::Rule {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"ruleId\":{},\"kind\":{},\"description\":{},\
             \"form\":{},\"accounts\":[{}]}}",
            q(&self.name), q(&self.rule_id), q(rule_kind_name(self.kind)),
            q(&self.description), q(&self.form), strings(&self.accounts)
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

impl JsonView for pb::Entry {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"entryId\":{},\"memo\":{},\"configDigest\":{},\
             \"postings\":[{}]}}",
            q(&self.name), q(&self.entry_id), q(&self.memo), q(&self.config_digest),
            self.postings.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(",")
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
             \"instrumentLabel\":{},\"quantity\":{},\"value\":{},\"openLotCount\":{},\"markDate\":{}}}",
            q(&self.name), q(&self.account), q(&self.account_label), q(&self.instrument),
            q(&self.instrument_label), q(&self.quantity), q(&self.value),
            q(&self.open_lot_count.to_string()),
            date_json(&self.mark_date)
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
             \"priceDate\":{}}}",
            q(&self.instrument), q(&self.instrument_label), q(&self.quantity),
            q(&self.carrying), q(&self.market), q(&self.movement), q(&self.price),
            date_json(&self.price_date)
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
             \"postingCount\":{},\"currencyTotals\":[{}]}}",
            q(&self.name), q(&self.display_name), q(&self.dimension),
            q(account_type_name(self.r#type)), q(&self.debit), q(&self.credit),
            q(&self.balance), self.abnormal, q(&self.posting_count),
            self.currency_totals.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(",")
        )
    }
}

impl JsonView for pb::CurrencyTotal {
    fn to_json(&self) -> String {
        format!(
            "{{\"currencyCode\":{},\"debit\":{},\"credit\":{},\"balance\":{},\
             \"rate\":{}}}",
            q(&self.currency_code), q(&self.debit), q(&self.credit),
            q(&self.balance), q(&self.rate)
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

impl JsonView for pb::Break {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"account\":{},\"accountDimension\":{},\"severity\":{},\
             \"explained\":{},\"cause\":{},\"ratioAmount\":{},\"reportedAmount\":{},\
             \"difference\":{},\"postings\":[{}],\"configDigest\":{}}}",
            q(&self.name), q(&self.account), q(&self.account_dimension.to_string()),
            q(severity_name(self.severity)), self.explained, q(&self.cause),
            q(&self.ratio_amount), q(&self.reported_amount), q(&self.difference),
            self.postings.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(","),
            q(&self.config_digest)
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
            "{{\"name\":{},\"valuationTime\":{},\"actor\":{},\"journalPosition\":{},\
             \"journalDigest\":{},\"netAssetValue\":{},\"trialBalanceDifference\":{},\
             \"configDigest\":{},\"qualification\":[{}]}}",
            q(&self.name),
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
