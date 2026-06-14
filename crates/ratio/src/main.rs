//! ratio — the binary entry point (git-style subcommands: account / transaction
//! / report / schedule / server). Phase-1 stub: exercises the Lean-authored
//! Money API so the full graph builds + runs.

use anyhow::Result;
use ratio_kernel::{money_add, money_is_zero, usd_zero, Currency, Money};

fn main() -> Result<()> {
    let z = usd_zero();
    let sum = money_add(z, Money { minor: 250, currency: Currency::Usd });
    println!(
        "ratio {} — zero={:?} is_zero={} | zero + 2.50 = {:?}",
        env!("CARGO_PKG_VERSION"),
        z,
        money_is_zero(z),
        sum,
    );
    Ok(())
}
