//! The three screens. PLAN.md's UI section, served from the book.
//!
//! | route | screen | why it exists |
//! |---|---|---|
//! | `/` | trial balance | the demo's payoff and the wedge's evidence — live, and every total drills to the postings behind it |
//! | `/breaks` | break report | the wedge's deliverable, the thing a customer pays for |
//! | `/rules` | rules and their checks | what was approved, which checks passed, what is still waiting on a person |
//!
//! Three, not four. PLAN.md is explicit that there is no portal, no dashboard
//! and no settings screen, and no rule editor — the MCP conversation *is* the
//! authoring interface, so building an editor would be building the thing the
//! demo exists to make unnecessary.
//!
//! # Why this is hand-rolled
//!
//! It serves three pages and four JSON endpoints to one browser on `localhost`
//! for the length of a demo. A web framework would be more code than this, not
//! less, and would put an async runtime and a dependency tree behind something
//! whose whole job is to render a table every 400ms. `TcpListener` and a thread
//! per connection are the right size for the problem.
//!
//! **A demo surface, not a product surface.** Loopback only, a fixed route
//! table, reads and never writes. There is no route by which a request reaches
//! the book's contents, and nothing here can approve, post, or configure
//! anything. Whatever eventually faces a network is the gRPC server's job.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use prost::Message;
use ratio_chart::{normal_side, Side};
use ratio_proto::ratio::console::v1 as pb;
// The kernel's contracts — a shadow run's report is a `ratio.v1` message, and
// the console's are `ratio.console.v1`. Two aliases because they are two APIs.
use ratio_proto::ratio::v1 as kernel;
use ratio_rules::{check, render as render_rule, RuleSet};
use ratio_store::{ConfigStore, FileBook, Journal};

/// Serve the screens until interrupted.
pub fn watch(book: PathBuf, port: u16) -> Result<()> {
    // Fail on a bad book here rather than rendering an error page later.
    FileBook::open(&book).with_context(|| format!("opening book at {}", book.display()))?;

    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = TcpListener::bind(addr)
        .with_context(|| format!("binding {addr} — is another `ratio watch` running?"))?;
    // Port 0 means "any free port", so report what was actually bound.
    let addr = listener.local_addr()?;

    println!("ratio  http://{addr}/");
    println!("  /         trial balance");
    println!("  /breaks   break report");
    println!("  /rules    rules and their checks");
    println!("book   {}", book.display());
    println!("(ctrl-c to stop)");

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue }; // one dropped connection is not the end
        let book = book.clone();
        std::thread::spawn(move || {
            let _ = handle(stream, &book);
        });
    }
    Ok(())
}

/// One parsed request. Only what the routes actually need.
struct Request {
    method: String,
    path: String,
    query: String,
    body: String,
    /// The `x-amzn-request-context` header, verbatim, or empty if absent.
    ///
    /// On the deployed surface the Lambda Web Adapter puts the API Gateway
    /// event's `requestContext` here as JSON, and a JWT authorizer's verified
    /// claims ride inside it. Empty on a local `ratio watch` — there is no
    /// gateway in front of it — which is how the `/v1` arm tells an
    /// authenticated request from a developer's loopback one.
    auth_context: String,

    /// The `Host` header, or empty. Only `Strict-Transport-Security` reads it:
    /// HSTS is sent on our own canonical host and nowhere else.
    host: String,
}

/// Read a request: the request line, the headers, and `Content-Length` bytes of
/// body if there are any.
///
/// The first version of this read only the request line, which was enough while
/// every route was a GET. `POST /mcp` carries the JSON-RPC call in the body, so
/// the body has to be read exactly — read too little and the call is truncated,
/// read greedily and the handler blocks waiting for bytes that never come.
fn read_request(reader: &mut impl BufRead) -> Result<Request> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();

    let mut length = 0usize;
    let mut auth_context = String::new();
    let mut host = String::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break; // end of headers, or the peer hung up
        }
        // `split_once` on the FIRST colon only — the request-context value is
        // JSON and carries its own colons, which must stay in the value.
        if let Some((name, value)) = h.split_once(':') {
            let name = name.trim();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("x-amzn-request-context") {
                auth_context = value.trim().to_string();
            } else if name.eq_ignore_ascii_case("host") {
                host = value.trim().to_string();
            }
        }
    }

    // Cap the body. This endpoint is reachable from the internet when deployed,
    // and an unbounded read is an invitation to fill memory with one request.
    const MAX_BODY: usize = 1 << 20;
    let mut body = String::new();
    if length > 0 {
        if length > MAX_BODY {
            bail!("request body of {length} bytes exceeds the {MAX_BODY}-byte limit");
        }
        let mut buf = vec![0u8; length];
        reader.read_exact(&mut buf)?;
        body = String::from_utf8_lossy(&buf).into_owned();
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Ok(Request { method, path, query, body, auth_context, host })
}

/// The security headers for a response, chosen by content type and whether the
/// request arrived on our own canonical host.
///
/// A demo surface facing the internet should not depend on a browser's
/// defaults. The headers below are cheap, static, and matter before there is a
/// login form on the page.
fn security_headers(
    content_type: &str,
    host: &str,
    canonical_host: Option<&str>,
    idp_domain: Option<&str>,
) -> String {
    let csp = if content_type.starts_with("text/html") {
        // ⚠ 'unsafe-inline' FOR SCRIPT/STYLE, DELIBERATELY. The inline blocks
        // are compile-time constants, and every dynamic node the pages build is
        // made with `textContent` / `createElement` — never `innerHTML`, which
        // `//crates/ratio:ratio_test` enforces — so there is no reflected-
        // injection vector a per-block hash would close here. The directives
        // that DO matter on this surface are the strong ones below:
        // `frame-ancestors` (clickjacking), `object-src`/`base-uri` (plugin and
        // base-tag hijacking), and `default-src 'self'` (no off-origin loads).
        // Hash-based tightening of script-src is a follow-on, gated on a browser
        // check that the built React bundle needs no 'unsafe-eval'.
        //
        // ⛔ THE SIGN-IN FLOW FETCHES ITS TOKENS FROM THE COGNITO DOMAIN, so
        // that origin joins connect-src when it is configured — without it the
        // PKCE token exchange is blocked and sign-in silently fails. The
        // authorize step is a top-level navigation, which CSP does not gate.
        let idp = match idp_domain {
            Some(d) if !d.is_empty() => format!(" https://{d}"),
            _ => String::new(),
        };
        format!(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' \
             'unsafe-inline'; img-src 'self' data:; connect-src 'self'{idp}; form-action \
             'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'"
        )
    } else {
        // An API or a plain-text probe renders nothing and loads nothing.
        "default-src 'none'; frame-ancestors 'none'".to_string()
    };
    let mut h = format!(
        "Content-Security-Policy: {csp}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         X-Frame-Options: DENY\r\n"
    );
    // ⛔ HSTS ONLY ON OUR OWN HOST. On the shared `*.execute-api.amazonaws.com`
    // hostname it would poison HSTS for every other AWS API served from it, so
    // it is sent only once traffic is on the canonical Ratio host.
    if let Some(canonical) = canonical_host {
        if !canonical.is_empty() && host.eq_ignore_ascii_case(canonical) {
            h.push_str("Strict-Transport-Security: max-age=63072000; includeSubDomains\r\n");
        }
    }
    h
}

fn handle(mut stream: TcpStream, book: &Path) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let req = match read_request(&mut reader) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e:#}");
            // A malformed request never parsed a Host, so HSTS cannot apply; the
            // rest of the headers do.
            let canonical = std::env::var("RATIO_CANONICAL_HOST").ok();
            write!(
                stream,
                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\n\
                 Content-Length: {}\r\n{}Connection: close\r\n\r\n{msg}",
                msg.len(),
                security_headers("text/plain; charset=utf-8", "", canonical.as_deref(), None)
            )?;
            return Ok(());
        }
    };

    let json = |r: Result<String>| match r {
        Ok(j) => ("200 OK", "application/json", j),
        // A book mid-write is a normal thing to catch; say so in a shape the
        // page can render rather than dropping the connection.
        Err(e) => (
            "200 OK",
            "application/json",
            format!("{{\"error\":{}}}", quote(&format!("{e:#}"))),
        ),
    };

    let (status, content_type, body) = match (req.method.as_str(), req.path.as_str()) {
        (_, "/") => ("200 OK", "text/html; charset=utf-8", page(CHAT_BODY, "chat")),
        (_, "/balance") => ("200 OK", "text/html; charset=utf-8", page(BALANCE_BODY, "balance")),
        (_, "/breaks") => ("200 OK", "text/html; charset=utf-8", page(BREAKS_BODY, "breaks")),
        (_, "/rules") => ("200 OK", "text/html; charset=utf-8", page(RULES_BODY, "rules")),
        (_, "/balance.json") => json(balance_json(book)),
        (_, "/postings.json") => json(postings_json(book, &req.query)),
        (_, "/breaks.json") => json(breaks_json(book)),
        (_, "/rules.json") => json(rules_json(book)),

        // MCP over Streamable HTTP, so a model can reach the same tools the
        // stdio transport exposes without a process on the caller's machine.
        // The dispatcher is shared with `ratio mcp` — there is one tool list
        // and one fence, not one per transport.
        // The console. Embedded at compile time from //web:console_html, so the
        // binary that serves the API also serves the client and there is no
        // second artifact to deploy, version or get out of step.
        (_, "/app") | (_, "/app/") => ("200 OK", "text/html; charset=utf-8", CONSOLE.to_string()),

        // The console's API, transcoded from ratio.v1.Console's google.api.http
        // rules. //crates/ratio-console:transcode_test asserts these routes are
        // exactly the ones the contract declares.
        (m, p) if p.starts_with("/v1/") => {
            // The console reads a FUNDS ROOT — a directory of books — while
            // every other screen, the MCP tools and the terminal operate on one
            // book. Pointing both at the same path would either give the console
            // a single fund or give the trial balance a directory that is not a
            // book. `RATIO_FUNDS` separates them, and defaults to the book so a
            // single-book deployment keeps working unchanged.
            let root = std::env::var("RATIO_FUNDS").ok();
            let root = root.as_deref().map(std::path::Path::new).unwrap_or(book);

            // Who is asking. On the deployed surface the gateway's JWT authorizer
            // has already verified the token and the claims ride in the
            // request-context header; here we only read them. A local `ratio
            // watch` carries no such header, so `from_request_context` is `None`
            // and the console runs unrestricted as `Local`.
            let subject = ratio_console::auth::from_request_context(&req.auth_context);

            // ⛔ FAIL CLOSED. `RATIO_AUTH=required` is set only in the deployed
            // environment. When it is set and the gateway attached no verified
            // identity, refuse — rather than fall back to the unrestricted
            // `Local` console. So a removed or misconfigured authorizer produces
            // refusal, not open access, and the boundary does not depend on a
            // CloudFormation resource the test suite cannot see. Authorization
            // (which funds this subject may open) stays in Rust, at `book_path`.
            let auth_required = std::env::var("RATIO_AUTH").as_deref() == Ok("required");
            if subject.is_none() && auth_required {
                // A shape the SPA renders as a sign-in prompt, not a blank page.
                (
                    "401 Unauthorized",
                    "application/json",
                    r#"{"error":"sign in required","signIn":true}"#.to_string(),
                )
            } else {
                // ⚠ AN OPEN DEMO, NOT A DROPPED BOUNDARY. `RATIO_DEMO_OPEN` (set
                // only on the shared demo, whose audience is not known ahead of
                // time) grants any AUTHENTICATED caller every fund, while the
                // write stays signed with their id. Sign-in is still required —
                // the 401 above is untouched — and the tenant path (`scoped`) and
                // its tests are unchanged, so turning this off restores per-fund
                // scoping with no other edit.
                let open = std::env::var("RATIO_DEMO_OPEN").map(|v| !v.is_empty()).unwrap_or(false);
                let c = match subject {
                    Some(s) if open => ratio_console::Console::open(root, s),
                    Some(s) => ratio_console::Console::scoped(root, s),
                    None => ratio_console::Console::new(root),
                };
                match ratio_console::transcode::serve(&c, m, p, &req.query, &req.body) {
                Ok(j) => ("200 OK", "application/json", j),
                // A bad resource name is the caller's mistake and a missing
                // fund is a 404; both are told apart by what the console said
                // rather than by guessing from the path.
                Err(e) => {
                    let msg = format!("{e:#}");
                    let status = if msg.contains("no fund") || msg.contains("no route")
                        || msg.contains("no break") || msg.contains("no change-log")
                    {
                        "404 Not Found"
                    } else if msg.contains("read-only") || msg.contains("does not accept POST") {
                        "405 Method Not Allowed"
                    } else if msg.contains("already in this journal") {
                        // A repeated event id is a conflict, not a malformed
                        // request — the caller sent something well-formed that
                        // the journal already has.
                        "409 Conflict"
                    } else {
                        "400 Bad Request"
                    };
                    (status, "application/json", format!("{{\"error\":{}}}", quote(&msg)))
                    }
                }
            }
        }

        // The other half of the fence. A person approves here, at something
        // shaped like the terminal they would really use — and the model has
        // no path to it: its six tools are the whole of what it can reach, and
        // this route is not one of them.
        ("POST", "/terminal.json") => json(terminal_json(book, &req.body)),
        (_, "/terminal.json") => (
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "Commands are POSTed to this path.".to_string(),
        ),

        // The chat demo. Same tools, same dispatcher, same fence as /mcp —
        // this endpoint only adds a model in front of them.
        ("POST", "/chat.json") => json(chat_json(book, &req.body)),
        (_, "/chat.json") => (
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "Chat messages are POSTed to this path.".to_string(),
        ),

        ("POST", "/mcp") => match ratio_mcp::handle_line(book, &req.body) {
            Some(response) => ("200 OK", "application/json", response),
            // A notification has no id and MUST NOT be answered. 202 with an
            // empty body is what the transport says to send.
            None => ("202 Accepted", "application/json", String::new()),
        },
        // Say what is wrong rather than 404ing a correct path with a wrong verb.
        (_, "/mcp") => (
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "MCP requests are POSTed to this path.".to_string(),
        ),

        // A liveness probe for whatever runs this. Cheap on purpose: it must
        // not touch the book, or a probe becomes a load test.
        (_, "/healthz") => ("200 OK", "text/plain; charset=utf-8", "ok".to_string()),

        // Which commit this binary was built from.
        //
        // Lambda serves warm containers on the previous image for a while after
        // a deploy, so "the URL returns 200" does not mean "the URL runs what
        // was just built". A deploy that asserts against the old image passes
        // while shipping nothing, which is how the first /app smoke test failed.
        (_, "/version") => (
            "200 OK",
            "text/plain; charset=utf-8",
            std::env::var("RATIO_BUILD").unwrap_or_else(|_| "dev".into()),
        ),

        // The public OIDC client configuration the sign-in flow bootstraps from,
        // read from the deployment's environment. Inherently public — a client
        // id and a hosted-UI domain are not secrets — so it is served BEFORE
        // authentication (it is one of the routes the gateway leaves open),
        // which is what lets the unauthenticated console start the login. Empty
        // strings on a local `ratio watch`, where none of these are set, so the
        // console stays unauthenticated on loopback.
        //
        // ⚠ Not a `ratio.console.v1` resource: it is deployment configuration,
        // not a fund's data, so it does not belong in the contract or its AIP
        // surface. It is served here beside `/version` for the same reason —
        // both describe the running process, not the book.
        (_, "/authconfig.json") => (
            "200 OK",
            "application/json",
            format!(
                "{{\"issuer\":{},\"clientId\":{},\"domain\":{},\"scopes\":[\"openid\",\"email\"],\
                 \"redirectPath\":\"/app\"}}",
                quote(&std::env::var("RATIO_COGNITO_ISSUER").unwrap_or_default()),
                quote(&std::env::var("RATIO_COGNITO_CLIENT_ID").unwrap_or_default()),
                quote(&std::env::var("RATIO_COGNITO_DOMAIN").unwrap_or_default()),
            ),
        ),

        _ => ("404 Not Found", "text/plain; charset=utf-8", "no".to_string()),
    };

    let canonical = std::env::var("RATIO_CANONICAL_HOST").ok();
    let idp = std::env::var("RATIO_COGNITO_DOMAIN").ok();
    write!(
        stream,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         {}Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n{body}",
        body.len(),
        security_headers(content_type, &req.host, canonical.as_deref(), idp.as_deref())
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// Run one command, from a strict list.
///
/// # Why a whitelist and not a shell
///
/// This endpoint is public. Anything that resolves a command through a shell,
/// or that dispatches on a string the caller controls without an exhaustive
/// match, is a remote-execution seam wearing a demo costume. There are exactly
/// two commands, both parsed here, and an unrecognized one is refused by
/// falling off the end of a `match` rather than by a filter somebody could
/// widen.
///
/// ⛔ `approve` is here and NOT in the MCP tool list, and that asymmetry is the
/// product's central claim rather than an oversight. A person operates this; a
/// model cannot reach it.
fn terminal_json(book: &Path, body: &str) -> Result<String> {
    let req: serde_json::Value =
        serde_json::from_str(body).context("the command is not JSON")?;
    let line = req["command"].as_str().unwrap_or("").trim();
    if line.len() > 200 {
        bail!("that is too long to be one of the two commands this accepts");
    }

    let words: Vec<&str> = line.split_whitespace().collect();
    let output = match words.as_slice() {
        // The command the chat screen tells you to run, running for real.
        ["ratio", "approve", id] | ["approve", id] => {
            // The id comes from the caller and becomes a filename. Anything
            // that is not a proposal id is refused before it reaches the
            // filesystem — `..` in particular.
            if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                bail!("{id:?} is not a proposal id");
            }
            crate::approve_text(book, id)?
        }
        ["ratio", "balance"] | ["balance"] => balance_text(book)?,
        [] => bail!("type a command"),
        _ => bail!(
            "This is a demo terminal, not a shell. It runs two commands:\n\
             \n  ratio approve <proposal-id>\n  ratio balance"
        ),
    };
    Ok(format!("{{\"output\":{}}}", quote(&output)))
}

/// The trial balance as the CLI prints it, for the terminal.
fn balance_text(book: &Path) -> Result<String> {
    let b = FileBook::open(book)?;
    // ⛔ COUNTED, NOT COLLECTED — the whole journal was held resident to print
    // one number in a header.
    let mut entry_count = 0usize;
    b.for_each_entry_since(0, &mut |_| {
        entry_count += 1;
        Ok(())
    })?;
    let names: std::collections::BTreeMap<i64, String> = b
        .accounts()?
        .into_iter()
        .map(|a| (a.dim, a.display_name))
        .collect();
    let tb = b.trial_balance()?;

    let mut out = format!("TRIAL BALANCE — {entry_count} entrie(s)\n");
    if let Some(c) = b.active()? {
        out.push_str(&format!("configuration  {c}\n"));
    }
    // ⛔ ONE ROW PER (ACCOUNT, CURRENCY) — see `Journal::balances_by_dim`. A
    // single row per account added dollars to euros under a currency-free
    // header.
    out.push_str(&format!("\n{:<30}{:<5}{:>16}{:>16}\n", "ACCOUNT", "CCY", "DEBIT", "CREDIT"));
    for ((dim, ccy), (debit, credit)) in b.balances_by_dim()? {
        out.push_str(&format!(
            "{:<30}{:<5}{:>16}{:>16}\n",
            names.get(&dim).cloned().unwrap_or_else(|| format!("dim {dim}")),
            ccy.as_deref().unwrap_or("—"),
            crate::minor(debit),
            crate::minor(credit)
        ));
    }
    out.push_str(&format!("{}\n", "-".repeat(66)));
    out.push_str(&format!(
        "{:<34}{:>16}{:>16}\n{:<34}{:>16}{:>16}\n",
        "Total",
        crate::minor(tb.debits),
        crate::minor(tb.credits),
        "Difference",
        crate::minor(tb.debits - tb.credits),
        crate::minor(0)
    ));
    Ok(out)
}

/// One exchange with the model.
///
/// The transcript lives in the browser and comes back on each turn, so this
/// stays stateless — which is what lets it run on Lambda without a session
/// store. `ratio-agent` bounds how much of it will be accepted.
fn chat_json(book: &Path, body: &str) -> Result<String> {
    let req: serde_json::Value =
        serde_json::from_str(body).context("the chat request is not JSON")?;
    let message = req["message"].as_str().unwrap_or("");
    let reply = ratio_agent::chat(book, &req["history"], message)?;

    let steps: Vec<String> = reply
        .steps
        .iter()
        .map(|s| match s {
            ratio_agent::Step::Said(text) => {
                format!("{{\"kind\":\"said\",\"text\":{}}}", quote(text))
            }
            ratio_agent::Step::Used { tool, input, output, refused } => format!(
                "{{\"kind\":\"used\",\"tool\":{},\"input\":{},\"output\":{},\"refused\":{}}}",
                quote(tool),
                quote(input),
                quote(output),
                refused
            ),
        })
        .collect();

    Ok(format!(
        "{{\"steps\":[{}],\"truncated\":{},\"history\":{}}}",
        steps.join(","),
        reply.truncated,
        serde_json::to_string(&reply.history)?
    ))
}

/// The trial balance.
///
/// Amounts go out as **strings in major units**. A JSON number would be parsed
/// as a double by every consumer, which is the exact failure the integer
/// kernel exists to prevent — a figure that has been exact all the way through
/// should not meet a float in the last six inches of its journey.
fn balance_json(book: &Path) -> Result<String> {
    let b = FileBook::open(book)?;
    // ⛔ COUNTED, NOT COLLECTED — see the note on the screen above.
    let mut entry_count = 0usize;
    b.for_each_entry_since(0, &mut |_| {
        entry_count += 1;
        Ok(())
    })?;
    let names: std::collections::BTreeMap<i64, ratio_store::Account> =
        b.accounts()?.into_iter().map(|a| (a.dim, a)).collect();
    let tb = b.trial_balance()?;

    let mut rows = Vec::new();
    for ((dim, ccy), (debit, credit)) in b.balances_by_dim()? {
        let (label, abnormal) = match names.get(&dim) {
            Some(a) => {
                let net = debit - credit;
                let abnormal = net != 0
                    && ((net > 0 && normal_side(a.account_type.into()) == Side::Credit)
                        || (net < 0 && normal_side(a.account_type.into()) == Side::Debit));
                (a.display_name.clone(), abnormal)
            }
            None => (format!("dim {dim}"), false),
        };
        // ⛔ THE CURRENCY IS PART OF THE ROW'S IDENTITY, not decoration. Two
        // rows now share an `account` and differ by `currency`; a consumer that
        // keys on `account` alone will collide them — which is the bug this
        // came from, moved to where it is visible.
        rows.push(format!(
            "{{\"account\":{dim},\"currency\":{},\"label\":{},\"debit\":{},\
             \"credit\":{},\"abnormal\":{}}}",
            match &ccy {
                Some(c) => quote(c),
                None => "null".to_string(),
            },
            quote(&label),
            quote(&crate::minor(debit)),
            quote(&crate::minor(credit)),
            abnormal
        ));
    }

    Ok(format!(
        "{{\"entries\":{},\"config\":{},\"debits\":{},\"credits\":{},\
         \"difference\":{},\"ties\":{},\"rows\":[{}]}}",
        entry_count,
        match b.active()? {
            Some(c) => quote(c.as_str()),
            None => "null".to_string(),
        },
        quote(&crate::minor(tb.debits)),
        quote(&crate::minor(tb.credits)),
        quote(&crate::minor(tb.debits - tb.credits)),
        tb.debits == tb.credits,
        rows.join(",")
    ))
}

/// The postings behind one account — the drill-down.
fn postings_json(book: &Path, query: &str) -> Result<String> {
    let dim: i64 = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("account="))
        .context("postings.json needs ?account=N")?
        .parse()
        .context("account must be a number")?;

    let b = FileBook::open(book)?;
    let mut rows = Vec::new();
    let mut net = 0i64;
    // ⛔ STREAMED. Reading back the postings behind ONE account never needed the
    // whole journal resident — and this is the book that grows forever.
    b.for_each_entry_since(0, &mut |e| {
        for p in &e.postings {
            if p.dim == dim {
                net += p.amount;
                // Each line carries the configuration in force when it was
                // posted, not the one active now — a period spanning a change
                // has to show which entry used which.
                rows.push(format!(
                    "{{\"id\":{},\"amount\":{},\"config\":{},\"memo\":{}}}",
                    quote(&e.id),
                    quote(&crate::minor(p.amount)),
                    quote(e.config.short()),
                    quote(&e.memo)
                ));
            }
        }
        Ok(())
    })?;
    // A demo book can hold ten thousand entries; sending all of them to draw a
    // drawer nobody scrolls is wasteful, and the count says what was cut.
    let total = rows.len();
    rows.truncate(200);
    Ok(format!(
        "{{\"account\":{dim},\"total\":{total},\"shown\":{},\"net\":{},\"rows\":[{}]}}",
        rows.len(),
        quote(&crate::minor(net)),
        rows.join(",")
    ))
}

/// The newest stored break report.
fn breaks_json(book: &Path) -> Result<String> {
    let dir = book.join("reports");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "pb"))
                .collect()
        })
        .unwrap_or_default();
    // Newest by modification time; the name carries a digest, not an order.
    found.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    let Some(path) = found.last() else {
        return Ok("{\"report\":null}".to_string());
    };

    let report = kernel::BreakReport::decode(&std::fs::read(path)?[..])
        .with_context(|| format!("reading {}", path.display()))?;

    let breaks: Vec<String> = report
        .breaks
        .iter()
        .map(|b| {
            format!(
                "{{\"account\":{},\"label\":{},\"ratio\":{},\"reported\":{},\
                 \"difference\":{},\"cause\":{},\"basis\":{}}}",
                b.account,
                quote(&b.display_name),
                quote(&crate::minor(b.ratio_amount)),
                quote(&crate::minor(b.reported_amount)),
                quote(&crate::minor(b.difference)),
                quote(match kernel::Cause::try_from(b.cause) {
                    Ok(kernel::Cause::AmountDiffers) => "figures differ",
                    Ok(kernel::Cause::AbsentFromReport) => "not in the report",
                    Ok(kernel::Cause::AbsentFromRatio) => "Ratio produced nothing",
                    _ => "unspecified",
                }),
                quote(&b.ratio_basis)
            )
        })
        .collect();

    let exceptions: Vec<String> = report
        .exceptions
        .iter()
        .map(|e| {
            format!(
                "{{\"id\":{},\"refusal\":{},\"detail\":{}}}",
                quote(&e.transaction_id),
                quote(match kernel::Refusal::try_from(e.refusal) {
                    Ok(kernel::Refusal::UnknownTransactionType) => "transaction type not covered",
                    Ok(kernel::Refusal::ForeignCurrency) => "foreign currency",
                    Ok(kernel::Refusal::DisposalWithoutBasis) => "disposal with no basis",
                    Ok(kernel::Refusal::NoRuleForType) => "no rule for this type",
                    _ => "unspecified",
                }),
                quote(&e.detail)
            )
        })
        .collect();

    let scope = report.scope.unwrap_or_default();
    Ok(format!(
        "{{\"report\":{{\"name\":{},\"config\":{},\"replayed\":{},\"posted\":{},\
         \"ties\":{},\"scope\":{},\"currency\":{},\"exclusions\":[{}],\
         \"breaks\":[{}],\"exceptions\":[{}]}}}}",
        quote(&report.name),
        quote(&report.config_digest),
        report.transactions_replayed,
        report.entries_posted,
        report.book_ties,
        quote(&scope.label),
        quote(&scope.base_currency),
        scope.exclusions.iter().map(|x| quote(x)).collect::<Vec<_>>().join(","),
        breaks.join(","),
        exceptions.join(",")
    ))
}

/// The active rules, their checks, and anything still waiting on a person.
fn rules_json(book: &Path) -> Result<String> {
    let b = FileBook::open(book)?;
    let chart = b.accounts()?;
    let digest = b.active()?;
    let set = match &digest {
        Some(d) => RuleSet::from_toml(&String::from_utf8_lossy(&b.get(d)?))?,
        None => RuleSet::default(),
    };

    let findings = check(&set, &chart);
    let rules: Vec<String> = set
        .rules
        .iter()
        .map(|r| {
            let mine: Vec<&ratio_rules::Finding> =
                findings.iter().filter(|f| f.rule == r.id).collect();
            format!(
                "{{\"id\":{},\"description\":{},\"rendered\":{},\
                 \"errors\":[{}],\"questions\":[{}]}}",
                quote(&r.id),
                quote(&r.description),
                quote(&render_rule(r, &chart)),
                mine.iter()
                    .filter(|f| !f.is_question)
                    .map(|f| quote(&f.message))
                    .collect::<Vec<_>>()
                    .join(","),
                mine.iter()
                    .filter(|f| f.is_question)
                    .map(|f| quote(&f.message))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect();

    // Proposals a model drafted that no person has approved. Showing them
    // beside the active rules is the point: the difference between the two
    // lists is exactly what a human decision bought.
    let mut pending = Vec::new();
    if let Ok(rd) = std::fs::read_dir(book.join("proposals")) {
        let mut paths: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        paths.sort();
        for p in paths {
            let id = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let rendered = match RuleSet::from_toml(&text) {
                Ok(s) => s
                    .rules
                    .iter()
                    .map(|r| render_rule(r, &chart))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Err(e) => format!("(does not parse: {e})"),
            };
            // Approving does not delete the proposal — it is a record, and
            // `ratio approve` is idempotent by design. But a proposal that is
            // already live must not keep saying "waiting on a person" after
            // somebody just approved it, which is what the screen did for
            // exactly as long as it took to notice.
            let already = match RuleSet::from_toml(&text) {
                Ok(proposed) => {
                    !proposed.rules.is_empty()
                        && proposed.rules.iter().all(|r| {
                            set.rule(&r.id).is_some_and(|active| active == r)
                        })
                }
                Err(_) => false,
            };
            pending.push(format!(
                "{{\"id\":{},\"rendered\":{},\"active\":{}}}",
                quote(&id),
                quote(&rendered),
                already
            ));
        }
    }

    Ok(format!(
        "{{\"config\":{},\"rules\":[{}],\"pending\":[{}]}}",
        match &digest {
            Some(d) => quote(d.as_str()),
            None => "null".to_string(),
        },
        rules.join(","),
        pending.join(",")
    ))
}

/// Minimal JSON string escaping.
///
/// Account names, memos and rule descriptions all come from the book, which a
/// customer edits, so this has to be correct rather than nearly correct — an
/// unescaped quote in an account name would break the page and look like the
/// ledger was wrong.
fn quote(s: &str) -> String {
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

// ---------------------------------------------------------------------------
// The pages
// ---------------------------------------------------------------------------

/// The operations console, built by //web:console_rs.
///
/// One string: the shell, its stylesheet and the React bundle, inlined. On
/// Lambda a second request for a stylesheet is a second invocation and possibly
/// a second cold start, and the whole page is smaller than the round trip it
/// would save.
#[path = "console_html.rs"]
mod console_html;
use console_html::CONSOLE;

/// Wrap a screen's body in the shared document, marking the current tab.
fn page(body: &str, current: &str) -> String {
    let tab = |slug: &str, href: &str, label: &str| {
        format!(
            r#"<a href="{href}"{}>{label}</a>"#,
            if slug == current { r#" aria-current="page""# } else { "" }
        )
    };
    format!(
        "{HEAD}<nav class=\"tabs\">{}{}{}{}</nav>{body}{FOOT}",
        tab("chat", "/", "Set up the books"),
        tab("balance", "/balance", "Trial balance"),
        tab("breaks", "/breaks", "Break report"),
        tab("rules", "/rules", "Rules")
    )
}

const HEAD: &str = r##"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Ratio</title>
<!-- Inline, so the server stays a fixed route table and the browser never
     404s on a favicon in front of a customer. -->
<link rel="icon" href="data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2064%2064%22%3E%3Crect%20width%3D%2264%22%20height%3D%2264%22%20rx%3D%2212%22%20fill%3D%22%231B6440%22%2F%3E%3Crect%20fill%3D%22%23FCFBF7%22%20x%3D%228.00%22%20y%3D%2219%22%20width%3D%2216.34%22%20height%3D%2210%22%20rx%3D%222%22%2F%3E%20%3Crect%20fill%3D%22%23FCFBF7%22%20x%3D%2229.34%22%20y%3D%2219%22%20width%3D%2226.66%22%20height%3D%2210%22%20rx%3D%222%22%2F%3E%20%3Crect%20fill%3D%22%23FCFBF7%22%20x%3D%228.00%22%20y%3D%2235%22%20width%3D%2248.00%22%20height%3D%2210%22%20rx%3D%222%22%2F%3E%3C%2Fsvg%3E">
<style>
:root{
  --ground:#FCFBF7; --surface:#E6EFE2; --raised:#FFFFFF;
  --text:#0F2418; --text-2:#2B4436; --muted:#4C6053;
  --rule:#C3D6BF; --accent:#1B6440; --warn:#BC4A18;
  color-scheme:light dark;
}
@media (prefers-color-scheme:dark){
  :root{
    --ground:#141816; --surface:#1C211E; --raised:#242A26;
    --text:#E8EAE8; --text-2:#B4BDB6; --muted:#8A938C;
    --rule:#2F3733; --accent:#7FC79D; --warn:#CE7A33;
  }
}
*{box-sizing:border-box}
body{margin:0;padding:28px 20px 64px;background:var(--ground);color:var(--text);
  font:16px/1.5 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}
.wrap{max-width:820px;margin:0 auto}
.tabs{max-width:820px;margin:0 auto 26px;display:flex;gap:4px;flex-wrap:wrap;
  border-bottom:1px solid var(--rule)}
.tabs a{padding:8px 14px;font-size:14px;font-weight:600;text-decoration:none;
  color:var(--muted);border-bottom:2px solid transparent;margin-bottom:-1px}
.tabs a:hover{color:var(--text)}
.tabs a[aria-current="page"]{color:var(--accent);border-bottom-color:var(--accent)}
h1{font-size:15px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);
  font-weight:600;margin:0 0 4px}
.meta{font-size:13px;color:var(--muted);margin:0 0 22px}
code,.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
.meta code{font-size:12px}
.card{background:var(--raised);border:1px solid var(--rule);border-radius:10px;
  overflow-x:auto}
.card+.card{margin-top:16px}
table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}
th{font-size:11px;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);
  font-weight:600;text-align:right;padding:14px 18px 10px;white-space:nowrap}
th:first-child{text-align:left}
td{padding:9px 18px;text-align:right;white-space:nowrap;
  font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:14px}
td:first-child{text-align:left;font-family:inherit;font-size:15px;color:var(--text-2)}
tbody tr+tr td{border-top:1px solid var(--rule)}
tfoot td{border-top:2px solid var(--rule);font-weight:600;padding-top:12px}
tfoot tr+tr td{border-top:1px solid var(--rule)}
.abnormal::after{content:" *";color:var(--warn)}
.tie{display:inline-flex;align-items:center;gap:7px;margin-top:18px;
  font-size:14px;font-weight:600}
.dot{width:9px;height:9px;border-radius:50%;background:var(--accent);flex:none}
.tie.broken{color:var(--warn)} .tie.broken .dot{background:var(--warn)}
.note{font-size:12.5px;color:var(--muted);margin-top:18px}
.empty{padding:28px 20px;color:var(--muted);font-size:14.5px}
/* A changed figure flashes once, so movement is visible without being a
   distraction. Reduced-motion simply does not animate. */
@keyframes flash{from{background:color-mix(in oklab,var(--accent) 26%,transparent)}to{background:transparent}}
.changed{animation:flash .7s ease-out}
@media (prefers-reduced-motion:reduce){.changed{animation:none}}
/* Drill-down. The whole row is the control, so the hit target is the row. */
tbody tr.drill{cursor:pointer}
tbody tr.drill:hover td{background:var(--surface)}
tbody tr.drill:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
tr.postings td{padding:0;background:var(--surface)}
.drawer{padding:10px 18px 14px;font-size:13px}
.drawer table{width:auto;min-width:100%}
.drawer td{padding:4px 12px 4px 0;font-size:12.5px;border:0!important}
.drawer td:first-child{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
.cfg{color:var(--muted)}
pre{margin:0;padding:14px 18px;font-size:13px;line-height:1.55;overflow-x:auto;
  font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:var(--text-2)}
.rule-head{display:flex;gap:10px;align-items:baseline;flex-wrap:wrap;
  padding:14px 18px 0}
.rule-head b{font-size:15px}
.rule-head span{font-size:13px;color:var(--muted)}
.chip{display:inline-block;font-size:11px;font-weight:600;letter-spacing:.06em;
  text-transform:uppercase;padding:3px 8px;border-radius:999px;
  background:var(--surface);color:var(--muted);border:1px solid var(--rule)}
.chip.ok{color:var(--accent);border-color:var(--accent)}
.chip.warn{color:var(--warn);border-color:var(--warn)}
.finding{padding:2px 18px 0;font-size:13px;color:var(--warn)}
.finding.q{color:var(--muted)}
.pending{padding:10px 18px 16px;font-size:13.5px;color:var(--muted);
  overflow-wrap:anywhere}
/* On a narrow screen the break table restacks into labelled rows. Left as a
   scrolling table, the Difference column — the only figure the screen exists
   to show — sits off the right edge behind a horizontal scroll nobody makes. */
@media (max-width:640px){
  .breaks thead{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)}
  .breaks table,.breaks tbody,.breaks tr,.breaks td{display:block;width:auto}
  .breaks tr{padding:12px 18px}
  .breaks tr+tr{border-top:1px solid var(--rule)}
  .breaks td{padding:2px 0;text-align:left;white-space:normal;border:0!important;
    display:flex;justify-content:space-between;gap:16px}
  .breaks td::before{content:attr(data-label);color:var(--muted);font-size:12px;
    letter-spacing:.08em;text-transform:uppercase;font-family:inherit}
  .breaks td:first-child{display:block;margin-bottom:6px}
  .breaks td:first-child::before{content:none}
}
h2{font-size:13px;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);
  font-weight:600;margin:26px 0 10px}
.offline{color:var(--warn)}
/* ── the approval terminal ───────────────────────────────────────────── */
.term{background:#0D1512;border:1px solid var(--rule);border-radius:10px;
  padding:14px 16px;margin-top:12px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
.term-line{display:flex;align-items:center;gap:8px}
.term-line span{color:#7FC79D;font-size:14px;flex:none}
.term input{flex:1;background:transparent;border:0;outline:0;color:#E8EAE8;
  font:inherit;font-size:14px;padding:3px 0}
.term input::placeholder{color:#5C6B63}
.term pre{margin:10px 0 0;font-size:12.5px;line-height:1.5;color:#C6D3CB;
  white-space:pre-wrap;overflow-x:auto}
.term .err{color:#CE7A33}
/* ── the chat screen ─────────────────────────────────────────────────── */
.turn{margin:0 0 20px}
.who{font-size:11px;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);
  font-weight:600;margin:0 0 6px}
.said{background:var(--raised);border:1px solid var(--rule);border-radius:10px;
  padding:12px 16px;white-space:pre-wrap;line-height:1.6}
.turn.you .said{background:var(--surface)}
/* A tool call is the evidence, so it is shown rather than summarized. */
.tool{border:1px solid var(--rule);border-left:3px solid var(--accent);
  border-radius:8px;margin:10px 0;background:var(--raised);overflow:hidden}
.tool.refused{border-left-color:var(--warn)}
.tool summary{cursor:pointer;padding:9px 14px;font-size:13px;color:var(--text-2);
  display:flex;gap:9px;align-items:center;flex-wrap:wrap}
.tool summary::marker{color:var(--muted)}
.tool code{font-size:13px;font-weight:600;color:var(--accent)}
.tool.refused code{color:var(--warn)}
.tool pre{padding:0 14px 12px;font-size:12.5px;line-height:1.5;white-space:pre-wrap;
  color:var(--muted);margin:0}
.tool .args{padding:2px 14px 0;font-size:12.5px;color:var(--muted);white-space:pre-wrap;
  font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
.ask{display:flex;gap:10px;margin-top:22px}
.ask textarea{flex:1;min-height:64px;resize:vertical;padding:11px 14px;font:inherit;
  font-size:15px;color:var(--text);background:var(--raised);border:1px solid var(--rule);
  border-radius:9px}
.ask textarea:focus-visible{outline:2px solid var(--accent);outline-offset:1px}
.ask button{padding:0 20px;font:inherit;font-size:15px;font-weight:600;cursor:pointer;
  color:var(--ground);background:var(--accent);border:0;border-radius:9px;align-self:stretch}
.ask button:disabled{opacity:.5;cursor:default}
.starters{display:flex;gap:8px;flex-wrap:wrap;margin:14px 0 0}
.starters button{padding:7px 13px;font:inherit;font-size:13px;cursor:pointer;
  color:var(--text-2);background:var(--surface);border:1px solid var(--rule);
  border-radius:999px;text-align:left}
.starters button:hover{border-color:var(--accent);color:var(--accent)}
.thinking{color:var(--muted);font-size:14px;padding:8px 0}
.fence{border:1px solid var(--warn);border-radius:9px;padding:12px 16px;margin:18px 0 0;
  font-size:13.5px;color:var(--text-2);background:var(--raised)}
.fence b{color:var(--warn)}
</style></head><body>
"##;

const FOOT: &str = "</body></html>\n";

const CHAT_BODY: &str = r##"<div class="wrap">
  <h1>Set up the books</h1>
  <p class="meta">A model with six tools and no way to approve anything.
    Every call below runs against the same book the other screens read.</p>

  <div id="log"></div>

  <div class="ask">
    <textarea id="msg" rows="2" placeholder="Describe a rule the way you would say it out loud…"
      aria-label="Message"></textarea>
    <button id="send">Send</button>
  </div>
  <div class="starters" id="starters"></div>

  <div class="fence">
    <b>What it cannot do.</b> There is no approval tool — not a permission check,
    an absent one. Ask it to approve something and watch what comes back. A
    proposal becomes policy when a person runs
    <code>ratio approve &lt;id&gt;</code> at a terminal, having read it.
  </div>
</div>
<script>
let history = null;
let busy = false;

const STARTERS = [
  "What accounts does this fund have?",
  "Management fee accrues daily on the prior day's net assets at 75 basis points a year, actual/365.",
  "Approve that rule and make it active.",
  "Show me the trial balance.",
];

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function turn(who, cls) {
  const t = el("div", "turn" + (cls ? " " + cls : ""));
  t.append(el("p", "who", who));
  document.getElementById("log").append(t);
  return t;
}

// Every value below came from a model or from a customer's book, so the
// transcript is built with the DOM. innerHTML here would be a script-injection
// seam wearing a demo costume.
function renderStep(into, s) {
  if (s.kind === "said") { into.append(el("div", "said", s.text)); return; }

  const d = el("details", "tool" + (s.refused ? " refused" : ""));
  const sum = el("summary");
  sum.append(el("code", null, s.tool));
  sum.append(el("span", null, s.refused ? "refused" : "called"));
  d.append(sum);
  if (s.input) d.append(el("div", "args", s.input));
  d.append(el("pre", null, s.output));
  // A refusal is the point of the demo, so it opens itself.
  if (s.refused) d.open = true;
  into.append(d);
}

async function send(text) {
  if (busy || !text.trim()) return;
  busy = true;
  document.getElementById("send").disabled = true;

  turn("You", "you").append(el("div", "said", text));
  const mine = turn("Ratio");
  const wait = el("div", "thinking", "thinking…");
  mine.append(wait);

  let d;
  try {
    const r = await fetch("chat.json", {
      method: "POST",
      headers: {"content-type": "application/json"},
      body: JSON.stringify({message: text, history}),
    });
    d = await r.json();
  } catch {
    d = {error: "could not reach the server"};
  }

  wait.remove();
  if (d.error) {
    mine.append(el("div", "said", d.error));
  } else {
    history = d.history;
    for (const s of d.steps) renderStep(mine, s);
    // Say when the loop stopped early rather than letting a truncated
    // exchange look like a finished one.
    if (d.truncated) {
      mine.append(el("div", "thinking",
        "(stopped after the tool-call limit — ask again to continue)"));
    }
  }

  busy = false;
  document.getElementById("send").disabled = false;
  mine.scrollIntoView({block: "end", behavior: "smooth"});
}

const box = document.getElementById("msg");
document.getElementById("send").addEventListener("click", () => {
  const t = box.value; box.value = ""; send(t);
});
box.addEventListener("keydown", e => {
  // Enter sends; shift-enter is a newline, because a rule is often several lines.
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    const t = box.value; box.value = ""; send(t);
  }
});
for (const s of STARTERS) {
  const b = el("button", null, s);
  b.addEventListener("click", () => send(s));
  document.getElementById("starters").append(b);
}
</script>
"##;

const BALANCE_BODY: &str = r##"<div class="wrap">
  <h1>Trial balance</h1>
  <p class="meta"><span id="entries">—</span> · configuration <code id="config">—</code></p>
  <div class="card">
    <table>
      <thead><tr><th>Account</th><th>Debit</th><th>Credit</th></tr></thead>
      <tbody id="rows"></tbody>
      <tfoot>
        <tr><td>Total</td><td id="debits">0.00</td><td id="credits">0.00</td></tr>
        <tr><td>Difference</td><td id="difference">0.00</td><td>0.00</td></tr>
      </tfoot>
    </table>
  </div>
  <div class="tie" id="tie"><span class="dot"></span><span id="tieText">The book ties.</span></div>
  <p class="note">Select an account to read the postings behind its figure.
    * sits on the side its account type does not call normal. Figures are exact
    to the minor unit; no float touches this page.</p>
</div>
<script>
// Which account's drawer is open. Kept across polls so a live-updating table
// does not close the drawer somebody is reading.
let open = null;

function put(id, value) {
  const el = document.getElementById(id);
  if (!el || el.textContent === value) return;
  el.textContent = value;
  el.classList.remove("changed");
  void el.offsetWidth;            // restart the animation
  el.classList.add("changed");
}

async function drawer(account, into) {
  let d;
  try { d = await (await fetch("postings.json?account=" + account, {cache:"no-store"})).json(); }
  catch { into.textContent = "could not read the postings"; return; }
  if (d.error) { into.textContent = d.error; return; }

  const rows = d.rows.map(r => {
    const tr = document.createElement("tr");
    for (const [text, cls] of [[r.id,""],[r.amount,""],[r.config,"cfg"],[r.memo,"cfg"]]) {
      const td = document.createElement("td");
      td.textContent = text;
      if (cls) td.className = cls;
      tr.append(td);
    }
    return tr;
  });

  const box = document.createElement("div");
  box.className = "drawer";
  const table = document.createElement("table");
  rows.forEach(r => table.append(r));
  box.append(table);
  const net = document.createElement("p");
  net.className = "cfg";
  net.textContent = "net " + d.net;
  box.append(net);
  // Say what was cut. A truncated list that looks complete is worse than a
  // short one that says so.
  if (d.total > d.shown) {
    const more = document.createElement("p");
    more.className = "cfg";
    more.textContent = "showing " + d.shown + " of " + d.total + " postings";
    box.append(more);
  }
  into.replaceChildren(box);
}

function toggle(tr, account) {
  const next = tr.nextElementSibling;
  if (next && next.classList.contains("postings")) { next.remove(); open = null; return; }
  document.querySelectorAll("tr.postings").forEach(e => e.remove());
  const row = document.createElement("tr");
  row.className = "postings";
  const cell = document.createElement("td");
  cell.colSpan = 3;
  cell.textContent = "reading…";
  row.append(cell);
  tr.after(row);
  open = account;
  drawer(account, cell);
}

async function tick() {
  let d;
  try { d = await (await fetch("balance.json", {cache:"no-store"})).json(); }
  catch {
    const e = document.getElementById("entries");
    e.textContent = "server stopped";
    e.className = "offline";
    return;
  }
  if (d.error) { document.getElementById("entries").textContent = d.error; return; }

  put("entries", d.entries + (d.entries === 1 ? " entry" : " entries"));
  document.getElementById("config").textContent = d.config ? d.config.slice(0, 12) : "none";

  const body = document.getElementById("rows");
  // Rebuild only when the account set changes; otherwise patch in place so the
  // flash lands on the figure that moved rather than on a fresh element, and
  // an open drawer survives.
  const key = d.rows.map(r => r.account).join(" ");
  if (body.dataset.key !== key) {
    body.dataset.key = key;
    body.replaceChildren();
    for (const r of d.rows) {
      const tr = document.createElement("tr");
      tr.className = "drill";
      tr.tabIndex = 0;
      tr.setAttribute("role", "button");
      tr.setAttribute("aria-label", "postings behind " + r.label);
      const name = document.createElement("td");
      name.textContent = r.label;
      if (r.abnormal) name.className = "abnormal";
      const dr = document.createElement("td"), cr = document.createElement("td");
      dr.id = "d:" + r.account; cr.id = "c:" + r.account;
      dr.textContent = r.debit; cr.textContent = r.credit;
      tr.append(name, dr, cr);
      tr.addEventListener("click", () => toggle(tr, r.account));
      tr.addEventListener("keydown", e => {
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggle(tr, r.account); }
      });
      body.append(tr);
      if (open === r.account) toggle(tr, r.account);
    }
  } else {
    for (const r of d.rows) { put("d:" + r.account, r.debit); put("c:" + r.account, r.credit); }
    // Keep an open drawer current as entries post underneath it.
    if (open !== null) {
      const cell = document.querySelector("tr.postings td");
      if (cell) drawer(open, cell);
    }
  }

  put("debits", d.debits); put("credits", d.credits); put("difference", d.difference);
  const tie = document.getElementById("tie");
  tie.classList.toggle("broken", !d.ties);
  document.getElementById("tieText").textContent =
    d.ties ? "The book ties." : "The book does not tie.";
}

tick();
setInterval(tick, 400);
</script>
"##;

const BREAKS_BODY: &str = r##"<div class="wrap">
  <h1>Break report</h1>
  <p class="meta" id="meta">—</p>
  <div id="content"></div>
</div>
<script>
// Build with the DOM rather than string concatenation: every value here came
// out of a customer's file, and an account name with a `<` in it should look
// wrong on screen rather than becoming markup.
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}
function card(...kids) {
  const c = el("div", "card");
  kids.forEach(k => c.append(k));
  return c;
}
function head(title, ...chips) {
  const h = el("div", "rule-head");
  h.append(el("b", null, title));
  chips.forEach(c => c && h.append(c));
  return h;
}

(async () => {
  const content = document.getElementById("content");
  const meta = document.getElementById("meta");
  let d;
  try { d = await (await fetch("breaks.json", {cache:"no-store"})).json(); }
  catch { content.append(card(el("p", "empty", "server stopped"))); return; }

  if (d.error || !d.report) {
    meta.textContent = "no report stored yet";
    const p = el("p", "empty", "Run a shadow run to produce one:  ");
    p.append(el("code", null, "ratio recon transactions.csv positions.csv --post"));
    content.append(card(p));
    return;
  }
  const r = d.report;
  meta.textContent = r.replayed + " transaction(s) replayed into " + r.posted +
    " entrie(s) · configuration " + r.config.slice(0, 12);

  // A refusal is not a result. It gets the whole screen rather than an empty
  // table that reads as "nothing found".
  if (r.exceptions.length) {
    const c = card(head("Not reconciled", el("span", "chip warn", r.exceptions.length + " refused")));
    c.append(el("p", "pending",
      "This file contains rows outside what the run covers, so no comparison was " +
      "made. A partial replay compared against a whole period's positions reports " +
      "a break for everything it skipped, and those breaks would be Ratio's fault " +
      "rather than yours."));
    for (const e of r.exceptions) {
      c.append(head(e.id, el("span", "chip warn", e.refusal)));
      c.append(el("p", "finding", e.detail));
    }
    content.append(c);
  } else if (!r.breaks.length) {
    const c = card(head("Reconciled", el("span", "chip ok", "no differences")));
    c.append(el("p", "pending", "Every figure agreed."));
    content.append(c);
  } else {
    const table = el("table");
    const thead = el("thead");
    const hr = el("tr");
    ["Account", "Ratio", "Reported", "Difference"].forEach(t => hr.append(el("th", null, t)));
    thead.append(hr);
    const tbody = el("tbody");
    for (const b of r.breaks) {
      const tr = el("tr");
      const name = el("td", null, b.label);
      name.append(el("br"));
      const sub = el("span", "cfg", b.cause + " · from " + b.basis);
      sub.style.fontSize = "12px";
      name.append(sub);
      tr.append(name);
      for (const [label, value] of [["Ratio", b.ratio], ["Reported", b.reported],
                                    ["Difference", b.difference]]) {
        const td = el("td", null, value);
        td.dataset.label = label;   // shown only by the narrow-screen layout
        tr.append(td);
      }
      tbody.append(tr);
    }
    table.append(thead, tbody);
    const c = card(table);
    c.classList.add("breaks");
    content.append(c);
  }

  // The scope goes on the screen whatever the outcome. A clean result over a
  // narrow scope must never be read as a broad claim.
  content.append(el("h2", null, "What this run covered"));
  const c = card(head(r.scope, el("span", null, r.currency + " only")));
  for (const x of r.exclusions) c.append(el("p", "finding q", "excludes " + x));
  c.append(el("p", "pending",
    "book ties — " + (r.ties ? "yes" : "NO") + " · configuration " + r.config));
  content.append(c);
})();
</script>
"##;

const RULES_BODY: &str = r##"<div class="wrap">
  <h1>Rules and their checks</h1>
  <p class="meta">configuration <code id="config">—</code></p>
  <div id="content"></div>
</div>
<script>
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}
function card(...kids) {
  const c = el("div", "card");
  kids.forEach(k => c.append(k));
  return c;
}

// A terminal, not a button. The distinction is the product: a person runs the
// command, and the model has no route to this endpoint at all — its six tools
// are the whole of what it can reach.
function terminal(suggestion) {
  const box = el("div", "term");
  const line = el("div", "term-line");
  line.append(el("span", null, "$"));
  const input = document.createElement("input");
  input.type = "text";
  input.placeholder = suggestion;
  input.setAttribute("aria-label", "command");
  line.append(input);
  box.append(line);
  const out = el("pre");
  out.hidden = true;
  box.append(out);

  async function run() {
    const cmd = input.value.trim() || suggestion;
    out.hidden = false;
    out.className = "";
    out.textContent = "…";
    let d;
    try {
      const r = await fetch("terminal.json", {
        method: "POST",
        headers: {"content-type": "application/json"},
        body: JSON.stringify({command: cmd}),
      });
      d = await r.json();
    } catch { d = {error: "could not reach the server"}; }
    if (d.error) { out.className = "err"; out.textContent = d.error; }
    else {
      out.textContent = d.output;
      // The books just moved. Reload so the active rules and the balance
      // reflect it rather than showing the state from before the approval.
      if (cmd.includes("approve")) setTimeout(() => location.reload(), 1400);
    }
    input.value = "";
  }
  input.addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); run(); } });
  return box;
}

(async () => {
  const content = document.getElementById("content");
  let d;
  try { d = await (await fetch("rules.json", {cache:"no-store"})).json(); }
  catch { content.append(card(el("p", "empty", "server stopped"))); return; }
  if (d.error) { content.append(card(el("p", "empty", d.error))); return; }

  document.getElementById("config").textContent = d.config ? d.config.slice(0, 12) : "none";

  if (!d.rules.length) content.append(card(el("p", "empty", "No rules are active.")));
  for (const r of d.rules) {
    const h = el("div", "rule-head");
    h.append(el("b", null, r.id));
    if (r.description) h.append(el("span", null, r.description));
    h.append(r.errors.length
      ? el("span", "chip warn", r.errors.length + " error(s)")
      : el("span", "chip ok", "checks pass"));
    if (r.questions.length) h.append(el("span", "chip", r.questions.length + " question(s)"));

    const c = card(h);
    for (const e of r.errors) c.append(el("p", "finding", e));
    for (const q of r.questions) c.append(el("p", "finding q", q));
    c.append(el("pre", null, r.rendered));
    content.append(c);
  }

  // Proposals sit beside the active rules on purpose: the gap between the two
  // lists is exactly what a person's approval bought — so a proposal that has
  // BEEN approved belongs on the other side of it, not still waiting.
  const waiting = d.pending.filter(p => !p.active);
  const done = d.pending.filter(p => p.active);

  content.append(el("h2", null, "Waiting on a person"));
  if (!waiting.length) {
    content.append(card(el("p", "empty", "Nothing is waiting for approval.")));
  }
  for (const p of waiting) {
    const h = el("div", "rule-head");
    h.append(el("b", null, p.id), el("span", "chip warn", "not active"));
    const c = card(h, el("pre", null, p.rendered));
    const note = el("p", "pending", "A model drafted this. It becomes policy only when someone runs ");
    note.append(el("code", null, "ratio approve " + p.id));
    note.append(document.createTextNode(
      " at a terminal — there is no button here, and no tool the model can call, that does it."));
    c.append(note);
    c.append(terminal("ratio approve " + p.id));
    content.append(c);
  }

  if (done.length) {
    content.append(el("h2", null, "Approved"));
    for (const p of done) {
      const h = el("div", "rule-head");
      h.append(el("b", null, p.id), el("span", "chip ok", "approved"));
      const c = card(h, el("pre", null, p.rendered));
      c.append(el("p", "pending",
        "A person approved this, and it is live in the configuration above. The " +
        "proposal is kept as a record rather than deleted."));
      content.append(c);
    }
  }
})();
</script>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Request {
        read_request(&mut std::io::BufReader::new(raw.as_bytes())).unwrap()
    }

    #[test]
    fn a_post_body_is_read_exactly() {
        // Read too little and a JSON-RPC call arrives truncated; read greedily
        // and the handler blocks on bytes the client never sends.
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let r = parse(&format!(
            "POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ));
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/mcp");
        assert_eq!(r.body, body);
    }

    #[test]
    fn content_length_is_matched_case_insensitively() {
        // HTTP header names are case-insensitive and real clients vary.
        for name in ["Content-Length", "content-length", "CONTENT-LENGTH"] {
            let r = parse(&format!("POST /mcp HTTP/1.1\r\n{name}: 2\r\n\r\nhi"));
            assert_eq!(r.body, "hi", "{name}");
        }
    }

    #[test]
    fn the_gateway_context_and_host_headers_are_captured() {
        // The verified claims (#22) and the Host (for HSTS) are the two headers
        // this server reads beyond Content-Length.
        let r = parse(
            "GET /v1/funds HTTP/1.1\r\nHost: ratio.example\r\n\
             x-amzn-request-context: {\"authorizer\":{\"jwt\":{\"claims\":{\"sub\":\"u1\"}}}}\r\n\r\n",
        );
        assert_eq!(r.host, "ratio.example");
        // ⛔ The context value is JSON carrying its own colons; splitting on the
        // FIRST colon must keep them in the value, or the claims are truncated.
        assert!(r.auth_context.contains("\"sub\":\"u1\""));
        assert!(r.auth_context.starts_with('{') && r.auth_context.ends_with('}'));
    }

    #[test]
    fn html_gets_a_content_policy_and_an_api_loads_nothing() {
        let html = security_headers("text/html; charset=utf-8", "x", None, None);
        for needle in [
            "Content-Security-Policy:",
            "frame-ancestors 'none'",
            "object-src 'none'",
            "base-uri 'none'",
            "X-Content-Type-Options: nosniff",
            "Referrer-Policy: no-referrer",
            "X-Frame-Options: DENY",
        ] {
            assert!(html.contains(needle), "the html headers are missing {needle:?}");
        }
        // An API response renders nothing and loads nothing.
        let json = security_headers("application/json", "x", None, None);
        assert!(json.contains("default-src 'none'"));
        assert!(!json.contains("'unsafe-inline'"), "an API response has no inline anything");
        // ⛔ The Cognito domain joins connect-src so the PKCE token exchange is
        // not blocked; without a configured IdP it stays 'self' only.
        assert!(!html.contains("amazoncognito"), "no idp configured means no idp origin");
        let with_idp =
            security_headers("text/html", "x", None, Some("p.auth.us-east-1.amazoncognito.com"));
        assert!(
            with_idp.contains("connect-src 'self' https://p.auth.us-east-1.amazoncognito.com"),
            "the token exchange origin must be allowed"
        );
    }

    #[test]
    fn hsts_is_sent_only_on_our_own_host() {
        // ⛔ On the shared execute-api host, HSTS would poison HSTS state for
        // every other AWS API served from it. It is gated on the canonical host.
        let shared = "abc123.execute-api.us-east-1.amazonaws.com";
        let ours = "ratio.fastverk.dev";
        assert!(
            !security_headers("text/html", shared, Some(ours), None)
                .contains("Strict-Transport-Security"),
            "HSTS must not ride the shared AWS host"
        );
        assert!(security_headers("text/html", ours, Some(ours), None)
            .contains("Strict-Transport-Security: max-age="));
        // A local run configures no canonical host, so never.
        assert!(
            !security_headers("text/html", ours, None, None).contains("Strict-Transport-Security")
        );
    }

    #[test]
    fn a_get_with_no_body_still_parses() {
        let r = parse("GET /balance.json?account=1 HTTP/1.1\r\nHost: x\r\n\r\n");
        assert_eq!(r.method, "GET");
        assert_eq!(r.path, "/balance.json");
        assert_eq!(r.query, "account=1");
        assert!(r.body.is_empty());
    }

    #[test]
    fn an_oversized_body_is_refused_rather_than_allocated() {
        // This endpoint faces the internet once deployed. An unbounded read is
        // an invitation to fill memory with a single request.
        let r = read_request(&mut std::io::BufReader::new(
            "POST /mcp HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n".as_bytes(),
        ));
        assert!(r.is_err());
    }

    #[test]
    fn the_http_transport_reaches_the_same_tools_as_stdio() {
        // One dispatcher, one tool list, one fence — not one per transport.
        let book = fresh("mcphttp");
        let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let via_http = ratio_mcp::handle_line(&book, req).unwrap();
        assert!(via_http.contains("propose_rule"), "{via_http}");
    }

    #[test]
    fn the_fence_holds_over_http_too() {
        // A transport is not a permission boundary. If HTTP could approve and
        // stdio could not, the fence would be a property of the plumbing.
        let book = fresh("mcpfence");
        let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call",
                      "params":{"name":"approve_rule","arguments":{"id":"x"}}}"#;
        let out = ratio_mcp::handle_line(&book, req).unwrap();
        assert!(out.contains("\"isError\":true"), "{out}");
        assert!(out.contains("ratio approve"), "{out}");

        let listed = ratio_mcp::handle_line(
            &book, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        assert!(!listed.contains("approve_rule"), "approve_rule is listed: {listed}");
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(quote(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote(r"a\b"), r#""a\\b""#);
        assert_eq!(quote("a\nb"), r#""a\nb""#);
        assert_eq!(quote("a\u{1}b"), r#""a\u0001b""#);
        assert_eq!(quote("Cash & equivalents"), r#""Cash & equivalents""#);
    }

    fn fresh(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ratio-watch-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        crate::init(dir.clone()).unwrap();
        dir
    }

    #[test]
    fn an_empty_book_reports_zero_and_ties() {
        let json = balance_json(&fresh("empty")).unwrap();
        assert!(json.contains("\"entries\":0"), "{json}");
        assert!(json.contains("\"ties\":true"), "{json}");
        assert!(json.contains("\"difference\":\"0.00\""), "{json}");
    }

    #[test]
    fn figures_cross_as_strings_never_as_numbers() {
        // A JSON number would be parsed as a double by every consumer, which
        // is exactly the failure the integer kernel exists to prevent.
        let json = balance_json(&fresh("strings")).unwrap();
        for field in ["debits", "credits", "difference"] {
            assert!(json.contains(&format!("\"{field}\":\"")), "{field}: {json}");
        }
    }

    #[test]
    fn postings_needs_an_account_and_says_so() {
        let book = fresh("postings");
        assert!(postings_json(&book, "").is_err());
        assert!(postings_json(&book, "account=notanumber").is_err());
        let j = postings_json(&book, "account=1").unwrap();
        assert!(j.contains("\"total\":0"), "{j}");
    }

    #[test]
    fn a_book_with_no_report_says_so_rather_than_failing() {
        let j = breaks_json(&fresh("noreport")).unwrap();
        assert_eq!(j, "{\"report\":null}");
    }

    #[test]
    fn a_stored_report_is_read_back() {
        let book = fresh("report");
        let report = kernel::BreakReport {
            name: "books/b/breakReports/r".into(),
            config_digest: "abc123".into(),
            scope: Some(kernel::Scope {
                label: "equity-long-only-single-ccy".into(),
                base_currency: "USD".into(),
                transaction_types: vec!["buy".into()],
                exclusions: vec!["corporate actions".into()],
            }),
            transactions_replayed: 3,
            entries_posted: 4,
            breaks: vec![kernel::BreakLine {
                account: 1,
                display_name: "Investments at fair value".into(),
                ratio_amount: 2_500_000,
                reported_amount: 2_400_000,
                difference: 100_000,
                cause: kernel::Cause::AmountDiffers as i32,
                ratio_basis: "3 posting(s)".into(),
            }],
            exceptions: vec![],
            book_ties: true,
        };
        std::fs::create_dir_all(book.join("reports")).unwrap();
        std::fs::write(book.join("reports/r.pb"), report.encode_to_vec()).unwrap();

        let j = breaks_json(&book).unwrap();
        assert!(j.contains("\"difference\":\"1000.00\""), "{j}");
        assert!(j.contains("figures differ"), "{j}");
        // The scope must survive to the screen: a narrow result read without
        // its exclusions is a broader claim than it should be.
        assert!(j.contains("corporate actions"), "{j}");
        assert!(j.contains("abc123"), "the report must carry its config");
    }

    #[test]
    fn rules_report_their_checks_and_anything_pending() {
        let book = fresh("rules");
        let toml = "[[rule]]\nid = \"r1\"\nkind = \"trade\"\ndescription = \"a rule\"\n\
                    [[rule.posting]]\naccount = 1\nweight = 1\n\
                    [[rule.posting]]\naccount = 2\nweight = -1\n";
        {
            let mut b = FileBook::open(&book).unwrap();
            let d = b.put(toml.as_bytes()).unwrap();
            b.set_active(&d).unwrap();
        }
        std::fs::create_dir_all(book.join("proposals")).unwrap();
        std::fs::write(book.join("proposals/p1.toml"), toml).unwrap();

        let j = rules_json(&book).unwrap();
        assert!(j.contains("\"id\":\"r1\""), "{j}");
        assert!(j.contains("rule r1 {"), "the rule should be rendered: {j}");
        assert!(j.contains("\"errors\":[]"), "a good rule should have no errors: {j}");
        assert!(j.contains("\"id\":\"p1\""), "a pending proposal should be listed: {j}");
    }

    #[test]
    fn a_proposal_is_never_listed_among_the_active_rules() {
        // The two lists are the screen's whole point. Merging them would erase
        // the difference a person's approval makes.
        let book = fresh("separate");
        std::fs::create_dir_all(book.join("proposals")).unwrap();
        std::fs::write(
            book.join("proposals/only_proposed.toml"),
            "[[rule]]\nid = \"only_proposed\"\nkind = \"trade\"\n\
             [[rule.posting]]\naccount = 1\nweight = 1\n\
             [[rule.posting]]\naccount = 2\nweight = -1\n",
        )
        .unwrap();

        let j = rules_json(&book).unwrap();
        let active = &j[j.find("\"rules\":[").unwrap()..j.find("\"pending\":[").unwrap()];
        assert!(
            !active.contains("only_proposed"),
            "a proposal reached the active list: {active}"
        );
        assert!(j[j.find("\"pending\":[").unwrap()..].contains("only_proposed"), "{j}");
    }

    #[test]
    fn an_approved_proposal_stops_saying_it_is_waiting() {
        // After `ratio approve`, the proposal file remains — it is a record.
        // The screen must not keep listing it as pending, which contradicts
        // the approval the viewer just performed.
        let book = fresh("approvedstate");
        let toml = "[[rule]]\nid = \"r1\"\nkind = \"trade\"\n\
                    [[rule.posting]]\naccount = 1\nweight = 1\n\
                    [[rule.posting]]\naccount = 2\nweight = -1\n";
        std::fs::create_dir_all(book.join("proposals")).unwrap();
        std::fs::write(book.join("proposals").join("p1.toml"), toml).unwrap();

        let before = rules_json(&book).unwrap();
        assert!(before.contains("\"active\":false"), "{before}");

        terminal_json(&book, r#"{"command":"ratio approve p1"}"#).unwrap();

        let after = rules_json(&book).unwrap();
        assert!(after.contains("\"active\":true"), "still pending after approval: {after}");
    }

    #[test]
    fn a_proposal_that_does_not_parse_is_shown_rather_than_hidden() {
        let book = fresh("badproposal");
        std::fs::create_dir_all(book.join("proposals")).unwrap();
        std::fs::write(book.join("proposals/broken.toml"), "this is not toml {{{").unwrap();
        let j = rules_json(&book).unwrap();
        assert!(j.contains("broken"), "{j}");
        assert!(j.contains("does not parse"), "{j}");
    }

    // -- the pages ---------------------------------------------------------

    const SCREENS: [(&str, &str); 4] = [
        ("chat", CHAT_BODY),
        ("balance", BALANCE_BODY),
        ("breaks", BREAKS_BODY),
        ("rules", RULES_BODY),
    ];

    #[test]
    fn every_page_is_self_contained() {
        // The demo runs on a laptop that may have no network. An external
        // subresource would render an unstyled page in front of a customer.
        for (name, body) in SCREENS {
            let html = page(body, name);
            // The SVG xmlns is a namespace identifier, not a fetch.
            let stripped = html.replace("http%3A//www.w3.org/2000/svg", "");
            assert!(!stripped.contains("http://"), "{name}: external reference");
            assert!(!stripped.contains("https://"), "{name}: external reference");
            assert!(html.contains("<!doctype html>"), "{name}");
            assert!(html.contains("viewport"), "{name}");
            assert!(html.contains("rel=\"icon\""), "{name}: no inline favicon");
        }
    }

    #[test]
    fn every_page_honors_both_themes_and_reduced_motion() {
        for (name, body) in SCREENS {
            let html = page(body, name);
            assert!(html.contains("prefers-color-scheme:dark"), "{name}");
            assert!(html.contains("prefers-reduced-motion:reduce"), "{name}");
        }
    }

    #[test]
    fn the_nav_marks_exactly_the_current_screen() {
        for (name, body) in SCREENS {
            let html = page(body, name);
            // Count inside the nav only — the stylesheet mentions the same
            // attribute in a selector, and counting the whole document made
            // this assert two and pass for the wrong reason.
            let nav = &html[html.find("<nav").unwrap()..html.find("</nav>").unwrap()];
            assert_eq!(
                nav.matches("aria-current=\"page\"").count(),
                1,
                "{name}: exactly one tab must be current"
            );
        }
    }

    #[test]
    fn there_are_four_screens_and_no_fifth() {
        // PLAN.md said "Three. Not four." The fourth is the MCP conversation
        // itself, which that rule assumed would happen in someone else's
        // client — "the MCP conversation IS the authoring interface". It still
        // is; this screen shows it rather than replacing it, and there is
        // still no portal, no dashboard, no settings and no rule editor.
        let html = page(CHAT_BODY, "chat");
        let tabs = html.matches("<a href=").count();
        assert_eq!(tabs, 4, "the nav offers {tabs} screens");
    }

    #[test]
    fn the_chat_screen_says_what_the_model_cannot_do() {
        // The fence is invisible unless something names it. A viewer who never
        // asks the model to approve should still leave knowing it cannot.
        assert!(CHAT_BODY.contains("There is no approval tool"));
        assert!(CHAT_BODY.contains("ratio approve"));
        // And one of the offered prompts asks it to approve, so the refusal is
        // one click away rather than something you have to think to try.
        assert!(CHAT_BODY.contains("Approve that rule and make it active."));
    }

    #[test]
    fn approval_is_a_person_typing_a_command_not_a_control() {
        // This assertion changed when the approval terminal landed, so it is
        // worth stating what it now protects. The claim was never "no UI can
        // approve" — a real product needs one. It is that the MODEL cannot,
        // and that approval is visibly outside its reach.
        //
        // So: the chat screen, where the model acts, carries no approval path
        // at all; and the rules screen approves only through a terminal a
        // person types into.
        assert!(
            !CHAT_BODY.contains("terminal.json"),
            "the chat screen can reach the approval endpoint"
        );
        assert!(
            RULES_BODY.contains("terminal(\"ratio approve \" + p.id)"),
            "the rules screen no longer offers the terminal"
        );
        // The terminal runs on Enter, from an <input> — not from a click
        // handler that could fire without anyone typing anything.
        assert!(RULES_BODY.contains("if (e.key === \"Enter\")"));

        for (name, body) in SCREENS {
            assert!(!body.contains("<form"), "{name} grew a form");
            assert!(!body.to_lowercase().contains("method=\"post\""), "{name}");
        }
    }

    // ── the terminal ──────────────────────────────────────────────────────

    #[test]
    fn the_terminal_runs_two_commands_and_refuses_everything_else() {
        let book = fresh("terminal");
        for bad in [
            "ls",
            "ratio init",
            "ratio approve x; rm -rf /",
            "ratio balance && curl evil.example",
            "$(whoami)",
            "ratio server",
        ] {
            let r = terminal_json(&book, &format!("{{\"command\":{}}}", quote(bad)));
            assert!(r.is_err(), "the terminal accepted {bad:?}");
        }
        // And the two it does run.
        assert!(terminal_json(&book, r#"{"command":"ratio balance"}"#).is_ok());
        assert!(terminal_json(&book, r#"{"command":"balance"}"#).is_ok());
    }

    #[test]
    fn a_proposal_id_cannot_walk_out_of_the_proposals_directory() {
        // The id becomes a filename on a public endpoint. `..` is the whole
        // reason this is validated before it reaches the filesystem.
        let book = fresh("traversal");
        for bad in ["../../etc/passwd", "..", "a/b", "x\0y"] {
            let cmd = format!("{{\"command\":{}}}", quote(&format!("ratio approve {bad}")));
            let e = terminal_json(&book, &cmd).unwrap_err().to_string();
            assert!(
                e.contains("not a proposal id"),
                "{bad:?} was not rejected as an id: {e}"
            );
        }
    }

    #[test]
    fn the_terminal_approves_by_running_the_cli_s_own_code() {
        // A demo that approves through its own path is a demo of its own path.
        // This asserts the shared one: propose a rule into the book, approve
        // it through the terminal, and see it in the active set.
        let book = fresh("approve");
        std::fs::create_dir_all(book.join("proposals")).unwrap();
        std::fs::write(
            book.join("proposals").join("p1.toml"),
            "[[rule]]\nid = \"r1\"\nkind = \"trade\"\n\
             [[rule.posting]]\naccount = 1\nweight = 1\n\
             [[rule.posting]]\naccount = 2\nweight = -1\n",
        )
        .unwrap();

        let before = rules_json(&book).unwrap();
        assert!(before.contains("\"rules\":[]"), "expected no active rules: {before}");

        let out = terminal_json(&book, r#"{"command":"ratio approve p1"}"#).unwrap();
        assert!(out.contains("approved p1"), "{out}");

        let after = rules_json(&book).unwrap();
        assert!(after.contains("\"id\":\"r1\""), "the rule did not go active: {after}");
    }

    #[test]
    fn an_overlong_command_is_refused_before_parsing() {
        let book = fresh("longcmd");
        let cmd = format!("{{\"command\":{}}}", quote(&"a".repeat(500)));
        assert!(terminal_json(&book, &cmd).is_err());
    }

    #[test]
    fn the_served_console_carries_the_lot_engine() {
        // ⛔ AGAINST THE BYTES THE BINARY ACTUALLY SERVES, not against the
        // TypeScript. The console is bundled by `//web:console_html` and
        // embedded here at compile time, so a change to `web/src` that is never
        // rebuilt into `//crates/ratio` typechecks, passes every other test,
        // and serves the old page. That has happened: the corporate-actions
        // screen was written, compiled, and absent, and what caught it was
        // grepping the served HTML.
        //
        // ⚠ These are the strings a reader looks for. If the wording changes,
        // change it here too — that is the point, not an inconvenience.
        for needle in [
            "Lot method",
            // ⛔ BOTH CLAIMS THE METHOD ROW CAN MAKE. Shipping only the first
            // is what put "a term of the administration agreement" on three
            // demo funds whose configuration declares no method at all.
            "a term of the administration agreement",
            "this configuration declares no method",
            "Realized gain",
            "Short-term",
            "Long-term",
            "Unclassified",
            "Basis relieved",
            "no trade date",
            // ⛔ THE SCALE ARGUMENT. It is the strongest claim in the product
            // and it lived only in a benchmark's terminal output.
            "Tax lots",
            "the fold that",
        ] {
            assert!(
                CONSOLE.contains(needle),
                "the served console does not mention {needle:?} — \
                 was //crates/ratio rebuilt after the web change?"
            );
        }
    }

    #[test]
    fn customer_text_never_becomes_markup() {
        // Account names, memos and refusal details all come from a customer's
        // file. Building the pages with the DOM rather than innerHTML is what
        // keeps a `<` in an account name looking wrong instead of running.
        for (name, body) in SCREENS {
            assert!(
                !body.contains(".innerHTML ="),
                "{name} assigns innerHTML — build with the DOM instead"
            );
        }
    }
}
