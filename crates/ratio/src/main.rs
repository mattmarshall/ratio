//! ratio — the binary entry point (git-style subcommands: account / transaction
//! / report / schedule / server). Phase-0 stub: links every crate + anyhow so
//! the full graph (incl. crate_universe external deps) builds and runs.

use anyhow::Result;

fn main() -> Result<()> {
    println!(
        "ratio {} — {} / {} / {} / {}",
        env!("CARGO_PKG_VERSION"),
        ratio_common::CRATE,
        ratio_kernel::CRATE,
        ratio_api::CRATE,
        ratio_tui::CRATE,
    );
    Ok(())
}
