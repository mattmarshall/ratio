//! ratio-mcp — the Model Context Protocol server. PLAN.md Stage 2.
//!
//! # The fence
//!
//! PLAN.md says *"the model can call `propose_rule` and nothing else that
//! writes… this must be true in the code, not just in the narration"*, and it
//! also lists `approve_rule` among the tools. **Those two cannot both hold**: a
//! tool the server exposes is a tool the model can call. So the fence wins, and
//! the tool list is the thing that changed.
//!
//! | | model, over MCP | human, at a terminal |
//! |---|---|---|
//! | read the chart, the balance, a figure's provenance | ✓ | ✓ |
//! | draft a rule and have it checked | ✓ | ✓ |
//! | **approve a rule into the active configuration** | ✗ | `ratio approve` |
//! | post events under an already-approved configuration | ✓ | ✓ |
//!
//! `approve_rule` is **not a tool**. It is not exposed, not dispatched, and not
//! reachable — a proposal becomes policy only when a person runs `ratio approve`
//! against a digest they can read. That is the demo's whole point, and it is
//! enforced by absence rather than by a permission check somebody could relax.
//!
//! Posting *is* allowed, and safely: an event can only be applied through the
//! configuration a human already approved, and every posting still goes through
//! the kernel's conservation check on the way into the book. The worst a bad
//! model proposal can do is fail a check and be discarded.
//!
//! # The protocol
//!
//! MCP over stdio: line-delimited JSON-RPC 2.0. Implemented directly because
//! the subset that matters is four methods, and a dependency for that would be
//! more code to audit than the code it replaces.

use std::io::{BufRead, Write};

use anyhow::{Context, Result};
use ratio_rules::{check, compile, render, Event, RuleSet};
use ratio_store::{ConfigStore, FileBook, Journal, JournalEntry};
use serde_json::{json, Value};

/// The protocol version answered when a client does not name one.
const DEFAULT_PROTOCOL: &str = "2024-11-05";

/// Run the server against a book, reading requests from `input` and writing
/// responses to `output`. One JSON object per line, in both directions.
pub fn serve(book: &std::path::Path, input: impl BufRead, mut output: impl Write) -> Result<()> {
    for line in input.lines() {
        let line = line.context("reading request")?;
        if line.trim().is_empty() {
            continue;
        }
        let Some(response) = handle_line(book, &line) else {
            continue; // a notification: no reply, by protocol
        };
        writeln!(output, "{response}").context("writing response")?;
        output.flush().context("flushing response")?;
    }
    Ok(())
}

/// Handle one line. `None` for a notification, which must not be answered.
pub fn handle_line(book: &std::path::Path, line: &str) -> Option<String> {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        // No id is recoverable from unparseable input, so this is the one case
        // where the spec wants a null id.
        Err(e) => return Some(error_response(Value::Null, -32700, &format!("parse error: {e}"))),
    };

    let id = req.get("id").cloned();
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");

    // A request without an id is a notification. `notifications/initialized`
    // is the one that matters; answering it is a protocol violation.
    let Some(id) = id else {
        return None;
    };

    let result = match method {
        "initialize" => Ok(initialize(&req)),
        "tools/list" => Ok(tools_list()),
        "tools/call" => call_tool(book, &req),
        "ping" => Ok(json!({})),
        other => {
            return Some(error_response(
                id,
                -32601,
                &format!("method not found: {other}"),
            ))
        }
    };

    Some(match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}).to_string(),
        // A tool that fails is reported as a *result* with `isError`, not a
        // JSON-RPC error: the model is meant to read it and try again, which is
        // exactly what happens when a check comes back with questions.
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{"type": "text", "text": format!("{e:#}")}],
                "isError": true
            }
        })
        .to_string(),
    })
}

fn error_response(id: Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

fn initialize(req: &Value) -> Value {
    // Echo the client's protocol version when it names one; disagreeing about
    // the version is the fastest way to a connection that half-works.
    let version = req
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "ratio", "version": env!("CARGO_PKG_VERSION")}
    })
}

/// The tool surface. Note what is absent: nothing here promotes a rule.
fn tools_list() -> Value {
    json!({"tools": [
        {
            "name": "list_accounts",
            "description":
                "The book's chart of accounts: each account's dimension, name, type and \
                 the side its type calls normal. Read this before drafting a rule — a \
                 rule may only post to an account that exists.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "propose_rule",
            "description":
                "Draft a posting rule and have it checked. Takes TOML for a rule set. \
                 Returns the findings and, if it is usable, a proposal id. THIS DOES NOT \
                 MAKE THE RULE ACTIVE — a person must approve it at a terminal. Weights \
                 are multipliers, not amounts, and must net to zero; positive debits and \
                 negative credits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "toml": {"type": "string", "description": "The rule set, as TOML. Every rule needs `id` and `kind` \
                         (trade | dividend | accrual); an accrual also needs `rate_bp` \
                         and `day_count` (act/365 | act/360 | 30/360). Each posting is \
                         an `[[rule.posting]]` with `account` and `weight`.\n\n\
                         [[rule]]\n\
                         id = \"management_fee_accrual\"\n\
                         kind = \"accrual\"\n\
                         description = \"75bp a year on net assets, ACT/365\"\n\
                         rate_bp = 75\n\
                         day_count = \"act/365\"\n\
                         [[rule.posting]]\n\
                         account = 10\n\
                         weight = 1\n\
                         [[rule.posting]]\n\
                         account = 40\n\
                         weight = -1"}
                },
                "required": ["toml"]
            }
        },
        {
            "name": "propose_template",
            "description":
                "Draft a MAPPING TEMPLATE for a counterparty's file and see what it would \
                 produce. Takes the template as TOML and a SAMPLE of the file (its header \
                 and a few rows). Runs the template against the sample and reports the \
                 facts it would read, what each would resolve to in the master, and what \
                 would pend — then saves it as a proposal. THIS DOES NOT MAKE THE TEMPLATE \
                 ACTIVE: a person approves it at a terminal, and what they approve is what \
                 you showed them it does. Use `list_accounts` to see the chart and \
                 `list_entities` to see what the master can already identify.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "toml": {"type": "string", "description":
                        "The template, as TOML. Shape:\n\n\
                         [[template]]\n\
                         id = \"<counterparty>_<what>\"\n\
                         reads = \"csv\"\n\
                         [[template.entity]]\n\
                         name = \"security\"          # your name for it in this file\n\
                         kind = \"instrument\"        # instrument | counterparty\n\
                         absent = \"pend\"            # pend | reject\n\
                         by = [                     # the identity ladder, best first\n\
                           { attribute = \"isin\", column = \"ISIN\" },\n\
                           { attribute = \"ticker\", column = \"Symbol\", within = { attribute = \"exchange\", column = \"Exch\" } },\n\
                         ]\n\
                         [template.fact]\n\
                         kind = \"trade\"\n\
                         reference = \"TradeRef\"     # the column holding the row's own id\n\
                         entities = { security = \"security\" }\n\
                         [[template.fact.value]]\n\
                         field = \"price\"\n\
                         as = \"money\"               # money | decimal | text | date | enum\n\
                         column = \"Price\"\n\
                         currency = \"Ccy\"           # money only: the column holding it\n\n\
                         `as = \"date\"` also needs `format` (YYYY-MM-DD | MM/DD/YYYY | \
                         DD/MM/YYYY). `as = \"enum\"` also needs `map`, e.g. \
                         map = { B = \"buy\", S = \"sell\" }.\n\n\
                         Add [template.fact.posts] ONLY if this file should reach the \
                         journal: by = \"<field>\", amount = \"consideration\" (quantity x \
                         price) or a money field name, rules = { <value> = \"<rule id>\" }. \
                         Reference data — prices, FX, a security master — has no posts \
                         block and is recorded without posting."},
                    "sample": {"type": "string", "description":
                        "The file's header row and a few data rows, verbatim."}
                },
                "required": ["toml", "sample"]
            }
        },
        {
            "name": "list_entities",
            "description":
                "The master: the instruments and counterparties this book can already \
                 identify, and which attributes identify each. Read this before drafting \
                 an identity ladder — a ladder that matches on an attribute the master \
                 does not carry will pend every row.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "check_rule",
            "description":
                "Check a rule set against the chart without proposing it. Returns every \
                 finding: errors are things that cannot be used as written, questions are \
                 for a human to answer. Same TOML shape as propose_rule.",
            "inputSchema": {
                "type": "object",
                "properties": {"toml": {"type": "string"}},
                "required": ["toml"]
            }
        },
        {
            "name": "post_events",
            "description":
                "Apply the ACTIVE configuration to events and post the result. Only rules \
                 a person has already approved can run. Every posting is checked for \
                 conservation on the way in and refused if it does not balance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "events": {
                        "type": "array",
                        "description":
                            "Each: rule (id), id, amount (minor units, integer), \
                             optionally days (accruals) and memo.",
                        "items": {"type": "object"}
                    }
                },
                "required": ["events"]
            }
        },
        {
            "name": "trial_balance",
            "description":
                "The book's trial balance: totals per account, the overall totals, and \
                 the difference. The difference is zero for any book this system built.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "explain_figure",
            "description":
                "Every posting that makes up one account's balance, with the entry it came \
                 from and the configuration digest in force when it was posted. Use this to \
                 explain a number rather than to guess at one.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "account": {"type": "integer", "description": "The account's dimension."}
                },
                "required": ["account"]
            }
        }
    ]})
}

fn call_tool(book: &std::path::Path, req: &Value) -> Result<Value> {
    let name = req
        .pointer("/params/name")
        .and_then(Value::as_str)
        .context("tools/call needs params.name")?;
    let args = req
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let text = dispatch(book, name, &args)?;
    Ok(json!({"content": [{"type": "text", "text": text}], "isError": false}))
}

/// The tool surface, for a caller that is not speaking JSON-RPC.
///
/// The in-process chat demo drives a model against these same tools. It goes
/// through here rather than reimplementing the list, because a second copy of
/// the dispatch table is a second place for the fence to be missing — and the
/// fence being *absent* rather than *denied* is the only reason it holds.
pub fn tools() -> Value {
    tools_list()
}

/// Run one tool by name. `Err` is a message meant for the model to read.
pub fn dispatch(book: &std::path::Path, name: &str, args: &Value) -> Result<String> {
    let args = args.clone();
    let text = match name {
        "list_accounts" => tool_list_accounts(book)?,
        "propose_rule" => tool_propose_rule(book, &args)?,
        "propose_template" => tool_propose_template(book, &args)?,
        "list_entities" => tool_list_entities(book)?,
        "check_rule" => tool_check_rule(book, &args)?,
        "post_events" => tool_post_events(book, &args)?,
        "trial_balance" => tool_trial_balance(book)?,
        "explain_figure" => tool_explain_figure(book, &args)?,
        // The fence, stated where a model will actually read it.
        "approve_rule" | "promote_config" | "set_active" => anyhow::bail!(
            "There is no such tool. Approving a rule is not something this server can do — \
             a person promotes a proposal by running `ratio approve <proposal-id>` at a \
             terminal, after reading it. Propose the rule and say that it is ready for review."
        ),
        other => anyhow::bail!("no tool named {other:?}"),
    };
    Ok(text)
}

/// The master, so a model can see what a ladder can match on.
fn tool_list_entities(book: &std::path::Path) -> Result<String> {
    use ratio_store::Plane;
    let b = FileBook::open(book)?;
    let master = ratio_ingest::current(&b.records::<ratio_ingest::Entity>(Plane::Entities)?);
    if master.is_empty() {
        return Ok("The master is empty. Every identity ladder will pend until \
                   somebody adds instruments and counterparties."
            .into());
    }
    let mut out = String::from("id                kind          identified by\n");
    for e in &master {
        let by: Vec<String> =
            e.attributes.iter().map(|(k, v)| format!("{k}={v}")).collect();
        out.push_str(&format!(
            "{:<18}{:<14}{}\n",
            e.id,
            e.kind.as_str(),
            by.join("  ")
        ));
    }
    out.push_str(
        "\nA ladder matches on these attribute names. One that names something \
         else will pend every row.",
    );
    Ok(out)
}

/// Draft a template and DRY-RUN it against a sample.
///
/// ⛔ The dry run is the point. A person approving a mapping specification has
/// to simulate it in their head; a person approving one they have SEEN RUN is
/// reading an outcome. Same fence, better-informed approval.
fn tool_propose_template(book: &std::path::Path, args: &Value) -> Result<String> {
    use ratio_store::Plane;
    let toml = args
        .get("toml")
        .and_then(Value::as_str)
        .context("propose_template needs `toml`")?;
    let sample = args
        .get("sample")
        .and_then(Value::as_str)
        .context("propose_template needs `sample` — the file's header and a few rows")?;

    let set = ratio_ingest::TemplateSet::from_toml(toml)?;
    let template = set
        .templates
        .first()
        .context("no [[template]] in that TOML")?;

    let mut out = String::new();
    let problems = template.check();
    if !problems.is_empty() {
        out.push_str("This template does not check:\n");
        for p in &problems {
            out.push_str(&format!("  x {p}\n"));
        }
        out.push_str("\nNothing was saved. Fix these and propose it again.\n");
        return Ok(out);
    }

    let rows = match template.reads {
        ratio_ingest::Reader::Csv => ratio_ingest::extract_csv(sample)?,
    };
    let delivery = ratio_ingest::Delivery {
        digest: "0".repeat(64),
        origin: "sample".into(),
        received: 0,
        bytes: sample.len() as i64,
    };
    let projection = ratio_ingest::project(template, &delivery, &rows, "draft");

    let b = FileBook::open(book)?;
    let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
    let resolved = ratio_ingest::resolve_all(&projection.facts, &master);

    out.push_str(&format!(
        "{}\n\nAgainst the sample: {} row(s) read, {} fact(s), {} refused.\n",
        template.render(),
        rows.len(),
        projection.facts.len(),
        projection.rejected.len(),
    ));
    for r in &projection.rejected {
        out.push_str(&format!("  refused  row {}: {}\n", r.row, r.reason));
    }
    for r in &resolved {
        if r.is_admissible() {
            let to: Vec<String> = r
                .entities
                .keys()
                .filter_map(|k| r.entity(k).map(|e| format!("{k}={e}")))
                .collect();
            out.push_str(&format!("  ready    {} -> {}\n", r.fact.reference, to.join(" ")));
        } else {
            out.push_str(&format!(
                "  pending  {} — {}\n",
                r.fact.reference,
                r.blocker().unwrap_or_default()
            ));
        }
    }

    let id = format!("template-{}", template.id);
    let dir = book.join("proposals");
    std::fs::create_dir_all(&dir).context("creating the proposals directory")?;
    std::fs::write(dir.join(format!("{id}.toml")), toml)
        .with_context(|| format!("writing proposal {id}"))?;

    out.push_str(&format!(
        "\nSaved as proposal `{id}`. THIS IS NOT ACTIVE. A person approves it by \
         running `ratio approve {id}` at a terminal — and what they approve is \
         exactly what you have shown them it does.\n\n\
         A pending row is not a fault in the template: it means the master does \
         not know that instrument yet, and the fact will clear on its own when \
         somebody adds it.\n",
    ));
    Ok(out)
}

fn tool_list_accounts(book: &std::path::Path) -> Result<String> {
    let b = FileBook::open(book)?;
    let accounts = b.accounts()?;
    if accounts.is_empty() {
        return Ok("The chart of accounts is empty. Run `ratio init` first.".into());
    }
    let mut out = String::from("dim  account                          type       normal\n");
    for a in accounts {
        let t: ratio_chart::AccountType = a.account_type.into();
        out.push_str(&format!(
            "{:<5}{:<33}{:<11}{}\n",
            a.dim,
            a.display_name,
            format!("{:?}", a.account_type).to_lowercase(),
            match ratio_chart::normal_side(t) {
                ratio_chart::Side::Debit => "debit",
                ratio_chart::Side::Credit => "credit",
            }
        ));
    }
    Ok(out)
}

fn findings_text(set: &RuleSet, chart: &[ratio_store::Account]) -> (usize, String) {
    let findings = check(set, chart);
    let errors = findings.iter().filter(|f| !f.is_question).count();
    let mut out = String::new();
    for f in &findings {
        out.push_str(&format!(
            "{} {}: {}\n",
            if f.is_question { "?" } else { "x" },
            f.rule,
            f.message
        ));
    }
    if findings.is_empty() {
        out.push_str("All checks passed.\n");
    }
    (errors, out)
}

fn tool_check_rule(book: &std::path::Path, args: &Value) -> Result<String> {
    let toml = args
        .get("toml")
        .and_then(Value::as_str)
        .context("check_rule needs `toml`")?;
    let set = RuleSet::from_toml(toml)?;
    let b = FileBook::open(book)?;
    let (errors, text) = findings_text(&set, &b.accounts()?);
    Ok(format!(
        "{text}\n{errors} error(s). {}",
        if errors == 0 {
            "This rule set can be used as written."
        } else {
            "It cannot be used until the errors are fixed."
        }
    ))
}

fn tool_propose_rule(book: &std::path::Path, args: &Value) -> Result<String> {
    let toml = args
        .get("toml")
        .and_then(Value::as_str)
        .context("propose_rule needs `toml`")?;
    let set = RuleSet::from_toml(toml)?;
    let b = FileBook::open(book)?;
    let chart = b.accounts()?;
    let (errors, text) = findings_text(&set, &chart);

    if errors > 0 {
        return Ok(format!(
            "{text}\nNot proposed: {errors} error(s) must be fixed first."
        ));
    }

    let id = ratio_store::Digest::of(toml.as_bytes());
    let dir = book.join("proposals");
    std::fs::create_dir_all(&dir).context("creating proposals directory")?;
    std::fs::write(dir.join(format!("{}.toml", id.short())), toml)
        .context("writing proposal")?;

    let mut rendered = String::new();
    for rule in &set.rules {
        rendered.push_str(&render(rule, &chart));
        rendered.push('\n');
    }

    Ok(format!(
        "{text}\nProposal {} recorded. It is NOT active.\n\n{rendered}\
         A person must review and approve it:\n\n    ratio approve {}\n\n\
         Until then no event can be posted against it.",
        id.short(),
        id.short()
    ))
}

fn tool_post_events(book: &std::path::Path, args: &Value) -> Result<String> {
    let events: Vec<Event> = serde_json::from_value(
        args.get("events").cloned().context("post_events needs `events`")?,
    )
    .context("`events` is not a list of events")?;

    let mut b = FileBook::open(book)?;
    let digest = b
        .active()?
        .context("no configuration is active — a person must approve one first")?;
    let set = RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest)?))?;

    let mut posted = 0usize;
    for event in &events {
        let rule = set.rule(&event.rule).with_context(|| {
            format!(
                "event {:?} names rule {:?}, which is not in the active configuration. \
                 Only approved rules can run.",
                event.id, event.rule
            )
        })?;
        b.append(&JournalEntry {
            id: event.id.clone(),
            memo: event.memo.clone(),
            config: digest.clone(),
            postings: compile(rule, event)?,
        
            trade_date: None,
            announcement: None,
        })?;
        posted += 1;
    }

    let tb = b.trial_balance()?;
    Ok(format!(
        "Posted {posted} event(s) under configuration {}.\n\
         Debits {}, credits {}, difference {}.",
        digest.short(),
        tb.debits,
        tb.credits,
        tb.debits - tb.credits
    ))
}

fn tool_trial_balance(book: &std::path::Path) -> Result<String> {
    let b = FileBook::open(book)?;
    let names: std::collections::BTreeMap<i64, String> = b
        .accounts()?
        .into_iter()
        .map(|a| (a.dim, a.display_name))
        .collect();
    let tb = b.trial_balance()?;
    // ⛔ ONE ROW PER (ACCOUNT, CURRENCY). This printed one row per account, so
    // an account holding dollars and euros showed their SUM under a header
    // that named no currency — a figure a model would read back and cite. A
    // trial balance is struck per denomination or not at all.
    let mut out = String::from("account                          ccy         debit           credit\n");
    for ((dim, ccy), (debit, credit)) in b.balances_by_dim()? {
        out.push_str(&format!(
            "{:<32}{:<5}{:>12}{:>17}\n",
            names.get(&dim).cloned().unwrap_or_else(|| format!("dim {dim}")),
            ccy.as_deref().unwrap_or("—"),
            debit,
            credit
        ));
    }
    out.push_str(&format!(
        "{:<32}{:>12}{:>17}\ndifference {}\n",
        "TOTAL",
        tb.debits,
        tb.credits,
        tb.debits - tb.credits
    ));

    // Cite the provenance. A figure a model reads back without saying what
    // produced it is a figure nobody can check, which is the failure this
    // whole system exists to prevent.
    // ⛔ COUNTED AND SET-COLLECTED IN ONE PASS. This materialized the journal to
    // learn two things: how many entries there are, and how many DISTINCT
    // configurations they name — of which there are a handful.
    let mut count = 0usize;
    let mut configs: std::collections::BTreeSet<String> = Default::default();
    b.for_each_entry_since(0, &mut |e| {
        count += 1;
        if !configs.contains(e.config.as_str()) {
            configs.insert(e.config.as_str().to_string());
        }
        Ok(())
    })?;
    out.push_str(&format!("\n{count} entrie(s)\n"));
    match configs.len() {
        // Nothing posted yet: the active configuration is all there is to cite.
        0 => {
            if let Some(active) = b.active()? {
                out.push_str(&format!("configuration {active}\n"));
            }
        }
        1 => out.push_str(&format!(
            "configuration {}\n",
            configs.iter().next().expect("len == 1")
        )),
        // Say so rather than naming the active one and implying it produced
        // every figure above. A period spanning a configuration change is a
        // normal thing that a single hash would misreport.
        n => {
            out.push_str(&format!(
                "posted under {n} configurations — this balance spans a change:\n"
            ));
            for c in &configs {
                out.push_str(&format!("  {c}\n"));
            }
            out.push_str("Use explain_figure to see which entry used which.\n");
        }
    }

    out.push_str("\nAmounts are minor units. The difference is zero because every \
                  entry was checked for conservation on the way in.");
    Ok(out)
}

fn tool_explain_figure(book: &std::path::Path, args: &Value) -> Result<String> {
    let dim = args
        .get("account")
        .and_then(Value::as_i64)
        .context("explain_figure needs `account`")?;
    let b = FileBook::open(book)?;
    let name = b
        .accounts()?
        .into_iter()
        .find(|a| a.dim == dim)
        .map(|a| a.display_name)
        .unwrap_or_else(|| format!("dim {dim}"));

    let mut lines = Vec::new();
    let mut total = 0i64;
    // ⛔ STREAMED — one account's postings, not the whole log.
    b.for_each_entry_since(0, &mut |entry| {
        for p in &entry.postings {
            if p.dim == dim {
                total += p.amount;
                lines.push(format!(
                    "  {:<14}{:>14}  config {}  {}",
                    entry.id,
                    p.amount,
                    entry.config.short(),
                    entry.memo
                ));
            }
        }
        Ok(())
    })?;
    if lines.is_empty() {
        return Ok(format!("{name} has no postings."));
    }
    Ok(format!(
        "{name} — {} posting(s), net {total} minor units\n\n  entry                  amount  provenance\n{}\n\nEvery line names the configuration in force when it was posted; \
         re-running that configuration reproduces the figure exactly.",
        lines.len(),
        lines.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_trial_balance_cites_the_configuration_that_produced_it() {
        // PLAN.md step 7: the model reads a figure back and cites the config
        // hash — reporting, not deciding. A figure with no provenance is a
        // figure nobody can check.
        let book = tmp_book();
        seed_fee_rule(&book);
        post_one_accrual(&book);

        let out = tool_trial_balance(&book).unwrap();
        let active = FileBook::open(&book).unwrap().active().unwrap().unwrap();
        assert!(
            out.contains(active.as_str()),
            "no configuration cited:\n{out}"
        );
    }

    #[test]
    fn a_balance_spanning_two_configurations_says_so() {
        // Naming one hash here would imply it produced every figure above it.
        let book = tmp_book();
        seed_fee_rule(&book);
        post_one_accrual(&book);

        // Change the configuration, then post again under the new one.
        bump_rate(&book);
        post_one_accrual(&book);

        let out = tool_trial_balance(&book).unwrap();
        assert!(out.contains("spans a change"), "{out}");
        assert!(out.contains("2 configurations"), "{out}");
    }

    fn tmp_book() -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("ratio-mcp-{n}-{:?}", std::thread::current().id()));
        let mut b = FileBook::open(&p).unwrap();
        b.put_accounts(&[
            acct(1, "Investments at fair value", ratio_store::AccountTypeRecord::Asset),
            acct(2, "Cash and equivalents", ratio_store::AccountTypeRecord::Asset),
            acct(10, "Management fee expense", ratio_store::AccountTypeRecord::Expense),
            acct(40, "Management fee payable", ratio_store::AccountTypeRecord::Liability),
        ])
        .unwrap();
        p
    }
    fn acct(dim: i64, name: &str, t: ratio_store::AccountTypeRecord) -> ratio_store::Account {
        ratio_store::Account {
            dim,
            display_name: name.into(),
            account_type: t,
        }
    }

    fn call(book: &PathBuf, name: &str, args: Value) -> Value {
        let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                         "params":{"name":name,"arguments":args}});
        serde_json::from_str(&handle_line(book, &req.to_string()).unwrap()).unwrap()
    }
    fn text_of(v: &Value) -> String {
        v.pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    const FEE: &str = r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
rate_bp = 75
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1
"#;

    /// A human approving the fee rule, out of band — what `ratio approve` does.
    fn seed_fee_rule(book: &PathBuf) {
        let mut b = FileBook::open(book).unwrap();
        let d = b.put(FEE.as_bytes()).unwrap();
        b.set_active(&d).unwrap();
    }

    /// Approve a *different* configuration, so the next posting carries a new
    /// hash. Any change to the bytes gives a different digest; the rate is the
    /// clearest thing to move.
    fn bump_rate(book: &PathBuf) {
        let mut b = FileBook::open(book).unwrap();
        let d = b
            .put(FEE.replace("rate_bp = 75", "rate_bp = 80").as_bytes())
            .unwrap();
        b.set_active(&d).unwrap();
    }

    fn post_one_accrual(book: &PathBuf) {
        let n = FileBook::open(book).unwrap().entries().unwrap().len();
        let r = call(
            book,
            "post_events",
            json!({"events": [
                {"rule":"management_fee_accrual","id":format!("a{n}"),
                 "amount":1_750_000_000i64,"days":30}
            ]}),
        );
        assert_ne!(r["result"]["isError"], true, "{}", text_of(&r));
    }

    #[test]
    fn initialize_echoes_the_clients_protocol_version() {
        let b = tmp_book();
        let req = json!({"jsonrpc":"2.0","id":1,"method":"initialize",
                         "params":{"protocolVersion":"2025-06-18"}});
        let r: Value = serde_json::from_str(&handle_line(&b, &req.to_string()).unwrap()).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "ratio");
    }

    #[test]
    fn a_notification_is_never_answered() {
        let b = tmp_book();
        let n = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        assert!(handle_line(&b, &n.to_string()).is_none());
    }

    #[test]
    fn unparseable_input_gets_a_parse_error_not_a_panic() {
        let b = tmp_book();
        let r: Value = serde_json::from_str(&handle_line(&b, "{not json").unwrap()).unwrap();
        assert_eq!(r["error"]["code"], -32700);
    }

    /// The fence. `approve_rule` must not be listed, must not dispatch, and the
    /// refusal must tell the model what to do instead.
    #[test]
    fn propose_rule_shows_the_shape_it_wants() {
        // Watched live: a model took three attempts to find this, discovering
        // `id` then `kind` one parse error at a time — while the schema knew
        // both. A tool description that omits its required fields turns every
        // caller into a bisector.
        let d = tools()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "propose_rule")
            .unwrap()["inputSchema"]["properties"]["toml"]["description"]
            .as_str()
            .unwrap()
            .to_string();
        for needed in ["id", "kind", "rate_bp", "day_count", "[[rule.posting]]", "weight"] {
            assert!(d.contains(needed), "propose_rule does not mention {needed}");
        }
        // And the example it shows must actually parse, or it teaches a lie.
        let example = d[d.find("[[rule]]").unwrap()..].to_string();
        let set = ratio_rules::RuleSet::from_toml(&example)
            .unwrap_or_else(|e| panic!("the example in propose_rule does not parse: {e}\n{example}"));
        assert_eq!(set.rules.len(), 1);
        assert_eq!(set.rules[0].id, "management_fee_accrual");
    }

    #[test]
    fn the_exposed_dispatcher_carries_the_same_fence() {
        // `dispatch` exists so the in-process chat demo does not need its own
        // copy of the tool table. A second copy is a second place for the
        // fence to go missing, and the fence works by ABSENCE — so the public
        // entry point has to refuse exactly as the JSON-RPC one does.
        let b = tmp_book();
        for name in ["approve_rule", "promote_config", "set_active"] {
            let e = dispatch(&b, name, &json!({})).unwrap_err().to_string();
            assert!(e.contains("no such tool"), "{name}: {e}");
            assert!(e.contains("ratio approve"), "{name}: {e}");
        }
        // And the exposed list is the same list, not a superset.
        let names: Vec<String> = tools()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(!names.iter().any(|n| n.contains("approve")), "{names:?}");
        assert!(names.contains(&"propose_rule".to_string()), "{names:?}");
    }

    #[test]
    fn dispatch_and_jsonrpc_agree() {
        // If these two ever diverge, the chat demo stops being evidence about
        // the MCP server — which is the whole reason it is worth showing.
        let b = tmp_book();
        let direct = dispatch(&b, "list_accounts", &json!({})).unwrap();
        let viajsonrpc = text_of(&call(&b, "list_accounts", json!({})));
        assert_eq!(direct, viajsonrpc);
    }

    #[test]
    fn there_is_no_approve_tool() {
        let b = tmp_book();
        let listed: Value =
            serde_json::from_str(&handle_line(&b, &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string()).unwrap())
                .unwrap();
        let names: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&"approve_rule"), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("approve") || n.contains("promote")));
        assert!(names.contains(&"propose_rule"));

        let r = call(&b, "approve_rule", json!({}));
        assert_eq!(r["result"]["isError"], true);
        assert!(text_of(&r).contains("ratio approve"), "{}", text_of(&r));
    }

    #[test]
    fn proposing_a_rule_does_not_make_it_active() {
        let b = tmp_book();
        let r = call(&b, "propose_rule", json!({"toml": FEE}));
        let t = text_of(&r);
        assert!(t.contains("is NOT active"), "{t}");
        assert!(t.contains("ratio approve"), "{t}");
        // …and nothing was promoted.
        assert!(FileBook::open(&b).unwrap().active().unwrap().is_none());
    }

    #[test]
    fn a_broken_rule_is_not_proposed_and_says_why() {
        let b = tmp_book();
        let bad = FEE.replace("weight = -1", "weight = -2");
        let t = text_of(&call(&b, "propose_rule", json!({"toml": bad})));
        assert!(t.contains("net to -1"), "{t}");
        assert!(t.contains("Not proposed"), "{t}");
        assert!(!std::path::Path::new(&b).join("proposals").exists());
    }

    #[test]
    fn posting_without_an_approved_configuration_is_refused() {
        let b = tmp_book();
        let r = call(&b, "post_events", json!({"events": []}));
        assert_eq!(r["result"]["isError"], true);
        assert!(text_of(&r).contains("must approve"), "{}", text_of(&r));
    }

    #[test]
    fn posting_runs_only_approved_rules_and_the_book_ties() {
        let b = tmp_book();
        // A human approves, out of band — exactly as the demo does.
        {
            let mut book = FileBook::open(&b).unwrap();
            let d = book.put(FEE.as_bytes()).unwrap();
            book.set_active(&d).unwrap();
        }
        let t = text_of(&call(
            &b,
            "post_events",
            json!({"events": [
                {"rule":"management_fee_accrual","id":"a1","amount":1750000000,"days":30}
            ]}),
        ));
        assert!(t.contains("Posted 1 event"), "{t}");
        assert!(t.contains("difference 0"), "{t}");

        // A rule nobody approved cannot run.
        let r = call(
            &b,
            "post_events",
            json!({"events": [{"rule":"not_approved","id":"x","amount":1}]}),
        );
        assert_eq!(r["result"]["isError"], true);
        assert!(text_of(&r).contains("Only approved rules"), "{}", text_of(&r));
    }

    #[test]
    fn explain_figure_cites_the_configuration() {
        let b = tmp_book();
        {
            let mut book = FileBook::open(&b).unwrap();
            let d = book.put(FEE.as_bytes()).unwrap();
            book.set_active(&d).unwrap();
        }
        call(
            &b,
            "post_events",
            json!({"events":[{"rule":"management_fee_accrual","id":"a1","amount":1750000000,"days":30,"memo":"Sep"}]}),
        );
        let t = text_of(&call(&b, "explain_figure", json!({"account": 10})));
        assert!(t.contains("Management fee expense"), "{t}");
        assert!(t.contains("config "), "{t}");
        assert!(t.contains("a1"), "{t}");
    }

    #[test]
    fn the_chart_reports_the_proved_normal_side() {
        let b = tmp_book();
        let t = text_of(&call(&b, "list_accounts", json!({})));
        assert!(t.contains("Management fee expense"));
        assert!(t.lines().any(|l| l.contains("expense") && l.contains("debit")));
        assert!(t.lines().any(|l| l.contains("liability") && l.contains("credit")));
    }

    #[test]
    fn an_unknown_method_is_a_jsonrpc_error() {
        let b = tmp_book();
        let r: Value = serde_json::from_str(
            &handle_line(&b, &json!({"jsonrpc":"2.0","id":9,"method":"nope"}).to_string()).unwrap(),
        )
        .unwrap();
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn a_failing_tool_is_a_result_not_a_transport_error() {
        // The model is meant to read the failure and try again, which it can
        // only do if the call succeeded at the protocol level.
        let b = tmp_book();
        let r = call(&b, "check_rule", json!({"toml": "this is not toml ["}));
        assert!(r.get("error").is_none(), "{r}");
        assert_eq!(r["result"]["isError"], true);
    }
}
