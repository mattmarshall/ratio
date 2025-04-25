# Feature Specification: Transaction Management

## Overview
Transaction Management is a core feature of Ratio that enables users to record, categorize, and manage all financial transactions while ensuring double-entry bookkeeping integrity. This feature forms the foundation of the accounting system, tracking the flow of money between accounts.

## User Stories
- As a user, I want to record transactions between my accounts so that I can track the movement of money.
- As a user, I want to categorize transactions so that I can analyze my spending patterns.
- As a user, I want transactions to maintain double-entry integrity so that my books always balance.
- As a user, I want to attach receipts or documents to transactions so that I can maintain proper records.
- As a user, I want to search and filter transactions based on various criteria so that I can find specific entries.
- As a user, I want to edit or void transactions when necessary while maintaining an audit trail.

## Requirements

### Functional Requirements
- Support for creating, reading, updating, and voiding transactions
- Double-entry validation ensuring debits and credits balance for each transaction
- Transaction categorization with hierarchical categories
- Document attachment support for receipts and supporting files
- Transaction status tracking (pending, posted, voided)
- Search and filtering capabilities by date, amount, account, category, etc.
- Memo and description fields for detailed record-keeping
- Support for split transactions (multiple debit or credit entries)
- Batch transaction operations for efficient data entry
- Transaction templates for frequently used entries
- Audit trail for all transaction modifications

### Non-functional Requirements
- Transaction creation should complete in under 200ms
- System should handle at least 100,000 transactions per book
- Transaction search should return results in under 500ms
- All transaction data must be encrypted at rest
- Transaction processing must be atomic to prevent partial updates

## Technical Approach

Transaction management will be implemented as a core module in the Rust accounting kernel, with double-entry validation enforced at the service layer.

### Core Components
- **Transaction Model**: Core data structure representing financial transactions
- **Split Model**: Represents individual debit/credit entries within a transaction
- **Transaction Service**: Business logic for transaction operations and validation
- **Transaction Repository**: Data access layer for transaction persistence
- **Transaction API**: gRPC service for transaction operations

### API Design

The Transaction service will be exposed through gRPC with the following methods:

```protobuf
service TransactionService {
  rpc CreateTransaction(CreateTransactionRequest) returns (Transaction);
  rpc GetTransaction(GetTransactionRequest) returns (Transaction);
  rpc ListTransactions(ListTransactionsRequest) returns (ListTransactionsResponse);
  rpc UpdateTransaction(UpdateTransactionRequest) returns (Transaction);
  rpc DeleteTransaction(DeleteTransactionRequest) returns (Empty);
  rpc PostTransaction(PostTransactionRequest) returns (Transaction);
  rpc VoidTransaction(VoidTransactionRequest) returns (Transaction);
  rpc AttachDocument(AttachDocumentRequest) returns (Attachment);
  rpc GetAttachment(GetAttachmentRequest) returns (Attachment);
}
```

### Data Model Changes

Transactions will be stored in the PostgreSQL database with the following schema:

```sql
CREATE TABLE transactions (
    id BIGSERIAL PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    transaction_date DATE NOT NULL,
    post_date TIMESTAMP WITH TIME ZONE,
    description TEXT,
    reference VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING' CHECK (status IN ('PENDING', 'POSTED', 'VOIDED')),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE splits (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT NOT NULL REFERENCES transactions(id),
    account_id BIGINT NOT NULL REFERENCES accounts(id),
    amount DECIMAL(19, 4) NOT NULL,
    debit_credit CHAR(1) NOT NULL CHECK (debit_credit IN ('D', 'C')),
    memo TEXT,
    reconciled BOOLEAN NOT NULL DEFAULT false,
    reconciled_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE attachments (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BIGINT REFERENCES transactions(id),
    name VARCHAR(255) NOT NULL,
    file_path TEXT NOT NULL,
    file_type VARCHAR(50),
    file_size BIGINT,
    content_hash VARCHAR(64),
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);
```

## Implementation Details

### Double-Entry Validation
Each transaction must satisfy the accounting equation: the sum of all debits must equal the sum of all credits. This validation will be enforced at the service layer:

```rust
fn validate_transaction_balance(splits: &[Split]) -> Result<(), ValidationError> {
    let total_debits: Decimal = splits
        .iter()
        .filter(|s| s.debit_credit == 'D')
        .map(|s| s.amount)
        .sum();
        
    let total_credits: Decimal = splits
        .iter()
        .filter(|s| s.debit_credit == 'C')
        .map(|s| s.amount)
        .sum();
        
    if total_debits != total_credits {
        return Err(ValidationError::UnbalancedTransaction { 
            debits: total_debits, 
            credits: total_credits 
        });
    }
    
    Ok(())
}
```

### Transaction Lifecycle
Transactions follow a defined lifecycle:
1. **Created**: Transaction is recorded but not yet posted
2. **Posted**: Transaction is committed and affects account balances
3. **Voided**: Transaction is invalidated but maintained for audit purposes

### Attachment Storage
Transaction attachments will be stored in the filesystem with metadata in the database:
- File paths will be relative to a configurable base directory
- Content hashes will be computed for file integrity validation
- File types will be restricted to safe formats

## Dependencies
- **Account Module**: Transactions affect account balances
- **Category Module**: Transactions can be categorized
- **Book Module**: Transactions belong to a financial "book"
- **Rules Engine**: For automated transaction processing

## Acceptance Criteria
- [ ] Users can successfully create, view, update, and void transactions
- [ ] Double-entry integrity is enforced with proper validation
- [ ] Transaction splits correctly update account balances when posted
- [ ] Attachments can be uploaded, downloaded, and viewed
- [ ] Transaction search returns accurate results within performance targets
- [ ] Transaction voiding properly reverses account balance effects
- [ ] Batch operations function correctly for multiple transactions
- [ ] Audit trail correctly tracks all modifications

## Out of Scope
- Auto-categorization of transactions (will be addressed in a separate feature)
- Bank statement import (separate feature)
- OCR receipt scanning (Phase 2 feature)
- Mobile app synchronization (Phase 3 feature)

## Open Questions
- How should we handle transactions that span multiple currencies?
- Should we implement a more granular approvals workflow for transactions?
- What's the retention policy for transaction data and attachments?
- Should void transactions automatically create reversing entries?
