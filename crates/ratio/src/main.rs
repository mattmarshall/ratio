//! ratio — the binary entry point. Subcommands: `server` (the Ledger gRPC
//! server over the conservation kernel) + a default demo of the Lean-emitted
//! Money API.

use anyhow::Result;
use ratio_api::LedgerService;
use ratio_kernel::{money_add, money_is_zero, usd_zero, Currency, Money};
use ratio_proto::ratio::v1::ledger_server::LedgerServer;

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("server") => serve().await,
        _ => {
            demo();
            Ok(())
        }
    }
}

/// Serve the Ledger gRPC API (research.tex: the kernel exposed over gRPC). Every
/// posted transaction must conserve value or it's rejected (FAILED_PRECONDITION).
async fn serve() -> Result<()> {
    let addr = "127.0.0.1:50051".parse()?;
    println!("ratio: Ledger gRPC server listening on {addr}");
    tonic::transport::Server::builder()
        .add_service(LedgerServer::new(LedgerService::default()))
        .serve(addr)
        .await?;
    Ok(())
}

/// Smoke of the Lean-authored Money API (default subcommand).
fn demo() {
    let z = usd_zero();
    let sum = money_add(z, Money { minor: 250, currency: Currency::Usd });
    println!(
        "ratio {} — zero={:?} is_zero={} | zero + 2.50 = {:?}",
        env!("CARGO_PKG_VERSION"),
        z,
        money_is_zero(z),
        sum,
    );
}
