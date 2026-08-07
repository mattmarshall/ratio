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
  ratio post FILE [--book DIR]         post entries; refuses any that do not balance
  ratio balance [--book DIR]           print the trial balance
  ratio server                         serve the Ledger gRPC API

The book defaults to ./book, or $RATIO_BOOK if set.
";

fn main() -> Result<()> {
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
        ["post", file] => post(book, file),
        ["balance"] => balance(book),
        ["server"] => serve(),
        other => {
            eprint!("{USAGE}");
            bail!("unrecognised command: {}", other.join(" "));
        }
    }
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

fn init(book: PathBuf) -> Result<()> {
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
            acct(40, "Management fee payable", AccountTypeRecord::Liability),
        ])?;
    }
    if b.active()?.is_none() {
        // An empty configuration is still a configuration: entries posted now
        // are attributable, and the digest changes the moment a rule is added.
        let digest = b.put(b"# ratio configuration\nrules = []\n")?;
        b.set_active(&digest)?;
    }
    println!("initialised book at {}", book.display());
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
fn minor(v: i64) -> String {
    let neg = v < 0;
    let a = v.unsigned_abs();
    let s = format!("{}.{:02}", a / 100, a % 100);
    if neg {
        format!("-{s}")
    } else {
        s
    }
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
}
