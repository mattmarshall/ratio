//! ratio-api — the kernel API: `ratio.v1.Ledger` and `ratio.v1.Chart` over a
//! book of record, served over gRPC and — from the same implementations — REST.
//!
//! The generated `ratio.v1` wire messages (//proto:ratio_rust_proto, crate
//! `ratio_proto`) are converted to the store's records and posted through
//! [`ratio_store::Journal::append`], which is the door: an entry that does not
//! conserve value on every dimension never reaches the record, and the client
//! is told FAILED_PRECONDITION. There is no second check here that could
//! disagree with the one the CLI, the MCP server and the console all go through.
//!
//! ⛔ ONE BOOK PER PROCESS. `ratio server --book DIR` serves that directory as
//! `books/<dirname>`, the key the store files the journal under. A request that
//! names any other book is NOT_FOUND. A root of many books is a different
//! program (the console's), and guessing a directory from a URL segment is how
//! a request for one client's book would be answered from another's.
//!
//! ⚠ READS STREAM THE JOURNAL. `GetTransaction` and `ListTransactions` walk the
//! log with [`Journal::for_each_entry_since`], so a lookup is O(journal), not
//! O(1). That is the right cost for the book of record; indexed lookup is what
//! `RATIO_PG_URL` and the projection crate are for, and this crate does not
//! grow a second index that could disagree with them.
//!
//! tonic here is the SAME crate the generated service links (//proto's custom
//! prost toolchain points both at ratio's `@crates`), so the trait impls unify.
//! The REST side (`rest.rs`) rides tonic's own axum, so one `axum::serve` carries
//! both: h2c for gRPC, HTTP/1.1 or HTTP/2 for JSON, on one port.

pub mod json;
pub mod rest;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ratio_chart::normal_side;
use ratio_kernel::{Posting, Transaction};
use ratio_proto::ratio::v1 as pb;
use ratio_proto::ratio::v1::chart_server::{Chart, ChartServer};
use ratio_proto::ratio::v1::ledger_server::{Ledger, LedgerServer};
use ratio_store::{
    Account, AccountTypeRecord, ConfigStore, FileBook, Journal, JournalEntry, PostingRecord,
};
use tonic::{Request, Response, Status};

/// Convert a wire posting to the kernel record.
pub fn posting_from_wire(p: &pb::Posting) -> Posting {
    Posting { dim: p.dim, amount: p.amount }
}

/// Convert a wire transaction to the kernel record (the vector of postings).
pub fn transaction_from_wire(t: &pb::Transaction) -> Transaction {
    Transaction { postings: t.postings.iter().map(posting_from_wire).collect() }
}

/// A transaction is admissible iff it conserves value — its postings net to zero
/// across every dimension (research.tex). The Ledger admits only these.
///
/// ⚠ THE KERNEL'S ONE-DIMENSIONAL CHECK. The store's door additionally nets
/// every currency group on its own (`JournalEntry::is_balanced`), which is
/// what `[USD +100, EUR −100]` fails. This is the sum; that is the law.
pub fn is_admissible(t: &pb::Transaction) -> bool {
    ratio_kernel::transaction_is_balanced(&transaction_from_wire(t))
}

/// Wire posting → the record the journal stores. An empty currency or
/// instrument is absence, not a name.
fn record_from_wire(p: &pb::Posting) -> PostingRecord {
    PostingRecord {
        dim: p.dim,
        amount: p.amount,
        currency: p.currency_code.clone().filter(|c| !c.is_empty()),
        instrument: p.instrument.clone().filter(|i| !i.is_empty()),
        quantity: p.quantity,
    }
}

fn wire_from_record(p: &PostingRecord) -> pb::Posting {
    pb::Posting {
        dim: p.dim,
        amount: p.amount,
        currency_code: p.currency.clone(),
        instrument: p.instrument.clone(),
        quantity: p.quantity,
    }
}

fn wire_from_entry(book: &str, e: &JournalEntry) -> pb::Transaction {
    pb::Transaction {
        name: format!("books/{book}/transactions/{}", e.id),
        postings: e.postings.iter().map(wire_from_record).collect(),
        control_plane_hash: e.config.to_string(),
    }
}

fn wire_account_type(t: AccountTypeRecord) -> pb::AccountType {
    match t {
        AccountTypeRecord::Asset => pb::AccountType::Asset,
        AccountTypeRecord::Liability => pb::AccountType::Liability,
        AccountTypeRecord::Equity => pb::AccountType::Equity,
        AccountTypeRecord::Income => pb::AccountType::Income,
        AccountTypeRecord::Expense => pb::AccountType::Expense,
    }
}

fn record_account_type(t: i32) -> Result<AccountTypeRecord, Status> {
    match pb::AccountType::try_from(t) {
        Ok(pb::AccountType::Asset) => Ok(AccountTypeRecord::Asset),
        Ok(pb::AccountType::Liability) => Ok(AccountTypeRecord::Liability),
        Ok(pb::AccountType::Equity) => Ok(AccountTypeRecord::Equity),
        Ok(pb::AccountType::Income) => Ok(AccountTypeRecord::Income),
        Ok(pb::AccountType::Expense) => Ok(AccountTypeRecord::Expense),
        _ => Err(Status::invalid_argument(
            "account.account_type must be one of ASSET, LIABILITY, EQUITY, INCOME, EXPENSE",
        )),
    }
}

fn wire_account(book: &str, a: &Account) -> pb::Account {
    // Derived, never stored: the proved function of the classification.
    let side = match normal_side(a.account_type.into()) {
        ratio_chart::Side::Debit => pb::Side::Debit,
        ratio_chart::Side::Credit => pb::Side::Credit,
    };
    pb::Account {
        name: format!("books/{book}/accounts/{}", a.dim),
        dim: a.dim,
        display_name: a.display_name.clone(),
        account_type: wire_account_type(a.account_type) as i32,
        normal_side: side as i32,
    }
}

/// The store's key for a book: its directory name. Same rule as
/// `ratio_store::book_key`, so `books/<key>` here is the prefix the journal
/// lives under there.
fn book_key(root: &Path) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("book")
        .to_string()
}

struct Inner {
    book: FileBook,
    /// Every entry id in the journal, so a client id that already exists is
    /// ALREADY_EXISTS and a generated one never collides. Loaded once at open
    /// by streaming the log; one string per entry, not one entry per entry.
    ids: BTreeSet<String>,
    count: u64,
}

/// The one book this process serves, behind the lock the journal append needs.
///
/// `std::sync::Mutex` on purpose: nothing awaits while holding it, and the
/// store's append is a file write that a tokio mutex would only make slower.
pub struct Book {
    key: String,
    root: PathBuf,
    inner: Mutex<Inner>,
}

impl Book {
    /// Open the book at `root` and index its entry ids.
    pub fn open(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let book = FileBook::open(&root)?;
        let mut ids = BTreeSet::new();
        let mut count = 0u64;
        book.for_each_entry_since(0, &mut |e| {
            ids.insert(e.id.clone());
            count += 1;
            Ok(())
        })?;
        Ok(Book { key: book_key(&root), root, inner: Mutex::new(Inner { book, ids, count }) })
    }

    /// The directory this book is kept in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<dirname>` — the segment after `books/`.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// `books/<dirname>` — the resource name every child name is under.
    pub fn resource(&self) -> String {
        format!("books/{}", self.key)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, Status> {
        self.inner
            .lock()
            .map_err(|_| Status::internal("the book's lock was poisoned by an earlier panic"))
    }

    /// `books/{book}` → `()`, or why not.
    fn check_parent(&self, parent: &str) -> Result<(), Status> {
        match parent.split('/').collect::<Vec<_>>().as_slice() {
            ["books", key] if !key.is_empty() => self.check_key(key),
            _ => Err(Status::invalid_argument(format!(
                "parent must be books/{{book}}, got {parent:?}"
            ))),
        }
    }

    /// `books/{book}/{collection}/{id}` → `id`, or why not.
    fn check_name<'a>(&self, name: &'a str, collection: &str) -> Result<&'a str, Status> {
        match name.split('/').collect::<Vec<_>>().as_slice() {
            ["books", key, c, id] if *c == collection && !key.is_empty() && !id.is_empty() => {
                self.check_key(key)?;
                Ok(*id)
            }
            _ => Err(Status::invalid_argument(format!(
                "name must be books/{{book}}/{collection}/{{id}}, got {name:?}"
            ))),
        }
    }

    /// `books/{book}/{singleton}` → `()`, or why not.
    fn check_singleton(&self, name: &str, singleton: &str) -> Result<(), Status> {
        match name.split('/').collect::<Vec<_>>().as_slice() {
            ["books", key, s] if *s == singleton && !key.is_empty() => self.check_key(key),
            _ => Err(Status::invalid_argument(format!(
                "name must be books/{{book}}/{singleton}, got {name:?}"
            ))),
        }
    }

    fn check_key(&self, key: &str) -> Result<(), Status> {
        if key == self.key {
            Ok(())
        } else {
            Err(Status::not_found(format!(
                "books/{key} is not served here; this process serves {}",
                self.resource()
            )))
        }
    }

    fn active_digest(inner: &Inner) -> Result<Option<String>, Status> {
        inner
            .book
            .active()
            .map(|d| d.map(|d| d.to_string()))
            .map_err(|e| Status::internal(format!("reading the active configuration: {e}")))
    }
}

/// AIP-158 page size: 0 is the default, negative is refused, large is capped.
fn page_size(requested: i32, default: usize, max: usize) -> Result<usize, Status> {
    match requested {
        n if n < 0 => Err(Status::invalid_argument("page_size must not be negative")),
        0 => Ok(default),
        n => Ok((n as usize).min(max)),
    }
}

/// Page tokens are the ordinal of the first entry on the page, in decimal.
/// Opaque to clients by contract, plain here so a token is checkable by a person.
fn page_offset(token: &str) -> Result<usize, Status> {
    if token.is_empty() {
        return Ok(0);
    }
    token
        .parse::<usize>()
        .map_err(|_| Status::invalid_argument(format!("page_token {token:?} was not issued by this server")))
}

/// `ratio.v1.Ledger` over a [`Book`].
#[derive(Clone)]
pub struct LedgerService(pub Arc<Book>);

#[tonic::async_trait]
impl Ledger for LedgerService {
    async fn get_transaction(
        &self,
        request: Request<pb::GetTransactionRequest>,
    ) -> Result<Response<pb::Transaction>, Status> {
        let name = request.into_inner().name;
        let id = self.0.check_name(&name, "transactions")?;
        let inner = self.0.lock()?;
        let mut found = None;
        inner
            .book
            .for_each_entry_since(0, &mut |e| {
                if found.is_none() && e.id == id {
                    found = Some(wire_from_entry(&self.0.key, e));
                }
                Ok(())
            })
            .map_err(|e| Status::internal(format!("reading the journal: {e}")))?;
        found
            .map(Response::new)
            .ok_or_else(|| Status::not_found(format!("{name} is not in the journal")))
    }

    async fn list_transactions(
        &self,
        request: Request<pb::ListTransactionsRequest>,
    ) -> Result<Response<pb::ListTransactionsResponse>, Status> {
        let req = request.into_inner();
        self.0.check_parent(&req.parent)?;
        let size = page_size(req.page_size, 50, 1000)?;
        let offset = page_offset(&req.page_token)?;
        let inner = self.0.lock()?;
        let mut transactions = Vec::new();
        let mut ordinal = 0usize;
        let mut more = false;
        inner
            .book
            .for_each_entry_since(0, &mut |e| {
                if ordinal >= offset {
                    if transactions.len() < size {
                        transactions.push(wire_from_entry(&self.0.key, e));
                    } else {
                        more = true;
                    }
                }
                ordinal += 1;
                Ok(())
            })
            .map_err(|e| Status::internal(format!("reading the journal: {e}")))?;
        let next_page_token = if more { (offset + size).to_string() } else { String::new() };
        Ok(Response::new(pb::ListTransactionsResponse { transactions, next_page_token }))
    }

    async fn create_transaction(
        &self,
        request: Request<pb::CreateTransactionRequest>,
    ) -> Result<Response<pb::Transaction>, Status> {
        let req = request.into_inner();
        self.0.check_parent(&req.parent)?;
        let txn = req
            .transaction
            .ok_or_else(|| Status::invalid_argument("transaction is required"))?;
        if txn.postings.is_empty() {
            return Err(Status::invalid_argument("transaction.postings must not be empty"));
        }
        if req.transaction_id.contains('/') {
            return Err(Status::invalid_argument("transaction_id must not contain '/'"));
        }

        let mut inner = self.0.lock()?;
        // ⛔ PROVENANCE FIRST. An entry names the configuration it was posted
        // under; with none promoted there is nothing to name, and a client that
        // pinned a digest is told when the rules have moved rather than posted
        // under ones it did not expect.
        let active = inner.book.active().map_err(|e| Status::internal(e.to_string()))?;
        let active = active.ok_or_else(|| {
            Status::failed_precondition("no configuration promoted — run `ratio init` or `ratio config set`")
        })?;
        if !txn.control_plane_hash.is_empty() && txn.control_plane_hash != active.to_string() {
            return Err(Status::failed_precondition(format!(
                "control_plane_hash {} is not the active configuration ({})",
                txn.control_plane_hash,
                active.short()
            )));
        }

        let id = if req.transaction_id.is_empty() {
            let mut n = inner.count;
            loop {
                let candidate = format!("txn-{n}");
                if !inner.ids.contains(&candidate) {
                    break candidate;
                }
                n += 1;
            }
        } else {
            if inner.ids.contains(&req.transaction_id) {
                return Err(Status::already_exists(format!(
                    "books/{}/transactions/{} already exists",
                    self.0.key, req.transaction_id
                )));
            }
            req.transaction_id
        };

        let entry = JournalEntry {
            id,
            memo: String::new(),
            config: active,
            postings: txn.postings.iter().map(record_from_wire).collect(),
            trade_date: None,
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
            kind: None,
        };
        // The door. Whatever the store refuses — an entry that does not
        // conserve value, a closed period — is a precondition the client can
        // read about in the message, not an internal error.
        inner
            .book
            .append(&entry)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        inner.ids.insert(entry.id.clone());
        inner.count += 1;
        Ok(Response::new(wire_from_entry(&self.0.key, &entry)))
    }
}

/// `ratio.v1.Chart` over a [`Book`].
#[derive(Clone)]
pub struct ChartService(pub Arc<Book>);

impl ChartService {
    fn accounts(&self, inner: &Inner) -> Result<Vec<Account>, Status> {
        let mut accounts = inner
            .book
            .accounts()
            .map_err(|e| Status::internal(format!("reading the chart: {e}")))?;
        accounts.sort_by_key(|a| a.dim);
        Ok(accounts)
    }
}

#[tonic::async_trait]
impl Chart for ChartService {
    async fn get_account(
        &self,
        request: Request<pb::GetAccountRequest>,
    ) -> Result<Response<pb::Account>, Status> {
        let name = request.into_inner().name;
        let id = self.0.check_name(&name, "accounts")?;
        let dim: i64 = id
            .parse()
            .map_err(|_| Status::invalid_argument(format!("account id {id:?} is not a dimension")))?;
        let inner = self.0.lock()?;
        self.accounts(&inner)?
            .iter()
            .find(|a| a.dim == dim)
            .map(|a| Response::new(wire_account(&self.0.key, a)))
            .ok_or_else(|| Status::not_found(format!("{name} is not in the chart")))
    }

    async fn list_accounts(
        &self,
        request: Request<pb::ListAccountsRequest>,
    ) -> Result<Response<pb::ListAccountsResponse>, Status> {
        let req = request.into_inner();
        self.0.check_parent(&req.parent)?;
        let size = page_size(req.page_size, 100, 1000)?;
        let offset = page_offset(&req.page_token)?;
        let inner = self.0.lock()?;
        let all = self.accounts(&inner)?;
        let accounts: Vec<pb::Account> = all
            .iter()
            .skip(offset)
            .take(size)
            .map(|a| wire_account(&self.0.key, a))
            .collect();
        let next_page_token =
            if offset + size < all.len() { (offset + size).to_string() } else { String::new() };
        Ok(Response::new(pb::ListAccountsResponse { accounts, next_page_token }))
    }

    async fn create_account(
        &self,
        request: Request<pb::CreateAccountRequest>,
    ) -> Result<Response<pb::Account>, Status> {
        let req = request.into_inner();
        self.0.check_parent(&req.parent)?;
        let account = req.account.ok_or_else(|| Status::invalid_argument("account is required"))?;
        if account.display_name.trim().is_empty() {
            return Err(Status::invalid_argument("account.display_name is required"));
        }
        // The id IS the dimension: `accounts/{dim}`. An explicit account_id
        // that says otherwise is two answers to one question.
        if !req.account_id.is_empty() && req.account_id != account.dim.to_string() {
            return Err(Status::invalid_argument(format!(
                "account_id {:?} does not name account.dim {}",
                req.account_id, account.dim
            )));
        }
        let record = Account {
            dim: account.dim,
            display_name: account.display_name.trim().to_string(),
            account_type: record_account_type(account.account_type)?,
        };
        let mut inner = self.0.lock()?;
        let mut all = self.accounts(&inner)?;
        if all.iter().any(|a| a.dim == record.dim) {
            return Err(Status::already_exists(format!(
                "books/{}/accounts/{} already exists",
                self.0.key, record.dim
            )));
        }
        all.push(record.clone());
        inner
            .book
            .put_accounts(&all)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        Ok(Response::new(wire_account(&self.0.key, &record)))
    }

    async fn get_trial_balance(
        &self,
        request: Request<pb::GetTrialBalanceRequest>,
    ) -> Result<Response<pb::TrialBalance>, Status> {
        let name = request.into_inner().name;
        self.0.check_singleton(&name, "trialBalance")?;
        let inner = self.0.lock()?;
        let tb = inner
            .book
            .trial_balance()
            .map_err(|e| Status::internal(format!("folding the trial balance: {e}")))?;
        let difference = i64::try_from(tb.debits as i128 - tb.credits as i128)
            .map_err(|_| Status::internal("the trial balance difference does not fit in 64 bits"))?;
        let rows = inner
            .book
            .balances_by_dim()
            .map_err(|e| Status::internal(format!("folding balances by account: {e}")))?;
        let account_balances = rows
            .iter()
            .map(|((dim, currency), (debits, credits))| pb::AccountBalance {
                account: format!("books/{}/accounts/{dim}", self.0.key),
                debits: *debits,
                credits: *credits,
                currency_code: currency.clone(),
            })
            .collect();
        Ok(Response::new(pb::TrialBalance {
            name,
            debits: tb.debits,
            credits: tb.credits,
            difference,
            account_balances,
            control_plane_hash: Book::active_digest(&inner)?.unwrap_or_default(),
        }))
    }
}

/// Everything the process serves over one book: the gRPC services and the
/// REST routes, as one router.
///
/// ⭐ ONE FALLBACK FOR BOTH PROTOCOLS. tonic's router answers an unknown path
/// with grpc-status UNIMPLEMENTED in an HTTP 200, which is right for a gRPC
/// client and invisible to a REST one. [`rest::fallback`] looks at the
/// content type and gives each what it can read.
pub fn app(book: Arc<Book>) -> axum::Router {
    let grpc = tonic::service::Routes::new(LedgerServer::new(LedgerService(book.clone())))
        .add_service(ChartServer::new(ChartService(book.clone())))
        .into_axum_router();
    grpc.merge(rest::router(book)).fallback(rest::fallback)
}

/// Bind `addr` and serve [`app`] on it until the process ends. hyper's auto
/// connection builder takes HTTP/2 prior knowledge (what a tonic client sends
/// over `http://`) and HTTP/1.1 (what `curl` sends) on the same socket.
pub async fn serve(addr: SocketAddr, book: Arc<Book>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(book)).await?;
    Ok(())
}

/// Serve on a listener that is already bound — for a test that wants the
/// ephemeral port back.
pub async fn serve_on(listener: tokio::net::TcpListener, book: Arc<Book>) -> anyhow::Result<()> {
    axum::serve(listener, app(book)).await?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// A fresh book under the system temp dir with one promoted configuration
    /// and a two-account chart, so an append has provenance to name.
    pub fn scratch_book(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ratio-api-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut b = FileBook::open(&dir).unwrap();
        let digest = b.put(b"# ratio-api test configuration\n").unwrap();
        b.set_active(&digest).unwrap();
        b.put_accounts(&[
            Account {
                dim: 1,
                display_name: "Investments at fair value".into(),
                account_type: AccountTypeRecord::Asset,
            },
            Account {
                dim: 2,
                display_name: "Cash and equivalents".into(),
                account_type: AccountTypeRecord::Asset,
            },
        ])
        .unwrap();
        dir
    }

    pub fn posting(dim: i64, amount: i64) -> pb::Posting {
        pb::Posting { dim, amount, currency_code: None, instrument: None, quantity: None }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{posting, scratch_book};
    use super::*;

    fn wire(postings: Vec<pb::Posting>) -> pb::Transaction {
        pb::Transaction { postings, ..Default::default() }
    }

    fn create_req(parent: &str, postings: Vec<pb::Posting>) -> Request<pb::CreateTransactionRequest> {
        Request::new(pb::CreateTransactionRequest {
            parent: parent.to_string(),
            transaction: Some(wire(postings)),
            transaction_id: String::new(),
        })
    }

    #[test]
    fn balanced_two_leg_is_admissible() {
        assert!(is_admissible(&wire(vec![posting(0, 500), posting(0, -500)])));
    }

    #[test]
    fn unbalanced_is_rejected() {
        assert!(!is_admissible(&wire(vec![posting(0, 500)])));
    }

    #[test]
    fn conversion_preserves_postings() {
        let t = transaction_from_wire(&wire(vec![posting(7, 42)]));
        assert_eq!(t.postings, vec![Posting { dim: 7, amount: 42 }]);
    }

    #[tokio::test]
    async fn create_admits_balanced_then_get_round_trips() {
        let book = Arc::new(Book::open(scratch_book("round-trip")).unwrap());
        let parent = book.resource();
        let svc = LedgerService(book.clone());
        let posted = svc
            .create_transaction(create_req(&parent, vec![posting(2, -500), posting(1, 500)]))
            .await
            .expect("balanced transaction is admitted")
            .into_inner();
        assert_eq!(posted.name, format!("{parent}/transactions/txn-0"));
        assert!(!posted.control_plane_hash.is_empty(), "the entry names its configuration");

        let got = svc
            .get_transaction(Request::new(pb::GetTransactionRequest { name: posted.name.clone() }))
            .await
            .expect("posted transaction is retrievable")
            .into_inner();
        assert_eq!(got, posted);

        // And it is in the journal the CLI reads, not a shadow of it.
        let entries = FileBook::open(book.root()).unwrap().entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "txn-0");
    }

    #[tokio::test]
    async fn create_rejects_unbalanced_with_failed_precondition() {
        let book = Arc::new(Book::open(scratch_book("unbalanced")).unwrap());
        let svc = LedgerService(book.clone());
        let err = svc
            .create_transaction(create_req(&book.resource(), vec![posting(1, 500)]))
            .await
            .expect_err("unbalanced transaction is rejected");
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
        assert!(err.message().contains("does not conserve value"), "{}", err.message());
        assert!(FileBook::open(book.root()).unwrap().entries().unwrap().is_empty());
    }

    #[tokio::test]
    async fn two_currencies_that_each_net_to_zero_are_admitted_and_one_that_does_not_is_refused() {
        let book = Arc::new(Book::open(scratch_book("currencies")).unwrap());
        let svc = LedgerService(book.clone());
        let usd = |dim, amount| pb::Posting { currency_code: Some("USD".into()), ..posting(dim, amount) };
        let eur = |dim, amount| pb::Posting { currency_code: Some("EUR".into()), ..posting(dim, amount) };
        svc.create_transaction(create_req(&book.resource(), vec![usd(2, -100), usd(1, 100), eur(2, -7), eur(1, 7)]))
            .await
            .expect("each currency nets to zero");
        // The sum nets to zero and neither currency does — the exact entry
        // `Ratio.Chart.Dimensions` names.
        let err = svc
            .create_transaction(create_req(&book.resource(), vec![usd(1, 100), eur(2, -100)]))
            .await
            .expect_err("a sum across currencies is not conservation");
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn a_client_id_is_kept_and_a_second_use_is_already_exists() {
        let book = Arc::new(Book::open(scratch_book("ids")).unwrap());
        let svc = LedgerService(book.clone());
        let req = || {
            Request::new(pb::CreateTransactionRequest {
                parent: book.resource(),
                transaction: Some(wire(vec![posting(2, -1), posting(1, 1)])),
                transaction_id: "trade-7".into(),
            })
        };
        let posted = svc.create_transaction(req()).await.unwrap().into_inner();
        assert!(posted.name.ends_with("/transactions/trade-7"));
        assert_eq!(svc.create_transaction(req()).await.unwrap_err().code(), tonic::Code::AlreadyExists);
    }

    #[tokio::test]
    async fn a_stale_control_plane_hash_is_refused() {
        let book = Arc::new(Book::open(scratch_book("stale-hash")).unwrap());
        let svc = LedgerService(book.clone());
        let mut txn = wire(vec![posting(2, -1), posting(1, 1)]);
        txn.control_plane_hash = "0".repeat(64);
        let err = svc
            .create_transaction(Request::new(pb::CreateTransactionRequest {
                parent: book.resource(),
                transaction: Some(txn),
                transaction_id: String::new(),
            }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    }

    #[tokio::test]
    async fn another_book_is_not_found_and_a_malformed_name_is_invalid() {
        let book = Arc::new(Book::open(scratch_book("names")).unwrap());
        let svc = LedgerService(book.clone());
        let err = svc
            .create_transaction(create_req("books/somebody-else", vec![posting(2, -1), posting(1, 1)]))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
        let err = svc
            .get_transaction(Request::new(pb::GetTransactionRequest { name: "transactions/x".into() }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn list_pages_through_the_journal_in_posting_order() {
        let book = Arc::new(Book::open(scratch_book("paging")).unwrap());
        let svc = LedgerService(book.clone());
        for _ in 0..5 {
            svc.create_transaction(create_req(&book.resource(), vec![posting(2, -1), posting(1, 1)]))
                .await
                .unwrap();
        }
        let page = |token: &str| {
            Request::new(pb::ListTransactionsRequest {
                parent: book.resource(),
                page_size: 2,
                page_token: token.to_string(),
            })
        };
        let first = svc.list_transactions(page("")).await.unwrap().into_inner();
        assert_eq!(first.transactions.iter().map(|t| t.name.rsplit('/').next().unwrap()).collect::<Vec<_>>(), ["txn-0", "txn-1"]);
        assert_eq!(first.next_page_token, "2");
        let last = svc.list_transactions(page("4")).await.unwrap().into_inner();
        assert_eq!(last.transactions.len(), 1);
        assert_eq!(last.next_page_token, "");
    }

    #[tokio::test]
    async fn the_chart_reads_creates_and_ties() {
        let book = Arc::new(Book::open(scratch_book("chart")).unwrap());
        let ledger = LedgerService(book.clone());
        let chart = ChartService(book.clone());
        let parent = book.resource();

        let listed = chart
            .list_accounts(Request::new(pb::ListAccountsRequest { parent: parent.clone(), page_size: 0, page_token: String::new() }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(listed.accounts.len(), 2);
        assert_eq!(listed.accounts[0].name, format!("{parent}/accounts/1"));
        assert_eq!(listed.accounts[0].normal_side, pb::Side::Debit as i32, "an asset is debit-normal, by theorem");

        let created = chart
            .create_account(Request::new(pb::CreateAccountRequest {
                parent: parent.clone(),
                account: Some(pb::Account {
                    dim: 20,
                    display_name: "Capital contributions".into(),
                    account_type: pb::AccountType::Equity as i32,
                    ..Default::default()
                }),
                account_id: String::new(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(created.normal_side, pb::Side::Credit as i32);
        let again = chart
            .get_account(Request::new(pb::GetAccountRequest { name: created.name.clone() }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(again, created);

        ledger
            .create_transaction(create_req(&parent, vec![posting(20, -125_000), posting(2, 125_000)]))
            .await
            .unwrap();
        let tb = chart
            .get_trial_balance(Request::new(pb::GetTrialBalanceRequest { name: format!("{parent}/trialBalance") }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!((tb.debits, tb.credits, tb.difference), (125_000, 125_000, 0));
        assert_eq!(tb.account_balances.len(), 2);
        assert!(!tb.control_plane_hash.is_empty());
    }
}
