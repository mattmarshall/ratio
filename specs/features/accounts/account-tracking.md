# Feature Specification: Account Tracking

## Overview
Account Tracking is a core feature that allows users to maintain a comprehensive record of all their financial accounts in a unified interface. This includes checking accounts, savings accounts, investment accounts, and other financial instruments.

## User Stories
- As a household financial manager, I want to add all my family's bank accounts so that I can see our complete financial picture in one place.
- As a user, I want to categorize my accounts by type so that I can better organize our financial structure.
- As a user, I want to see the current balance of each account so that I know how much money we have.
- As a user, I want to set up parent-child relationships between accounts so that I can model hierarchical account structures.
- As a user, I want to track accounts in different currencies so that I can manage international finances.

## Requirements

### Functional Requirements
- Support for creating, reading, updating, and deleting accounts
- Support for multiple account types (checking, savings, investment, etc.)
- Ability to assign accounts to categories or groups
- Support for hierarchical account structures (parent-child relationships)
- Multi-currency support with automatic conversion based on exchange rates
- Balance calculation that reflects pending and cleared transactions
- Account reconciliation tools to match statements with tracked transactions
- Account history and activity tracking

### Non-functional Requirements
- Account operations should complete in under 100ms
- Support for at least 1000 accounts per user
- All account data must be encrypted at rest
- Account operations must be transactional (ACID compliant)

## Technical Approach

Account tracking will be implemented as a core module in the Rust accounting kernel, with a gRPC API layer for client-server communication.

### Core Components
- **Account Model**: Core data structure representing financial accounts
- **Account Service**: Business logic for account operations
- **Account Repository**: Data access layer for account persistence
- **Account API**: gRPC service for account operations

### API Design

The Account service will be exposed through gRPC with the following methods:

```protobuf
service AccountService {
  rpc CreateAccount(CreateAccountRequest) returns (Account);
  rpc GetAccount(GetAccountRequest) returns (Account);
  rpc ListAccounts(ListAccountsRequest) returns (ListAccountsResponse);
  rpc UpdateAccount(UpdateAccountRequest) returns (Account);
  rpc DeleteAccount(DeleteAccountRequest) returns (Empty);
  rpc ReconcileAccount(ReconcileAccountRequest) returns (ReconciliationResult);
  rpc GetAccountBalance(GetAccountBalanceRequest) returns (AccountBalance);
  rpc GetAccountHistory(GetAccountHistoryRequest) returns (stream AccountHistoryEntry);
}
```

### Data Model Changes

Accounts will be stored in the PostgreSQL database with the following schema:

```sql
CREATE TABLE accounts (
    id BIGSERIAL PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    account_type_id INT NOT NULL REFERENCES account_types(id),
    name VARCHAR(255) NOT NULL,
    code VARCHAR(50),
    description TEXT,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    parent_id BIGINT REFERENCES accounts(id),
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    UNIQUE(book_id, code),
    UNIQUE(book_id, name, parent_id)
);

CREATE INDEX idx_accounts_book_id ON accounts(book_id);
CREATE INDEX idx_accounts_parent_id ON accounts(parent_id);
CREATE INDEX idx_accounts_account_type_id ON accounts(account_type_id);
```

## Dependencies
- **Transaction Module**: Accounts need to track transactions that affect their balance
- **Currency Module**: For supporting multiple currencies and conversion
- **Book Module**: Accounts belong to a financial "book" that represents a household or business entity

## Acceptance Criteria
- [ ] Users can successfully create, view, update, and delete accounts
- [ ] Account hierarchy (parent-child relationships) functions correctly
- [ ] Balances correctly reflect all transactions affecting the account
- [ ] Multi-currency support works with accurate conversion rates
- [ ] All account operations are properly secured with appropriate authentication
- [ ] Account listings and details load within performance targets

## Out of Scope
- Integration with external bank APIs (will be addressed in a future feature)
- Automatic transaction categorization (separate feature)
- Investment portfolio analytics (separate feature)
- Mobile app synchronization (Phase 3 feature)

## Open Questions
- Should we support account merging/splitting operations?
- How will we handle dormant accounts vs. closed accounts?
- What level of detail should we track for investment accounts?
- Should we implement soft delete or hard delete for accounts?
