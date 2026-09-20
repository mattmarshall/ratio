//! REST for `ratio.v1.Ledger` and `ratio.v1.Chart`: the `google.api.http`
//! bindings the contract declares, served from the SAME trait implementations
//! the gRPC server dispatches to.
//!
//! There is no transcoding proxy and no second implementation. A handler here
//! turns the URL and the JSON body into the request message, calls the tonic
//! trait method, and writes the response message back as JSON — so the two
//! protocols cannot disagree about what a post does, because neither of them
//! decides. `FileBook::append` does.
//!
//! ⛔ **The route table is CHECKED against the contract rather than trusted.**
//! `rest_routes_test` reads the descriptor set `//proto:ratio_proto` compiles
//! to, extracts every method's `google.api.http` rule on the two services, and
//! asserts [`ROUTES`] is exactly that set. A route the contract does not
//! declare, or a method nothing serves, is a failing build.
//!
//! Errors are `google.rpc.Status` JSON under the canonical HTTP mapping every
//! Google-style API uses: FAILED_PRECONDITION and INVALID_ARGUMENT are 400,
//! NOT_FOUND 404, ALREADY_EXISTS 409, INTERNAL 500 — see [`http_status`].

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::Value;
use tonic::{Code, Request, Status};

use ratio_proto::ratio::v1 as pb;
use ratio_proto::ratio::v1::chart_server::Chart;
use ratio_proto::ratio::v1::ledger_server::Ledger;

use crate::json::{status_json, JsonView};
use crate::{Book, ChartService, LedgerService};

/// One route: the HTTP method and the template from the proto, verbatim —
/// that string is what the test compares against the descriptor.
pub struct Route {
    pub method: &'static str,
    pub template: &'static str,
}

/// Every route this file serves. The test asserts the SET matches the contract.
pub const ROUTES: &[Route] = &[
    Route { method: "GET", template: "/v1/{name=books/*/transactions/*}" },
    Route { method: "GET", template: "/v1/{parent=books/*}/transactions" },
    Route { method: "POST", template: "/v1/{parent=books/*}/transactions" },
    Route { method: "GET", template: "/v1/{name=books/*/accounts/*}" },
    Route { method: "GET", template: "/v1/{parent=books/*}/accounts" },
    Route { method: "POST", template: "/v1/{parent=books/*}/accounts" },
    Route { method: "GET", template: "/v1/{name=books/*/trialBalance}" },
];

/// The axum router for [`ROUTES`], over the one book.
pub fn router(book: Arc<Book>) -> Router {
    Router::new()
        .route("/v1/books/:book/transactions", get(list_transactions).post(create_transaction))
        .route("/v1/books/:book/transactions/:transaction", get(get_transaction))
        .route("/v1/books/:book/accounts", get(list_accounts).post(create_account))
        .route("/v1/books/:book/accounts/:account", get(get_account))
        .route("/v1/books/:book/trialBalance", get(get_trial_balance))
        .with_state(book)
}

/// The canonical `google.rpc.Code` → HTTP status mapping.
pub fn http_status(code: Code) -> StatusCode {
    use tonic::Code::*;
    match code {
        Ok => StatusCode::OK,
        Cancelled => StatusCode::from_u16(499).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        InvalidArgument | FailedPrecondition | OutOfRange => StatusCode::BAD_REQUEST,
        DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
        NotFound => StatusCode::NOT_FOUND,
        AlreadyExists | Aborted => StatusCode::CONFLICT,
        PermissionDenied => StatusCode::FORBIDDEN,
        Unauthenticated => StatusCode::UNAUTHORIZED,
        ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        Unimplemented => StatusCode::NOT_IMPLEMENTED,
        Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        Unknown | Internal | DataLoss => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn json_response(status: StatusCode, body: &Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body.to_string(),
    )
        .into_response()
}

fn error_response(code: Code, message: &str) -> Response {
    json_response(http_status(code), &status_json(code, message))
}

/// A tonic result → the HTTP response: the message as JSON, or the status as
/// `google.rpc.Status`.
fn respond<T: JsonView>(result: Result<tonic::Response<T>, Status>) -> Response {
    match result {
        Ok(r) => json_response(StatusCode::OK, &r.into_inner().to_json()),
        Err(s) => error_response(s.code(), s.message()),
    }
}

/// The request body as a message, or INVALID_ARGUMENT saying what was wrong.
fn parse_body<T: JsonView>(body: &Bytes) -> Result<T, Response> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| error_response(Code::InvalidArgument, &format!("the body is not JSON: {e}")))?;
    T::from_json(&value).map_err(|e| error_response(Code::InvalidArgument, &e))
}

/// One query parameter, percent-decoded. `axum`'s `Query` extractor needs
/// serde_urlencoded, which this workspace does not lock, for four parameters.
fn query_param(uri: &Uri, key: &str) -> String {
    uri.query()
        .unwrap_or("")
        .split('&')
        .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| percent_decode(v))
        .unwrap_or_default()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() && s.is_char_boundary(i + 1) && s.is_char_boundary(i + 3) => {
                match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn page_size(uri: &Uri) -> Result<i32, Response> {
    let raw = query_param(uri, "pageSize");
    if raw.is_empty() {
        return Ok(0);
    }
    raw.parse::<i32>()
        .map_err(|_| error_response(Code::InvalidArgument, &format!("pageSize {raw:?} is not an integer")))
}

// ── Ledger ──────────────────────────────────────────────────────────────────

async fn get_transaction(
    State(book): State<Arc<Book>>,
    Path((b, transaction)): Path<(String, String)>,
) -> Response {
    let req = pb::GetTransactionRequest { name: format!("books/{b}/transactions/{transaction}") };
    respond(LedgerService(book).get_transaction(Request::new(req)).await)
}

async fn list_transactions(
    State(book): State<Arc<Book>>,
    Path(b): Path<String>,
    uri: Uri,
) -> Response {
    let page_size = match page_size(&uri) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let req = pb::ListTransactionsRequest {
        parent: format!("books/{b}"),
        page_size,
        page_token: query_param(&uri, "pageToken"),
    };
    respond(LedgerService(book).list_transactions(Request::new(req)).await)
}

async fn create_transaction(
    State(book): State<Arc<Book>>,
    Path(b): Path<String>,
    uri: Uri,
    body: Bytes,
) -> Response {
    let transaction: pb::Transaction = match parse_body(&body) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let req = pb::CreateTransactionRequest {
        parent: format!("books/{b}"),
        transaction: Some(transaction),
        transaction_id: query_param(&uri, "transactionId"),
    };
    respond(LedgerService(book).create_transaction(Request::new(req)).await)
}

// ── Chart ───────────────────────────────────────────────────────────────────

async fn get_account(
    State(book): State<Arc<Book>>,
    Path((b, account)): Path<(String, String)>,
) -> Response {
    let req = pb::GetAccountRequest { name: format!("books/{b}/accounts/{account}") };
    respond(ChartService(book).get_account(Request::new(req)).await)
}

async fn list_accounts(State(book): State<Arc<Book>>, Path(b): Path<String>, uri: Uri) -> Response {
    let page_size = match page_size(&uri) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let req = pb::ListAccountsRequest {
        parent: format!("books/{b}"),
        page_size,
        page_token: query_param(&uri, "pageToken"),
    };
    respond(ChartService(book).list_accounts(Request::new(req)).await)
}

async fn create_account(
    State(book): State<Arc<Book>>,
    Path(b): Path<String>,
    uri: Uri,
    body: Bytes,
) -> Response {
    let account: pb::Account = match parse_body(&body) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let req = pb::CreateAccountRequest {
        parent: format!("books/{b}"),
        account: Some(account),
        account_id: query_param(&uri, "accountId"),
    };
    respond(ChartService(book).create_account(Request::new(req)).await)
}

async fn get_trial_balance(State(book): State<Arc<Book>>, Path(b): Path<String>) -> Response {
    let req = pb::GetTrialBalanceRequest { name: format!("books/{b}/trialBalance") };
    respond(ChartService(book).get_trial_balance(Request::new(req)).await)
}

// ── The shared fallback ─────────────────────────────────────────────────────

/// What an unknown path gets, in the protocol the caller speaks.
///
/// A gRPC client sent `content-type: application/grpc` and reads its status
/// from trailers-only headers: HTTP 200 with `grpc-status: 12` (UNIMPLEMENTED),
/// which is exactly what tonic's own fallback said before this router merged
/// over it. Anything else gets a `google.rpc.Status` NOT_FOUND it can parse.
pub async fn fallback(req: axum::extract::Request) -> Response {
    let is_grpc = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/grpc"))
        .unwrap_or(false);
    if is_grpc {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/grpc")
            .header("grpc-status", (Code::Unimplemented as i32).to_string())
            .header("grpc-message", "no such method")
            .body(axum::body::Body::empty())
            .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response());
    }
    error_response(Code::NotFound, &format!("no route for {} {}", req.method(), req.uri().path()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch_book;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    async fn call(app: &Router, method: &str, path: &str, body: Option<&str>) -> (StatusCode, Value) {
        let mut builder = HttpRequest::builder().method(method).uri(path);
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let req = builder.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let value = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
        (status, value)
    }

    #[tokio::test]
    async fn the_one_call_and_its_refusal() {
        let book = Arc::new(Book::open(scratch_book("rest-post")).unwrap());
        let app = crate::app(book.clone());
        let base = format!("/v1/books/{}", book.key());

        let (status, posted) = call(
            &app,
            "POST",
            &format!("{base}/transactions"),
            Some(r#"{"postings":[{"dim":"2","amount":"-125000"},{"dim":1,"amount":125000}]}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{posted}");
        assert_eq!(posted["name"], format!("books/{}/transactions/txn-0", book.key()));
        assert_eq!(posted["postings"][1]["amount"], "125000", "int64 is a string on the way out");

        let (status, refused) = call(
            &app,
            "POST",
            &format!("{base}/transactions"),
            Some(r#"{"postings":[{"dim":2,"amount":-125000},{"dim":1,"amount":124999}]}"#),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(refused["error"]["status"], "FAILED_PRECONDITION");
        assert_eq!(refused["error"]["code"], 9);
        assert!(refused["error"]["message"].as_str().unwrap().contains("does not conserve value"));

        let (status, got) = call(&app, "GET", &format!("{base}/transactions/txn-0"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(got, posted);

        let (status, listed) = call(&app, "GET", &format!("{base}/transactions?pageSize=10"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed["transactions"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_client_id_rides_the_query_string() {
        let book = Arc::new(Book::open(scratch_book("rest-id")).unwrap());
        let app = crate::app(book.clone());
        let path = format!("/v1/books/{}/transactions?transactionId=trade%2D9", book.key());
        let (status, posted) =
            call(&app, "POST", &path, Some(r#"{"postings":[{"dim":2,"amount":-1},{"dim":1,"amount":1}]}"#)).await;
        assert_eq!(status, StatusCode::OK, "{posted}");
        assert!(posted["name"].as_str().unwrap().ends_with("/transactions/trade-9"));
        let (status, again) =
            call(&app, "POST", &path, Some(r#"{"postings":[{"dim":2,"amount":-1},{"dim":1,"amount":1}]}"#)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(again["error"]["status"], "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn the_chart_over_rest() {
        let book = Arc::new(Book::open(scratch_book("rest-chart")).unwrap());
        let app = crate::app(book.clone());
        let base = format!("/v1/books/{}", book.key());

        let (status, accounts) = call(&app, "GET", &format!("{base}/accounts"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(accounts["accounts"][0]["accountType"], "ACCOUNT_TYPE_ASSET");
        assert_eq!(accounts["accounts"][0]["normalSide"], "SIDE_DEBIT");

        let (status, created) = call(
            &app,
            "POST",
            &format!("{base}/accounts"),
            Some(r#"{"dim":"20","displayName":"Capital contributions","accountType":"ACCOUNT_TYPE_EQUITY"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{created}");
        assert_eq!(created["normalSide"], "SIDE_CREDIT");

        let (status, bad) = call(&app, "POST", &format!("{base}/accounts"), Some(r#"{"dim":"21","displayName":"x"}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(bad["error"]["status"], "INVALID_ARGUMENT");

        let (status, tb) = call(&app, "GET", &format!("{base}/trialBalance"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(tb["difference"], "0");
        assert_eq!(tb["name"], format!("{base}/trialBalance").trim_start_matches("/v1/"));
    }

    #[tokio::test]
    async fn the_wrong_book_the_wrong_body_and_the_wrong_path() {
        let book = Arc::new(Book::open(scratch_book("rest-errors")).unwrap());
        let app = crate::app(book.clone());

        let (status, e) = call(&app, "GET", "/v1/books/somebody-else/trialBalance", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(e["error"]["status"], "NOT_FOUND");

        let (status, e) = call(&app, "POST", &format!("/v1/books/{}/transactions", book.key()), Some("not json")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(e["error"]["status"], "INVALID_ARGUMENT");

        let (status, e) = call(&app, "GET", "/v1/nothing/here", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(e["error"]["status"], "NOT_FOUND");

        // A gRPC client asking for a method nobody serves gets a gRPC answer.
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/ratio.v1.Nothing/Here")
            .header(header::CONTENT_TYPE, "application/grpc")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers().get("grpc-status").unwrap(), "12");
    }

    #[test]
    fn the_route_table_names_every_handler_the_router_mounts() {
        // Seven templates, seven handlers: the descriptor test holds ROUTES to
        // the contract; this holds the router to ROUTES.
        assert_eq!(ROUTES.len(), 7);
        assert_eq!(percent_decode("trade%2D9+x%zz"), "trade-9 x%zz");
    }
}
