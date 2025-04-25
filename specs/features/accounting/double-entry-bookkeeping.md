# Double-Entry Bookkeeping Feature Specification

## Overview
This document outlines the double-entry bookkeeping feature for Ratio, which forms the core accounting model of the application. Double-entry bookkeeping ensures that all transactions maintain balance across accounts, providing financial integrity and accurate reporting.

## Goals
- Implement a robust double-entry bookkeeping system
- Ensure all financial transactions maintain balance
- Provide validation to prevent accounting errors
- Support multi-currency transactions with proper handling
- Enable accurate financial reporting based on double-entry principles
- Maintain audit trail of all financial operations

## User Stories

### Core Accounting Stories
1. As a user, I want all my financial transactions to follow double-entry principles so that my books always balance
2. As a user, I want to see both sides of each transaction so that I understand the complete flow of money
3. As a user, I want validation when entering transactions to prevent accounting errors
4. As a user, I want to be warned of unbalanced transactions before they are recorded
5. As a user, I want each account to have the correct normal balance based on its account type

### Financial Management Stories
1. As a user, I want to track transfers between accounts accurately
2. As a user, I want to record income and expenses with proper accounting treatment
3. As a user, I want multi-currency transactions to be handled correctly with appropriate exchange rates
4. As a user, I want reconciliation tools to confirm my books match external statements
5. As a user, I want my reports to reflect proper accounting principles

## Feature Requirements

### Double-Entry Model

#### Core Principles
- Every transaction must have equal debits and credits
- Every account has a normal balance type (debit or credit)
- Account balances increase or decrease based on normal balance type
- Every financial event affects at least two accounts
- The accounting equation must always balance: Assets = Liabilities + Equity

#### Account Types
- **Asset Accounts**: Debit normal balance
  - Cash accounts
  - Bank accounts
  - Investment accounts
  - Accounts receivable
  - Fixed assets

- **Liability Accounts**: Credit normal balance
  - Credit cards
  - Loans
  - Mortgages
  - Accounts payable

- **Equity Accounts**: Credit normal balance
  - Owner's equity
  - Retained earnings
  - Opening balances

- **Income Accounts**: Credit normal balance
  - Salary
  - Interest income
  - Dividend income
  - Sales revenue

- **Expense Accounts**: Debit normal balance
  - Rent
  - Utilities
  - Groceries
  - Transportation

#### Account Hierarchy
- Support parent-child relationships between accounts
- Roll-up balances from child accounts to parents
- Allow filtering and reporting at different hierarchy levels
- Enforce account type consistency within hierarchies

### Transactions and Splits

#### Transaction Structure
- Each transaction is composed of two or more splits
- Each split references a specific account
- Splits are either debits or credits to their accounts
- The sum of all debits must equal the sum of all credits
- Transactions include metadata like date, description, and reference

#### Split Components
- Account reference
- Amount (value and currency)
- Debit/credit flag
- Memo or description
- Reconciliation status
- Tags or categories

#### Complex Transactions
- Support multiple-split transactions (more than two accounts)
- Handle split transactions across different currencies
- Allow transaction templates for common entry patterns
- Support bulk transaction imports with validation

### Validation System

#### Transaction Validation
- Enforce balanced debits and credits
- Validate account types and normal balances
- Check for required transaction metadata
- Verify date constraints and fiscal periods
- Ensure currency consistency or provide exchange rates

#### Transaction Status
- **Pending**: Initial state, may be unbalanced
- **Posted**: Final state, must be balanced and validated
- **Voided**: Canceled transaction, maintains audit trail
- **Reconciled**: Matched to external statement

#### Error Prevention
- Real-time validation during transaction entry
- Clear error messages for invalid transactions
- Suggestions for fixing unbalanced transactions
- Warnings for unusual transaction patterns
- Prevention of backdated changes to reconciled transactions

### Multi-Currency Support

#### Currency Handling
- Each account has a base currency
- Transactions can involve multiple currencies
- Exchange rates are recorded at transaction time
- Gains and losses from exchange rate changes are tracked

#### Currency Conversion
- Support manual entry of exchange rates
- Optionally fetch rates from external sources
- Calculate implied exchange rates for multi-currency transactions
- Generate appropriate splits for currency exchange fees

#### Balance Calculation
- Account balances are maintained in account's base currency
- Reports can convert to a common currency for consolidated view
- Historical exchange rates are used for point-in-time reporting
- Unrealized and realized gains/losses are properly tracked

### Audit and Traceability

#### Immutability Principles
- Posted transactions cannot be deleted
- Changes to posted transactions are tracked
- Voiding transactions creates audit records
- All changes maintain double-entry integrity

#### Audit Trail
- Record who created and modified each transaction
- Track when transactions are posted, modified, or voided
- Maintain history of reconciliation status changes
- Enable audit reporting across any date range

## User Interfaces

### Transaction Entry Form

```
┌─────────────────────────────────────────────────────────────────────────┐
│ New Transaction                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Date: [04/20/2025]  Status: [Pending ▼]  Reference: [Invoice #12345]   │
│                                                                          │
│  Description: [Monthly rent payment                                  ]   │
│                                                                          │
│  ┌──────┬─────────────────────┬────────────┬─────┬───────────────────┐  │
│  │ Type │ Account             │ Amount     │ Cur │ Memo              │  │
│  ├──────┼─────────────────────┼────────────┼─────┼───────────────────┤  │
│  │ DEBIT│ Expenses:Rent       │ 1,500.00   │ USD │ April Rent        │  │
│  │ CREDIT│ Assets:Checking    │ 1,500.00   │ USD │                   │  │
│  │      │                     │            │     │                   │  │
│  │      │                     │            │     │                   │  │
│  └──────┴─────────────────────┴────────────┴─────┴───────────────────┘  │
│                                                                          │
│  [ Add Split ]        Debit Total: 1,500.00    Credit Total: 1,500.00   │
│                                                                          │
│                        [ Cancel ]    [ Save ]    [ Post ]                │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Split Transaction Form

```
┌─────────────────────────────────────────────────────────────────────────┐
│ New Transaction                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Date: [04/15/2025]  Status: [Pending ▼]  Reference: [                ] │
│                                                                          │
│  Description: [Grocery shopping with household supplies             ]   │
│                                                                          │
│  ┌──────┬─────────────────────┬────────────┬─────┬───────────────────┐  │
│  │ Type │ Account             │ Amount     │ Cur │ Memo              │  │
│  ├──────┼─────────────────────┼────────────┼─────┼───────────────────┤  │
│  │ DEBIT│ Expenses:Groceries  │ 85.75      │ USD │ Weekly groceries  │  │
│  │ DEBIT│ Expenses:Household  │ 34.25      │ USD │ Cleaning supplies │  │
│  │ CREDIT│ Assets:Credit Card │ 120.00     │ USD │                   │  │
│  │      │                     │            │     │                   │  │
│  └──────┴─────────────────────┴────────────┴─────┴───────────────────┘  │
│                                                                          │
│  [ Add Split ]        Debit Total: 120.00    Credit Total: 120.00       │
│                                                                          │
│                        [ Cancel ]    [ Save ]    [ Post ]                │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Multi-Currency Transaction Form

```
┌─────────────────────────────────────────────────────────────────────────┐
│ New Transaction                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Date: [04/22/2025]  Status: [Pending ▼]  Reference: [                ] │
│                                                                          │
│  Description: [Purchase of EUR from USD account                      ]   │
│                                                                          │
│  ┌──────┬─────────────────────┬────────────┬─────┬───────────────────┐  │
│  │ Type │ Account             │ Amount     │ Cur │ Memo              │  │
│  ├──────┼─────────────────────┼────────────┼─────┼───────────────────┤  │
│  │ DEBIT│ Assets:EUR Account  │ 500.00     │ EUR │ Currency purchase │  │
│  │ CREDIT│ Assets:USD Account │ 545.00     │ USD │ @ Rate: 1.09      │  │
│  │      │                     │            │     │                   │  │
│  │      │                     │            │     │                   │  │
│  └──────┴─────────────────────┴────────────┴─────┴───────────────────┘  │
│                                                                          │
│  Exchange rate: 1 EUR = [1.09] USD                                       │
│                                                                          │
│  [ Add Split ]                                                           │
│                                                                          │
│                        [ Cancel ]    [ Save ]    [ Post ]                │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Account Register View

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Account Register: Assets:Checking                                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Balance: $2,450.67 USD                    [ New Transaction ]           │
│                                                                          │
│  ┌─────┬────────────┬───────────────────┬────────┬────────┬──────────┐  │
│  │ Date│ Reference  │ Description       │ Debit  │ Credit │ Balance  │  │
│  ├─────┼────────────┼───────────────────┼────────┼────────┼──────────┤  │
│  │4/20 │Inv #12345  │Monthly rent       │        │1,500.00│ 2,450.67 │  │
│  │4/18 │            │Grocery shopping   │        │  75.63 │ 3,950.67 │  │
│  │4/15 │DEPOSIT     │Paycheck           │2,500.00│        │ 4,026.30 │  │
│  │4/10 │#123        │Utility bill       │        │ 143.50 │ 1,526.30 │  │
│  │4/05 │            │Transfer to savings│        │ 500.00 │ 1,669.80 │  │
│  │4/01 │DEPOSIT     │Initial balance    │2,169.80│        │ 2,169.80 │  │
│  └─────┴────────────┴───────────────────┴────────┴────────┴──────────┘  │
│                                                                          │
│  ⓘ Debits increase and credits decrease the balance of asset accounts   │
│                                                                          │
│  [ Filter ▼ ]  [ Export ]  [ Reconcile ]                                 │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Workflows

### Basic Transaction Entry Workflow

1. User initiates new transaction entry
2. User selects transaction date and enters description
3. User selects first account (typically the expense or receiving account)
4. User enters amount and debit/credit type
5. System automatically creates second split with opposite debit/credit type
6. User selects second account
7. System validates that debits equal credits
8. User adds any additional details (memo, reference)
9. User saves or posts transaction
10. System performs final validation before posting
11. Transaction is recorded in the system

### Split Transaction Workflow

1. User follows basic transaction steps 1-4
2. User clicks "Add Split" to create additional splits
3. User enters account, amount, and type for each split
4. System validates transaction balance after each entry
5. System calculates remaining amount needed to balance
6. User can optionally have system create final balancing split
7. User completes transaction entry with memo and reference
8. System performs validation checks
9. User posts transaction
10. System records the multi-split transaction

### Multi-Currency Transaction Workflow

1. User initiates transaction with accounts in different currencies
2. User enters amount for first account in its base currency
3. System detects multi-currency transaction
4. System prompts for exchange rate or fetches current rate
5. System calculates equivalent amount in second currency
6. User reviews and adjusts if needed
7. System validates that transaction balances in base reporting currency
8. User adds any fees or spread as additional splits
9. System performs currency validation
10. User posts transaction
11. System records transaction with currency information

### Reconciliation Workflow

1. User initiates account reconciliation
2. User enters statement ending balance and date
3. System displays all unreconciled transactions for the account
4. User marks transactions that appear on the statement
5. System updates running reconciled balance
6. User resolves any discrepancies
7. When reconciled balance matches statement, user completes reconciliation
8. System marks transactions as reconciled
9. System records reconciliation event for audit purposes

## Technical Implementation Considerations

### Data Model
The implementation will use the data model defined in [data-model.md](../../architecture/data-model.md), particularly the core tables:
- Books
- Account Types
- Accounts
- Transactions
- Splits

### Transaction Processing
- Implement optimistic concurrency control for transaction processing
- Use database transactions to ensure ACID properties
- Apply double-entry validation as a pre-commit hook
- Calculate and update account balances efficiently

### Balance Calculation
- Use running balances for performance when possible
- Recalculate balances on-demand when needed
- Cache account balances with proper invalidation
- Support both inclusive and exclusive date range queries

### Performance Considerations
- Optimize for high transaction volume
- Use appropriate indices for common query patterns
- Consider partitioning strategies for large transaction volumes
- Implement efficient balance calculation algorithms

## Business Rules

### Double-Entry Validation
1. For each transaction, the sum of all debits must equal the sum of all credits
2. Each transaction must affect at least two accounts
3. Each transaction must have at least one debit and one credit

### Account Rules
1. Asset and expense accounts increase with debits, decrease with credits
2. Liability, equity, and income accounts increase with credits, decrease with debits
3. Balance sheet accounts carry balances forward to the next period
4. Income and expense accounts reset at the end of each fiscal period

### Currency Rules
1. Accounts have a base currency that cannot be changed if transactions exist
2. Transactions in foreign currencies must be converted to the account's base currency
3. Exchange rate gains and losses must be properly recognized
4. Multi-currency transactions must balance in both currencies

### Fiscal Period Rules
1. Posted transactions cannot be modified after a period is closed
2. System prevents posting to closed periods
3. Opening balances for a period match closing balances of the prior period
4. Income and expense account balances reset to zero at the start of each fiscal year

## Testing Requirements

### Unit Testing
- Test transaction balance validation
- Test account normal balance logic
- Test multi-currency conversion
- Test split creation and validation

### Integration Testing
- Test end-to-end transaction workflows
- Test interaction between transactions and account balances
- Test reconciliation process
- Test currency exchange functionality

### Financial Correctness Testing
- Test accounting equation balancing
- Test trial balance generation
- Test that books remain balanced after all operations
- Test proper handling of edge case transactions

## Documentation Requirements

### User Documentation
- Double-entry accounting principles
- Account type guide
- Transaction entry instructions
- Multi-currency management guide
- Reconciliation procedure

### Developer Documentation
- Double-entry validation implementation
- Balance calculation algorithms
- Currency handling approach
- Transaction processing pipeline

## Dependencies

- Authentication and authorization systems must be in place
- Data model and database schema must be implemented
- Money type implementation must be complete
- API endpoints for transaction processing must be available
- UI framework for transaction entry must be functional
