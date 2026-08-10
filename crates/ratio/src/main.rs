//! ratio — the command line.
//!
//! PLAN.md Stage 0 is done when `ratio post events.json && ratio balance`
//! prints a trial balance that ties. That is what this provides, over the
//! append-only book in `ratio-store`.
//!
//! Argument parsing is hand-rolled. There are six subcommands and no flags
//! worth a dependency; adding `clap` would mean another crate-universe repin
//! for a help string.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use ratio_api::LedgerService;
use ratio_chart::{normal_side, Side};
use ratio_proto::ratio::v1::ledger_server::LedgerServer;
use ratio_rules::{check, compile, render, Event, RuleSet};
use ratio_store::{
    Account, AccountTypeRecord, ConfigStore, FileBook, Journal, JournalEntry, PostingRecord,
};
use serde::Deserialize;

/// An entry as a caller writes it: no `config`, because the book stamps the
/// version in force rather than letting the caller assert one.
#[derive(Debug, Deserialize)]
struct EntryInput {
    id: String,
    #[serde(default)]
    memo: String,
    postings: Vec<PostingRecord>,
}

const USAGE: &str = "\
ratio — a ledger that cannot go out of balance

usage:
  ratio init [--book DIR]              create a book and seed a chart of accounts
  ratio config set FILE [--book DIR]   store a configuration and promote it
  ratio config show [--book DIR]       the active configuration and its history
  ratio rules check FILE [--book DIR]  check a rule set against the chart
  ratio rules show [--book DIR]        render the active rules for a human
  ratio apply FILE [--book DIR]        apply rules to events and post the result
  ratio post FILE [--book DIR]         post entries directly; refuses unbalanced
  ratio balance [--book DIR]           print the trial balance
  ratio explain ACCOUNT [--book DIR]   read back the postings behind a figure
  ratio strike [--as-of YYYY-MM-DD]    strike a NAV, pinned to the journal
        [--book DIR]                   with --as-of it REFUSES an unpriced position
  ratio navs [--book DIR]              every NAV struck on this book
  ratio replay STRIKE-ID [--book DIR]  re-derive a strike and prove it again
  ratio recon TXNS.csv POSITIONS.csv   shadow-run a period and report breaks
        [--book DIR] [--out FILE.pb] [--post]
  ratio watch [--book DIR] [--port N]  live trial balance in a browser
  ratio ingest FILE --template ID      read a counterparty file into facts
  ratio pending [--book DIR]           facts held back, and what blocks each
  ratio entities [--book DIR]          the master: instruments, counterparties
  ratio entity add --kind K --id I …   add one to the master
  ratio admit [--book DIR]             post every fact that fully resolves
  ratio mark --as-of YYYY-MM-DD        mark positions to market
  ratio action ID INST N-for-M --ex-date YYYY-MM-DD
                                       apply a split; refuses a second time
  ratio stale [--book DIR]             NAVs struck without an action since arrived
  ratio gen [--book DIR] [--securities N] [--lots-per N] [--turnover N]
        [--currencies N] [--open-actions N] [--capital N] [--seed N]
                                       generate a fund INTO a book, to serve
  ratio bench [--securities N] [--lots-per N] [--turnover N]
        [--currencies N]
                                       generate a fund and measure a period end
  ratio closure [--securities N] [--currencies N] [--lots-per N]
        [--open-actions N] [--capital N]
                                       what a period end costs, before running it
  ratio mcp [--book DIR]               serve the MCP tools on stdio
  ratio approve ID [--book DIR]        promote a proposal — humans only
  ratio server                         serve the Ledger gRPC API

The book defaults to ./book, or $RATIO_BOOK if set.
";

mod watch;

fn main() -> Result<()> {
    // Restore the default SIGPIPE behavior that Rust turns off at startup.
    //
    // Without this, `ratio balance | head` panics: `head` closes the pipe, the
    // write fails, and `println!` unwraps it. Every command here prints enough
    // to be piped into `head` or `less`, and a panic trace on screen during a
    // demo is not a thing to explain away.
    //
    // Safe in the way this call is always safe: it runs before any thread is
    // spawned, and SIG_DFL for SIGPIPE is the behavior every other command-line
    // tool on the system already has.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let (positional, book) = split_book_flag(&args)?;
    let cmd: Vec<&str> = positional.iter().map(String::as_str).collect();

    match cmd.as_slice() {
        [] | ["help"] | ["--help"] | ["-h"] => {
            print!("{USAGE}");
            Ok(())
        }
        ["init"] => init(book),
        ["config", "set", file] => config_set(book, file),
        ["config", "show"] => config_show(book),
        ["rules", "check", file] => rules_check(book, file),
        ["rules", "show"] => rules_show(book),
        ["apply", file] => apply(book, file),
        ["post", file] => post(book, file),
        ["balance"] => balance(book),
        ["explain", account] => explain(book, account),
        ["strike"] => strike(book, None),
        ["strike", "--as-of", d] => strike(book, Some(d)),
        ["navs"] => navs(book),
        ["replay", id] => replay_strike(book, id),
        ["recon", txns, positions] => recon(book, txns, positions, None, false),
        ["recon", txns, positions, "--post"] => recon(book, txns, positions, None, true),
        ["recon", txns, positions, "--out", out] => {
            recon(book, txns, positions, Some(out), false)
        }
        ["recon", txns, positions, "--out", out, "--post"]
        | ["recon", txns, positions, "--post", "--out", out] => {
            recon(book, txns, positions, Some(out), true)
        }
        ["watch"] => watch::watch(book, 7373),
        ["watch", "--port", p] => {
            watch::watch(book, p.parse().context("--port must be a number")?)
        }
        ["ingest", file, "--template", id] | ["ingest", file, "-t", id] => {
            ingest(book, file, id)
        }
        ["entities"] => entities(book),
        ["entity", "add", rest @ ..] => entity_add(book, rest),
        ["pending"] => pending(book),
        ["admit"] => admit(book),
        ["mark", "--as-of", d] | ["mark", d] => mark(book, d),
        ["action", id, inst, r, "--ex-date", d] => action(book, id, inst, r, d),
        ["stale"] => stale(book),
        ["closure", rest @ ..] => closure(book, rest),
        ["gen", rest @ ..] => gen(book, rest),
        ["bench", rest @ ..] => bench(rest),
        ["mcp"] => mcp(book),
        ["approve", id] => approve(book, id),
        ["server"] => serve(),
        other => {
            eprint!("{USAGE}");
            bail!("unrecognized command: {}", other.join(" "));
        }
    }
}

/// The configuration with its `[[rule]]` and `[[template]]` sections replaced
/// and everything else left exactly as it was.
///
/// Round-tripping the whole document through `RuleSet` would drop every key
/// that struct does not know about — which is how approving a rule came to
/// delete the templates beside it.
fn replace_sections(
    previous: &str,
    merged: &RuleSet,
    templates: &ratio_ingest::TemplateSet,
) -> Result<String> {
    let mut doc: toml::Table = if previous.trim().is_empty() {
        toml::Table::new()
    } else {
        previous.parse().context("the configuration in force is not valid TOML")?
    };
    // Serialize just the rules, then lift the array out of it, so the rule
    // encoding stays `RuleSet`'s business rather than being duplicated here.
    let only_rules: toml::Table = merged
        .to_toml()?
        .parse()
        .context("the merged rules did not round-trip through TOML")?;
    match only_rules.get("rule") {
        Some(rules) => {
            doc.insert("rule".into(), rules.clone());
        }
        // A merge that produced no rules removes the key rather than leaving a
        // stale one behind.
        None => {
            doc.remove("rule");
        }
    }

    let only_templates: toml::Table = toml::to_string_pretty(templates)
        .context("serializing the templates")?
        .parse()
        .context("the merged templates did not round-trip through TOML")?;
    match only_templates.get("template") {
        Some(t) => {
            doc.insert("template".into(), t.clone());
        }
        None => {
            doc.remove("template");
        }
    }

    toml::to_string_pretty(&doc).context("re-serializing the configuration")
}

/// Seconds since the epoch, for a received-at stamp.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ─── the data plane ───────────────────────────────────────────────────────

/// The templates in the configuration in force.
///
/// A configuration holds BOTH the posting rules and the mapping templates, and
/// each parser reads its own sections and ignores the other's — which is how
/// one digest comes to determine the whole derivation, from the counterparty's
/// bytes to the NAV.
///
/// ⚠ Because unknown sections are ignored rather than rejected, a misspelt
/// `[[templat]]` would silently yield zero templates. That is why every command
/// that reads them reports HOW MANY it found: a count of nothing is visible in
/// a way that a missing section is not.
fn templates_of(b: &FileBook) -> Result<(ratio_ingest::TemplateSet, ratio_store::Digest)> {
    let digest = b
        .active()?
        .context("no configuration is in force — run `ratio config set`")?;
    let text = String::from_utf8_lossy(&b.get(&digest)?).into_owned();
    Ok((ratio_ingest::TemplateSet::from_toml(&text)?, digest))
}

/// Read a counterparty file into facts.
fn ingest(book: PathBuf, file: &str, template_id: &str) -> Result<()> {
    use ratio_store::Plane;

    let mut b = FileBook::open(&book)?;
    let (set, digest) = templates_of(&b)?;
    let template = set.template(template_id).with_context(|| {
        format!(
            "no template {template_id:?} in the configuration in force \
             ({} template(s) there: {})",
            set.templates.len(),
            set.templates.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", "),
        )
    })?;
    let problems = template.check();
    if !problems.is_empty() {
        for p in &problems {
            println!("  x {p}");
        }
        bail!("the template does not check; fix it before mapping a file with it");
    }

    let bytes = std::fs::read(file).with_context(|| format!("reading {file}"))?;
    // Hashed on arrival, so every fact below can name the exact bytes it came
    // from — and re-delivering the same file is recognizable rather than a
    // second copy.
    let delivery = ratio_ingest::Delivery {
        digest: ratio_store::Digest::of(&bytes).as_str().to_string(),
        origin: file.to_string(),
        received: now(),
        bytes: bytes.len() as i64,
    };

    let rows = match template.reads {
        ratio_ingest::Reader::Csv => {
            ratio_ingest::extract_csv(&String::from_utf8_lossy(&bytes))?
        }
    };
    let projection =
        ratio_ingest::project(template, &delivery, &rows, digest.as_str());

    // A fact id is derived from the delivery digest and the row, so the same
    // bytes under the same template do not produce a second set.
    let known: std::collections::BTreeSet<String> = b
        .records::<ratio_ingest::Fact>(Plane::Facts)?
        .into_iter()
        .map(|f| f.id)
        .collect();
    let fresh: Vec<&ratio_ingest::Fact> =
        projection.facts.iter().filter(|f| !known.contains(&f.id)).collect();

    b.append_record(Plane::Deliveries, &delivery)?;
    for f in &fresh {
        b.append_record(Plane::Facts, f)?;
    }

    println!("read     {file}");
    println!("  bytes  {} ({})", delivery.bytes, &delivery.digest[..12]);
    println!("  rows   {}", rows.len());
    println!(
        "  facts  {} new{}",
        fresh.len(),
        if fresh.len() == projection.facts.len() {
            String::new()
        } else {
            format!(", {} already read", projection.facts.len() - fresh.len())
        },
    );
    if !projection.rejected.is_empty() {
        // Per row. A bad row does not take the file down with it.
        println!("  refused {}", projection.rejected.len());
        for r in &projection.rejected {
            println!("    row {:<5} {}", r.row, r.reason);
        }
    }

    let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
    report_resolution(&b.records::<ratio_ingest::Fact>(Plane::Facts)?, &master);
    Ok(())
}

/// Resolve every fact against the master and say where things stand.
///
/// A FOLD, recomputed every time. Nothing is cached, so a fact that was pending
/// this morning and is admissible now needs no re-ingestion — only the master
/// changed.
fn report_resolution(facts: &[ratio_ingest::Fact], master: &[ratio_ingest::Entity]) {
    let resolved = ratio_ingest::resolve_all(facts, master);
    let ok = resolved.iter().filter(|r| r.is_admissible()).count();
    println!("  ready  {ok} of {} ({} pending)", resolved.len(), resolved.len() - ok);
}

fn pending(book: PathBuf) -> Result<()> {
    use ratio_store::Plane;
    let b = FileBook::open(&book)?;
    let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
    let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
    let resolved = ratio_ingest::resolve_all(&facts, &master);

    let held: Vec<&ratio_ingest::Resolved> =
        resolved.iter().filter(|r| !r.is_admissible()).collect();
    if held.is_empty() {
        println!("nothing pending — {} fact(s), all resolved", facts.len());
        return Ok(());
    }
    println!("PENDING — {} of {} fact(s)", held.len(), facts.len());
    println!();
    for r in &held {
        println!("  {:<12} {}", r.fact.reference, r.blocker().unwrap_or_default());
        println!(
            "  {:<12} row {} of {} · template {}",
            "",
            r.fact.provenance.row,
            &r.fact.provenance.delivery[..12],
            r.fact.provenance.template_id,
        );
    }
    println!();
    println!("These clear when the master does. Nothing needs re-reading.");
    Ok(())
}

fn entities(book: PathBuf) -> Result<()> {
    use ratio_store::Plane;
    let b = FileBook::open(&book)?;
    let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
    if master.is_empty() {
        println!("the master is empty — `ratio entity add` puts something in it");
        return Ok(());
    }
    println!("{:<16}{:<14}{:<28}{}", "ID", "KIND", "NAME", "IDENTIFIED BY");
    for e in &master {
        let by: Vec<String> =
            e.attributes.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!(
            "{:<16}{:<14}{:<28}{}",
            e.id,
            e.kind.as_str(),
            e.display_name,
            by.join("  "),
        );
    }
    Ok(())
}

fn entity_add(book: PathBuf, args: &[&str]) -> Result<()> {
    use ratio_store::Plane;
    let mut kind: Option<ratio_ingest::EntityKind> = None;
    let (mut id, mut name) = (String::new(), String::new());
    let mut attributes = std::collections::BTreeMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut next = || it.next().copied().with_context(|| format!("{a} needs a value"));
        match *a {
            "--kind" => {
                kind = Some(match next()? {
                    "instrument" => ratio_ingest::EntityKind::Instrument,
                    "counterparty" => ratio_ingest::EntityKind::Counterparty,
                    k => bail!("{k:?} is not a kind — instrument or counterparty"),
                })
            }
            "--id" => id = next()?.to_string(),
            "--name" => name = next()?.to_string(),
            "--attr" => {
                let kv = next()?;
                let (k, v) = kv
                    .split_once('=')
                    .with_context(|| format!("--attr wants key=value, got {kv:?}"))?;
                attributes.insert(k.to_string(), v.to_string());
            }
            other => bail!("unrecognized flag {other:?}"),
        }
    }
    let kind = kind.context("--kind is required")?;
    if id.is_empty() {
        bail!("--id is required");
    }
    if attributes.is_empty() {
        // An entity with no attributes can never be matched by anything, so
        // adding one would look like progress and be none.
        bail!("--attr is required at least once, or nothing can ever resolve to this");
    }

    let mut b = FileBook::open(&book)?;
    let entity = ratio_ingest::Entity {
        id: id.clone(),
        kind,
        display_name: if name.is_empty() { id.clone() } else { name },
        attributes,
    };
    b.append_record(Plane::Entities, &entity)?;
    println!("added    {id}");

    // ⭐ The point of the design, shown rather than described: nothing is
    // re-read, and facts that were waiting on this are simply resolved now.
    let facts: Vec<ratio_ingest::Fact> = b.records(Plane::Facts)?;
    let master: Vec<ratio_ingest::Entity> = b.records(Plane::Entities)?;
    report_resolution(&facts, &master);
    Ok(())
}

/// Apply a corporate action.
///
/// ⛔ REFUSES A SECOND APPLICATION. Every other verb here is safe to retry —
/// `admit`, `ingest` and `mark` are all no-ops the second time. This one is
/// not: `Ratio.Actions.applying_twice_is_not_applying_once`, and the position
/// would double while the trial balance went on tying.
fn action(book: PathBuf, id: &str, instrument: &str, ratio: &str, ex_date: &str) -> Result<()> {
    if ex_date.split('-').count() != 3 {
        bail!("{ex_date:?} is not a date — YYYY-MM-DD");
    }
    let (num, den) = ratio
        .split_once("-for-")
        .context("the ratio reads like `2-for-1` or `1-for-10`")?;
    let s = ratio_ingest::actions::Split {
        num: num.parse().context("units received")?,
        den: den.parse().context("units given up")?,
    };
    // Announced first, applied second. The record of the ANNOUNCEMENT carries
    // the ex-date, and it is what makes a NAV struck before this answerable
    // afterwards — see `ratio stale`.
    //
    // ⛔ THE ANNOUNCEMENT GOES IN THE JOURNAL, not in `Plane::Actions`, and it
    // is a real entry with no postings — it moves no money, so it nets to zero
    // trivially and passes the balance check at the door. What it is doing
    // there is taking a POSITION in the order, so every strike after it pins
    // it. `Ratio.Actions.Factor.replay_is_determined_by_the_prefix`.
    //
    // A side log has no relationship to a strike's pinned prefix, so a figure
    // that depended on it would answer differently on every replay as the world
    // told us more: `//tla:announcements_in_side_log_check`.
    {
        let mut b = FileBook::open(&book)?;
        let announce_id = format!("announce-{id}");
        let known = b
            .entries()?
            .iter()
            .filter_map(|e| e.announcement.clone())
            .any(|a| a.id == id);
        if !known {
            let cfg = b.active()?.context("no configuration is in force")?;
            b.append(&JournalEntry {
                id: announce_id,
                memo: format!("announced {id}: {instrument} {ratio} ex {ex_date}"),
                config: cfg,
                postings: Vec::new(),
                trade_date: None,
                announcement: Some(ratio_store::AnnouncementRecord {
                    id: id.to_string(),
                    instrument: instrument.to_string(),
                    numerator: s.num,
                    denominator: s.den,
                    ex_date: ex_date.to_string(),
                    announced: now(),
                }),
            })?;
        }
    }
    let (moved, dim) =
        ratio_console::Console::new(&book).apply_action("demo", id, instrument, s)?;

    println!("applied  {id}");
    println!("  {instrument} {}-for-{}", s.num, s.den);
    println!("  units    {}{moved} on account {dim}", if moved > 0 { "+" } else { "" });
    println!("  cost     unchanged — a split moves units, not value");
    println!();
    println!("Applying it again is refused: an action is not idempotent, and the");
    println!("trial balance would not notice.");
    Ok(())
}

/// NAVs struck without an action that has since arrived.
///
/// ⛔ THE OBLIGATION REFUSING RESTATEMENT CREATES. A valuation point has one
/// answer and it is never replaced, so a late action cannot correct the NAVs it
/// should have been in — and a fund that cannot NAME them is publishing figures
/// it cannot qualify.
fn stale(book: PathBuf) -> Result<()> {
    let rows = ratio_console::Console::new(&book).stale_strikes("demo")?;
    if rows.is_empty() {
        println!("No struck NAV is missing a corporate action.");
        return Ok(());
    }
    println!("STRUCK WITHOUT AN ACTION — {} figure(s)", rows.len());
    println!();
    for (strike, action, why) in &rows {
        println!("  {strike:<24}{action}");
        println!("  {:<24}{why}", "");
    }
    println!();
    println!("These cannot be restated — a valuation point has one answer, and the");
    println!("first is what somebody was paid on. What they can be is QUALIFIED.");
    Ok(())
}

/// The dials, parsed once, for every command that takes them.
///
/// ⛔ ONE PARSER, BECAUSE `gen` AND `bench` MUST AGREE ABOUT WHAT A SHAPE IS.
/// Two would drift — and the failure is that a book seeded for the demo and a
/// book measured by the benchmark stop being the same book, while both commands
/// go on reporting figures about "the twenty-security fund".
///
/// Returns the shape and anything that was not a dial, so a caller with its own
/// flags (`--book`) can take them without this having to know about them.
fn shape_from<'a>(args: &[&'a str]) -> Result<(ratio_gen::Shape, Vec<&'a str>)> {
    let mut shape = ratio_gen::Shape::default();
    let mut rest = Vec::new();
    let mut it = args.iter().peekable();
    while let Some(flag) = it.next() {
        let mut v = || -> Result<i64> {
            it.next()
                .ok_or_else(|| anyhow::anyhow!("{flag} needs a number"))?
                .parse::<i64>()
                .with_context(|| format!("{flag} needs a number"))
        };
        match *flag {
            "--securities" => shape.securities = v()?,
            // ⛔ A DIAL BENCH DID NOT HAVE, so the currency term of
            // `Ratio.Closure.navCost` was fixed at three and the one-currency
            // case — where translation is free — could not be measured at all.
            "--currencies" => shape.currencies = v()?,
            "--lots-per" => shape.lots_per = v()?,
            "--turnover" => shape.turnover = v()?,
            "--open-actions" => shape.open_actions = v()?,
            "--capital" => shape.capital_txns = v()?,
            "--seed" => shape.seed = v()? as u64,
            other => rest.push(other),
        }
    }
    Ok((shape, rest))
}

/// Generate a fund into a book, so something other than a benchmark can read it.
///
/// ⛔ THE GENERATOR EXISTED AND NOTHING COULD SERVE WHAT IT MADE. `bench` wrote
/// its fund to a temp directory, measured it and discarded it, so the only book
/// exercising tax lots, holding periods and several currencies was one no screen
/// could open. Everything the lot engine does was invisible on the demo for want
/// of this command.
fn gen(book: PathBuf, args: &[&str]) -> Result<()> {
    let (shape, rest) = shape_from(args)?;
    // ⚠ `--book` NEVER REACHES HERE. `split_book_flag` pulls it out before
    // dispatch, wherever it appears, and hands it in — so anything left over is
    // a flag nobody defined. I wrote a second `--book` parser here first; it
    // silently lost to the real one and generated a fund into `./book`.
    if let Some(other) = rest.first() {
        bail!("{other:?} is not a dial — see `ratio help`");
    }

    let entries = ratio_gen::generate(&book, shape)?;
    let proj = ratio_project::Projection::of_book(&book)?;
    println!("generated {} into {}", plural(entries as i64, "entry", "entries"), book.display());
    println!("  {:<22}{:>12}", "securities", shape.securities);
    println!("  {:<22}{:>12}", "currencies", shape.currencies);
    println!("  {:<22}{:>12}", "open tax lots", proj.open_lots());
    println!("  {:<22}{:>12}", "open positions", proj.positions().value.held.len());
    Ok(())
}

/// `1 entry` / `2 entries`, because a demo script prints this.
fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Generate a fund and measure what a period end actually takes.
///
/// ⛔ IT REPORTS TWO CURVES BECAUSE THERE ARE TWO, and reporting one would be
/// the easiest overclaim this repo has available:
///
///   COLD BUILD   folding the journal into a projection. O(entries), and the
///                journal holds every buy AND every sale forever — an
///                append-only log does not forget a closed lot. This grows.
///   NAV STRIKE   reading the maintained projection. `Ratio.Closure.navCost` —
///                one price per security, one rate per currency, and the tax
///                lots nowhere in it. This does NOT grow.
///
/// "Twenty million lots are free" is true of the second and false of the first.
fn bench(args: &[&str]) -> Result<()> {
    use std::time::Instant;

    let (shape, _) = shape_from(args)?;

    println!("A FUND OF THIS SHAPE, GENERATED AND MEASURED");
    println!();
    println!("  {:<22}{:>12}", "securities", shape.securities);
    println!("  {:<22}{:>12}", "currencies", shape.currencies);
    println!("  {:<22}{:>12}   at steady state", "open lots / security", shape.lots_per);
    println!("  {:<22}{:>12}   opened per lot left open", "turnover", shape.turnover);
    println!("  {:<22}{:>12}", "open corp. actions", shape.open_actions);
    println!();

    let dir = std::env::temp_dir().join("ratio-bench-book");
    let t = Instant::now();
    let entries = ratio_gen::generate(&dir, shape)?;
    let gen_ns = t.elapsed().as_nanos() as i64;
    println!(
        "  generated {entries} journal entries in {}",
        ratio_nav::closure::human_nanos(gen_ns)
    );

    // COLD: fold the whole journal into a projection. O(entries).
    let t = Instant::now();
    let proj = ratio_project::Projection::of_book(&dir)?;
    let cold_ns = t.elapsed().as_nanos() as i64;

    // The prices and rates a NAV reads. `Ratio.Closure.navCost` is
    // `markCost + fxCost + ...` — one price per SECURITY, one rate per
    // CURRENCY, and the tax lots in neither.
    let b = FileBook::open(&dir)?;
    let facts: Vec<ratio_ingest::Fact> = b.records(ratio_store::Plane::Facts)?;
    let prices: std::collections::BTreeMap<String, i64> = facts
        .iter()
        .filter(|f| f.kind == "price")
        .filter_map(|f| {
            Some((f.reference.clone(), f.values.get("price")?.as_minor()?))
        })
        .collect();
    // ⛔ ONE READER OF THE RATE FACTS, shared with the console. Two would be two
    // chances to disagree about which field is the rate, and the disagreement
    // would surface as a fund valued at a number nobody chose.
    let rates = ratio_project::Rates::of_facts(ratio_gen::currency_code_of(0), &facts);

    // MARK: one price per security, units read through any outstanding
    // corporate action. ⛔ THIS IS THE TERM `Ratio.Closure.markCost` NAMES, and
    // it is the one that grows with the CHART rather than with the lots.
    let t = Instant::now();
    let mut marked = 0i64;
    let mut gross: i64 = 0;
    for i in 0..shape.securities {
        let tkr = ratio_gen::ticker(i);
        let units = proj.units_as_of(1, &tkr, "2026-06-30")?.value;
        let px = prices.get(&tkr).copied().unwrap_or(0);
        // Per-currency subtotals are what FX translates, so the local figure is
        // accumulated here and translated once per currency below.
        gross += units.saturating_mul(px);
        marked += 1;
    }
    let mark_ns = t.elapsed().as_nanos() as i64;

    // FX: one rate per CURRENCY, not one per position.
    // `Ratio.Closure.fx_does_not_grow_with_the_chart`.
    //
    // ⚠ THIS USED TO MEASURE A MULTIPLICATION AND THROW IT AWAY — `let _ =` —
    // so the cost was honest and nothing was ever translated. The strike below
    // now does the real translation through `Projection::nav`; what is timed
    // here is the same arithmetic on the marked figure, which is the term
    // `Ratio.Closure.fxCost` names.
    let t = Instant::now();
    let mut translated = 0i64;
    for c in 1..shape.currencies {
        let code = ratio_gen::currency_code_of(c);
        let rate = rates.factor_of(code).unwrap_or(ratio_project::RATE_SCALE);
        let _ = gross.saturating_mul(rate) / ratio_project::RATE_SCALE;
        translated += 1;
    }
    let fx_ns = t.elapsed().as_nanos() as i64;

    // STRIKE: the fold, off maintained totals rather than the journal.
    let types: std::collections::BTreeMap<i64, ratio_store::AccountTypeRecord> =
        b.accounts()?.into_iter().map(|a| (a.dim, a.account_type)).collect();
    let t = Instant::now();
    let struck = proj.nav(
        &|dim| {
        matches!(
            types.get(&dim),
            Some(ratio_store::AccountTypeRecord::Asset)
                | Some(ratio_store::AccountTypeRecord::Liability)
            )
        },
        &rates,
    )?;
    let strike_ns = t.elapsed().as_nanos() as i64;

    let open_positions = proj.positions().value.held.len() as i64;
    let open_lots = proj.open_lots();
    let breaks = proj.lot_breaks().len();
    let nav_ns = mark_ns + fx_ns + strike_ns;
    println!();
    println!("  {:<26}{:>14}", "journal entries", entries);
    println!("  {:<26}{:>14}   ⛔ steady state, not cumulative", "open positions", open_positions);
    // ⛔ THE TWO NUMBERS THE SCALE ARGUMENT IS ACTUALLY ABOUT. Positions are a
    // CHART — five hundred entries whatever the fund's history. Lots are a
    // HISTORY, and this is where the memory is.
    println!(
        "  {:<26}{:>14}   ≈ {} MB resident at 40 bytes each",
        "open tax lots",
        open_lots,
        open_lots * 40 / 1_048_576
    );
    if breaks > 0 {
        println!("  {:<26}{:>14}   ⚠ sales that could not be relieved", "lot breaks", breaks);
        for b in proj.lot_breaks().iter().take(2) {
            println!("      {b}");
        }
    }
    println!();
    println!("  {:<26}{:>14}", "COLD BUILD  (O(journal))", ratio_nav::closure::human_nanos(cold_ns));
    println!("  {:<26}{:>14}   {marked} prices", "  mark", ratio_nav::closure::human_nanos(mark_ns));
    println!("  {:<26}{:>14}   {translated} rates, not {marked}", "  fx", ratio_nav::closure::human_nanos(fx_ns));
    println!("  {:<26}{:>14}   off maintained totals", "  strike", ratio_nav::closure::human_nanos(strike_ns));
    println!("  {:<26}{:>14}", "NAV  (O(chart))", ratio_nav::closure::human_nanos(nav_ns));
    println!();
    // ⭐ THE FIGURE THIS ENGINE EXISTS FOR. Six lot methods, holding-period
    // classification and the whole relief layer decide it, and until the sale
    // posted three legs it was computed and discarded.
    let realized = proj.nav(&|dim| dim == 30, &rates)?.value.0;
    println!("  net asset value {:>20}   over {} entries", struck.value.0, struck.prefix);
    println!(
        "  realized gain   {:>20}   credit-normal, so a gain reads negative",
        realized
    );
    println!("  basis relieved  {:>20}", proj.relieved_cost());
    println!("  trial balance   {:>20}", struck.value.1);
    println!();
    println!("⛔ Two curves, and only the second is flat in fragmentation. Folding");
    println!("   the journal grows with every trade ever made — an append-only log");
    println!("   does not forget a closed lot. Striking the NAV off the maintained");
    println!("   projection does not: `Ratio.Closure.factored_nav_never_reads_the_");
    println!("   lots`. Quoting the second as though it were the first is the");
    println!("   overclaim this command exists to make hard.");
    Ok(())
}

/// Turn the dial: what does a period end of this shape cost?
///
/// ⛔ THE ARITHMETIC IS NOT HERE. Every term comes from `ratio_nav::closure`,
/// which is emitted from `Ratio.Closure`. This function parses flags and prints;
/// if it computed anything, the printed number would stop being the proved one.
fn closure(book: PathBuf, args: &[&str]) -> Result<()> {
    use ratio_nav::closure::{estimate, Calibration, Dials};

    // An S&P tracker in its twentieth year, which is the case worth arguing
    // about: five hundred names, three currencies, twenty million open lots.
    let mut d = Dials {
        securities: 500,
        currencies: 3,
        lots_per: 40_000,
        open_actions: 0,
        capital_txns: 0,
    };

    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let v = || -> Result<i64> {
            it.clone()
                .next()
                .ok_or_else(|| anyhow::anyhow!("{flag} needs a number"))?
                .parse::<i64>()
                .with_context(|| format!("{flag} needs a number"))
        };
        match *flag {
            "--securities" => d.securities = v()?,
            "--currencies" => d.currencies = v()?,
            "--lots-per" => d.lots_per = v()?,
            "--open-actions" => d.open_actions = v()?,
            "--capital" => d.capital_txns = v()?,
            other => bail!("{other:?} is not a dial — see `ratio help`"),
        }
        it.next();
    }
    for (name, n) in [
        ("--securities", d.securities),
        ("--currencies", d.currencies),
        ("--lots-per", d.lots_per),
        ("--open-actions", d.open_actions),
        ("--capital", d.capital_txns),
    ] {
        if n < 0 {
            bail!("{name} cannot be negative");
        }
    }

    // Measure this machine if there is a book to measure against, and fall back
    // to the shipped constant when there is not.
    //
    // ⛔ THE REASON IS KEPT, not discarded with `unwrap_or_else(|_| …)`. Saying
    // which rate was used is half of it; a reader looking at a duration built on
    // a constant is owed WHY it could not be measured, because "there is no book
    // here" and "the book is unreadable" are different situations and only one
    // of them is fine.
    let (cal, fell_back) = match ratio_nav::closure::measure(&book) {
        Ok(c) => (c, None),
        Err(e) => (Calibration::measured(), Some(format!("{e:#}"))),
    };
    let e = estimate(d, &cal)?;

    println!("A PERIOD END OF THIS SHAPE");
    println!();
    println!("  {:<22}{:>12}", "securities", d.securities);
    println!("  {:<22}{:>12}", "currencies", d.currencies);
    println!("  {:<22}{:>12}", "open corp. actions", d.open_actions);
    println!("  {:<22}{:>12}", "capital transactions", d.capital_txns);
    println!("  {:<22}{:>12}   ⛔ not read by the NAV", "open tax lots", e.open_lots);
    println!();
    println!("READS");
    println!("  {:<22}{:>12}   one price per security", "marking", e.marks);
    println!("  {:<22}{:>12}   one rate per CURRENCY, not per position", "fx", e.fx);
    println!(
        "  {:<22}{:>12}   {}% of the work — the LOTS of one name",
        "corporate actions",
        e.actions,
        e.actions_share()
    );
    println!("  {:<22}{:>12}", "capital", e.capital);
    println!("  {:<22}{:>12}", "", "────────────");
    println!("  {:<22}{:>12}", "total", e.reads);
    println!();
    println!("  ≈ {}   {}", ratio_nav::closure::human_nanos(e.nanos), e.provenance);
    if let Some(why) = &fell_back {
        println!("           not measured here: {why}");
    }
    println!();
    if d.open_actions == 0 {
        println!("⛔ The tax lots are not in the total. `Ratio.Closure.a_quiet_nav_never_");
        println!("   reads_the_lots` — hold everything else fixed and the cost does not");
        println!("   move when fragmentation does. A fund's twentieth year strikes as");
        println!("   fast as its first.");
    } else {
        // ⛔ THE HONEST VERSION. With something outstanding the flat claim does
        // NOT hold, because applying an action by rewriting IS a walk over the
        // lots. Printing the reassuring sentence here would be the same
        // overclaim the cost model itself made for weeks.
        println!("⛔ WITH AN ACTION OUTSTANDING, THIS NAV READS THE LOTS. Applying one by");
        println!("   rewriting is a walk over every lot of that instrument, so the");
        println!("   flat-in-fragmentation claim does NOT hold today:");
        println!("   `Ratio.Closure.an_open_action_makes_the_nav_read_the_lots`.");
        println!();
        println!(
            "   as a factor instead   {:>10} reads   ≈ {}",
            e.factored_reads,
            ratio_nav::closure::human_nanos(e.factored_nanos)
        );
        println!("   `Ratio.Actions.Factor` — nothing is written, so the claim holds for");
        println!("   ANY number of open actions. ⚠ It removes a CLIFF, not a slope: on a");
        println!("   quiet day the two models are the same number.");
    }
    Ok(())
}

/// Mark positions to market at a valuation date.
fn mark(book: PathBuf, as_of: &str) -> Result<()> {
    let p: Vec<&str> = as_of.split('-').collect();
    if p.len() != 3 {
        bail!("{as_of:?} is not a date — YYYY-MM-DD");
    }
    let c = ratio_console::Console::new(&book);
    let out = c.mark_positions(&ratio_proto::ratio::console::v1::MarkPositionsRequest {
        parent: "funds/demo".into(),
        valuation_date: Some(ratio_proto::date_proto::google::r#type::Date {
            year: p[0].parse().context("year")?,
            month: p[1].parse().context("month")?,
            day: p[2].parse().context("day")?,
        }),
        validate_only: false,
    })?;

    println!("MARKED TO MARKET — {as_of}");
    println!();
    if !out.marks.is_empty() {
        println!("{:<34}{:>10}{:>16}{:>16}{:>14}", "INSTRUMENT", "UNITS", "CARRYING", "MARKET", "MOVEMENT");
        for m in &out.marks {
            println!(
                "{:<34}{:>10}{:>16}{:>16}{:>14}",
                m.instrument_label,
                m.quantity,
                minor(m.carrying.parse().unwrap_or(0)),
                minor(m.market.parse().unwrap_or(0)),
                minor(m.movement.parse().unwrap_or(0)),
            );
        }
        println!();
    }
    // ⛔ Not marked at zero, and not silently skipped. A position nobody has
    // priced is a NAV nobody should strike.
    if !out.unpriced.is_empty() {
        println!("UNPRICED — these are NOT held at zero, they are unvalued:");
        for m in &out.unpriced {
            println!("  {:<32}{:>10} units", m.instrument_label, m.quantity);
        }
        println!();
    }
    for i in &out.inexact {
        println!("  refused  {i}");
    }
    println!("  posted   {} entrie(s)", out.posted_count);
    println!(
        "  nav      {} → {}",
        minor(out.previous_net_asset_value.parse().unwrap_or(0)),
        minor(out.net_asset_value.parse().unwrap_or(0)),
    );
    Ok(())
}

/// Post every fact that fully resolves.
///
/// ⛔ CALLS THE CONSOLE'S IMPLEMENTATION. There were briefly two — one here and
/// one in `ratio-console` for the API — and a commit message of mine claimed
/// there was one. Two implementations of "what posts" is two sets of decisions
/// about the books, and the second silently lacked the instrument the first had
/// just learned to carry, which is how the positions view came up empty.
fn admit(book: PathBuf) -> Result<()> {
    let c = ratio_console::Console::new(&book);
    let out = c.admit_facts(&ratio_proto::ratio::console::v1::AdmitFactsRequest {
        parent: "funds/demo".into(),
        validate_only: false,
    })?;

    println!("  posted     {}", out.posted_count);
    if out.recorded_count != "0" {
        println!(
            "  recorded   {} (reference data — posts nothing by design)",
            out.recorded_count
        );
    }
    if !out.refused.is_empty() {
        println!("  refused    {}", out.refused.len());
        for r in &out.refused {
            println!("    {r}");
        }
    }
    if out.pending_count != "0" {
        println!(
            "  pending    {} (they clear when the master does)",
            out.pending_count
        );
    }
    Ok(())
}

/// Pull `--book DIR` out of the argument list, wherever it appears.
fn split_book_flag(args: &[String]) -> Result<(Vec<String>, PathBuf)> {
    let mut positional = Vec::new();
    let mut book: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--book" {
            let dir = it.next().context("--book needs a directory")?;
            book = Some(PathBuf::from(dir));
        } else {
            positional.push(a.clone());
        }
    }
    let book = book
        .or_else(|| std::env::var_os("RATIO_BOOK").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("book"));
    Ok((positional, book))
}

pub(crate) fn init(book: PathBuf) -> Result<()> {
    let mut b = FileBook::open(&book)?;
    if b.accounts()?.is_empty() {
        // A minimal chart that a single-currency equity fund can actually post
        // against — the fund type PLAN.md scopes the first shadow run to.
        b.put_accounts(&[
            acct(1, "Investments at fair value", AccountTypeRecord::Asset),
            acct(2, "Cash and equivalents", AccountTypeRecord::Asset),
            acct(3, "Dividends receivable", AccountTypeRecord::Asset),
            acct(10, "Management fee expense", AccountTypeRecord::Expense),
            acct(20, "Capital contributions", AccountTypeRecord::Equity),
            acct(21, "Unrealized gain", AccountTypeRecord::Equity),
            acct(30, "Dividend income", AccountTypeRecord::Income),
            // A disposal relieves the investment at cost and books the
            // difference here. Without it the scope in ratio-recon claims
            // coverage this chart cannot deliver.
            acct(31, "Realized gain on investments", AccountTypeRecord::Income),
            acct(40, "Management fee payable", AccountTypeRecord::Liability),
        ])?;
    }
    if b.active()?.is_none() {
        // An empty configuration is still a configuration: entries posted now
        // are attributable, and the digest changes the moment a rule is added.
        let digest = b.put(b"# ratio configuration\nrules = []\n")?;
        b.set_active(&digest)?;
    }
    println!("initialized book at {}", book.display());
    println!("  accounts  {}", b.accounts()?.len());
    println!("  config    {}", b.active()?.expect("just set").short());
    Ok(())
}

fn acct(dim: i64, name: &str, t: AccountTypeRecord) -> Account {
    Account {
        dim,
        display_name: name.to_string(),
        account_type: t,
    }
}

fn config_set(book: PathBuf, file: &str) -> Result<()> {
    let mut b = FileBook::open(&book)?;
    let bytes = std::fs::read(file).with_context(|| format!("reading {file}"))?;
    let digest = b.put(&bytes)?;
    b.set_active(&digest)?;
    println!("config {} promoted ({} bytes)", digest.short(), bytes.len());
    Ok(())
}

fn config_show(book: PathBuf) -> Result<()> {
    let b = FileBook::open(&book)?;
    match b.active()? {
        None => println!("no configuration promoted"),
        Some(d) => {
            println!("active   {d}");
            let hist = b.history()?;
            println!("history  {} promotion(s), newest first", hist.len());
            for d in hist {
                println!("  {}", d.short());
            }
        }
    }
    Ok(())
}

/// Check a rule set against the book's chart, without promoting it.
///
/// Reports every finding rather than the first, so a configuration is fixed in
/// one pass. Questions do not fail the check — they are for a human to answer.
fn rules_check(book: PathBuf, file: &str) -> Result<()> {
    let b = FileBook::open(&book)?;
    let text = std::fs::read_to_string(file).with_context(|| format!("reading {file}"))?;
    let set = RuleSet::from_toml(&text)?;
    let chart = b.accounts()?;
    let findings = check(&set, &chart);

    let errors = findings.iter().filter(|f| !f.is_question).count();
    let questions = findings.iter().filter(|f| f.is_question).count();

    println!("checked  {} rule(s) against {} account(s)", set.rules.len(), chart.len());
    for f in &findings {
        let mark = if f.is_question { "?" } else { "x" };
        println!("  {mark} {}: {}", f.rule, f.message);
    }
    if findings.is_empty() {
        println!("  all checks passed");
    }
    println!();
    println!("{errors} error(s), {questions} question(s)");

    if errors > 0 {
        bail!("{errors} rule(s) cannot be used as written");
    }
    Ok(())
}

/// Render the active rule set in the readable form.
///
/// Nobody writes this syntax and nothing parses it — the rules are the TOML.
/// This is what a reviewer or an examiner is shown.
fn rules_show(book: PathBuf) -> Result<()> {
    let b = FileBook::open(&book)?;
    let digest = b
        .active()?
        .context("no configuration promoted — run `ratio init` or `ratio config set`")?;
    let bytes = b.get(&digest)?;
    let set = RuleSet::from_toml(&String::from_utf8_lossy(&bytes))?;
    let chart = b.accounts()?;

    println!("config {digest}\n");
    if set.rules.is_empty() {
        println!("(no rules)");
    }
    for rule in &set.rules {
        println!("{}", render(rule, &chart));
    }
    Ok(())
}

/// Apply the active rules to a list of events and post what comes out.
///
/// The postings are balanced by theorem, not by inspection: `ratio rules check`
/// verifies each template's weights net to zero, and
/// `Ratio.Chart.balanced_template_balances` proves every instantiation of such
/// a template balances at any amount. The book still checks on the way in.
fn apply(book: PathBuf, file: &str) -> Result<()> {
    let mut b = FileBook::open(&book)?;
    let digest = b
        .active()?
        .context("no configuration promoted — run `ratio init` or `ratio config set`")?;
    let set = RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest)?))?;

    // Refuse to apply a rule set that would not pass its own check — otherwise
    // a bad template quietly produces bad postings on every event.
    let findings = check(&set, &b.accounts()?);
    let errors: Vec<_> = findings.iter().filter(|f| !f.is_question).collect();
    if !errors.is_empty() {
        for f in &errors {
            println!("  x {}: {}", f.rule, f.message);
        }
        bail!("the active configuration has {} error(s); fix it before applying", errors.len());
    }

    let text = std::fs::read_to_string(file).with_context(|| format!("reading {file}"))?;
    let events: Vec<Event> =
        serde_json::from_str(&text).with_context(|| format!("{file} is not a list of events"))?;

    let mut posted = 0usize;
    for event in &events {
        let rule = set
            .rule(&event.rule)
            .with_context(|| format!("event {:?} names rule {:?}, which is not in the active configuration", event.id, event.rule))?;
        let postings = compile(rule, event)?;
        b.append(&JournalEntry {
            id: event.id.clone(),
            memo: if event.memo.is_empty() {
                format!("{} via {}", event.id, rule.id)
            } else {
                event.memo.clone()
            },
            config: digest.clone(),
            postings,
        
            trade_date: None,
            announcement: None,
        })?;
        posted += 1;
    }
    println!("applied  {posted} event(s) under config {}", digest.short());
    Ok(())
}

fn post(book: PathBuf, file: &str) -> Result<()> {
    let mut b = FileBook::open(&book)?;
    let config = b
        .active()?
        .context("no configuration promoted — run `ratio init` or `ratio config set`")?;

    let text = std::fs::read_to_string(file).with_context(|| format!("reading {file}"))?;
    let inputs: Vec<EntryInput> =
        serde_json::from_str(&text).with_context(|| format!("{file} is not a list of entries"))?;

    let mut posted = 0usize;
    let mut refused = Vec::new();
    for input in inputs {
        let entry = JournalEntry {
            id: input.id.clone(),
            memo: input.memo,
            config: config.clone(),
            postings: input.postings,
        
            trade_date: None,
            announcement: None,
        };
        // The book refuses an unbalanced entry; report every one rather than
        // stopping at the first, so a bad file is fixed in one pass.
        match b.append(&entry) {
            Ok(()) => posted += 1,
            Err(e) => refused.push(format!("  {}: {e}", input.id)),
        }
    }

    println!("posted   {posted} entrie(s) under config {}", config.short());
    if !refused.is_empty() {
        println!("refused  {}", refused.len());
        for r in &refused {
            println!("{r}");
        }
        bail!("{} entrie(s) did not conserve value", refused.len());
    }
    Ok(())
}

fn balance(book: PathBuf) -> Result<()> {
    let b = FileBook::open(&book)?;
    let entries = b.entries()?;
    let names: std::collections::BTreeMap<i64, Account> =
        b.accounts()?.into_iter().map(|a| (a.dim, a)).collect();
    let by_dim = b.balances_by_dim()?;
    let tb = b.trial_balance()?;

    println!("TRIAL BALANCE — {} entrie(s)", entries.len());
    if let Some(c) = b.active()? {
        println!("configuration  {c}");
    }
    println!();
    println!("{:<34}{:>18}{:>18}", "ACCOUNT", "DEBIT", "CREDIT");
    for (dim, (debit, credit)) in &by_dim {
        let label = match names.get(dim) {
            Some(a) => {
                // Flag anything sitting on the side its type calls abnormal —
                // legal, and usually worth a second look.
                let net = debit - credit;
                let abnormal = net != 0
                    && ((net > 0 && normal_side(a.account_type.into()) == Side::Credit)
                        || (net < 0 && normal_side(a.account_type.into()) == Side::Debit));
                format!("{}{}", a.display_name, if abnormal { " *" } else { "" })
            }
            None => format!("dim {dim}"),
        };
        println!(
            "{:<34}{:>18}{:>18}",
            label,
            minor(*debit),
            minor(*credit)
        );
    }
    println!("{}", "-".repeat(70));
    println!(
        "{:<34}{:>18}{:>18}",
        "Total",
        minor(tb.debits),
        minor(tb.credits)
    );
    println!(
        "{:<34}{:>18}{:>18}",
        "Difference",
        minor(tb.debits - tb.credits),
        minor(0)
    );
    if by_dim.values().any(|(d, c)| {
        let net = d - c;
        net != 0
    }) && names.is_empty()
    {
        println!("\n(no chart of accounts — run `ratio init`)");
    }
    println!("\n* sits on the side its account type does not call normal");

    // This cannot fail for a book built through `ratio post`: every entry
    // conserved on the way in, and `Ratio.Chart.trial_balance_ties` proves the
    // rest. Asserted anyway — if it ever fires, something reached the journal
    // without passing the kernel.
    if !ratio_chart::trial_balance_ties(tb) {
        bail!(
            "the book does not tie: debits {} credits {} — an entry reached the \
             journal without passing the conservation check",
            tb.debits,
            tb.credits
        );
    }
    Ok(())
}

/// Minor units as a decimal string. Integer arithmetic throughout — the value
/// is never converted to a float, not even to print it.
pub(crate) fn minor(v: i64) -> String {
    let neg = v < 0;
    let a = v.unsigned_abs();
    let s = format!("{}.{:02}", a / 100, a % 100);
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

/// A short name for a book directory, for the `books/{book}/...` resource name.
fn book_label(book: &std::path::Path) -> String {
    book.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "book".into())
}

/// Strike a NAV over the journal as it stands.
///
/// The valuation point is now. A real system takes it from the fund's calendar
/// — 16:00 New York for a US mutual fund — and that belongs in configuration
/// rather than in a flag, so it is deliberately not one.
fn strike(book: PathBuf, as_of: Option<&str>) -> Result<()> {
    // ⛔ A NAV struck over a position nobody has priced is a number with a hole
    // in it, and the hole is invisible in the figure. With a valuation date,
    // that is a refusal.
    //
    // Without one it is not checked — because "unpriced" is not a well-formed
    // question until somebody says as of WHEN, and a book does not know what
    // day it is. That is the honest limit of this check, not an oversight.
    if let Some(day) = as_of {
        if day.split('-').count() != 3 {
            bail!("{day:?} is not a date — YYYY-MM-DD");
        }
        let blocked = ratio_console::Console::new(&book).unpriced_at("demo", day)?;
        if !blocked.is_empty() {
            println!("REFUSED — {} position(s) have no price on or before {day}:", blocked.len());
            for (name, units) in &blocked {
                println!("  {name:<38}{units:>10} units");
            }
            println!();
            println!("These are not held at zero; they are unvalued. Deliver a price");
            println!("file and `ratio mark --as-of {day}`, then strike.");
            bail!("the NAV was not struck");
        }
    }
    let actor = actor_name();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let s = ratio_nav::strike_and_record(&book, now, &actor)?;

    println!("struck {}", s.id);
    println!("  NAV        {}", minor(s.net_asset_value));
    println!("  difference {}", minor(s.trial_balance_difference));
    println!("  journal    {} entrie(s)", s.journal_position);
    println!("  digest     {}", &s.journal_digest[..12]);
    println!("  config     {}", &s.config_digest[..7.min(s.config_digest.len())]);
    println!("  by         {}", s.actor);
    println!();
    println!("Re-derive it any time with:  ratio replay {}", s.id);
    Ok(())
}

/// Every NAV struck on this book.
fn navs(book: PathBuf) -> Result<()> {
    let all = ratio_nav::list(&book)?;
    if all.is_empty() {
        println!("No NAV has been struck on this book. `ratio strike` takes one.");
        return Ok(());
    }
    println!("{:<20}{:>18}{:>10}  {:<10}{}", "AS OF", "NAV", "ENTRIES", "CONFIG", "BY");
    for s in &all {
        println!(
            "{:<20}{:>18}{:>10}  {:<10}{}",
            ratio_nav::rfc3339(s.valuation_time),
            minor(s.net_asset_value),
            s.journal_position,
            &s.config_digest[..7.min(s.config_digest.len())],
            s.actor
        );
    }
    Ok(())
}

/// Re-derive a strike and report what was found.
///
/// Exits non-zero when either check fails. A replay that reports a broken NAV
/// on stdout and exits 0 is a replay nothing can be built on.
fn replay_strike(book: PathBuf, id: &str) -> Result<()> {
    let s = ratio_nav::get(&book, id)?;
    let r = ratio_nav::replay(&book, &s)?;

    println!("replaying {}", s.id);
    println!("  struck     {} by {}", ratio_nav::rfc3339(s.valuation_time), s.actor);
    println!("  folding    {} entrie(s) of the journal", s.journal_position);
    println!();
    println!(
        "  history    {}",
        if r.history_intact {
            "intact — the journal prefix hashes as it did".to_string()
        } else {
            format!("REWRITTEN — {} now, {} then", &r.journal_digest[..12.min(r.journal_digest.len())], &s.journal_digest[..12])
        }
    );
    println!(
        "  arithmetic {}",
        if r.reproduced {
            format!("reproduced — {}", minor(r.net_asset_value))
        } else {
            format!("DIVERGED — {} now, {} then", minor(r.net_asset_value), minor(s.net_asset_value))
        }
    );
    println!();
    if r.ok() {
        println!("{} is exactly the NAV it was.", minor(s.net_asset_value));
        Ok(())
    } else {
        bail!("this NAV did not replay")
    }
}

/// Who is running this. Not authentication — a record of who ran a command on
/// a machine they were already trusted with, which is what a CLI is.
fn actor_name() -> String {
    std::env::var("RATIO_ACTOR")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "operator".into())
}

/// Read back every posting behind one account's balance.
///
/// The break report tells a reader to run this, so it has to exist: an
/// instruction pointing at a missing command is worse than no instruction.
/// Takes a dimension number or an account name.
fn explain(book: PathBuf, account: &str) -> Result<()> {
    let b = FileBook::open(&book)?;
    let chart = b.accounts()?;
    let dim = match account.parse::<i64>() {
        Ok(d) => d,
        Err(_) => {
            chart
                .iter()
                .find(|a| a.display_name.eq_ignore_ascii_case(account.trim()))
                .with_context(|| format!("no account named `{account}` in the chart"))?
                .dim
        }
    };
    let name = chart
        .iter()
        .find(|a| a.dim == dim)
        .map(|a| a.display_name.clone())
        .unwrap_or_else(|| format!("dim {dim}"));

    let mut total = 0i64;
    let mut rows = Vec::new();
    for entry in b.entries()? {
        for p in &entry.postings {
            if p.dim == dim {
                total += p.amount;
                // Every line carries the configuration in force when it was
                // posted, not the one active now — a period spanning a change
                // has to show which entry used which.
                rows.push(format!(
                    "  {:<18}{:>16}  {}  {}",
                    entry.id,
                    minor(p.amount),
                    entry.config.short(),
                    entry.memo
                ));
            }
        }
    }

    if rows.is_empty() {
        println!("{name} has no postings.");
        return Ok(());
    }
    println!("{name} — {} posting(s)", rows.len());
    println!();
    println!("  {:<18}{:>16}  {:<8}  {}", "ENTRY", "AMOUNT", "CONFIG", "MEMO");
    for r in &rows {
        println!("{r}");
    }
    println!();
    println!("  {:<18}{:>16}", "net", minor(total));
    println!();
    println!(
        "Re-running the configuration each line names reproduces that line exactly."
    );
    Ok(())
}

/// Shadow-run a period: replay a transactions file through the active
/// configuration and compare against the positions the incumbent reported.
///
/// # Exit codes
///
/// A shadow run has three outcomes and they are not the same thing, so they
/// get different codes rather than all being "success with different text":
///
/// | code | meaning |
/// |---|---|
/// | 0 | reconciled, no differences |
/// | 2 | reconciled, differences found |
/// | 3 | **refused** — the file contains rows outside the run's scope |
///
/// 3 is not a failure of the run; it is the run declining to produce a
/// comparison it cannot stand behind. Conflating it with 2 would let a
/// refusal be scripted as "breaks found" and quietly investigated as data.
fn recon(
    book: PathBuf,
    txns: &str,
    positions: &str,
    out: Option<&str>,
    post: bool,
) -> Result<()> {
    let mut b = FileBook::open(&book)?;
    let digest = b
        .active()?
        .context("no configuration promoted — run `ratio approve` first")?;
    let set = RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest)?))?;
    let chart = b.accounts()?;

    let txn_text = std::fs::read_to_string(txns).with_context(|| format!("reading {txns}"))?;
    let pos_text =
        std::fs::read_to_string(positions).with_context(|| format!("reading {positions}"))?;

    let parsed = ratio_recon::parse_transactions(&txn_text)
        .with_context(|| format!("parsing {txns}"))?;
    let reported = ratio_recon::parse_reported(&pos_text, &chart)
        .with_context(|| format!("parsing {positions}"))?;

    let scope = ratio_recon::equity_long_only_single_ccy("USD");
    let (report, entries) =
        ratio_recon::reconcile_with_entries(&parsed, &reported, &scope, &set, &chart, &digest)?;

    print!("{}", ratio_recon::render(&report));

    if post {
        // Only entries from a run that was not refused ever reach the book —
        // `reconcile_with_entries` hands back none for a refused file, so a
        // partial period cannot be written even by asking.
        if !entries.is_empty() {
            // Keep the report with the book it describes. `ratio watch` reads
            // the newest one; a report that only ever existed on somebody's
            // terminal is not evidence anybody else can look at.
            use prost::Message;
            let dir = book.join("reports");
            std::fs::create_dir_all(&dir).context("creating the reports directory")?;
            let name = format!("{}-{}.pb", digest.short(), parsed.len());
            std::fs::write(
                dir.join(&name),
                report.to_proto(&book_label(&book), &name).encode_to_vec(),
            )
            .context("storing the report")?;

            for entry in &entries {
                b.append(entry)
                    .with_context(|| format!("posting {}", entry.id))?;
            }
            println!("\nposted {} entrie(s) into {}", entries.len(), book.display());
            println!("  report   reports/{name}");
            println!("  ratio explain <account> --book {}", book.display());
        }
    }

    if let Some(path) = out {
        use prost::Message;
        let book_name = book_label(&book);
        // The run is named for the configuration that produced it, so two
        // reports over the same period under different configurations do not
        // land on the same name.
        let id = format!("{}-{}", digest.short(), parsed.len());
        std::fs::write(path, report.to_proto(&book_name, &id).encode_to_vec())
            .with_context(|| format!("writing {path}"))?;
        println!("\nwrote {path}");
    }

    std::process::exit(if !report.was_reconciled() {
        3
    } else if report.breaks.is_empty() {
        0
    } else {
        2
    });
}

/// Serve the MCP tools on stdio, for a model to call.
///
/// Note what this cannot do: promote a rule. `approve` below is a CLI command
/// and is not exposed as a tool, so a proposal becomes policy only when a
/// person runs it. See `ratio-mcp`'s module docs.
fn mcp(book: PathBuf) -> Result<()> {
    FileBook::open(&book)?; // fail here rather than mid-conversation
    let stdin = std::io::stdin();
    ratio_mcp::serve(&book, stdin.lock(), std::io::stdout())
}

/// Promote a proposal to the active configuration.
///
/// **Deliberately not an MCP tool.** This is the step a human takes, at a
/// terminal, having read the rule. The proposal is merged into the active rule
/// set rather than replacing it, so approving a fee rule does not silently
/// retire the trade rules.
fn approve(book: PathBuf, id: &str) -> Result<()> {
    print!("{}", approve_text(&book, id)?);
    Ok(())
}

/// The approval itself, returning what it would have printed.
///
/// Split out so the demo's terminal runs THIS rather than a second
/// implementation. A demo that approves by its own code path is a demo of its
/// own code path; the only thing worth showing is the command a person really
/// runs.
pub(crate) fn approve_text(book: &std::path::Path, id: &str) -> Result<String> {
    let book = book.to_path_buf();
    let mut b = FileBook::open(&book)?;
    let path = book.join("proposals").join(format!("{id}.toml"));
    let proposed = std::fs::read_to_string(&path)
        .with_context(|| format!("no proposal {id} — expected {}", path.display()))?;
    let incoming = RuleSet::from_toml(&proposed)?;

    // Re-check at approval time. The chart may have moved since the proposal
    // was made, and approving something that no longer passes would put a bad
    // template into production with a human's name on it.
    let chart = b.accounts()?;
    let findings = check(&incoming, &chart);
    let errors: Vec<_> = findings.iter().filter(|f| !f.is_question).collect();
    if !errors.is_empty() {
        let detail: Vec<String> =
            errors.iter().map(|f| format!("  x {}: {}", f.rule, f.message)).collect();
        bail!(
            "proposal {id} does not pass its checks and cannot be approved\n{}",
            detail.join("\n")
        );
    }

    let previous_text = match b.active()? {
        Some(d) => String::from_utf8_lossy(&b.get(&d)?).into_owned(),
        None => String::new(),
    };
    let mut merged = RuleSet::from_toml(&previous_text)?;
    let mut replaced = 0usize;
    for rule in incoming.rules {
        match merged.rules.iter_mut().find(|r| r.id == rule.id) {
            Some(existing) => {
                *existing = rule;
                replaced += 1;
            }
            None => merged.rules.push(rule),
        }
    }

    // ⛔ WRITE BACK THE WHOLE CONFIGURATION, NOT JUST THE RULES.
    //
    // A configuration holds the posting rules AND the mapping templates, and
    // serializing a `RuleSet` emits only `[[rule]]`. Approving one rule was
    // therefore DELETING every template in the configuration — silently, with
    // the changelog recording it as a routine approval, and the next ingest
    // reporting "0 templates" as if none had ever been declared.
    //
    // So the merge round-trips through a generic table and replaces only the
    // `rule` key. Anything else in the configuration survives, including
    // sections added after this code was written — which is the property that
    // matters, because the next one will not be templates.
    // Templates merge the same way, by id. A proposal may carry rules,
    // templates or both — the fence is the same either way, because both
    // decide how a figure is derived.
    let incoming_templates = ratio_ingest::TemplateSet::from_toml(&proposed)?;
    let mut templates = ratio_ingest::TemplateSet::from_toml(&previous_text)?;
    let mut templates_replaced = 0usize;
    for t in incoming_templates.templates {
        let problems = t.check();
        if !problems.is_empty() {
            for p in &problems {
                println!("  x {}: {p}", t.id);
            }
            bail!("template {:?} does not check and cannot be approved", t.id);
        }
        match templates.templates.iter_mut().find(|x| x.id == t.id) {
            Some(existing) => {
                *existing = t;
                templates_replaced += 1;
            }
            None => templates.templates.push(t),
        }
    }

    let toml = replace_sections(&previous_text, &merged, &templates)?;
    let digest = b.put(toml.as_bytes())?;
    b.set_active(&digest)?;

    // Record WHO did this, not just that the configuration moved.
    //
    // `config/HISTORY` already lists every digest in order, which is enough to
    // replay and not enough to audit: it cannot say whether a change was
    // approved by a person or what it was approved from. An approval is
    // exactly the moment that distinction matters, so it is written here.
    //
    // The actor comes from RATIO_ACTOR, falling back to the OS user. Neither
    // is authentication — this is a record of who ran the command on a machine
    // they were already trusted with, which is what the CLI is.
    let actor = actor_name();
    let when = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("{when}\t{actor}\tapproved\t{id}\t{}\n", digest.as_str());
    let log = book.join("CHANGELOG");
    let mut prior = std::fs::read_to_string(&log).unwrap_or_default();
    prior.push_str(&line);
    std::fs::write(&log, prior).context("recording the approval")?;

    let mut out = format!(
        "approved {id}\n  {} rule(s) now active ({replaced} replaced)\n  \
         {} template(s) now active ({templates_replaced} replaced)\n  config {}\n",
        merged.rules.len(),
        templates.templates.len(),
        digest.short()
    );
    for rule in &merged.rules {
        out.push('\n');
        out.push_str(&render(rule, &chart));
    }
    Ok(out)
}

/// Serve the Ledger gRPC API. Every posted transaction must conserve value or
/// it is rejected (FAILED_PRECONDITION).
#[tokio::main]
async fn serve() -> Result<()> {
    let addr = "127.0.0.1:50051".parse()?;
    println!("ratio: Ledger gRPC server listening on {addr}");
    tonic::transport::Server::builder()
        .add_service(LedgerServer::new(LedgerService::default()))
        .serve(addr)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minor_units_never_touch_a_float() {
        assert_eq!(minor(0), "0.00");
        assert_eq!(minor(5), "0.05");
        assert_eq!(minor(250), "2.50");
        assert_eq!(minor(-250), "-2.50");
        assert_eq!(minor(1_204_880_11), "1204880.11");
        // i64::MIN would overflow a naive `-v`; unsigned_abs does not.
        assert_eq!(minor(i64::MIN).starts_with('-'), true);
    }

    #[test]
    fn the_book_flag_is_positional_agnostic() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (p, b) = split_book_flag(&args(&["post", "e.json", "--book", "/tmp/x"])).unwrap();
        assert_eq!(p, vec!["post", "e.json"]);
        assert_eq!(b, PathBuf::from("/tmp/x"));

        let (p, b) = split_book_flag(&args(&["--book", "/tmp/y", "balance"])).unwrap();
        assert_eq!(p, vec!["balance"]);
        assert_eq!(b, PathBuf::from("/tmp/y"));
    }

    #[test]
    fn a_missing_book_directory_is_an_error_not_a_default() {
        let args = vec!["--book".to_string()];
        assert!(split_book_flag(&args).is_err());
    }

    /// A book with a chart and an empty rule set, plus a proposal on disk —
    /// the state the MCP server leaves behind after `propose_rule`.
    fn book_with_a_proposal(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ratio-approve-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        init(dir.clone()).unwrap();

        let empty = dir.join("empty.toml");
        std::fs::write(&empty, "").unwrap();
        config_set(dir.clone(), empty.to_str().unwrap()).unwrap();

        std::fs::create_dir_all(dir.join("proposals")).unwrap();
        std::fs::write(
            dir.join("proposals").join("p1.toml"),
            r#"
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
"#,
        )
        .unwrap();
        dir
    }

    fn active_rules(book: &PathBuf) -> RuleSet {
        let b = FileBook::open(book).unwrap();
        let d = b.active().unwrap().unwrap();
        RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&d).unwrap())).unwrap()
    }

    #[test]
    fn a_proposal_is_inert_until_a_person_approves_it() {
        let book = book_with_a_proposal("inert");
        assert!(
            active_rules(&book).rules.is_empty(),
            "a proposal on disk must not be active"
        );

        approve(book.clone(), "p1").unwrap();
        assert_eq!(active_rules(&book).rules.len(), 1);
    }

    #[test]
    fn approving_records_who_did_it() {
        // config/HISTORY lists digests, which is enough to replay and not
        // enough to audit. An approval is the moment that gap matters.
        let book = book_with_a_proposal("changelog");
        std::env::set_var("RATIO_ACTOR", "e.marsh");
        approve(book.clone(), "p1").unwrap();

        let log = std::fs::read_to_string(book.join("CHANGELOG")).unwrap();
        let fields: Vec<&str> = log.trim().split('\t').collect();
        assert_eq!(fields.len(), 5, "expected 5 tab-separated fields: {log:?}");
        assert!(fields[0].parse::<u64>().unwrap() > 1_700_000_000, "no timestamp");
        assert_eq!(fields[1], "e.marsh");
        assert_eq!(fields[2], "approved");
        assert_eq!(fields[3], "p1");
        assert_eq!(fields[4].len(), 64, "expected the full digest");
        std::env::remove_var("RATIO_ACTOR");
    }

    #[test]
    fn approving_a_rule_does_not_delete_the_templates_beside_it() {
        // ⛔ THE BUG THIS EXISTS FOR. A configuration holds the posting rules
        // AND the mapping templates. `approve` used to merge into a `RuleSet`
        // and serialize that, which emits only `[[rule]]` — so approving one
        // rule silently deleted every template, the changelog recorded a
        // routine approval, and the next ingest reported "0 templates" as if
        // none had ever been declared.
        let book = book_with_a_proposal("keepstemplates");
        let mut b = FileBook::open(&book).unwrap();
        let before = String::from_utf8_lossy(&b.get(&b.active().unwrap().unwrap()).unwrap())
            .into_owned();
        let with_template = format!(
            "{before}\n[[template]]\nid = \"gs_trades\"\nreads = \"csv\"\n\
             [template.fact]\nkind = \"trade\"\nreference = \"Ref\"\n",
        );
        let d = b.put(with_template.as_bytes()).unwrap();
        b.set_active(&d).unwrap();
        drop(b);

        // ⚠ Deliberately does NOT touch RATIO_ACTOR. It is a process-global,
        // these tests run in parallel, and removing it here made
        // `approving_records_who_did_it` read an empty actor. Nothing below
        // asserts on the actor, so nothing below needs to set one.
        approve(book.clone(), "p1").unwrap();

        let b = FileBook::open(&book).unwrap();
        let after = String::from_utf8_lossy(&b.get(&b.active().unwrap().unwrap()).unwrap())
            .into_owned();
        assert!(
            after.contains("gs_trades"),
            "approving a rule deleted the template beside it:\n{after}",
        );
        // …and the rule it was approving is in there too, so the fix did not
        // simply stop writing.
        let set = RuleSet::from_toml(&after).unwrap();
        assert!(!set.rules.is_empty(), "the approved rule should be in force");
    }

    #[test]
    fn approving_merges_rather_than_replaces() {
        // The trap this guards: approving a fee rule must not retire the trade
        // rules that were already live.
        let book = book_with_a_proposal("merge");
        std::fs::write(
            book.join("proposals").join("p2.toml"),
            "[[rule]]\nid = \"other\"\nkind = \"trade\"\n[[rule.posting]]\naccount = 10\nweight = 1\n[[rule.posting]]\naccount = 40\nweight = -1\n",
        )
        .unwrap();

        approve(book.clone(), "p1").unwrap();
        approve(book.clone(), "p2").unwrap();

        let ids: Vec<_> = active_rules(&book).rules.iter().map(|r| r.id.clone()).collect();
        assert!(ids.contains(&"management_fee_accrual".to_string()), "{ids:?}");
        assert!(ids.contains(&"other".to_string()), "{ids:?}");
    }

    #[test]
    fn re_approving_the_same_id_replaces_it_and_does_not_duplicate() {
        let book = book_with_a_proposal("replace");
        approve(book.clone(), "p1").unwrap();
        approve(book.clone(), "p1").unwrap();
        assert_eq!(active_rules(&book).rules.len(), 1);
    }

    #[test]
    fn a_proposal_naming_an_account_outside_the_chart_cannot_be_approved() {
        let book = book_with_a_proposal("badaccount");
        std::fs::write(
            book.join("proposals").join("bad.toml"),
            "[[rule]]\nid = \"r\"\nkind = \"trade\"\n[[rule.posting]]\naccount = 99999\nweight = 1\n[[rule.posting]]\naccount = 40\nweight = -1\n",
        )
        .unwrap();
        assert!(approve(book.clone(), "bad").is_err());
        assert!(active_rules(&book).rules.is_empty(), "a refused proposal must not go active");
    }

    #[test]
    fn approving_a_proposal_that_is_not_there_names_the_path() {
        let book = book_with_a_proposal("missing");
        let e = approve(book, "nope").unwrap_err().to_string();
        assert!(e.contains("nope"), "{e}");
    }
}
