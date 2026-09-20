//! ratio-client — the Rust SDK for the kernel API.
//!
//! A typed wrapper over the generated tonic clients in
//! [`ratio_proto::ratio::v1`], scoped to one book, so posting looks like this:
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use ratio_client::{posting, Client};
//!
//! let mut ratio = Client::connect("http://127.0.0.1:50051", "fund-1").await?;
//! let txn = ratio.post(vec![posting(2, -125_000), posting(1, 125_000)]).await?;
//! println!("{}", txn.name);            // books/fund-1/transactions/txn-0
//! let tb = ratio.trial_balance().await?;
//! assert_eq!(tb.difference, 0);        // by theorem, not by check
//! # Ok(()) }
//! ```
//!
//! An entry that does not conserve value comes back as
//! [`tonic::Status`] with [`tonic::Code::FailedPrecondition`] and the store's
//! own message; nothing reaches the journal. That is the whole API.
//!
//! gRPC only, on purpose. Rust callers have the generated types and a real
//! HTTP/2 client already; the REST binding exists for the languages that do
//! not, and is the same service either way.

pub use ratio_proto::ratio::v1 as pb;
pub use tonic::{Code, Status};

use pb::chart_client::ChartClient;
use pb::ledger_client::LedgerClient;
use tonic::transport::{Channel, Endpoint};

/// A posting with no currency, instrument or quantity: the book's untyped
/// group. Set the optional fields on the result for the typed ones.
pub fn posting(dim: i64, amount: i64) -> pb::Posting {
    pb::Posting { dim, amount, currency_code: None, instrument: None, quantity: None }
}

/// A posting denominated in `currency` — its own conservation law.
pub fn posting_in(dim: i64, amount: i64, currency: &str) -> pb::Posting {
    pb::Posting { currency_code: Some(currency.to_string()), ..posting(dim, amount) }
}

/// One book, over one channel.
#[derive(Clone, Debug)]
pub struct Client {
    ledger: LedgerClient<Channel>,
    chart: ChartClient<Channel>,
    book: String,
}

impl Client {
    /// Connect to `endpoint` (`http://host:port`, h2c) for `books/{book}`.
    pub async fn connect(
        endpoint: impl Into<String>,
        book: impl Into<String>,
    ) -> Result<Self, tonic::transport::Error> {
        let channel = Endpoint::try_from(endpoint.into())?.connect().await?;
        Ok(Self::over(channel, book))
    }

    /// The same, over a channel the caller built (TLS, timeouts, a lazy
    /// connection).
    pub fn over(channel: Channel, book: impl Into<String>) -> Self {
        Client { ledger: LedgerClient::new(channel.clone()), chart: ChartClient::new(channel), book: book.into() }
    }

    /// `books/{book}`.
    pub fn parent(&self) -> String {
        format!("books/{}", self.book)
    }

    /// Post a transaction with a server-assigned id.
    pub async fn post(&mut self, postings: Vec<pb::Posting>) -> Result<pb::Transaction, Status> {
        self.post_transaction(String::new(), postings, String::new()).await
    }

    /// Post a transaction under a client-assigned id; a second post with the
    /// same id is ALREADY_EXISTS.
    pub async fn post_with_id(&mut self, id: &str, postings: Vec<pb::Posting>) -> Result<pb::Transaction, Status> {
        self.post_transaction(id.to_string(), postings, String::new()).await
    }

    /// Post, pinned to a configuration digest: refused (FAILED_PRECONDITION) if
    /// the book's active configuration is a different one.
    pub async fn post_under(
        &mut self,
        control_plane_hash: &str,
        postings: Vec<pb::Posting>,
    ) -> Result<pb::Transaction, Status> {
        self.post_transaction(String::new(), postings, control_plane_hash.to_string()).await
    }

    async fn post_transaction(
        &mut self,
        transaction_id: String,
        postings: Vec<pb::Posting>,
        control_plane_hash: String,
    ) -> Result<pb::Transaction, Status> {
        let req = pb::CreateTransactionRequest {
            parent: self.parent(),
            transaction: Some(pb::Transaction { name: String::new(), postings, control_plane_hash }),
            transaction_id,
        };
        Ok(self.ledger.create_transaction(req).await?.into_inner())
    }

    /// One transaction by id.
    pub async fn transaction(&mut self, id: &str) -> Result<pb::Transaction, Status> {
        let name = format!("{}/transactions/{id}", self.parent());
        Ok(self.ledger.get_transaction(pb::GetTransactionRequest { name }).await?.into_inner())
    }

    /// One page of the journal, in posting order. `page_size` 0 is the
    /// server's default; the response carries the next token or an empty one.
    pub async fn transactions(&mut self, page_size: i32, page_token: &str) -> Result<pb::ListTransactionsResponse, Status> {
        let req = pb::ListTransactionsRequest { parent: self.parent(), page_size, page_token: page_token.to_string() };
        Ok(self.ledger.list_transactions(req).await?.into_inner())
    }

    /// Every account in the chart, by dimension.
    pub async fn accounts(&mut self) -> Result<Vec<pb::Account>, Status> {
        let mut out = Vec::new();
        let mut page_token = String::new();
        loop {
            let req = pb::ListAccountsRequest { parent: self.parent(), page_size: 0, page_token };
            let page = self.chart.list_accounts(req).await?.into_inner();
            out.extend(page.accounts);
            if page.next_page_token.is_empty() {
                return Ok(out);
            }
            page_token = page.next_page_token;
        }
    }

    /// One account by dimension.
    pub async fn account(&mut self, dim: i64) -> Result<pb::Account, Status> {
        let name = format!("{}/accounts/{dim}", self.parent());
        Ok(self.chart.get_account(pb::GetAccountRequest { name }).await?.into_inner())
    }

    /// Add an account to the chart. The normal side comes back derived.
    pub async fn create_account(
        &mut self,
        dim: i64,
        display_name: &str,
        account_type: pb::AccountType,
    ) -> Result<pb::Account, Status> {
        let req = pb::CreateAccountRequest {
            parent: self.parent(),
            account: Some(pb::Account {
                dim,
                display_name: display_name.to_string(),
                account_type: account_type as i32,
                ..Default::default()
            }),
            account_id: String::new(),
        };
        Ok(self.chart.create_account(req).await?.into_inner())
    }

    /// The trial balance: totals per side, the difference (zero for any book
    /// the kernel admitted), and a row per (account, currency).
    pub async fn trial_balance(&mut self) -> Result<pb::TrialBalance, Status> {
        let name = format!("{}/trialBalance", self.parent());
        Ok(self.chart.get_trial_balance(pb::GetTrialBalanceRequest { name }).await?.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::{Account, AccountTypeRecord, ConfigStore, FileBook};
    use std::sync::Arc;

    /// A book with a promoted configuration and a two-account chart, served on
    /// an ephemeral port by the real server.
    async fn serve_scratch(name: &str) -> (String, String) {
        let dir = std::env::temp_dir().join(format!("ratio-client-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut b = FileBook::open(&dir).unwrap();
        let digest = b.put(b"# ratio-client test configuration\n").unwrap();
        b.set_active(&digest).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: AccountTypeRecord::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: AccountTypeRecord::Asset },
        ])
        .unwrap();
        let book = Arc::new(ratio_api::Book::open(&dir).unwrap());
        let key = book.key().to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { ratio_api::serve_on(listener, book).await.unwrap() });
        (format!("http://{addr}"), key)
    }

    #[tokio::test]
    async fn post_read_and_tie_over_a_real_socket() {
        let (endpoint, key) = serve_scratch("e2e").await;
        let mut ratio = Client::connect(endpoint, key.clone()).await.unwrap();

        let txn = ratio.post(vec![posting(2, -125_000), posting(1, 125_000)]).await.unwrap();
        assert_eq!(txn.name, format!("books/{key}/transactions/txn-0"));
        assert_eq!(ratio.transaction("txn-0").await.unwrap(), txn);

        let err = ratio.post(vec![posting(2, -125_000), posting(1, 124_999)]).await.unwrap_err();
        assert_eq!(err.code(), Code::FailedPrecondition);
        assert!(err.message().contains("does not conserve value"));

        let page = ratio.transactions(10, "").await.unwrap();
        assert_eq!(page.transactions.len(), 1, "the refused entry never reached the journal");

        let equity = ratio.create_account(20, "Capital contributions", pb::AccountType::Equity).await.unwrap();
        assert_eq!(equity.normal_side, pb::Side::Credit as i32);
        assert_eq!(ratio.accounts().await.unwrap().len(), 3);

        let tb = ratio.trial_balance().await.unwrap();
        assert_eq!((tb.debits, tb.credits, tb.difference), (125_000, 125_000, 0));

        let pinned = ratio.post_under(&txn.control_plane_hash, vec![posting_in(2, -1, "USD"), posting_in(1, 1, "USD")]).await.unwrap();
        assert_eq!(pinned.postings[0].currency_code.as_deref(), Some("USD"));
        let moved = ratio.post_under(&"0".repeat(64), vec![posting(2, -1), posting(1, 1)]).await.unwrap_err();
        assert_eq!(moved.code(), Code::FailedPrecondition);
    }
}
