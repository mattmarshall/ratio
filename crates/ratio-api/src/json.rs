//! The proto3 canonical JSON mapping for the `ratio.v1` messages, by hand.
//!
//! Hand-written for the reason the console's `transcode.rs` is: a JSON codegen
//! plugin for prost (pbjson) would be a second toolchain in the Bazel graph for
//! seven messages that change when the contract does. What makes hand-written
//! acceptable is that it is CHECKED — `//proto:sdk_mirrors_test` reads
//! `ledger.proto` and `chart.proto` and asserts every key written or read here
//! is a field the contract declares, under its canonical name.
//!
//! The mapping follows protobuf's JSON spec where a client library would:
//!
//! * field names are lowerCamelCase — `controlPlaneHash`, `displayName`;
//! * **every int64 is a string** on output (`"amount": "-125000"`), because
//!   JavaScript numbers lose integers past 2^53 and an amount is exact or it
//!   is wrong; on input a number is accepted too;
//! * enums are their names (`"ACCOUNT_TYPE_ASSET"`); a number is accepted;
//! * an absent `optional` field is omitted; an empty string is omitted; a
//!   required int64 is always written, zero included.
//!
//! Errors use the `google.rpc.Status` shape every Google-style REST API uses,
//! so a client written against any of them reads ours without a special case.

use serde_json::{Map, Value};

use ratio_proto::ratio::v1 as pb;

/// A message that has a JSON view.
pub trait JsonView: Sized {
    fn to_json(&self) -> Value;
    fn from_json(v: &Value) -> Result<Self, String>;
}

type Obj = Map<String, Value>;

fn put_i64(o: &mut Obj, key: &str, v: i64) {
    o.insert(key.to_string(), Value::String(v.to_string()));
}

fn put_opt_i64(o: &mut Obj, key: &str, v: Option<i64>) {
    if let Some(v) = v {
        put_i64(o, key, v);
    }
}

fn put_str(o: &mut Obj, key: &str, v: &str) {
    if !v.is_empty() {
        o.insert(key.to_string(), Value::String(v.to_string()));
    }
}

fn put_opt_str(o: &mut Obj, key: &str, v: &Option<String>) {
    if let Some(v) = v {
        o.insert(key.to_string(), Value::String(v.clone()));
    }
}

fn put_enum(o: &mut Obj, key: &str, name: &str) {
    o.insert(key.to_string(), Value::String(name.to_string()));
}

fn put_list<T: JsonView>(o: &mut Obj, key: &str, items: &[T]) {
    o.insert(key.to_string(), Value::Array(items.iter().map(JsonView::to_json).collect()));
}

fn object(v: &Value) -> Result<&Obj, String> {
    v.as_object().ok_or_else(|| "expected a JSON object".to_string())
}

/// An int64: a decimal string per the spec, or a number for a client that
/// sends one. A float or a fraction is refused — an amount is exact or wrong.
fn get_i64(v: &Value, key: &str) -> Result<i64, String> {
    Ok(get_opt_i64(v, key)?.unwrap_or(0))
}

fn get_opt_i64(v: &Value, key: &str) -> Result<Option<i64>, String> {
    match object(v)?.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("{key}: {n} is not an integer that fits in 64 bits")),
        Some(Value::String(s)) => s
            .trim()
            .parse::<i64>()
            .map(Some)
            .map_err(|_| format!("{key}: {s:?} is not a 64-bit integer")),
        Some(other) => Err(format!("{key}: expected an integer, got {other}")),
    }
}

fn get_str(v: &Value, key: &str) -> Result<String, String> {
    Ok(get_opt_str(v, key)?.unwrap_or_default())
}

fn get_opt_str(v: &Value, key: &str) -> Result<Option<String>, String> {
    match object(v)?.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(format!("{key}: expected a string, got {other}")),
    }
}

fn get_list<T: JsonView>(v: &Value, key: &str) -> Result<Vec<T>, String> {
    match object(v)?.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, item)| T::from_json(item).map_err(|e| format!("{key}[{i}]: {e}")))
            .collect(),
        Some(other) => Err(format!("{key}: expected a list, got {other}")),
    }
}

/// An enum: its name per the spec, or its number.
fn get_enum(v: &Value, key: &str, from_name: fn(&str) -> Option<i32>) -> Result<i32, String> {
    match object(v)?.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(Value::String(s)) => from_name(s).ok_or_else(|| format!("{key}: {s:?} is not a known value")),
        Some(Value::Number(n)) => n
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| format!("{key}: {n} is not an enum number")),
        Some(other) => Err(format!("{key}: expected an enum name, got {other}")),
    }
}

fn account_type_name(n: i32) -> &'static str {
    pb::AccountType::try_from(n).map(|t| t.as_str_name()).unwrap_or("ACCOUNT_TYPE_UNSPECIFIED")
}

fn side_name(n: i32) -> &'static str {
    pb::Side::try_from(n).map(|s| s.as_str_name()).unwrap_or("SIDE_UNSPECIFIED")
}

impl JsonView for pb::Posting {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_i64(&mut o, "dim", self.dim);
        put_i64(&mut o, "amount", self.amount);
        put_opt_str(&mut o, "currencyCode", &self.currency_code);
        put_opt_str(&mut o, "instrument", &self.instrument);
        put_opt_i64(&mut o, "quantity", self.quantity);
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::Posting {
            dim: get_i64(v, "dim")?,
            amount: get_i64(v, "amount")?,
            currency_code: get_opt_str(v, "currencyCode")?,
            instrument: get_opt_str(v, "instrument")?,
            quantity: get_opt_i64(v, "quantity")?,
        })
    }
}

impl JsonView for pb::Transaction {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_str(&mut o, "name", &self.name);
        put_list(&mut o, "postings", &self.postings);
        put_str(&mut o, "controlPlaneHash", &self.control_plane_hash);
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::Transaction {
            name: get_str(v, "name")?,
            postings: get_list(v, "postings")?,
            control_plane_hash: get_str(v, "controlPlaneHash")?,
        })
    }
}

impl JsonView for pb::ListTransactionsResponse {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_list(&mut o, "transactions", &self.transactions);
        put_str(&mut o, "nextPageToken", &self.next_page_token);
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::ListTransactionsResponse {
            transactions: get_list(v, "transactions")?,
            next_page_token: get_str(v, "nextPageToken")?,
        })
    }
}

impl JsonView for pb::Account {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_str(&mut o, "name", &self.name);
        put_i64(&mut o, "dim", self.dim);
        put_str(&mut o, "displayName", &self.display_name);
        put_enum(&mut o, "accountType", account_type_name(self.account_type));
        put_enum(&mut o, "normalSide", side_name(self.normal_side));
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::Account {
            name: get_str(v, "name")?,
            dim: get_i64(v, "dim")?,
            display_name: get_str(v, "displayName")?,
            account_type: get_enum(v, "accountType", |s| pb::AccountType::from_str_name(s).map(|t| t as i32))?,
            normal_side: get_enum(v, "normalSide", |s| pb::Side::from_str_name(s).map(|t| t as i32))?,
        })
    }
}

impl JsonView for pb::ListAccountsResponse {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_list(&mut o, "accounts", &self.accounts);
        put_str(&mut o, "nextPageToken", &self.next_page_token);
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::ListAccountsResponse {
            accounts: get_list(v, "accounts")?,
            next_page_token: get_str(v, "nextPageToken")?,
        })
    }
}

impl JsonView for pb::AccountBalance {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_str(&mut o, "account", &self.account);
        put_i64(&mut o, "debits", self.debits);
        put_i64(&mut o, "credits", self.credits);
        put_opt_str(&mut o, "currencyCode", &self.currency_code);
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::AccountBalance {
            account: get_str(v, "account")?,
            debits: get_i64(v, "debits")?,
            credits: get_i64(v, "credits")?,
            currency_code: get_opt_str(v, "currencyCode")?,
        })
    }
}

impl JsonView for pb::TrialBalance {
    fn to_json(&self) -> Value {
        let mut o = Obj::new();
        put_str(&mut o, "name", &self.name);
        put_i64(&mut o, "debits", self.debits);
        put_i64(&mut o, "credits", self.credits);
        put_i64(&mut o, "difference", self.difference);
        put_list(&mut o, "accountBalances", &self.account_balances);
        put_str(&mut o, "controlPlaneHash", &self.control_plane_hash);
        Value::Object(o)
    }

    fn from_json(v: &Value) -> Result<Self, String> {
        Ok(pb::TrialBalance {
            name: get_str(v, "name")?,
            debits: get_i64(v, "debits")?,
            credits: get_i64(v, "credits")?,
            difference: get_i64(v, "difference")?,
            account_balances: get_list(v, "accountBalances")?,
            control_plane_hash: get_str(v, "controlPlaneHash")?,
        })
    }
}

/// The `google.rpc.Status` JSON an error is reported as:
/// `{"error": {"code": 9, "message": "…", "status": "FAILED_PRECONDITION"}}`.
pub fn status_json(code: tonic::Code, message: &str) -> Value {
    let mut error = Obj::new();
    error.insert("code".into(), Value::from(code as i32));
    error.insert("message".into(), Value::String(message.to_string()));
    error.insert("status".into(), Value::String(code_name(code).to_string()));
    let mut o = Obj::new();
    o.insert("error".into(), Value::Object(error));
    Value::Object(o)
}

/// The `google.rpc.Code` constant name.
pub fn code_name(code: tonic::Code) -> &'static str {
    use tonic::Code::*;
    match code {
        Ok => "OK",
        Cancelled => "CANCELLED",
        Unknown => "UNKNOWN",
        InvalidArgument => "INVALID_ARGUMENT",
        DeadlineExceeded => "DEADLINE_EXCEEDED",
        NotFound => "NOT_FOUND",
        AlreadyExists => "ALREADY_EXISTS",
        PermissionDenied => "PERMISSION_DENIED",
        ResourceExhausted => "RESOURCE_EXHAUSTED",
        FailedPrecondition => "FAILED_PRECONDITION",
        Aborted => "ABORTED",
        OutOfRange => "OUT_OF_RANGE",
        Unimplemented => "UNIMPLEMENTED",
        Internal => "INTERNAL",
        Unavailable => "UNAVAILABLE",
        DataLoss => "DATA_LOSS",
        Unauthenticated => "UNAUTHENTICATED",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int64_is_a_string_on_the_way_out_and_either_on_the_way_in() {
        let p = pb::Posting { dim: 2, amount: -125_000, currency_code: None, instrument: None, quantity: None };
        let j = p.to_json();
        assert_eq!(j["dim"], "2");
        assert_eq!(j["amount"], "-125000");
        assert!(j.get("currencyCode").is_none(), "absent optional is omitted");
        let from_string: pb::Posting = JsonView::from_json(&j).unwrap();
        assert_eq!(from_string, p);
        let from_number: pb::Posting =
            JsonView::from_json(&serde_json::json!({"dim": 2, "amount": -125000})).unwrap();
        assert_eq!(from_number, p);
        assert!(pb::Posting::from_json(&serde_json::json!({"dim": 2, "amount": 1.5})).is_err());
        assert!(pb::Posting::from_json(&serde_json::json!({"dim": 2, "amount": "1e3"})).is_err());
    }

    #[test]
    fn a_transaction_round_trips_with_every_field() {
        let t = pb::Transaction {
            name: "books/b/transactions/t".into(),
            postings: vec![pb::Posting {
                dim: 1,
                amount: 7,
                currency_code: Some("USD".into()),
                instrument: Some("AAPL".into()),
                quantity: Some(3),
            }],
            control_plane_hash: "abc".into(),
        };
        let j = t.to_json();
        assert_eq!(j["controlPlaneHash"], "abc");
        assert_eq!(j["postings"][0]["quantity"], "3");
        assert_eq!(pb::Transaction::from_json(&j).unwrap(), t);
    }

    #[test]
    fn enums_are_names_out_and_names_or_numbers_in() {
        let a = pb::Account {
            name: "books/b/accounts/1".into(),
            dim: 1,
            display_name: "Cash".into(),
            account_type: pb::AccountType::Asset as i32,
            normal_side: pb::Side::Debit as i32,
        };
        let j = a.to_json();
        assert_eq!(j["accountType"], "ACCOUNT_TYPE_ASSET");
        assert_eq!(j["normalSide"], "SIDE_DEBIT");
        assert_eq!(pb::Account::from_json(&j).unwrap(), a);
        let by_number = pb::Account::from_json(&serde_json::json!({"dim": "1", "displayName": "Cash", "accountType": 1})).unwrap();
        assert_eq!(by_number.account_type, pb::AccountType::Asset as i32);
        assert!(pb::Account::from_json(&serde_json::json!({"accountType": "ASSET"})).is_err(), "the short name is not the enum's name");
    }

    #[test]
    fn errors_are_google_rpc_status() {
        let e = status_json(tonic::Code::FailedPrecondition, "nope");
        assert_eq!(e["error"]["code"], 9);
        assert_eq!(e["error"]["status"], "FAILED_PRECONDITION");
        assert_eq!(e["error"]["message"], "nope");
    }
}
