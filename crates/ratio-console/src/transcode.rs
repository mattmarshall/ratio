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

use anyhow::{bail, Result};

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
    Route { method: "GET", template: "/v1/{name=funds/*}" },
];

/// Serve one request. `path` excludes the query; `query` is the raw string.
///
/// Returns the JSON body. An unroutable path is an error, not an empty result —
/// a console that silently receives `{}` from a mistyped URL debugs badly.
pub fn serve(console: &Console, method: &str, path: &str, query: &str) -> Result<String> {
    if method != "GET" {
        bail!("the console API is read-only; {method} is not accepted");
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
        ["funds", id, "breaks", b] => to_json(&console.get_break(&format!("funds/{id}/breaks/{b}"))?)?,
        ["funds", id, "changeLogEntries", e] => {
            to_json(&console.get_change_log_entry(&format!("funds/{id}/changeLogEntries/{e}"))?)?
        }
        ["funds", id] => to_json(&console.get_fund(&format!("funds/{id}"))?)?,
        _ => bail!("no route for {path:?}"),
    };
    Ok(json)
}

/// `?filter=blocking` → `"blocking"`.
fn filter_of(query: &str) -> &str {
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix("filter="))
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

use ratio_proto::ratio::v1 as pb;

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
             \"netAssetValue\":{},\"trialBalanceDifference\":{},\"openDifference\":{},\
             \"entryCount\":{},\"openBreakCount\":{},\"configDigest\":{}}}",
            q(&self.name), q(&self.display_name), q(&self.currency_code),
            q(state_name(self.state)), q(&self.net_asset_value),
            q(&self.trial_balance_difference), q(&self.open_difference),
            q(&self.entry_count.to_string()), q(&self.open_break_count.to_string()),
            q(&self.config_digest)
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

impl JsonView for pb::ListChangeLogEntriesResponse {
    fn to_json(&self) -> String {
        format!(
            "{{\"changeLogEntries\":[{}],\"nextPageToken\":{}}}",
            self.change_log_entries.iter().map(|e| e.to_json()).collect::<Vec<_>>().join(","),
            q(&self.next_page_token)
        )
    }
}
