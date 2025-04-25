# Data Model Specification

## Overview
This document outlines the database schema for Ratio, a personal finance application built on double-entry bookkeeping principles. The schema is designed to ensure data integrity, enable efficient queries, and support complex financial reporting.

## Database Engine
PostgreSQL 15+ is used as the primary database engine, selected for its reliability, ACID compliance, and advanced features like JSON support, arrays, and robust indexing.

## Schema Design Principles
- **Double-Entry Integrity**: The schema enforces double-entry bookkeeping rules
- **Referential Integrity**: Foreign key constraints maintain data consistency
- **Temporal Awareness**: All financial data includes timestamps for audit trails
- **Performance Optimization**: Indexes support common query patterns
- **Extensibility**: Schema allows for future extensions without major redesigns

## Core Entities

### Books
The top-level container for a set of accounts, representing a financial entity (e.g., household, business).

```sql
CREATE TABLE books (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);
```

### Account Types
Defines the fundamental account types in double-entry bookkeeping.

```sql
CREATE TABLE account_types (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL UNIQUE,
    normal_balance CHAR(1) NOT NULL CHECK (normal_balance IN ('D', 'C')),
    description TEXT
);

-- Initial data
INSERT INTO account_types (name, normal_balance, description) VALUES
('ASSET', 'D', 'Resources owned by the entity'),
('LIABILITY', 'C', 'Obligations owed by the entity'),
('EQUITY', 'C', 'Residual interest in the assets after deducting liabilities'),
('INCOME', 'C', 'Increases in economic benefits'),
('EXPENSE', 'D', 'Decreases in economic benefits');
```

### Accounts
Individual financial accounts within a book, such as bank accounts, credit cards, or categories.

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

### Transactions
Financial events that affect multiple accounts, maintaining the double-entry balance.

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

CREATE INDEX idx_transactions_book_id ON transactions(book_id);
CREATE INDEX idx_transactions_transaction_date ON transactions(transaction_date);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_reference ON transactions(reference);
```

### Splits
Individual entries that make up a transaction, representing debits and credits to specific accounts.

```sql
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

CREATE INDEX idx_splits_transaction_id ON splits(transaction_id);
CREATE INDEX idx_splits_account_id ON splits(account_id);
CREATE INDEX idx_splits_reconciled ON splits(reconciled);
```

### Categories
Classification hierarchy for transactions, enabling more detailed reporting.

```sql
CREATE TABLE categories (
    id BIGSERIAL PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    name VARCHAR(255) NOT NULL,
    parent_id BIGINT REFERENCES categories(id),
    description TEXT,
    color VARCHAR(7),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    UNIQUE(book_id, name, parent_id)
);

CREATE INDEX idx_categories_book_id ON categories(book_id);
CREATE INDEX idx_categories_parent_id ON categories(parent_id);
```

### Scheduled Transactions
Definitions for recurring transactions like bills, subscriptions, and income.

```sql
CREATE TABLE scheduled_transactions (
    id BIGSERIAL PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    description TEXT NOT NULL,
    frequency VARCHAR(50) NOT NULL, -- DAILY, WEEKLY, MONTHLY, etc.
    frequency_config JSONB, -- Custom configuration for complex schedules
    start_date DATE NOT NULL,
    end_date DATE,
    next_due_date DATE,
    template JSONB NOT NULL, -- Template for generating actual transactions
    auto_post BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_scheduled_transactions_book_id ON scheduled_transactions(book_id);
CREATE INDEX idx_scheduled_transactions_next_due_date ON scheduled_transactions(next_due_date);
```

### Rules
Custom rules for transaction processing, categorization, and automation.

```sql
CREATE TABLE rules (
    id BIGSERIAL PRIMARY KEY,
    book_id BIGINT NOT NULL REFERENCES books(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    condition JSONB NOT NULL, -- Conditions for rule matching
    action JSONB NOT NULL, -- Actions to perform when conditions match
    priority INT NOT NULL DEFAULT 0,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_rules_book_id ON rules(book_id);
CREATE INDEX idx_rules_active ON rules(active);
```

### Attachments
Documents or receipts linked to transactions for record-keeping.

```sql
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

CREATE INDEX idx_attachments_transaction_id ON attachments(transaction_id);
```

## Entity Relationships

```
books
  ↓ 1:N
accounts ←←←←←┐
  ↓ 1:N       │
splits         │
  ↑ N:1        │
transactions   │
  ↓ 1:N        │
attachments    │
               │
categories     │
  ↓ 1:N        │
rules ─────────┘
               │
scheduled_transactions
```

## View Definitions

### Account Balances View
Provides current balances for all accounts.

```sql
CREATE VIEW v_account_balances AS
SELECT 
    a.id AS account_id,
    a.name AS account_name,
    a.book_id,
    at.name AS account_type,
    a.currency,
    COALESCE(SUM(CASE 
        WHEN s.debit_credit = 'D' THEN s.amount 
        ELSE -s.amount 
    END), 0) AS balance
FROM accounts a
JOIN account_types at ON a.account_type_id = at.id
LEFT JOIN splits s ON a.id = s.account_id
LEFT JOIN transactions t ON s.transaction_id = t.id AND t.status = 'POSTED'
WHERE a.deleted_at IS NULL
GROUP BY a.id, a.name, a.book_id, at.name, a.currency;
```

### Transaction Summary View
Summarizes transaction details with category information.

```sql
CREATE VIEW v_transaction_summary AS
SELECT 
    t.id AS transaction_id,
    t.description,
    t.transaction_date,
    t.reference,
    t.status,
    b.name AS book_name,
    (SELECT STRING_AGG(a.name, ', ') 
     FROM splits s 
     JOIN accounts a ON s.account_id = a.id 
     WHERE s.transaction_id = t.id) AS accounts,
    (SELECT SUM(CASE WHEN s.debit_credit = 'D' THEN s.amount ELSE 0 END)
     FROM splits s
     WHERE s.transaction_id = t.id) AS total_debits
FROM transactions t
JOIN books b ON t.book_id = b.id
WHERE t.deleted_at IS NULL;
```

## Migration Strategy

Migrations will be managed using `sqlx-cli` for Rust integration. The migration files will be stored in a dedicated `migrations/` directory and applied automatically during development and deployment.

Initial migration commands:

```bash
sqlx migrate add create_initial_schema
sqlx migrate run
```

## Performance Considerations

- Indexes are created for commonly queried fields
- The schema uses appropriate data types for storage efficiency
- Complex reporting queries may utilize materialized views for performance
- Large datasets will be paginated in the application layer

## Security Considerations

- Financial data is sensitive and should be encrypted at rest
- Access control should be implemented at the application layer
- Audit trails are maintained through created_at/updated_at timestamps
- Soft deletes (deleted_at) are used to preserve historical data
