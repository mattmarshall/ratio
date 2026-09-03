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

use crate::scale::{self, Launcher as _};
use prost::Message;
use ratio_chart::{normal_side, Side};
use ratio_proto::ratio::console::v1 as pb;
// The kernel's contracts — a shadow run's report is a `ratio.v1` message, and
// the console's are `ratio.console.v1`. Two aliases because they are two APIs.
use ratio_proto::ratio::v1 as kernel;
use ratio_rules::{check, render as render_rule, RuleSet};
use ratio_store::{ConfigStore, FileBook, Journal};

/// Wire the durable journal, if the environment asked for one.
///
/// ⚠ UNSET IS THE LOCAL SHAPE, not a misconfiguration. `ratio watch` on a
/// laptop writes `journal.jsonl` as it always has. The deployed demo sets
/// `RATIO_JOURNAL_BUCKET` (the same bucket as the scale runner, a `journals/`
/// prefix) so a write survives the next cold start and two containers share
/// one log. `RATIO_JOURNAL_LOCAL` is the same claim on a directory, for a
/// laptop that wants to see the seam without AWS.
fn install_journal_store() -> Result<()> {
    if let Some(bucket) = std::env::var("RATIO_JOURNAL_BUCKET").ok().filter(|b| !b.is_empty()) {
        let prefix = std::env::var("RATIO_JOURNAL_PREFIX")
            .ok()
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "journals/".to_string());
        let store = scale::S3::open(&bucket, prefix)
            .with_context(|| format!("opening the journal store in s3://{bucket}"))?;
        ratio_store::install_object_store(std::sync::Arc::new(store));
        return Ok(());
    }
    if let Some(dir) = std::env::var("RATIO_JOURNAL_LOCAL").ok().filter(|d| !d.is_empty()) {
        ratio_store::install_object_store(std::sync::Arc::new(ratio_store::DirStore::at(dir)));
    }
    Ok(())
}

/// Serve the screens until interrupted.
pub fn watch(book: PathBuf, port: u16) -> Result<()> {
    // ⛔ BEFORE THE FIRST OPEN. FileBook reads the process-wide store on
    // `open`, so installing after the first request would leave that request
    // writing `/tmp` while later ones write the object store — two journals
    // under one URL, which is the fork this exists to close.
    install_journal_store()?;
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
        // check that the pages need no 'unsafe-eval'.
        //
        // ⚠ `connect-src 'self'` WITH NO EXCEPTION, WHERE THE COGNITO DOMAIN
        // USED TO BE ADMITTED. The console ran authorization-code + PKCE in the
        // tab and had to reach the IdP's token endpoint from it; that console is
        // a Next.js application on its own origin now and runs the exchange on
        // its server, so no page this binary serves talks to Cognito. The
        // parameter went with the flow it existed for. The pages left here —
        // /balance, /breaks, /rules, /chat — never signed anybody in.
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' \
         'unsafe-inline'; img-src 'self' data:; connect-src 'self'; form-action \
         'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'"
            .to_string()
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
                security_headers("text/plain; charset=utf-8", "", canonical.as_deref())
            )?;
            return Ok(());
        }
    };

    // ⛔ THE FRONT DOOR MOVED, AND IT REDIRECTS RATHER THAN 404s. This URL is
    // the "Try the live demo" link in README.md and on the marketing site;
    // whatever else happens to it, it must not become a dead link somebody
    // clicks from a page we published. `/app` is here for the same reason —
    // it is what the old console's OAuth callback returned to, so an
    // in-flight bookmark lands somewhere useful rather than nowhere.
    //
    // ⚠ 302, not 301. A permanent redirect is cached by the browser forever and
    // this destination is a deployment detail read from the environment.
    if matches!(req.path.as_str(), "/" | "/app" | "/app/") {
        let to = console_url();
        let canonical = std::env::var("RATIO_CANONICAL_HOST").ok();
        let headers = security_headers("text/plain; charset=utf-8", &req.host, canonical.as_deref());
        if to.is_empty() {
            // A local `ratio watch`. There is no console to point at, so say
            // what this process does serve instead of bouncing to an empty URL.
            let body = "The operations console is a separate application; this \
                        process serves the API at /v1 and the screens at \
                        /balance, /breaks, /rules and /chat.";
            write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain; charset=utf-8\r\n\
                 Content-Length: {}\r\n{headers}Cache-Control: no-store\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len(),
            )?;
        } else {
            write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Type: text/plain; charset=utf-8\r\n\
                 Content-Length: 0\r\n{headers}Cache-Control: no-store\r\n\
                 Connection: close\r\n\r\n",
            )?;
        }
        return Ok(());
    }

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
        // The three public read-only screens, reachable without signing in.
        // ⚠ THESE STAY, AND THEY ARE WHY THE BINARY STILL HAS A BROWSER STORY
        // WITHOUT NODE. They are Rust string literals with no build step, no
        // bundler and no dependency — `bazel run //crates/ratio -- watch` still
        // opens a book in a browser on loopback. What left is the authenticated
        // multi-fund console, not this.
        (_, "/chat") => ("200 OK", "text/html; charset=utf-8", page(CHAT_BODY, "chat")),
        (_, "/balance") => ("200 OK", "text/html; charset=utf-8", page(BALANCE_BODY, "balance")),
        (_, "/breaks") => ("200 OK", "text/html; charset=utf-8", page(BREAKS_BODY, "breaks")),
        (_, "/rules") => ("200 OK", "text/html; charset=utf-8", page(RULES_BODY, "rules")),
        (_, "/scale") => ("200 OK", "text/html; charset=utf-8", lead_page(SCALE_BODY)),
        (_, "/scale.json") => json(scale_json(book, &req.query)),
        (_, "/scale-runs.json") => json(scale_runs_json()),
        ("POST", "/scale/start") => json(scale_start(&req.body)),
        ("POST", "/scale/unlock") => json(scale_unlock(&req.body)),
        (_, "/scale/unlock") => (
            "405 Method Not Allowed",
            "application/json",
            "{\"error\":\"unlocking is a POST\"}".to_string(),
        ),
        (_, "/scale/start") => (
            "405 Method Not Allowed",
            "application/json",
            "{\"error\":\"starting a fold is a POST\"}".to_string(),
        ),
        (_, "/balance.json") => json(balance_json(book)),
        (_, "/postings.json") => json(postings_json(book, &req.query)),
        (_, "/breaks.json") => json(breaks_json(book)),
        (_, "/rules.json") => json(rules_json(book)),

        // The console's API, transcoded from ratio.v1.Console's google.api.http
        // rules. //crates/ratio-console:transcode_test asserts these routes are
        // exactly the ones the contract declares.
        // The report page a lead's email links to. The id is read by the page's
        // own script from its URL and fetched as JSON below — the HTML is one
        // static shell for every run, so a bad id 404s at the fetch, rendered
        // as a sentence rather than a broken page.
        (_, p) if p.starts_with("/scale/runs/") && !p.ends_with(".json") => {
            ("200 OK", "text/html; charset=utf-8", lead_page(REPORT_BODY))
        }

        // A run's published report — the permalink a lead's email points into.
        // ⛔ The id is validated before it goes near a store key; an invalid one
        // is a 404, indistinguishable from a run that never happened.
        (_, p) if p.starts_with("/scale/runs/") && p.ends_with(".json") => {
            let id = p.trim_start_matches("/scale/runs/").trim_end_matches(".json");
            match scale_runner().and_then(|(r, _)| scale::report(r.store(), id)) {
                Some(doc) => ("200 OK", "application/json", doc),
                None => ("404 Not Found", "application/json", "{\"error\":\"no such run\"}".to_string()),
            }
        }

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

        // The public OIDC client configuration, read from the deployment's
        // environment. Inherently public — a client id and a hosted-UI domain
        // are not secrets — so it is served BEFORE authentication, and it is one
        // of the routes the gateway leaves open. Empty strings on a local `ratio
        // watch`, where none of these are set.
        //
        // ⭐ THE CONSOLE'S SERVER READS THIS, NOT THE CONSOLE'S BROWSER. That is
        // the point of keeping it: `deploy/app.yaml` declares the pool, the
        // client and the domain ONCE, and the Next.js application fetches them
        // from here rather than carrying a second copy as Vercel environment
        // variables. Two places that must agree about which pool a token comes
        // from is two places that will one day not.
        //
        // ⚠ EMPTY MEANS "NO IDENTITY PROVIDER", AND THAT IS THE LOCAL STORY. The
        // console skips its sign-in gate when these are blank, which is exactly
        // when this process answers as `Subject::Local` — so `next dev` against
        // a local book needs no Cognito, no secret and no network.
        //
        // ⚠ Not a `ratio.console.v1` resource: it is deployment configuration,
        // not a fund's data, so it does not belong in the contract or its AIP
        // surface. It is served here beside `/version` for the same reason —
        // both describe the running process, not the book.
        (_, "/authconfig.json") => (
            "200 OK",
            "application/json",
            auth_config_json(
                if std::env::var("RATIO_WORKOS_CLIENT_ID")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .is_some()
                {
                    "https://api.workos.com/"
                } else {
                    ""
                },
                &std::env::var("RATIO_WORKOS_CLIENT_ID").unwrap_or_default(),
                "",
                // ⛔ THE SAME ACCESSOR THE `/` REDIRECT USES, NOT A FRESH
                // `env::var`. `console_url()` applies `safe_redirect()`, so a
                // control character in the CloudFormation value is stripped
                // once, in one place, for both readers of it.
                &console_url(),
            ),
        ),

        _ => ("404 Not Found", "text/plain; charset=utf-8", "no".to_string()),
    };

    let canonical = std::env::var("RATIO_CANONICAL_HOST").ok();
    write!(
        stream,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         {}Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n{body}",
        body.len(),
        security_headers(content_type, &req.host, canonical.as_deref())
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
/// The run record and the launcher, resolved ONCE from the environment.
///
/// ⛔ NOT PER REQUEST. `Ecs::from_env` builds a tokio runtime and an SDK client;
/// doing that on every button press would be a fresh thread pool per visitor.
///
/// ⚠ `None` IS A VALID, WORKING STATE, not a misconfiguration. `ratio watch` on
/// a laptop sets none of `RATIO_SCALE_*`, so there is no bucket to keep a record
/// in and no cluster to run on — the screen then serves its estimate and its
/// recorded figures, and offers no button. Same shape as an unset
/// `RATIO_COGNITO_*` meaning "no identity provider here".
type DynStore = Box<dyn scale::Store + Send + Sync>;
type Scale = (scale::Runs<DynStore>, Box<dyn scale::Launcher + Send + Sync>);

fn scale_runner() -> Option<&'static Scale> {
    static SCALE: std::sync::OnceLock<Option<Scale>> = std::sync::OnceLock::new();
    SCALE
        .get_or_init(|| {
            // The deployed shape: the record in S3, the fold in a Fargate task.
            if let Some(bucket) =
                std::env::var("RATIO_SCALE_BUCKET").ok().filter(|b| !b.is_empty())
            {
                let ecs = match scale::Ecs::from_env()? {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("the scale runner is configured but unusable: {e:#}");
                        return None;
                    }
                };
                return match scale::S3::open(bucket, "runs/") {
                    Ok(s) => Some((
                        scale::Runs::over(Box::new(s) as DynStore),
                        Box::new(ecs) as Box<dyn scale::Launcher + Send + Sync>,
                    )),
                    Err(e) => {
                        eprintln!("the scale record is configured but unusable: {e:#}");
                        None
                    }
                };
            }

            // ⚠ THE LOCAL SHAPE IS OPT-IN, NOT INFERRED. `RATIO_SCALE_LOCAL=<dir>`
            // folds on a thread in this process with the record on disk — which
            // is how the progress UI is watched working before a deploy. It is
            // NOT the default for a bare `ratio watch`, because the full shape
            // generates a ~40 GB journal, and a button that can do that to a
            // laptop must be asked for in the environment, not offered by one.
            let dir = std::env::var("RATIO_SCALE_LOCAL").ok().filter(|d| !d.is_empty())?;
            let dir = std::path::PathBuf::from(dir);
            Some((
                scale::Runs::over(Box::new(scale::Files::at(dir.join("runs"))) as DynStore),
                Box::new(scale::Here { books: dir.join("books"), root: dir.join("runs") })
                    as Box<dyn scale::Launcher + Send + Sync>,
            ))
        })
        .as_ref()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The calendar month, as `YYYYMM`, for the spend counter.
///
/// ⚠ DERIVED FROM THE EPOCH RATHER THAN A DATE LIBRARY, because the only
/// property needed is that it CHANGES once a month and never goes backwards.
/// Which day a month turns over on is not a figure anybody is paid on.
fn this_month(now: i64) -> i64 {
    let days = now / 86_400;
    let years = days / 365;
    1970 * 100 + years * 100 + ((days % 365) / 31) + 1
}

/// Start a fold, or say why not.
///
/// ⛔ A SHAPE NAME FROM A FIXED SET, MATCHED EXHAUSTIVELY — never dials, never a
/// path, never anything a caller composes. `terminal_json` sets the standard in
/// this file and its reason applies unchanged: "anything that dispatches on a
/// string the caller controls without an exhaustive match is a remote-execution
/// seam wearing a demo costume". The only thing a caller supplies here is which
/// of three words they want, and `scale::shape` refuses everything else.
fn scale_start(body: &str) -> Result<String> {
    let Some((runs, launcher)) = scale_runner() else {
        bail!(
            "this server has no scale runner configured, so there is nothing to start — the \
             estimate and the recorded run above need none"
        );
    };
    let req: serde_json::Value = serde_json::from_str(body).context("that is not JSON")?;
    let asked = req["size"].as_str().unwrap_or("");
    let Some(shape) = scale::shape(asked) else {
        bail!("{}", scale::Refusal::NoSuchShape);
    };

    // ⛔ THE EMAIL IS A MAILING-LIST ENTRY, NEVER A CONTROL. It is recorded and
    // it earns the follow-up report; whether the fold may START is decided
    // entirely by the lock, the cooldown and the ceiling, exactly as before —
    // so a fabricated address buys nothing that costs money. Optional, because
    // the gate is a page affordance: a caller who skips it just gets no email.
    let email = req["email"].as_str().unwrap_or("").trim().to_string();
    if !email.is_empty() {
        if !scale::valid_email(&email) {
            bail!("that does not look like an email address");
        }
        let _ = scale::record_lead(runs.store(), &email, now_secs());
    }

    let now = now_secs();
    match runs.start(shape.name, now, this_month(now), "pending")? {
        Err(refusal) => {
            // ⭐ A LEAD WHO ARRIVES DURING THE COOLDOWN STILL GETS THE REPORT.
            // The latest run of this shape is the same fold they asked for —
            // deterministic book, same dials, same seed — so the follow-up
            // email cites it, sent now because no task exists to send it later.
            let replay = scale::latest(runs.store(), shape.name);
            if let (false, Some(id)) = (email.is_empty(), &replay) {
                if let Some(Ok(mailer)) = scale::Mailer::from_env() {
                    let summary = scale::report(runs.store(), id)
                        .map(|j| scale::summarize_report(&j))
                        .unwrap_or_default();
                    if let Err(e) = mailer.send_report(&email, id, &summary) {
                        eprintln!("replay report email failed: {e:#}");
                    }
                }
            }
            Ok(format!(
                "{{\"started\":false,\"why\":{},\"report\":{}}}",
                quote(&refusal.to_string()),
                match &replay {
                    Some(id) => quote(id),
                    None => "null".to_string(),
                }
            ))
        }
        Ok(_) => {
            // The run's identity, minted here so the page can show the
            // permalink before the fold begins and the task publishes to it.
            let id = format!("{}-{now}", shape.name);
            if !email.is_empty() {
                let _ = scale::await_report(runs.store(), &id, &email);
            }
            // ⛔ THE LOCK IS TAKEN BEFORE THE TASK IS ASKED FOR, and released if
            // asking fails. The other order would let two visitors both reach
            // RunTask while neither holds anything.
            match launcher.launch(shape.name, &id) {
                Ok(token) => Ok(format!(
                    "{{\"started\":true,\"size\":{},\"id\":{},\"where\":{},\"token\":{}}}",
                    quote(shape.name),
                    quote(&id),
                    quote(&launcher.describe()),
                    quote(&token)
                )),
                Err(e) => {
                    // ⛔ `abandon`, NOT `finish`. No compute happened — the task
                    // was refused before it existed — so the cooldown and the
                    // charge are walked back with the lock. `finish` here left
                    // the deployed demo cooling for a run that never ran.
                    let _ = runs.abandon(shape.name, this_month(now));
                    Err(e).context("the fold was allowed but could not be started")
                }
            }
        }
    }
}

/// Register an address on the mailing list, unlocking the demo button.
///
/// ⛔ THE UNLOCK IS A PAGE AFFORDANCE, NOT A BOUNDARY. Skipping this route and
/// POSTing /scale/start directly starts the same fold under the same money
/// controls — what a skipper forfeits is the report email. Pretending this were
/// authentication would be the lie; recording the list here and at start is the
/// honest shape.
fn scale_unlock(body: &str) -> Result<String> {
    let Some((runs, _)) = scale_runner() else {
        bail!("this server has no scale runner configured, so there is no demo to unlock");
    };
    let req: serde_json::Value = serde_json::from_str(body).context("that is not JSON")?;
    let email = req["email"].as_str().unwrap_or("").trim();
    if !scale::valid_email(email) {
        bail!("that does not look like an email address");
    }
    let new = scale::record_lead(runs.store(), email, now_secs())?;
    Ok(format!("{{\"unlocked\":true,\"new\":{new}}}"))
}

/// What has run, what is running, and what each shape costs to run.
///
/// ⛔ EVERY SHAPE IS LISTED WHETHER OR NOT IT HAS EVER RUN. A screen that only
/// showed shapes with results would be empty on a cold bucket and would look
/// broken rather than idle.
fn scale_runs_json() -> Result<String> {
    let runner = scale_runner();
    let now = now_secs();
    let current = match runner {
        Some((runs, _)) => runs.current()?,
        None => None,
    };
    let mut rows = Vec::new();
    for s in scale::SHAPES.iter() {
        let (result, progress, last) = match runner {
            Some((runs, _)) => (
                runs.result(s.name),
                runs.progress(s.name),
                runs.last_started(s.name),
            ),
            None => (None, None, 0),
        };
        rows.push(format!(
            "{{\"size\":{},\"securities\":{},\"lots_per\":{},\"open_lots\":{},\"entries\":{},\
             \"recorded_cold_build_ms\":{},\"cents\":{},\"cooldown\":{},\"again_at\":{},\
             \"progress\":{},\"result\":{}}}",
            quote(s.name),
            s.securities,
            s.lots_per,
            s.open_lots,
            s.entries,
            s.recorded_cold_build_ms,
            s.cents,
            s.cooldown,
            if last > 0 { last + s.cooldown } else { 0 },
            // ⚠ EMBEDDED RAW, BECAUSE IT IS A DOCUMENT NOW — a phase and a
            // sample series, not a display string. The `starts_with` guard is
            // for a stale key written by the previous format; quoting a JSON
            // document would hand the page a string it then had to parse out of
            // a string, and embedding a NON-document raw would corrupt the
            // whole response.
            match &progress {
                Some(p) if p.starts_with('{') => p.clone(),
                _ => "null".to_string(),
            },
            result.unwrap_or_else(|| "null".to_string()),
        ));
    }
    Ok(format!(
        "{{\"now\":{now},\"runner\":{},\"running\":{},\"shapes\":[{}]}}",
        match runner {
            Some((_, l)) => quote(&l.describe()),
            // ⚠ `null`, and the page turns the button off rather than offering
            // one that cannot work.
            None => "null".to_string(),
        },
        match &current {
            Some(c) => format!(
                "{{\"size\":{},\"started\":{}}}",
                quote(&c.size),
                c.started
            ),
            None => "null".to_string(),
        },
        rows.join(","),
    ))
}

/// This machine's read rate, measured ONCE.
///
/// ⛔ NOT PER REQUEST, AND THAT IS A SECURITY PROPERTY AS MUCH AS A CORRECTNESS
/// ONE. `closure::measure` makes twenty full passes over the journal to average
/// out the fixed cost of opening it. On a public endpoint that is a lever: a
/// visitor who reloads a page would spend twenty journal reads of somebody
/// else's CPU per reload, and the demo runs on a function with a concurrency
/// limit of ten and a five-dollar monthly budget behind it.
///
/// ⚠ A rate is a property of the MACHINE, so measuring it repeatedly answers
/// the same question at somebody's expense. Once per process is the honest
/// frequency, and a Lambda cold start is where it lands.
/// ⚠ THE MEMO AND THE FALLBACK MOVED TO `closure::rate_for`, AND ONLY BECAUSE A
/// SECOND CALLER ARRIVED. `Console::explain_nav_strike` quotes the same rate
/// beside the same kind of duration, and two memoized copies would be two
/// answers to "how fast is this machine" — differing by whichever book each was
/// first asked about, which on a multi-fund process is not the same book.
fn read_rate(book: &Path) -> &'static ratio_nav::closure::Calibration {
    ratio_nav::closure::rate_for(book)
}

/// What a period end of a given shape COSTS, before anyone runs one.
///
/// ⭐ THE ONLY HALF OF THE SCALE ARGUMENT THAT FITS IN A WEB REQUEST. Folding a
/// twenty-million-lot journal takes 995 seconds and a gigabyte; this reads 503
/// things and takes microseconds, because `Ratio.Closure.a_quiet_nav_never_
/// reads_the_lots` says the lots are not among them. The dials are the visitor's;
/// the arithmetic is emitted from the proof.
///
/// ⛔ IT IS THE FLAT CURVE, AND IT SAYS SO. `cold_build` is deliberately absent
/// from this response rather than zero: a NAV strike costing microseconds over
/// twenty million lots is true, and quoting it as what the whole book costs is
/// the overclaim `ratio bench` exists to make hard. The page carries the other
/// curve beside it.
fn scale_json(book: &Path, query: &str) -> Result<String> {
    use ratio_nav::closure::{estimate, Dials};

    let dial = |name: &str, fallback: i64| -> Result<i64> {
        match query.split('&').find_map(|kv| kv.strip_prefix(&format!("{name}="))) {
            None => Ok(fallback),
            Some(v) => {
                let n: i64 = v.parse().with_context(|| format!("{name} must be a number"))?;
                // ⛔ REFUSED, NOT CLAMPED. `estimate` bounds-checks its products
                // and an estimate that silently clamped would be one somebody
                // quotes. A ceiling here keeps the refusal cheap: the guard in
                // `estimate` is about overflow, and this is about a caller who
                // typed enough zeroes to make the page meaningless.
                if !(0..=100_000_000).contains(&n) {
                    bail!("{name} must be between 0 and 100,000,000 — {n} is not a fund");
                }
                Ok(n)
            }
        }
    };

    // An S&P tracker in its twentieth year: five hundred names, three
    // currencies, twenty million open lots. The same default `ratio closure`
    // takes, because it is the case worth arguing about.
    let d = Dials {
        securities: dial("securities", 500)?,
        currencies: dial("currencies", 3)?,
        lots_per: dial("lots-per", 40_000)?,
        open_actions: dial("open-actions", 0)?,
        capital_txns: dial("capital", 0)?,
    };
    let cal = read_rate(book);
    let e = estimate(d, cal)?;

    Ok(format!(
        "{{\"securities\":{},\"currencies\":{},\"lots_per\":{},\"open_actions\":{},\
         \"capital\":{},\"open_lots\":{},\"reads\":{},\"marks\":{},\"fx\":{},\
         \"actions\":{},\"capital_reads\":{},\"nanos\":{},\"human\":{},\
         \"nanos_per_read\":{},\"provenance\":{}}}",
        d.securities,
        d.currencies,
        d.lots_per,
        d.open_actions,
        d.capital_txns,
        e.open_lots,
        e.reads,
        e.marks,
        e.fx,
        e.actions,
        e.capital,
        e.nanos,
        quote(&ratio_nav::closure::human_nanos(e.nanos)),
        cal.nanos_per_read,
        quote(&cal.provenance),
    ))
}

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
pub(crate) fn quote(s: &str) -> String {
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

/// Where the operations console lives, for the redirect at `/`.
///
/// ⛔ THE CONSOLE IS NO LONGER IN THIS BINARY. It was a React bundle inlined
/// into one HTML document and embedded here as a `&str` at compile time, which
/// meant a stylesheet change needed an image build and a CloudFormation deploy,
/// and meant the whole console was one URL — so a break an operator found could
/// not be sent to anybody. It is a Next.js application deployed separately now,
/// with a route per resource.
///
/// Empty on a local `ratio watch`, where there is nothing to redirect to and
/// saying so beats bouncing somebody to an empty string.
///
/// ⚠ A VALUE WITH A CONTROL CHARACTER IN IT IS TREATED AS UNSET. This string is
/// interpolated straight into a `Location:` header, and a CR or LF in it would
/// end that header and start another — response splitting. It is operator
/// configuration rather than caller input, so this is a guard against a typo in
/// a CloudFormation template rather than against an attacker; it costs one line
/// and removes the question.
fn console_url() -> String {
    // ⚠ The guard is a separate pure function so a test can reach it without
    // `std::env::set_var`, which is a race against every other test in this
    // binary.
    safe_redirect(&std::env::var("RATIO_CONSOLE_URL").unwrap_or_default()).to_string()
}

/// The public OIDC client configuration, as `/authconfig.json` serves it.
///
/// ⚠ A PURE FUNCTION SO A TEST CAN REACH IT, for the same reason `safe_redirect`
/// is one: the alternative is `std::env::set_var`, which is a race against every
/// other test in this binary.
///
/// ⛔ `consoleOrigin` IS HERE SO THERE IS ONE COPY OF IT, NOT TWO. Cognito
/// registers `${ConsoleOrigin}/api/auth/callback` from `deploy/app.yaml`, and
/// the console must send that byte-identical string as its OAuth `redirect_uri`
/// or the IdP refuses the sign-in. It used to hold its own copy as a Vercel
/// environment variable, and on the first deployment that had both, the two
/// disagreed — `https://ratio-console.vercel.app` against
/// `https://ratio-ims.vercel.app` — which is exactly the failure the comment on
/// the `/authconfig.json` arm predicts for the issuer and the client id. The
/// rule was already written down; this field applies it to the last value that
/// was still escaping it.
///
/// Empty on a local `ratio watch`, where the console falls back to its own
/// `RATIO_CONSOLE_ORIGIN` and signs in against `http://localhost:3000`.
fn auth_config_json(issuer: &str, client_id: &str, domain: &str, console_origin: &str) -> String {
    format!(
        // ⛔ `/callback`, AuthKit's documented redirect URI. The console no
        // longer runs a Cognito Hosted UI exchange. Empty clientId means a
        // local `ratio watch` with no identity provider.
        "{{\"issuer\":{},\"clientId\":{},\"domain\":{},\"scopes\":[\"openid\",\"email\"],\
         \"redirectPath\":\"/callback\",\"consoleOrigin\":{}}}",
        quote(issuer),
        quote(client_id),
        quote(domain),
        quote(console_origin),
    )
}

/// The configured console URL, or "" if it could not safely be a `Location:`.
fn safe_redirect(value: &str) -> &str {
    if value.chars().any(|c| c.is_ascii_control()) {
        return "";
    }
    value
}

/// Wrap a screen's body in the shared document, marking the current tab.
/// A page served WITHOUT the operator nav — the lead-gen surfaces.
///
/// ⛔ SAME HEAD, SAME FOOT, NO TABS, and that is a product decision recorded in
/// PLAN.md: a prospect landing on the demo should meet one claim and one
/// button, not an operator's tab bar. The operator screens keep their nav;
/// `/scale` and the run reports stand alone. Everything the hygiene tests hold
/// a page to — self-contained, both themes, no external URL, no form element,
/// no innerHTML — applies unchanged, because these bodies stay in `SCREENS`.
fn lead_page(body: &str) -> String {
    format!("{HEAD}{body}{FOOT}")
}

fn page(body: &str, current: &str) -> String {
    let tab = |slug: &str, href: &str, label: &str| {
        format!(
            r#"<a href="{href}"{}>{label}</a>"#,
            if slug == current { r#" aria-current="page""# } else { "" }
        )
    };
    format!(
        "{HEAD}<nav class=\"tabs\">{}{}{}{}</nav>{body}{FOOT}",
        tab("chat", "/chat", "Set up the books"),
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

/// The scale screen — what a period end costs, at a size you choose.
///
/// ⛔ TWO CURVES, SIDE BY SIDE, ALWAYS. The dial answers "what does the NAV
/// read", which is flat in the tax lots and is the persuasive number. The panel
/// beside it is the cold build, which is NOT flat and which nobody can run from
/// a browser — 995 seconds and a gigabyte at twenty million lots. A screen
/// showing only the first would be the overclaim `ratio bench` exists to make
/// hard, published at a public URL.
///
/// ⚠ The recorded figures below are quoted from HANDOFF.md and labelled as
/// recorded. They are a measurement taken on a named machine on a named day, not
/// something this process did — and the page says which is which, because a
/// reader who cannot tell them apart will take the recorded one for a live one.

/// The report a lead's email links to: one run, every figure, the fold as it
/// happened.
///
/// ⛔ ONE STATIC SHELL FOR EVERY RUN. The id comes from the URL in the page's
/// own script and the document from `/scale/runs/{id}.json` — so nothing a
/// caller typed is ever interpolated into HTML on the server, and a bad id is a
/// sentence, not a broken page. Built with the DOM throughout, same as every
/// screen: the document under it embeds strings this server did not write.
const REPORT_BODY: &str = r##"<div class="wrap">
<header class="hero">
  <p class="brand">ratio</p>
  <h1 id="r-title">A fold, and the figures it struck</h1>
  <p class="lede" id="r-sub">Reading the run…</p>
</header>

<div class="panels">
  <section class="panel warn">
    <h2>The cold build <span class="grows">grows with every trade</span></h2>
    <dl id="r-cold" class="rep"></dl>
  </section>
  <section class="panel">
    <h2>What it proves <span class="flat">flat in the lots</span></h2>
    <dl id="r-flat" class="rep"></dl>
    <p class="note">⛔ Two curves, deliberately side by side. Folding the journal
    is O(entries) and grows forever; the NAV struck off the finished projection
    reads one price per security and one rate per currency, and the tax lots are
    not among them — <code>Ratio.Closure.factored_nav_never_reads_the_lots</code>.</p>
  </section>
</div>

<figure class="livefig">
  <svg id="r-chart" viewBox="0 0 640 200" role="img"
       aria-label="Entries processed against elapsed seconds, as this run happened"></svg>
  <figcaption class="note" id="r-cap">The fold as it happened — entries read
  against elapsed seconds, from the run's own progress record.</figcaption>
</figure>

<section class="gate">
  <h2>Reproduce it, or run your own</h2>
  <p class="note" id="r-repro"></p>
  <p class="note"><a href="/scale">Run the demo yourself</a> ·
  <a href="/">open the operations console</a> ·
  the code and the proofs: <code>github.com/mattmarshall/ratio</code></p>
</section>
</div>
<style>
.hero{margin:8px 0 26px}
.hero .brand{font-size:13px;font-weight:700;letter-spacing:.14em;
  text-transform:uppercase;color:var(--accent);margin:0 0 10px}
.hero h1{font-size:30px;line-height:1.15;margin:0 0 12px;max-width:24ch}
.lede{color:var(--text-2);max-width:60ch}
.panels{display:grid;grid-template-columns:1fr;gap:18px}
@media(min-width:760px){.panels{grid-template-columns:1fr 1fr}}
.panel{padding:16px;border:1px solid var(--rule);border-radius:8px;
  background:var(--raised)}
.panel h2{font-size:15px;margin:0 0 12px;display:flex;gap:8px;
  align-items:baseline;flex-wrap:wrap}
.flat,.grows{font-size:11px;font-weight:700;letter-spacing:.04em;
  text-transform:uppercase;padding:2px 7px;border-radius:99px}
.flat{background:var(--accent);color:var(--ground)}
.grows{background:var(--warn);color:var(--ground)}
.rep{display:grid;grid-template-columns:1fr auto;gap:6px 16px;margin:0}
.rep dt{color:var(--muted);font-size:14px}
.rep dd{margin:0;text-align:right;font-variant-numeric:tabular-nums;font-weight:600}
.rep .big{font-size:20px;color:var(--accent)}
.livefig{margin:20px 0 0}
#r-chart{width:100%;height:auto;display:block}
.gate{padding:16px;border:1px solid var(--rule);border-radius:8px;
  background:var(--raised);margin:22px 0 0}
.gate h2{font-size:15px;margin:0 0 8px}
.gate a{color:var(--accent)}
.note{font-size:13px;color:var(--muted);max-width:62ch}
</style>
<script>
const fmt = n => n.toLocaleString('en-US');
const ms = n => n >= 1000 ? (n / 1000).toFixed(1) + ' s' : n + ' ms';

function row(dl, label, value, big) {
  const dt = document.createElement('dt');
  dt.textContent = label;
  const dd = document.createElement('dd');
  dd.textContent = value;
  if (big) dd.className = 'big';
  dl.append(dt, dd);
}

// The run's own series, redrawn from its embedded record — never from the live
// progress key, which the next run of this size overwrites.
function draw(samples, expected) {
  const svg = document.getElementById('r-chart');
  const NS = 'http://www.w3.org/2000/svg';
  svg.replaceChildren();
  if (!samples || samples.length < 2) return;
  const W = 640, H = 200, PAD = 34;
  const tMax = Math.max(samples[Math.max(samples.length - 1, 0)][0], 1);
  const nMax = Math.max(expected || 0, samples[samples.length - 1][1], 1);
  const x = t => PAD + (W - 2 * PAD) * t / tMax;
  const y = n => H - PAD - (H - 2 * PAD) * n / nMax;
  const el = (k, at) => {
    const e = document.createElementNS(NS, k);
    for (const [a, v] of Object.entries(at)) e.setAttribute(a, v);
    svg.append(e);
    return e;
  };
  el('line', {x1: PAD, y1: H - PAD, x2: W - PAD, y2: H - PAD,
              stroke: 'var(--rule)', 'stroke-width': 1});
  el('line', {x1: PAD, y1: PAD, x2: PAD, y2: H - PAD,
              stroke: 'var(--rule)', 'stroke-width': 1});
  el('path', {
    d: samples.map((p, i) => (i ? 'L' : 'M') + x(p[0]).toFixed(1) + ' ' + y(p[1]).toFixed(1)).join(' '),
    fill: 'none', stroke: 'var(--warn)', 'stroke-width': 2,
  });
  const label = el('text', {x: W - PAD, y: PAD - 6, 'text-anchor': 'end',
    'font-size': 11, fill: 'var(--muted)'});
  label.textContent = fmt(samples[samples.length - 1][1]) + ' entries over '
    + samples[samples.length - 1][0] + ' s';
}

async function load() {
  // ⛔ THE ID NEVER TOUCHES MARKUP. It rides only in the fetch URL, and the
  // server validates it again before it goes near a store key.
  const id = location.pathname.split('/').pop();
  const sub = document.getElementById('r-sub');
  const r = await fetch('/scale/runs/' + encodeURIComponent(id) + '.json');
  if (!r.ok) {
    sub.textContent = 'No run is published under this id. Runs expire from '
      + 'nobody\u2019s hands — this link never existed.';
    return;
  }
  const d = await r.json();
  const run = d.run || {};
  sub.textContent = fmt(run.open_lots || 0) + ' open tax lots over '
    + fmt(run.journal_entries || 0) + ' journal entries, folded cold from an '
    + 'empty projection. Trial balance: ' + fmt(run.trial_balance || 0)
    + '. Build ' + String(run.build || '').slice(0, 12) + '.';

  const cold = document.getElementById('r-cold');
  row(cold, 'cold build', ms(Math.round((run.cold_build_ns || 0) / 1e6)), true);
  row(cold, 'of which parse', ms(Math.round((run.parse_ns || 0) / 1e6)));
  row(cold, 'of which fold', ms(Math.round((run.fold_ns || 0) / 1e6)));
  row(cold, 'reliefs', fmt(run.reliefs || 0));
  row(cold, 'recorded run of this shape', ms(run.recorded_cold_build_ms || 0));

  const flat = document.getElementById('r-flat');
  row(flat, 'open tax lots', fmt(run.open_lots || 0), true);
  row(flat, 'journal entries', fmt(run.journal_entries || 0));
  row(flat, 'trial balance', fmt(run.trial_balance || 0), true);
  row(flat, 'securities', fmt(run.securities || 0));
  row(flat, 'lots per security', fmt(run.lots_per || 0));

  document.getElementById('r-repro').textContent =
    'This book is a pure function of its dials and seed \u2014 '
    + 'ratio bench --securities ' + (run.securities || 0)
    + ' --lots-per ' + (run.lots_per || 0)
    + ' --currencies 3 --seed 1 reproduces these figures on your machine, '
    + 'byte for byte.';

  const prog = d.progress || {};
  draw(prog.samples || [], prog.expected || run.journal_entries || 0);
}
load();
</script>
"##;

const SCALE_BODY: &str = r##"<div class="wrap">
<header class="hero">
  <p class="brand">ratio</p>
  <h1>Twenty million tax lots.<br>Folded cold in seventeen minutes.<br>
  <span class="accent">Struck in twelve microseconds.</span></h1>
  <p class="lede">A brand-new fund — 140 million journal entries, three
  currencies, every lot dated — generated from a seed, folded from an empty
  projection on real hardware, and the trial balance ties at zero. Not a video,
  not a mock: the button below runs it, and you watch both curves happen.</p>
  <p class="lede">The seventeen minutes is the recorded cold build. The NAV
  struck off the finished projection is <strong>microseconds</strong>, because a
  NAV never reads the tax lots — that is the theorem, and this page is where you
  check it rather than take it.</p>
</header>

<!-- The mailing-list gate. ⛔ AN AFFORDANCE, NOT A BOUNDARY, and the comment in
     `scale_unlock` says so in the same words: skipping it starts the same fold
     under the same money controls; what it earns is the follow-up report with
     the permalink to every figure. -->
<section class="gate" id="gate">
  <h2>Run it, and keep the figures</h2>
  <p class="note">Leave an email and the demo unlocks. When your fold lands you
  get the report — every figure, the fold as it happened, and the shape and seed
  that reproduce it. No account, no card; you are joining the mailing list, and
  that is the whole trade.</p>
  <div class="gaterow">
    <input id="lead-email" type="email" autocomplete="email"
           placeholder="you@fund.example" aria-label="Work email">
    <button class="fold" id="unlock">Unlock the demo</button>
  </div>
  <p class="note" id="gate-note"></p>
</section>

<!-- ⛔ A PLAIN DIV, NOT A FORM ELEMENT, AND THE FENCE IS WORTH MORE THAN THE
     CONVENIENCE. No screen this binary serves has one: `approval_is_a_person_
     typing_a_command_not_a_control` asserts it, because a form is how a page
     grows a write. These dials only ever produce a query string on a GET, so
     they need nothing of the kind — and widening that assertion to admit "a
     read-only one" would remove the guard for every screen after this.
     ⚠ The assertion is a grep over this source, so naming the tag here at all
     would trip it. That is the check working, not a false alarm to route
     around. -->

<details class="depth">
<summary>The arithmetic, if you want to turn the dials yourself</summary>

<div id="dials" class="dials">
  <label>Securities <input id="securities" type="number" min="0" max="100000000" value="500"></label>
  <label>Currencies <input id="currencies" type="number" min="0" max="100000000" value="3"></label>
  <label>Open lots per security <input id="lots-per" type="number" min="0" max="100000000" value="40000"></label>
  <label>Open corporate actions <input id="open-actions" type="number" min="0" max="100000000" value="0"></label>
</div>

<div class="panels">
  <section class="panel">
    <h2>What the NAV reads <span class="flat">flat in the lots</span></h2>
    <dl id="estimate"><dt>open tax lots</dt><dd>—</dd></dl>
    <p class="prov" id="prov"></p>
    <p class="note" id="cliff"></p>
  </section>

  <section class="panel warn">
    <h2>What folding the journal costs <span class="grows">grows with every trade</span></h2>
    <p>An append-only log does not forget a closed lot, so building a projection
    from nothing is <em>O(entries)</em> and is the curve that does not flatten.
    This is a recorded run, not this page:</p>
    <table class="mini"><tbody>
      <tr><th>open tax lots</th><td>20,004,324</td></tr>
      <tr><th>journal entries</th><td>140,030,274</td></tr>
      <tr><th>cold build</th><td>995.0 s</td></tr>
      <tr><th class="sub">of which parse</th><td class="sub">655.2 s</td></tr>
      <tr><th class="sub">of which fold</th><td class="sub">339.7 s</td></tr>
      <tr><th>NAV strike off it</th><td>12 µs</td></tr>
      <tr><th>peak memory footprint</th><td>1.00 GB</td></tr>
      <tr><th>trial balance</th><td>0</td></tr>
    </tbody></table>
    <p class="note">⛔ <strong>Peak footprint, not resident set size.</strong> At
    this size macOS reports 52 MB for a process whose real footprint is 1.00 GB,
    because the lot data is written once and then cold, which is exactly what the
    OS compresses. A nineteen-fold understatement of the number that decides
    whether a book can be folded at all.</p>
    <p class="note">Reproduce it with
    <code>ratio bench --securities 500 --lots-per 40000 --currencies 3</code>,
    or fold a book you already have with
    <code>ratio bench --fold --book DIR</code>.</p>
  </section>
</div>
</details>

<section class="runs">
  <h2>Run it now</h2>
  <p class="note" id="runner-note"></p>
  <table class="mini runs-table">
    <thead><tr><th>Shape</th><th class="n">Open lots</th><th class="n">Recorded</th><th class="n">This run</th><th></th></tr></thead>
    <tbody id="runs-body"></tbody>
  </table>
  <p class="note">⛔ <strong>Both curves, always.</strong> "This run" is the cold
  build — folding the journal from nothing, which grows with every trade ever
  made. The NAV strike off the finished projection is the flat one, and it is the
  panel above. Quoting the second as though it were the first is the overclaim
  this screen exists to make hard.</p>
  <p class="note">⚠ A fold runs on somebody's money, so there is one at a time,
  a cooldown per shape and a monthly ceiling. A second visitor arriving mid-run
  <em>joins</em> it — the book is generated from a seed, so it is the same fold
  they would have started.</p>

  <div id="live" hidden>
    <h3 id="live-title"></h3>
    <div class="meter"><div id="live-bar"></div></div>
    <dl class="readout">
      <div><dt>entries</dt><dd id="live-n">—</dd></div>
      <div><dt>rate</dt><dd id="live-rate">—</dd></div>
      <div><dt>eta</dt><dd id="live-eta">—</dd></div>
      <div><dt>elapsed</dt><dd id="live-t">—</dd></div>
    </dl>
    <figure class="livefig">
      <svg id="live-chart" viewBox="0 0 640 200" role="img"
           aria-label="Entries processed against elapsed seconds, drawn as the run happens"></svg>
      <figcaption class="note">⛔ This is the GROWING curve — the journal being
      read from nothing. The flat one is the strike in the panel above, and the
      dashes mark where the recorded run of this shape finished.</figcaption>
    </figure>
  </div>
</section>

<p class="note">⛔ <strong>Quoting the second curve as though it were the
first is the overclaim this screen exists to make hard.</strong> "Twenty million
lots are free" is true of the strike and false of the fold. Both are on this
page for that reason.</p>

<footer class="tail">
  <p>Ratio keeps a fund's books as an append-only journal of conserved postings
  and proves what must be true of them. The proofs are Lean 4, the model checks
  are TLA+, the kernel is Rust emitted from the Lean.</p>
  <!-- ⛔ NO EXTERNAL URL ON ANY SCREEN — `every_page_is_self_contained` is
       blunt on purpose. The console link is `/`, which already redirects
       there; the repository is cited as text, and the follow-up email carries
       the clickable link. -->
  <p><a href="/">Open the operations console</a> ·
  <a href="/balance">See a live trial balance</a> ·
  the code and the proofs: <code>github.com/mattmarshall/ratio</code></p>
</footer>
</div>
<style>
.lede{color:var(--text-2);max-width:60ch}
.dials{display:flex;gap:18px;flex-wrap:wrap;margin:22px 0;padding:16px;
  background:var(--surface);border:1px solid var(--rule);border-radius:8px}
.dials label{display:flex;flex-direction:column;gap:6px;font-size:13px;
  font-weight:600;color:var(--muted)}
.dials input{width:150px;padding:6px 8px;font:inherit;font-size:14px;
  background:var(--raised);color:var(--text);border:1px solid var(--rule);
  border-radius:6px}
.panels{display:grid;grid-template-columns:1fr;gap:18px}
@media(min-width:760px){.panels{grid-template-columns:1fr 1fr}}
.panel{padding:16px;border:1px solid var(--rule);border-radius:8px;
  background:var(--raised)}
.panel h2{font-size:15px;margin:0 0 12px;display:flex;gap:8px;
  align-items:baseline;flex-wrap:wrap}
.flat,.grows{font-size:11px;font-weight:700;letter-spacing:.04em;
  text-transform:uppercase;padding:2px 7px;border-radius:99px}
.flat{background:var(--accent);color:var(--ground)}
.grows{background:var(--warn);color:var(--ground)}
#estimate{display:grid;grid-template-columns:1fr auto;gap:6px 16px;margin:0}
#estimate dt{color:var(--muted);font-size:14px}
#estimate dd{margin:0;text-align:right;font-variant-numeric:tabular-nums;
  font-weight:600}
#estimate .big{font-size:22px;color:var(--accent)}
.mini{width:100%;border-collapse:collapse;font-size:14px}
.mini th{text-align:left;font-weight:400;color:var(--muted);padding:3px 0}
.mini td{text-align:right;font-variant-numeric:tabular-nums;padding:3px 0}
.mini .sub{padding-left:14px;color:var(--muted);font-size:13px}
.prov{font-size:12px;color:var(--muted);margin:14px 0 0;font-style:italic}
.runs{margin:26px 0 0;padding:16px;border:1px solid var(--rule);border-radius:8px;
  background:var(--raised)}
.runs h2{font-size:15px;margin:0 0 10px}
.runs-table th.n,.runs-table td{text-align:right}
.runs-table thead th{font-size:12px;text-transform:uppercase;letter-spacing:.04em}
.runs-table td:first-child,.runs-table th:first-child{text-align:left}
.runs-table tbody td{padding:6px 0;border-top:1px solid var(--rule)}
button.fold{font:inherit;font-size:13px;font-weight:600;padding:5px 12px;
  border-radius:6px;border:1px solid var(--accent);background:var(--accent);
  color:var(--ground);cursor:pointer}
button.fold[disabled]{background:transparent;color:var(--muted);
  border-color:var(--rule);cursor:not-allowed}
#live{margin-top:18px;padding-top:14px;border-top:1px solid var(--rule)}
#live h3{font-size:14px;margin:0 0 8px}
.meter{height:10px;border-radius:99px;background:var(--surface);overflow:hidden}
#live-bar{height:100%;width:0%;background:var(--accent);border-radius:99px;
  transition:width .8s linear}
.readout{display:flex;gap:26px;margin:10px 0 4px;flex-wrap:wrap}
.readout div{display:flex;flex-direction:column}
.readout dt{font-size:11px;text-transform:uppercase;letter-spacing:.05em;
  color:var(--muted)}
.readout dd{margin:0;font-variant-numeric:tabular-nums;font-weight:600}
.livefig{margin:12px 0 0}
#live-chart{width:100%;height:auto;display:block}
.note{font-size:13px;color:var(--muted);max-width:62ch}
.hero{margin:8px 0 26px}
.hero .brand{font-size:13px;font-weight:700;letter-spacing:.14em;
  text-transform:uppercase;color:var(--accent);margin:0 0 10px}
.hero h1{font-size:34px;line-height:1.15;margin:0 0 14px;max-width:22ch}
.hero .accent{color:var(--accent)}
.gate{padding:16px;border:1px solid var(--accent);border-radius:8px;
  background:var(--raised);margin:0 0 20px}
.gate h2{font-size:15px;margin:0 0 8px}
.gaterow{display:flex;gap:10px;flex-wrap:wrap;margin:10px 0 6px}
.gaterow input{flex:1;min-width:220px;padding:8px 10px;font:inherit;
  font-size:14px;background:var(--surface);color:var(--text);
  border:1px solid var(--rule);border-radius:6px}
.depth{margin:20px 0}
.depth>summary{cursor:pointer;font-size:14px;font-weight:600;
  color:var(--muted);padding:6px 0}
.tail{margin:30px 0 0;padding-top:16px;border-top:1px solid var(--rule);
  font-size:13px;color:var(--muted)}
.tail a{color:var(--accent)}
</style>
<script>
// ⛔ THE ARITHMETIC IS NOT HERE. Every figure below comes from /scale.json,
// which calls `ratio_nav::closure::estimate` — emitted from `Ratio.Closure`. A
// copy of the formula in JavaScript would be a second answer to a proved
// question, and the two would drift the first time the proof changed.
const fmt = n => n.toLocaleString('en-US');
// ⛔ localStorage HOLDS ONLY WHAT THE VISITOR TYPED ABOUT THEMSELVES, and it is
// a convenience, not a session: the server re-validates the address on every
// call and the money controls never read it.
const LEAD = 'ratio-lead-email';
const lead = () => localStorage.getItem(LEAD) || '';
const DIALS = ['securities', 'currencies', 'lots-per', 'open-actions'];
const dials = document.getElementById('dials');
let pending = null;

// ⛔ BUILT WITH THE DOM, NEVER innerHTML. Two of the strings rendered here are
// not ours: a refusal quotes the caller's own dial back at them, and the
// provenance carries a filesystem path. Assigning either as markup would turn a
// query string into script, on a page served without a sign-in.
function row(dl, label, value, big) {
  const dt = document.createElement('dt');
  dt.textContent = label;
  const dd = document.createElement('dd');
  dd.textContent = value;
  if (big) dd.className = 'big';
  dl.append(dt, dd);
}

async function tick() {
  const q = new URLSearchParams();
  for (const name of DIALS) q.set(name, document.getElementById(name).value);
  const r = await fetch('/scale.json?' + q.toString());
  const d = await r.json();
  const est = document.getElementById('estimate');
  est.replaceChildren();
  if (d.error) { row(est, 'refused', d.error); return; }
  row(est, 'open tax lots', fmt(d.open_lots));
  row(est, 'marks — one per security', fmt(d.marks));
  row(est, 'rates — one per currency', fmt(d.fx));
  row(est, 'corporate actions', fmt(d.actions));
  row(est, 'reads a period end makes', fmt(d.reads), true);
  row(est, 'at this machine’s rate', d.human, true);
  document.getElementById('prov').textContent =
    d.nanos_per_read + ' ns per read — ' + d.provenance;
  // ⭐ THE CLIFF. `Ratio.Closure.an_open_action_costs_the_lots_of_its_instrument`
  // — one outstanding action makes the NAV read the lots of that one name, and
  // the cost goes from the chart to the history. It is the single dial on this
  // page that changes the shape of the answer rather than its size.
  document.getElementById('cliff').textContent = d.open_actions > 0
    ? 'With ' + fmt(d.open_actions) + ' action(s) outstanding the NAV reads '
      + fmt(d.actions) + ' lots — the tax lots are in the total. That is the '
      + 'cliff Ratio.Closure.the_cliff names, and it is why an action is carried '
      + 'as a factor rather than applied by rewriting.'
    : 'With nothing outstanding the tax lots are not in the total at all: '
      + 'Ratio.Closure.a_quiet_nav_never_reads_the_lots. Raise the open actions '
      + 'above zero to see what a rewrite would have cost.';
}

// ── the gate ────────────────────────────────────────────────────────────
function gatePaint() {
  const has = !!lead();
  document.getElementById('gate-note').textContent = has
    ? 'Unlocked for ' + lead() + ' — your report lands when the fold does.'
    : '';
  document.getElementById('gate').classList.toggle('open', has);
  return has;
}

document.getElementById('unlock').addEventListener('click', async () => {
  const email = document.getElementById('lead-email').value.trim();
  const note = document.getElementById('gate-note');
  const r = await (await fetch('/scale/unlock', {
    method: 'POST',
    headers: {'content-type': 'application/json'},
    body: JSON.stringify({email}),
  })).json();
  if (r.error) { note.textContent = r.error; return; }
  localStorage.setItem(LEAD, email);
  gatePaint();
  runs();
});
gatePaint();

// ── the runs panel ──────────────────────────────────────────────────────
const ms = n => n >= 1000 ? (n / 1000).toFixed(1) + ' s' : n + ' ms';
const secs = t => t >= 90 ? Math.round(t / 60) + ' min' : Math.round(t) + ' s';

function cell(row, text, cls) {
  const td = document.createElement('td');
  td.textContent = text;
  if (cls) td.className = cls;
  row.append(td);
  return td;
}

// SVG built with the DOM, like everything else on these screens — the chart is
// data plus two labels, and none of it may pass through markup.
const SVG = 'http://www.w3.org/2000/svg';
function el(name, attrs, parent) {
  const e = document.createElementNS(SVG, name);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  parent.append(e);
  return e;
}

// The rate over a trailing window, never since the start: parse and fold move
// at different speeds, and generation at a third, so a cumulative average is
// wrong in every phase.
function windowed(samples) {
  if (samples.length < 2) return 0;
  const last = samples[samples.length - 1];
  let i = samples.length - 2;
  while (i > 0 && last[0] - samples[i][0] < 30) i--;
  const dt = last[0] - samples[i][0];
  return dt > 0 ? (last[1] - samples[i][1]) / dt : 0;
}

// Entries against elapsed seconds, with the recorded run's finish as a dashed
// vertical marker. ⚠ A MARKER, NOT A PROJECTED LINE: the recorded run left one
// number — where it ended — and drawing a slope to it would claim a trajectory
// nobody measured.
function drawChart(svg, samples, expected, recordedS) {
  svg.replaceChildren();
  const W = 640, H = 200, L = 46, R = 12, T = 12, B = 24;
  const last = samples[samples.length - 1] || [0, 0];
  const xMax = Math.max(last[0] * 1.15, recordedS * 1.05, 10);
  const yMax = Math.max(expected, last[1]);
  const px = t => L + (W - L - R) * (t / xMax);
  const py = n => H - B - (H - T - B) * (n / yMax);

  // Recessive grid: three lines, no box.
  for (const f of [0.25, 0.5, 0.75, 1]) {
    el('line', {x1: L, x2: W - R, y1: py(yMax * f), y2: py(yMax * f),
      stroke: 'var(--rule)', 'stroke-width': 1}, svg);
  }
  el('line', {x1: L, x2: W - R, y1: py(0), y2: py(0),
    stroke: 'var(--muted)', 'stroke-width': 1}, svg);

  // Axis extents in text tokens — the only numbers on the frame.
  const label = (x, y, text, anchor) => {
    const t = el('text', {x, y, 'text-anchor': anchor, fill: 'var(--muted)',
      'font-size': 11}, svg);
    t.textContent = text;
    return t;
  };
  label(L - 6, py(yMax) + 4, fmt(yMax), 'end');
  label(L - 6, py(0) + 4, '0', 'end');
  label(W - R, H - 6, secs(xMax), 'end');
  label(L, H - 6, '0', 'start');

  // Where the recorded run of this shape FINISHED.
  if (recordedS <= xMax) {
    el('line', {x1: px(recordedS), x2: px(recordedS), y1: py(yMax), y2: py(0),
      stroke: 'var(--muted)', 'stroke-width': 1.5,
      'stroke-dasharray': '5 4'}, svg);
    label(px(recordedS), H - 6, 'recorded ' + secs(recordedS), 'middle');
  }

  // The series: one line, the page's accent, 2px.
  if (samples.length > 1) {
    el('polyline', {
      points: samples.map(([t, n]) => px(t) + ',' + py(n)).join(' '),
      fill: 'none', stroke: 'var(--accent)', 'stroke-width': 2,
      'stroke-linejoin': 'round', 'stroke-linecap': 'round'}, svg);
  }
  const dot = el('circle', {cx: px(last[0]), cy: py(last[1]), r: 4,
    fill: 'var(--accent)'}, svg);

  // The hover layer: nearest sample under the pointer, as a crosshair + text.
  const hoverLine = el('line', {y1: py(yMax), y2: py(0), stroke: 'var(--text-2)',
    'stroke-width': 1, visibility: 'hidden'}, svg);
  const hoverText = el('text', {fill: 'var(--text-2)', 'font-size': 11,
    'text-anchor': 'middle', visibility: 'hidden'}, svg);
  const overlay = el('rect', {x: L, y: T, width: W - L - R, height: H - T - B,
    fill: 'transparent'}, svg);
  overlay.addEventListener('mousemove', ev => {
    const box = svg.getBoundingClientRect();
    const t = ((ev.clientX - box.left) * (W / box.width) - L) / (W - L - R) * xMax;
    let best = samples[0];
    for (const s of samples) if (Math.abs(s[0] - t) < Math.abs(best[0] - t)) best = s;
    if (!best) return;
    hoverLine.setAttribute('x1', px(best[0]));
    hoverLine.setAttribute('x2', px(best[0]));
    hoverText.setAttribute('x', px(best[0]));
    hoverText.setAttribute('y', Math.max(py(best[1]) - 8, T + 10));
    hoverText.textContent = fmt(best[1]) + ' @ ' + secs(best[0]);
    hoverLine.setAttribute('visibility', 'visible');
    hoverText.setAttribute('visibility', 'visible');
  });
  overlay.addEventListener('mouseleave', () => {
    hoverLine.setAttribute('visibility', 'hidden');
    hoverText.setAttribute('visibility', 'hidden');
  });
  svg.append(dot, hoverLine, hoverText, overlay);
}

function showLive(d) {
  const live = document.getElementById('live');
  if (!d.running) { live.hidden = true; return false; }
  const s = d.shapes.find(x => x.size === d.running.size);
  const p = s && s.progress;
  live.hidden = false;
  const phase = p ? p.phase : 'starting';
  document.getElementById('live-title').textContent =
    'The ' + d.running.size + ' fold, ' + phase;
  if (!p || !p.samples || !p.samples.length) return true;

  const last = p.samples[p.samples.length - 1];
  const frac = p.expected > 0 ? last[1] / p.expected : 0;
  // ⚠ CLAMPED: `expected` is the DECLARED entry count, and if the generator
  // ever drifted from the table a bar past 100% would be the symptom. The
  // chart deliberately does not clamp, so the drift stays visible somewhere.
  document.getElementById('live-bar').style.width =
    Math.min(100, frac * 100).toFixed(1) + '%';
  document.getElementById('live-n').textContent =
    fmt(last[1]) + ' / ' + fmt(p.expected);
  document.getElementById('live-t').textContent = secs(last[0]);
  const rate = windowed(p.samples);
  document.getElementById('live-rate').textContent =
    rate > 0 ? fmt(Math.round(rate)) + '/s' : '—';
  document.getElementById('live-eta').textContent =
    rate > 0 && p.expected > last[1]
      ? '~' + secs((p.expected - last[1]) / rate) : '—';
  drawChart(document.getElementById('live-chart'), p.samples, p.expected,
    s.recorded_cold_build_ms / 1000);
  return true;
}

async function runs() {
  const d = await (await fetch('/scale-runs.json')).json();
  const body = document.getElementById('runs-body');
  const note = document.getElementById('runner-note');
  body.replaceChildren();

  // ⚠ NO RUNNER IS A WORKING STATE, NOT AN ERROR. A local `ratio watch`
  // configures none of RATIO_SCALE_*, and the screen says so rather than
  // offering a button that cannot do anything.
  note.textContent = d.runner
    ? 'A fold runs as ' + d.runner + '.'
    : 'This server has no runner configured, so the shapes below are the '
      + 'recorded figures only. The estimate above is live either way.';

  for (const s of d.shapes) {
    const tr = document.createElement('tr');
    cell(tr, s.size);
    cell(tr, fmt(s.open_lots));
    cell(tr, ms(s.recorded_cold_build_ms));

    // ⛔ THE COLD BUILD, NOT THE STRIKE. The flat curve is the panel above;
    // putting it here unlabelled beside "recorded" would invite reading one
    // as the other.
    const running = d.running && d.running.size === s.size;
    const mine = running
      ? (s.progress ? s.progress.phase + '…' : 'starting…')
      : (s.result ? ms(Math.round(s.result.cold_build_ns / 1e6)) : '—');
    cell(tr, mine);

    const td = document.createElement('td');
    const b = document.createElement('button');
    b.className = 'fold';
    b.textContent = running ? 'running' : 'Fold it';
    const cooling = s.again_at > d.now;
    b.disabled = !d.runner || !!d.running || cooling || !lead();
    if (!lead()) b.title = 'Unlock the demo above first';
    if (cooling && !d.running) b.textContent = 'cooling';
    b.addEventListener('click', async () => {
      b.disabled = true;
      const r = await (await fetch('/scale/start', {
        method: 'POST',
        headers: {'content-type': 'application/json'},
        body: JSON.stringify({size: s.size, email: lead()}),
      })).json();
      if (r.error || r.why) note.textContent = r.error || r.why;
      runs();
    });
    td.append(b);
    tr.append(td);
    body.append(tr);
  }

  return showLive(d);
}

// ⚠ Two speeds: a run in flight redraws every 2 s off samples that move every
// 5, and an idle panel asks every 10 — a fold is minutes long, and there is no
// third state worth a third speed. (No SSE and no streaming: this API sits
// behind a gateway and an adapter that give both up, so polling is the whole
// option space.)
let timer = null;
function pace(active) {
  const want = active ? 2000 : 10000;
  if (timer && timer.want === want) return;
  if (timer) clearInterval(timer.id);
  timer = {want, id: setInterval(async () => pace(await runs()), want)};
}
runs().then(pace);

dials.addEventListener('input', () => {
  clearTimeout(pending);
  pending = setTimeout(tick, 150);
});
tick();
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
        let html = security_headers("text/html; charset=utf-8", "x", None);
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
        let json = security_headers("application/json", "x", None);
        assert!(json.contains("default-src 'none'"));
        assert!(!json.contains("'unsafe-inline'"), "an API response has no inline anything");
        // ⛔ NO PAGE THIS BINARY SERVES TALKS TO AN IDENTITY PROVIDER, AND THE
        // POLICY HAD BETTER NOT SAY OTHERWISE. The console used to run PKCE in
        // the tab, so the Cognito origin joined connect-src; it is a Next.js
        // application on its own origin now and exchanges the code on its
        // server. The screens left here sign nobody in, so admitting an off-site
        // origin would be widening the policy for a flow that no longer exists.
        assert!(
            html.contains("connect-src 'self';"),
            "these pages talk to their own origin and nothing else"
        );
        assert!(!html.contains("amazoncognito"), "no page here reaches the IdP");
    }

    #[test]
    fn hsts_is_sent_only_on_our_own_host() {
        // ⛔ On the shared execute-api host, HSTS would poison HSTS state for
        // every other AWS API served from it. It is gated on the canonical host.
        let shared = "abc123.execute-api.us-east-1.amazonaws.com";
        let ours = "ratio.fastverk.dev";
        assert!(
            !security_headers("text/html", shared, Some(ours))
                .contains("Strict-Transport-Security"),
            "HSTS must not ride the shared AWS host"
        );
        assert!(security_headers("text/html", ours, Some(ours))
            .contains("Strict-Transport-Security: max-age="));
        // A local run configures no canonical host, so never.
        assert!(
            !security_headers("text/html", ours, None).contains("Strict-Transport-Security")
        );
    }

    #[test]
    fn the_auth_config_publishes_the_origin_cognito_registered() {
        // ⛔ THE CONSOLE READS ITS OWN `redirect_uri` FROM THIS FIELD, so this
        // test is the thing standing between a rename and a sign-in that every
        // account is refused from. `deploy/app.yaml` builds both the Cognito
        // callback and `RATIO_CONSOLE_URL` out of one `ConsoleOrigin`
        // parameter; the point of publishing it is that the console cannot then
        // hold a copy that has drifted, which is precisely what happened when it
        // did — `ratio-console.vercel.app` in Vercel against `ratio-ims` here.
        let cfg = auth_config_json(
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC",
            "clientid",
            "ratio-demo-1.auth.us-east-1.amazoncognito.com",
            "https://ratio-ims.vercel.app",
        );
        assert!(
            cfg.contains(r#""consoleOrigin":"https://ratio-ims.vercel.app""#),
            "the console origin must be published verbatim: {cfg}"
        );
        // The path is separate from the origin and the console joins the two.
        assert!(cfg.contains(r#""redirectPath":"/callback""#));

        // ⚠ EMPTY IS A REAL ANSWER, NOT A MISSING FIELD. A local `ratio watch`
        // sets no `RATIO_CONSOLE_URL`, and the console reads empty as "nothing
        // published here" and falls back to its own environment — which is what
        // makes `next dev` against a local book work with no Cognito at all. A
        // field that vanished instead would be indistinguishable from an older
        // server, and the console would have to guess which.
        let local = auth_config_json("", "", "", "");
        assert!(local.contains(r#""consoleOrigin":"""#), "{local}");

        // ⛔ A CONTROL CHARACTER CANNOT BREAK OUT OF THE JSON STRING. The value
        // reaches here through `console_url()`, which strips these already, so
        // this is the second of two guards — but `quote` is what makes this
        // function safe to call with anything, and a caller added later will
        // not know about the first guard.
        let hostile = auth_config_json("", "", "", "https://x/\"\r\n");
        assert!(hostile.contains(r#""consoleOrigin":"https://x/\"\r\n""#), "{hostile}");
    }

    #[test]
    fn a_console_url_with_a_newline_in_it_is_treated_as_unset() {
        // ⛔ THIS VALUE IS INTERPOLATED INTO A `Location:` HEADER. A CR or LF in
        // it ends that header and begins another, which is response splitting.
        // It is operator configuration rather than caller input, so this guards
        // a typo in a CloudFormation template — but the failure it prevents is
        // the kind nobody notices until somebody is looking for it.
        assert_eq!(safe_redirect("https://console.example/"), "https://console.example/");
        assert_eq!(safe_redirect("https://x/\r\nSet-Cookie: a=b"), "");
        assert_eq!(safe_redirect("https://x/\nLocation: elsewhere"), "");
        // Unset stays unset, which is what makes `/` say what it does serve.
        assert_eq!(safe_redirect(""), "");
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

    // ⛔ A PAGE MISSING FROM THIS LIST IS A PAGE NOTHING BELOW CHECKS, and the
    // omission looks exactly like a suite that passes. Every screen the route
    // table serves belongs here; `every_screen_the_router_serves_is_listed_here`
    // is what stops the next one being forgotten.
    const SCREENS: [(&str, &str); 6] = [
        ("chat", CHAT_BODY),
        ("balance", BALANCE_BODY),
        ("breaks", BREAKS_BODY),
        ("rules", RULES_BODY),
        ("scale", SCALE_BODY),
        ("report", REPORT_BODY),
    ];

    #[test]
    fn every_page_is_self_contained() {
        // The demo runs on a laptop that may have no network. An external
        // subresource would render an unstyled page in front of a customer.
        for (name, body) in SCREENS {
            let html = page(body, name);
            // The SVG xmlns is a namespace identifier, not a fetch — in the
            // favicon's URL-encoded form and in `createElementNS`'s plain one.
            // ⚠ Stripping exactly these two strings is the point: anything else
            // that looks like a URL is still an external reference.
            let stripped = html
                .replace("http%3A//www.w3.org/2000/svg", "")
                .replace("http://www.w3.org/2000/svg", "");
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
            // ⚠ The lead pages are served WITHOUT the nav — `lead_page` — so
            // "which tab is current" is a question they never answer.
            if name == "scale" || name == "report" {
                continue;
            }
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
    fn the_nav_has_four_tabs_and_the_lead_pages_stand_alone() {
        // PLAN.md said "Three. Not four." The fourth is the MCP conversation.
        // The fifth screen — scale — was in this nav for one release and LEFT
        // it deliberately: it became the lead-gen front door, and a prospect
        // landing there should meet one claim and one button, not an operator's
        // tab bar. It and the run reports are served by `lead_page`, WITHOUT
        // the nav, and are still held to every page fence because they stay in
        // SCREENS. This assertion is the moment to notice if the nav grows —
        // or if the lead page quietly rejoins it.
        let html = page(CHAT_BODY, "chat");
        let tabs = html.matches("<a href=").count();
        assert_eq!(tabs, 4, "the nav offers {tabs} screens");
        // And the standalone wrapper really is nav-free: a lead page with a tab
        // bar is an operator screen again.
        assert!(!lead_page(SCALE_BODY).contains("<nav"), "the lead page grew the nav");
        assert!(!lead_page(REPORT_BODY).contains("<nav"), "the report page grew the nav");
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

    // ⛔ THE CONSOLE'S RENDER GUARD LIVES IN `//console:` NOW, AND IT HAD TO
    // MOVE RATHER THAN GO.
    //
    // `the_served_console_carries_the_lot_engine` used to stand here and assert
    // eleven literals against the console string compiled into this binary —
    // "Lot method", "a term of the administration agreement", "this
    // configuration declares no method", "Realized gain", "Short-term",
    // "Long-term", "Unclassified", "Basis relieved", "no trade date", "Tax
    // lots", "the fold that". It existed because the corporate-actions screen
    // was once "written, compiled, and absent", and grepping the served HTML by
    // hand was what caught it.
    //
    // There is no console in this binary to grep. Every one of those eleven
    // needles is now asserted by `//console:fields_test`, and the screens that
    // carry them are RENDERED against fixtures by
    // `console/src/app/screens.test.tsx` — which is what the old test's own
    // header said it wished it could do.
    //
    // ⚠ Do not restore this without restoring the console it read.


    #[test]
    fn every_screen_the_router_serves_is_listed_here() {
        // ⛔ THE TEST THAT NEARLY WAS NOT WRITTEN, AND THE OMISSION IT IS FOR
        // ACTUALLY HAPPENED. `SCALE_BODY` was added, routed, and served while
        // `SCREENS` still listed four pages — so every hygiene test below ran
        // over the old four and passed, and the new page went out unchecked by
        // any of them. A suite that silently narrows its own scope is the exact
        // failure mode this repository keeps finding.
        //
        // The router is the source of truth: a page is served iff `route` maps a
        // path to `page(BODY, slug)`. This reads the router's own source for
        // those calls and insists each one is listed above.
        let router = include_str!("watch.rs");
        let mut served: Vec<&str> = router
            .lines()
            // The route arms, not this test's own mention of them.
            // ⛔ ROUTE ARMS ONLY, for both wrappers. A bare `lead_page(` match
            // also catches this test's own assertions and the wrapper's
            // definition — include_str! reads the whole file, tests included —
            // and the first version of this branch did exactly that.
            .filter(|l| {
                l.contains("text/html; charset=utf-8\", page(")
                    || l.contains("text/html; charset=utf-8\", lead_page(")
            })
            .filter_map(|l| l.split("page(").nth(1))
            .filter_map(|rest| rest.split(',').next())
            // ⚠ A `lead_page(BODY))` arm carries no `, "slug"` tail, so the
            // token keeps the closing parens; a `page(BODY, "slug")` one does
            // not. Trim both to the bare const name.
            .map(|body| body.trim().trim_end_matches(')'))
            .collect();
        served.sort_unstable();
        served.dedup();

        assert!(!served.is_empty(), "found no page routes at all — did the router change shape?");
        for body in &served {
            assert!(
                SCREENS.iter().any(|(_, b)| std::ptr::eq(
                    *b as *const str,
                    match *body {
                        "CHAT_BODY" => CHAT_BODY,
                        "BALANCE_BODY" => BALANCE_BODY,
                        "BREAKS_BODY" => BREAKS_BODY,
                        "RULES_BODY" => RULES_BODY,
                        "SCALE_BODY" => SCALE_BODY,
                        "REPORT_BODY" => REPORT_BODY,
                        other => panic!(
                            "the router serves {other}, which this test does not know how to \
                             resolve — add it to both SCREENS and this match"
                        ),
                    } as *const str
                )),
                "the router serves {body} but SCREENS does not list it, so every page \
                 check below skips it"
            );
        }
        assert_eq!(
            served.len(),
            SCREENS.len(),
            "the router serves {} pages and SCREENS lists {} — they must be the same set",
            served.len(),
            SCREENS.len()
        );
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
