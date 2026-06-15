//! ratio-api — the gRPC `Ledger` service over the kernel.
//!
//! The generated `ratio.v1` wire messages (//proto:ratio_rust_proto, crate
//! `ratio_proto`) are converted to the Lean-authored kernel records; a
//! transaction is admitted only if it conserves value — its postings net to
//! zero (`ratio_kernel::transaction_is_balanced`), the kernel's sole invariant
//! (research.tex). `CreateTransaction` rejects the rest with FAILED_PRECONDITION.
//!
//! tonic here is the SAME crate the generated service links (//proto's custom
//! prost toolchain points both at ratio's `@crates`), so the trait impl unifies.

use std::sync::Mutex;

use ratio_kernel::{Posting, Transaction};
use ratio_proto::ratio::v1 as pb;
use ratio_proto::ratio::v1::ledger_server::Ledger;

/// Convert a wire posting to the kernel record.
pub fn posting_from_wire(p: &pb::Posting) -> Posting {
    Posting { dim: p.dim, amount: p.amount }
}

/// Convert a wire transaction to the kernel record (the vector of postings).
pub fn transaction_from_wire(t: &pb::Transaction) -> Transaction {
    Transaction {
        postings: t.postings.iter().map(posting_from_wire).collect(),
    }
}

/// A transaction is admissible iff it conserves value — its postings net to zero
/// across every dimension (research.tex). The Ledger admits only these.
pub fn is_admissible(t: &pb::Transaction) -> bool {
    ratio_kernel::transaction_is_balanced(&transaction_from_wire(t))
}

/// In-memory `Ledger` — the append-only transaction log (research.tex). The
/// conservation invariant is enforced on every post; reads are exact replays.
#[derive(Default)]
pub struct LedgerService {
    log: Mutex<Vec<pb::Transaction>>,
}

#[tonic::async_trait]
impl Ledger for LedgerService {
    async fn get_transaction(
        &self,
        request: tonic::Request<pb::GetTransactionRequest>,
    ) -> Result<tonic::Response<pb::Transaction>, tonic::Status> {
        let name = request.into_inner().name;
        let log = self.log.lock().unwrap();
        log.iter()
            .find(|t| t.name == name)
            .cloned()
            .map(tonic::Response::new)
            .ok_or_else(|| tonic::Status::not_found(format!("transaction {name} not found")))
    }

    async fn list_transactions(
        &self,
        request: tonic::Request<pb::ListTransactionsRequest>,
    ) -> Result<tonic::Response<pb::ListTransactionsResponse>, tonic::Status> {
        let prefix = format!("{}/transactions/", request.into_inner().parent);
        let log = self.log.lock().unwrap();
        let transactions = log
            .iter()
            .filter(|t| t.name.starts_with(&prefix))
            .cloned()
            .collect();
        Ok(tonic::Response::new(pb::ListTransactionsResponse {
            transactions,
            next_page_token: String::new(),
        }))
    }

    async fn create_transaction(
        &self,
        request: tonic::Request<pb::CreateTransactionRequest>,
    ) -> Result<tonic::Response<pb::Transaction>, tonic::Status> {
        let req = request.into_inner();
        let mut txn = req
            .transaction
            .ok_or_else(|| tonic::Status::invalid_argument("transaction is required"))?;

        // The kernel's sole invariant: conservation. Reject anything that doesn't
        // net to zero across every dimension.
        if !is_admissible(&txn) {
            return Err(tonic::Status::failed_precondition(
                "transaction does not conserve value: postings must net to zero",
            ));
        }

        let mut log = self.log.lock().unwrap();
        let id = if req.transaction_id.is_empty() {
            format!("txn-{}", log.len())
        } else {
            req.transaction_id
        };
        txn.name = format!("{}/transactions/{}", req.parent, id);
        log.push(txn.clone());
        Ok(tonic::Response::new(txn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(postings: Vec<pb::Posting>) -> pb::Transaction {
        pb::Transaction { postings, ..Default::default() }
    }

    #[test]
    fn balanced_two_leg_is_admissible() {
        let t = wire(vec![
            pb::Posting { dim: 0, amount: 500 },
            pb::Posting { dim: 0, amount: -500 },
        ]);
        assert!(is_admissible(&t));
    }

    #[test]
    fn unbalanced_is_rejected() {
        assert!(!is_admissible(&wire(vec![pb::Posting { dim: 0, amount: 500 }])));
    }

    #[test]
    fn conversion_preserves_postings() {
        let t = transaction_from_wire(&wire(vec![pb::Posting { dim: 7, amount: 42 }]));
        assert_eq!(t.postings, vec![Posting { dim: 7, amount: 42 }]);
    }

    fn create_req(parent: &str, postings: Vec<pb::Posting>) -> tonic::Request<pb::CreateTransactionRequest> {
        tonic::Request::new(pb::CreateTransactionRequest {
            parent: parent.to_string(),
            transaction: Some(wire(postings)),
            transaction_id: String::new(),
        })
    }

    #[tokio::test]
    async fn create_admits_balanced_then_get_round_trips() {
        let svc = LedgerService::default();
        let posted = svc
            .create_transaction(create_req(
                "books/main",
                vec![
                    pb::Posting { dim: 0, amount: 500 },
                    pb::Posting { dim: 0, amount: -500 },
                ],
            ))
            .await
            .expect("balanced transaction is admitted")
            .into_inner();
        assert_eq!(posted.name, "books/main/transactions/txn-0");

        let got = svc
            .get_transaction(tonic::Request::new(pb::GetTransactionRequest { name: posted.name.clone() }))
            .await
            .expect("posted transaction is retrievable")
            .into_inner();
        assert_eq!(got.name, posted.name);
        assert_eq!(got.postings.len(), 2);
    }

    #[tokio::test]
    async fn create_rejects_unbalanced_with_failed_precondition() {
        let svc = LedgerService::default();
        let err = svc
            .create_transaction(create_req("books/main", vec![pb::Posting { dim: 0, amount: 500 }]))
            .await
            .expect_err("unbalanced transaction is rejected");
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    }
}
