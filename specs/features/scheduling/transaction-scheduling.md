# Feature Specification: Transaction Scheduling

## Overview
Transaction Scheduling is a key feature of Ratio that enables users to automate recurring financial transactions such as bills, subscriptions, and regular income sources. This feature saves time on manual data entry and ensures consistent financial tracking over time.

## User Stories
- As a user, I want to set up recurring transactions for my bills so that I don't have to manually create them each month.
- As a user, I want to define different frequency patterns (daily, weekly, monthly, yearly) for various transactions.
- As a user, I want to preview upcoming scheduled transactions so I can plan my finances.
- As a user, I want to skip or modify a specific instance of a scheduled transaction when exceptions occur.
- As a user, I want scheduled transactions to automatically post to my accounts, so my balances are always up to date.
- As a user, I want to set end dates for temporary recurring transactions, such as loan payments with a fixed term.

## Requirements

### Functional Requirements
- Support for creating, reading, updating, and deleting scheduled transactions
- Flexible scheduling options (daily, weekly, monthly, yearly)
- Custom frequency configurations (e.g., every 2 weeks, first Monday of the month)
- Template-based approach with the ability to define transaction details once
- Option to automatically post transactions when due or keep them pending for review
- Preview of upcoming scheduled transactions
- Ability to skip individual instances without affecting the schedule
- End date support for temporary recurring transactions
- Transaction generation that maintains double-entry integrity
- Support for notifications before scheduled transactions are due

### Non-functional Requirements
- Schedule generation should complete in under 500ms
- System should support at least 1000 scheduled transactions per book
- Upcoming schedule preview should load in under 1 second
- Scheduled transactions should not affect database performance
- Generated transactions should be indistinguishable from manual transactions

## Technical Approach

Transaction scheduling will be implemented as a module in the accounting kernel with a dedicated service and database tables.

### Core Components
- **Scheduled Transaction Model**: Core data structure representing recurring transaction patterns
- **Frequency Configuration**: Flexible configuration system for different schedule patterns
- **Transaction Generator**: Service to create actual transactions from schedules
- **Schedule Calculator**: Library to determine the next occurrence dates
- **Scheduling API**: gRPC service for schedule operations

### API Design

The Scheduled Transaction service will be exposed through gRPC with the following methods:

```protobuf
service ScheduledTransactionService {
  rpc CreateScheduledTransaction(CreateScheduledTransactionRequest) returns (ScheduledTransaction);
  rpc GetScheduledTransaction(GetScheduledTransactionRequest) returns (ScheduledTransaction);
  rpc ListScheduledTransactions(ListScheduledTransactionsRequest) returns (ListScheduledTransactionsResponse);
  rpc UpdateScheduledTransaction(UpdateScheduledTransactionRequest) returns (ScheduledTransaction);
  rpc DeleteScheduledTransaction(DeleteScheduledTransactionRequest) returns (Empty);
  rpc GenerateInstancesForPeriod(GenerateInstancesRequest) returns (GenerateInstancesResponse);
  rpc GetUpcomingInstances(GetUpcomingInstancesRequest) returns (GetUpcomingInstancesResponse);
  rpc CreateInstanceNow(CreateInstanceNowRequest) returns (Transaction);
  rpc SkipNextInstance(SkipNextInstanceRequest) returns (ScheduledTransaction);
}
```

### Data Model Changes

Scheduled transactions will be stored in the PostgreSQL database with the following schema:

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

## Implementation Details

### Frequency Configurations

The system will support various frequency types through a flexible configuration system:

```rust
enum FrequencyType {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

struct DailyConfig {
    every_n_days: u32,
}

struct WeeklyConfig {
    every_n_weeks: u32,
    days_of_week: Vec<u8>,  // 1-7, where 1 is Monday
}

struct MonthlyConfig {
    every_n_months: u32,
    day_selection: DaySelection,
}

enum DaySelection {
    DayOfMonth(u8),  // 1-31
    RelativeDay { position: Position, day_of_week: u8 },
}

enum Position {
    First,
    Second,
    Third,
    Fourth,
    Last,
}

struct YearlyConfig {
    every_n_years: u32,
    month: u8,  // 1-12
    day_selection: DaySelection,
}
```

### Transaction Template

The transaction template will contain all necessary information to generate a complete transaction:

```rust
struct TransactionTemplate {
    description: String,
    reference: Option<String>,
    splits: Vec<SplitTemplate>,
    metadata: HashMap<String, String>,
}

struct SplitTemplate {
    account_id: i64,
    amount: Decimal,
    debit_credit: DebitCredit,
    memo: Option<String>,
}
```

### Next Due Date Calculation

The system will use a scheduling algorithm to determine the next occurrence based on the frequency configuration:

```rust
fn calculate_next_due_date(
    frequency: &FrequencyType,
    config: &FrequencyConfig,
    last_date: Date,
) -> Date {
    match frequency {
        FrequencyType::Daily => {
            last_date + Duration::days(config.daily.every_n_days as i64)
        },
        FrequencyType::Weekly => {
            // Calculate next weekly occurrence
            // ...
        },
        FrequencyType::Monthly => {
            // Calculate next monthly occurrence
            // ...
        },
        FrequencyType::Yearly => {
            // Calculate next yearly occurrence
            // ...
        },
    }
}
```

### Transaction Generation Process

1. A scheduled job runs daily to identify due transactions
2. For each due scheduled transaction:
   - Create a new transaction based on the template
   - Set the transaction date to the due date
   - Update the next_due_date field in the scheduled transaction
   - If auto_post is true, automatically post the transaction
3. For advanced users, manual generation for specific periods will be available

## Dependencies
- **Transaction Module**: Generated transactions are regular transactions
- **Account Module**: Transactions affect account balances
- **Book Module**: Scheduled transactions belong to a book
- **Notification System**: For alerting users about upcoming transactions

## Acceptance Criteria
- [ ] Users can create scheduled transactions with various frequency patterns
- [ ] Scheduled transactions correctly generate actual transactions on their due dates
- [ ] Transaction generation maintains double-entry integrity
- [ ] Users can preview upcoming scheduled transactions for financial planning
- [ ] Users can modify, skip, or delete individual instances when needed
- [ ] Auto-posting works correctly for transactions marked as automatic
- [ ] End dates properly terminate recurring schedules
- [ ] Schedules with complex patterns (e.g., "first Monday of the month") work correctly

## Out of Scope
- Integration with external calendar systems
- Bill detection from email (Phase 2 feature)
- Subscription management and tracking (separate feature)
- Mobile notifications for upcoming transactions (Phase 3 feature)

## Open Questions
- How should we handle scheduled transactions when the due date falls on a weekend or holiday?
- Should we implement a "fuzzy date" feature for transactions that don't have an exact due date?
- How do we handle changes in currency exchange rates for scheduled transactions with multiple currencies?
- Should we support retroactive changes to scheduled transactions?
