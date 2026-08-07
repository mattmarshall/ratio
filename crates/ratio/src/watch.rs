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

use anyhow::{Context, Result};
use prost::Message;
use ratio_chart::{normal_side, Side};
use ratio_proto::ratio::v1 as pb;
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

fn handle(mut stream: TcpStream, book: &Path) -> Result<()> {
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target.as_str(), ""),
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

    let (status, content_type, body) = match path {
        "/" => ("200 OK", "text/html; charset=utf-8", page(BALANCE_BODY, "balance")),
        "/breaks" => ("200 OK", "text/html; charset=utf-8", page(BREAKS_BODY, "breaks")),
        "/rules" => ("200 OK", "text/html; charset=utf-8", page(RULES_BODY, "rules")),
        "/balance.json" => json(balance_json(book)),
        "/postings.json" => json(postings_json(book, query)),
        "/breaks.json" => json(breaks_json(book)),
        "/rules.json" => json(rules_json(book)),
        _ => ("404 Not Found", "text/plain; charset=utf-8", "no".to_string()),
    };

    write!(
        stream,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// The trial balance.
///
/// Amounts go out as **strings in major units**. A JSON number would be parsed
/// as a double by every consumer, which is the exact failure the integer
/// kernel exists to prevent — a figure that has been exact all the way through
/// should not meet a float in the last six inches of its journey.
fn balance_json(book: &Path) -> Result<String> {
    let b = FileBook::open(book)?;
    let entries = b.entries()?;
    let names: std::collections::BTreeMap<i64, ratio_store::Account> =
        b.accounts()?.into_iter().map(|a| (a.dim, a)).collect();
    let tb = b.trial_balance()?;

    let mut rows = Vec::new();
    for (dim, (debit, credit)) in b.balances_by_dim()? {
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
        rows.push(format!(
            "{{\"account\":{dim},\"label\":{},\"debit\":{},\"credit\":{},\"abnormal\":{}}}",
            quote(&label),
            quote(&crate::minor(debit)),
            quote(&crate::minor(credit)),
            abnormal
        ));
    }

    Ok(format!(
        "{{\"entries\":{},\"config\":{},\"debits\":{},\"credits\":{},\
         \"difference\":{},\"ties\":{},\"rows\":[{}]}}",
        entries.len(),
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
    for e in b.entries()? {
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
    }
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

    let report = pb::BreakReport::decode(&std::fs::read(path)?[..])
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
                quote(match pb::Cause::try_from(b.cause) {
                    Ok(pb::Cause::AmountDiffers) => "figures differ",
                    Ok(pb::Cause::AbsentFromReport) => "not in the report",
                    Ok(pb::Cause::AbsentFromRatio) => "Ratio produced nothing",
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
                quote(match pb::Refusal::try_from(e.refusal) {
                    Ok(pb::Refusal::UnknownTransactionType) => "transaction type not covered",
                    Ok(pb::Refusal::ForeignCurrency) => "foreign currency",
                    Ok(pb::Refusal::DisposalWithoutBasis) => "disposal with no basis",
                    Ok(pb::Refusal::NoRuleForType) => "no rule for this type",
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
            pending.push(format!(
                "{{\"id\":{},\"rendered\":{}}}",
                quote(&id),
                quote(&rendered)
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

/// Wrap a screen's body in the shared document, marking the current tab.
fn page(body: &str, current: &str) -> String {
    let tab = |slug: &str, href: &str, label: &str| {
        format!(
            r#"<a href="{href}"{}>{label}</a>"#,
            if slug == current { r#" aria-current="page""# } else { "" }
        )
    };
    format!(
        "{HEAD}<nav class=\"tabs\">{}{}{}</nav>{body}{FOOT}",
        tab("balance", "/", "Trial balance"),
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
<link rel="icon" href="data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22%20viewBox%3D%220%200%2016%2016%22%3E%3Crect%20width%3D%2216%22%20height%3D%2216%22%20rx%3D%223%22%20fill%3D%22%231B6440%22/%3E%3Crect%20x%3D%223%22%20y%3D%223.5%22%20width%3D%2210%22%20height%3D%221.6%22%20fill%3D%22%23FCFBF7%22/%3E%3Crect%20x%3D%224%22%20y%3D%226.6%22%20width%3D%222.2%22%20height%3D%226%22%20fill%3D%22%23FCFBF7%22/%3E%3Crect%20x%3D%229.8%22%20y%3D%226.6%22%20width%3D%222.2%22%20height%3D%226%22%20fill%3D%22%23FCFBF7%22/%3E%3C/svg%3E">
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
</style></head><body>
"##;

const FOOT: &str = "</body></html>\n";

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
  // lists is exactly what a person's approval bought.
  content.append(el("h2", null, "Waiting on a person"));
  if (!d.pending.length) {
    content.append(card(el("p", "empty", "Nothing is waiting for approval.")));
  }
  for (const p of d.pending) {
    const h = el("div", "rule-head");
    h.append(el("b", null, p.id), el("span", "chip warn", "not active"));
    const c = card(h, el("pre", null, p.rendered));
    const note = el("p", "pending", "A model drafted this. It becomes policy only when someone runs ");
    note.append(el("code", null, "ratio approve " + p.id));
    note.append(document.createTextNode(
      " at a terminal — there is no button here, and no tool the model can call, that does it."));
    c.append(note);
    content.append(c);
  }
})();
</script>
"##;

#[cfg(test)]
mod tests {
    use super::*;

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
        let report = pb::BreakReport {
            name: "books/b/breakReports/r".into(),
            config_digest: "abc123".into(),
            scope: Some(pb::Scope {
                label: "equity-long-only-single-ccy".into(),
                base_currency: "USD".into(),
                transaction_types: vec!["buy".into()],
                exclusions: vec!["corporate actions".into()],
            }),
            transactions_replayed: 3,
            entries_posted: 4,
            breaks: vec![pb::Break {
                account: 1,
                display_name: "Investments at fair value".into(),
                ratio_amount: 2_500_000,
                reported_amount: 2_400_000,
                difference: 100_000,
                cause: pb::Cause::AmountDiffers as i32,
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
    fn a_proposal_that_does_not_parse_is_shown_rather_than_hidden() {
        let book = fresh("badproposal");
        std::fs::create_dir_all(book.join("proposals")).unwrap();
        std::fs::write(book.join("proposals/broken.toml"), "this is not toml {{{").unwrap();
        let j = rules_json(&book).unwrap();
        assert!(j.contains("broken"), "{j}");
        assert!(j.contains("does not parse"), "{j}");
    }

    // -- the pages ---------------------------------------------------------

    const SCREENS: [(&str, &str); 3] = [
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
    fn there_are_three_screens_and_no_fourth() {
        // PLAN.md: "Three. Not four." No portal, no dashboard, no settings,
        // and no rule editor — the MCP conversation is the authoring surface.
        let html = page(BALANCE_BODY, "balance");
        let tabs = html.matches("<a href=").count();
        assert_eq!(tabs, 3, "the nav offers {tabs} screens");
    }

    #[test]
    fn no_screen_offers_a_way_to_approve_anything() {
        // The fence has to hold on the screen too. A button here would be a
        // second path to the thing the MCP server refuses.
        for (name, body) in SCREENS {
            assert!(!body.contains("<button"), "{name} grew a button");
            assert!(!body.contains("<form"), "{name} grew a form");
            assert!(!body.to_lowercase().contains("method=\"post\""), "{name}");
        }
        // And the rules screen says so in as many words.
        assert!(RULES_BODY.contains("no tool the model can call"));
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
