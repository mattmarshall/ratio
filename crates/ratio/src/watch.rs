//! A live trial balance in a browser — the demo's visual payoff.
//!
//! PLAN.md Stage 2 asks for "a live trial-balance page that updates as events
//! post." Step 6 of the five-minute script is *post ten thousand transactions
//! and watch the difference stay `0.00`*, and that only lands if somebody who
//! does not read a terminal can see it happen.
//!
//! # Why this is hand-rolled
//!
//! It serves one page and one JSON endpoint to one browser on `localhost` for
//! the length of a demo. A web framework would be more code than this, not
//! less, and would put an async runtime and a dependency tree behind a feature
//! whose entire job is to render a table every 400ms. `TcpListener` and a
//! thread per connection are the right size for the problem.
//!
//! **It is a demo surface, not a product surface.** It binds loopback only,
//! serves two fixed paths, reads and never writes, and there is no route by
//! which a request reaches the book. Anything facing a network is the gRPC
//! server's job.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;

use anyhow::{Context, Result};
use ratio_chart::{normal_side, Side};
use ratio_store::{ConfigStore, FileBook, Journal};

/// Serve the live trial balance until interrupted.
pub fn watch(book: PathBuf, port: u16) -> Result<()> {
    // Fail on a bad book here rather than rendering an error page later.
    FileBook::open(&book).with_context(|| format!("opening book at {}", book.display()))?;

    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = TcpListener::bind(addr)
        .with_context(|| format!("binding {addr} — is another `ratio watch` running?"))?;
    // Port 0 means "any free port", so report what was actually bound.
    let addr = listener.local_addr()?;

    println!("live trial balance  http://{addr}/");
    println!("book                {}", book.display());
    println!("(ctrl-c to stop)");

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            // One dropped connection is not a reason to end the demo.
            Err(_) => continue,
        };
        let book = book.clone();
        std::thread::spawn(move || {
            let _ = handle(stream, &book);
        });
    }
    Ok(())
}

fn handle(mut stream: TcpStream, book: &PathBuf) -> Result<()> {
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/");

    let (status, content_type, body) = match path {
        "/" => ("200 OK", "text/html; charset=utf-8", PAGE.to_string()),
        "/balance.json" => match snapshot(book) {
            Ok(json) => ("200 OK", "application/json", json),
            // A book mid-write is a normal thing to catch; say so in a shape
            // the page can render rather than dropping the connection.
            Err(e) => (
                "200 OK",
                "application/json",
                format!("{{\"error\":{}}}", quote(&e.to_string())),
            ),
        },
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

/// The trial balance as JSON.
///
/// Hand-written rather than derived: the shape here is a view, and letting it
/// track the storage types would mean a change to the book silently changing
/// the page. Amounts go out as **strings in major units** — a figure that has
/// been exact all the way through the kernel should not meet a float in the
/// last six inches of its journey.
fn snapshot(book: &PathBuf) -> Result<String> {
    let b = FileBook::open(book)?;
    let entries = b.entries()?;
    let names: std::collections::BTreeMap<i64, ratio_store::Account> =
        b.accounts()?.into_iter().map(|a| (a.dim, a)).collect();
    let by_dim = b.balances_by_dim()?;
    let tb = b.trial_balance()?;

    let mut rows = Vec::new();
    for (dim, (debit, credit)) in &by_dim {
        let (label, abnormal) = match names.get(dim) {
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
            "{{\"label\":{},\"debit\":{},\"credit\":{},\"abnormal\":{}}}",
            quote(&label),
            quote(&crate::minor(*debit)),
            quote(&crate::minor(*credit)),
            abnormal
        ));
    }

    let config = match b.active()? {
        Some(c) => quote(c.as_str()),
        None => "null".to_string(),
    };

    Ok(format!(
        "{{\"entries\":{},\"config\":{},\"debits\":{},\"credits\":{},\
         \"difference\":{},\"ties\":{},\"rows\":[{}]}}",
        entries.len(),
        config,
        quote(&crate::minor(tb.debits)),
        quote(&crate::minor(tb.credits)),
        quote(&crate::minor(tb.debits - tb.credits)),
        tb.debits == tb.credits,
        rows.join(",")
    ))
}

/// Minimal JSON string escaping.
///
/// Account names come from the book, which a customer edits, so this has to be
/// correct rather than nearly correct — an unescaped quote in an account name
/// would break the page and look like the ledger was wrong.
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

/// The page. Self-contained, no subresources, same palette as the website.
const PAGE: &str = r##"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Trial balance — Ratio</title>
<!-- Inline, so the page stays two routes and the browser never 404s on
     a favicon in front of a customer. -->
<link rel="icon" href="data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2016%2016%22%3E%3Crect%20width%3D%2216%22%20height%3D%2216%22%20rx%3D%223%22%20fill%3D%22%231B6440%22%2F%3E%3Crect%20x%3D%223%22%20y%3D%223.5%22%20width%3D%2210%22%20height%3D%221.6%22%20fill%3D%22%23FCFBF7%22%2F%3E%3Crect%20x%3D%224%22%20y%3D%226.6%22%20width%3D%222.2%22%20height%3D%226%22%20fill%3D%22%23FCFBF7%22%2F%3E%3Crect%20x%3D%229.8%22%20y%3D%226.6%22%20width%3D%222.2%22%20height%3D%226%22%20fill%3D%22%23FCFBF7%22%2F%3E%3C%2Fsvg%3E">
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
body{margin:0;padding:32px 20px;background:var(--ground);color:var(--text);
  font:16px/1.5 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}
.wrap{max-width:760px;margin:0 auto}
h1{font-size:15px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);
  font-weight:600;margin:0 0 4px}
.meta{font-size:13px;color:var(--muted);margin:0 0 24px}
.meta code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px}
.card{background:var(--raised);border:1px solid var(--rule);border-radius:10px;
  overflow-x:auto}
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
.dot{width:9px;height:9px;border-radius:50%;background:var(--accent)}
.tie.broken{color:var(--warn)} .tie.broken .dot{background:var(--warn)}
.note{font-size:12.5px;color:var(--muted);margin-top:20px}
/* A changed figure flashes once, so movement is visible without being a
   distraction. Respects reduced-motion by simply not animating. */
@keyframes flash{from{background:color-mix(in oklab,var(--accent) 26%,transparent)}to{background:transparent}}
.changed{animation:flash .7s ease-out}
@media (prefers-reduced-motion:reduce){.changed{animation:none}}
.offline{color:var(--warn)}
</style></head><body>
<div class="wrap">
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
  <p class="note">* sits on the side its account type does not call normal.
    Figures are exact to the minor unit; no float touches this page.</p>
</div>
<script>
const prev = new Map();

// Set text, flashing only when the value actually moved — flashing everything
// on every poll would make a still book look busy.
function put(id, value) {
  const el = document.getElementById(id);
  if (el.textContent === value) return;
  el.textContent = value;
  el.classList.remove("changed");
  void el.offsetWidth;            // restart the animation
  el.classList.add("changed");
}

async function tick() {
  let d;
  try {
    d = await (await fetch("balance.json", {cache: "no-store"})).json();
  } catch {
    document.getElementById("entries").innerHTML =
      '<span class="offline">server stopped</span>';
    return;
  }
  if (d.error) { document.getElementById("entries").textContent = d.error; return; }

  put("entries", d.entries + (d.entries === 1 ? " entry" : " entries"));
  document.getElementById("config").textContent = d.config ? d.config.slice(0, 12) : "none";

  const body = document.getElementById("rows");
  // Rebuild only when the account set changes; otherwise patch in place so the
  // flash lands on the figure that moved rather than on a fresh element.
  const key = d.rows.map(r => r.label).join(" ");
  if (body.dataset.key !== key) {
    body.dataset.key = key;
    body.innerHTML = "";
    for (const r of d.rows) {
      const tr = document.createElement("tr");
      const name = document.createElement("td");
      name.textContent = r.label;
      if (r.abnormal) name.className = "abnormal";
      const dr = document.createElement("td"), cr = document.createElement("td");
      dr.id = "d:" + r.label; cr.id = "c:" + r.label;
      dr.textContent = r.debit; cr.textContent = r.credit;
      tr.append(name, dr, cr);
      body.append(tr);
      prev.set(r.label, [r.debit, r.credit]);
    }
  } else {
    for (const r of d.rows) { put("d:" + r.label, r.debit); put("c:" + r.label, r.credit); }
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
</body></html>
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

    #[test]
    fn an_empty_book_reports_zero_and_ties() {
        let dir = std::env::temp_dir().join("ratio-watch-empty");
        let _ = std::fs::remove_dir_all(&dir);
        crate::init(dir.clone()).unwrap();

        let json = snapshot(&dir).unwrap();
        assert!(json.contains("\"entries\":0"), "{json}");
        assert!(json.contains("\"ties\":true"), "{json}");
        assert!(json.contains("\"difference\":\"0.00\""), "{json}");
    }

    #[test]
    fn the_snapshot_carries_figures_as_strings_never_numbers() {
        // A JSON number here would be parsed as a double by every consumer,
        // which is exactly the failure the integer kernel exists to prevent.
        let dir = std::env::temp_dir().join("ratio-watch-strings");
        let _ = std::fs::remove_dir_all(&dir);
        crate::init(dir.clone()).unwrap();

        let json = snapshot(&dir).unwrap();
        for field in ["debits", "credits", "difference"] {
            assert!(
                json.contains(&format!("\"{field}\":\"")),
                "{field} must be a string: {json}"
            );
        }
    }

    #[test]
    fn the_page_is_self_contained() {
        // The demo runs on a laptop that may have no network. An external
        // subresource would render an unstyled page in front of a customer.
        // The SVG xmlns is a namespace identifier, not a fetch — exclude it,
        // then insist nothing else names a remote host.
        let body = PAGE.replace("http://www.w3.org/2000/svg", "");
        assert!(!body.contains("http://"), "external reference in the page");
        assert!(!body.contains("https://"), "external reference in the page");
        assert!(PAGE.contains("rel=\"icon\""), "no inline favicon — the browser will 404");
        assert!(PAGE.contains("<!doctype html>"));
        assert!(PAGE.contains("viewport"));
    }

    #[test]
    fn the_page_honors_both_themes_and_reduced_motion() {
        assert!(PAGE.contains("prefers-color-scheme:dark"));
        assert!(PAGE.contains("prefers-reduced-motion:reduce"));
    }
}
