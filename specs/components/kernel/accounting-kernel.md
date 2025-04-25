# Component Specification: Accounting Kernel

## Overview
The Accounting Kernel is the core engine of Ratio, responsible for maintaining the integrity of the double-entry bookkeeping system. It manages books, accounts, entries, and transactions, ensuring that all accounting rules are properly enforced while providing a stable API for higher-level components.

## Responsibilities
- Maintain the integrity of the double-entry bookkeeping system
- Process and validate all financial transactions
- Calculate account balances and financial positions
- Enforce business rules and constraints on financial operations
- Manage the lifecycle of financial entities (books, accounts, transactions)
- Provide a stable API for higher-level components
- Coordinate between specialized sub-components

## Design
The Accounting Kernel follows a domain-driven design approach with a clear separation of concerns. It is implemented in Rust for performance, safety, and reliability.

### Architecture Pattern
- **Layered Architecture**: The kernel is structured in layers with clear boundaries
- **Repository Pattern**: Data access is abstracted through repositories
- **Service Layer**: Business logic is organized into domain services
- **Event-driven**: Important state changes are communicated through events
- **Extension Point Pattern**: Well-defined hooks allow for extensions

### Sub-Components
The Accounting Kernel is composed of several specialized sub-components:

1. **[Money Handling](money-handling.md)**: Provides precise financial calculations with currency support
2. **[Extension System](extension-system.md)**: Enables extensibility through hooks and Python integration
3. **Transaction Management**: Core transaction processing logic
4. **Account Management**: Account hierarchy and balance tracking
5. **Validation Engine**: Ensures financial data integrity

### Key Abstractions

#### Domain Models
```rust
/// A financial book representing a household, business, or other entity
pub struct Book {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub default_currency: Rc<Currency>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An account within a book, with a specific type
pub struct Account {
    pub id: i64,
    pub book_id: i64,
    pub name: String,
    pub account_type: AccountType,
    pub currency: String,
    pub code: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The fundamental account types in double-entry bookkeeping
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}

/// A financial transaction representing a set of splits
pub struct Transaction {
    pub id: i64,
    pub book_id: i64,
    pub transaction_date: NaiveDate,
    pub post_date: Option<DateTime<Utc>>,
    pub description: String,
    pub reference: Option<String>,
    pub status: TransactionStatus,
    pub splits: Vec<Split>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The status of a transaction
pub enum TransactionStatus {
    Pending,
    Posted,
    Voided,
}

/// An individual entry within a transaction
pub struct Split {
    pub id: i64,
    pub transaction_id: i64,
    pub account_id: i64,
    pub money: Money,  // Using Money type from money-handling.md
    pub debit_credit: DebitCredit,
    pub memo: Option<String>,
    pub reconciled: bool,
    pub reconciled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Debit or Credit indicator for a split
pub enum DebitCredit {
    Debit,
    Credit,
}
```

#### Services
```rust
/// Service for managing books
pub trait BookService {
    /// Create a new book
    async fn create_book(&self, book: NewBook) -> Result<Book, Error>;
    
    /// Get a book by ID
    async fn get_book(&self, id: i64) -> Result<Book, Error>;
    
    /// List all books
    async fn list_books(&self, pagination: Pagination) -> Result<(Vec<Book>, PageInfo), Error>;
    
    /// Update a book
    async fn update_book(&self, id: i64, book: UpdateBook) -> Result<Book, Error>;
    
    /// Delete a book
    async fn delete_book(&self, id: i64) -> Result<(), Error>;
}

/// Service for managing accounts
pub trait AccountService {
    /// Create a new account
    async fn create_account(&self, account: NewAccount) -> Result<Account, Error>;
    
    /// Get an account by ID
    async fn get_account(&self, id: i64) -> Result<Account, Error>;
    
    /// List accounts in a book
    async fn list_accounts(&self, book_id: i64, filters: AccountFilters, pagination: Pagination) -> Result<(Vec<Account>, PageInfo), Error>;
    
    /// Update an account
    async fn update_account(&self, id: i64, account: UpdateAccount) -> Result<Account, Error>;
    
    /// Delete an account
    async fn delete_account(&self, id: i64) -> Result<(), Error>;
    
    /// Get the balance of an account as of a specific date
    async fn get_account_balance(&self, id: i64, as_of: Option<DateTime<Utc>>) -> Result<AccountBalance, Error>;
    
    /// Reconcile an account with a statement
    async fn reconcile_account(&self, id: i64, reconciliation: AccountReconciliation) -> Result<ReconciliationResult, Error>;
}

/// Service for managing transactions
pub trait TransactionService {
    /// Create a new transaction
    async fn create_transaction(&self, transaction: NewTransaction) -> Result<Transaction, Error>;
    
    /// Get a transaction by ID
    async fn get_transaction(&self, id: i64) -> Result<Transaction, Error>;
    
    /// List transactions in a book
    async fn list_transactions(&self, book_id: i64, filters: TransactionFilters, pagination: Pagination) -> Result<(Vec<Transaction>, PageInfo), Error>;
    
    /// Update a transaction
    async fn update_transaction(&self, id: i64, transaction: UpdateTransaction) -> Result<Transaction, Error>;
    
    /// Delete a transaction
    async fn delete_transaction(&self, id: i64) -> Result<(), Error>;
    
    /// Post a transaction
    async fn post_transaction(&self, id: i64) -> Result<Transaction, Error>;
    
    /// Void a transaction
    async fn void_transaction(&self, id: i64, reason: String) -> Result<Transaction, Error>;
    
    /// Validate a transaction for double-entry balance
    fn validate_transaction(&self, transaction: &Transaction) -> Result<(), ValidationError>;
}
```

#### Repositories
```rust
/// Repository for book persistence
pub trait BookRepository {
    async fn create(&self, book: &NewBook) -> Result<Book, Error>;
    async fn find_by_id(&self, id: i64) -> Result<Book, Error>;
    async fn find_all(&self, pagination: &Pagination) -> Result<(Vec<Book>, PageInfo), Error>;
    async fn update(&self, id: i64, book: &UpdateBook) -> Result<Book, Error>;
    async fn delete(&self, id: i64) -> Result<(), Error>;
}

/// Repository for account persistence
pub trait AccountRepository {
    async fn create(&self, account: &NewAccount) -> Result<Account, Error>;
    async fn find_by_id(&self, id: i64) -> Result<Account, Error>;
    async fn find_by_book(&self, book_id: i64, filters: &AccountFilters, pagination: &Pagination) -> Result<(Vec<Account>, PageInfo), Error>;
    async fn update(&self, id: i64, account: &UpdateAccount) -> Result<Account, Error>;
    async fn delete(&self, id: i64) -> Result<(), Error>;
}

/// Repository for transaction persistence
pub trait TransactionRepository {
    async fn create(&self, transaction: &NewTransaction) -> Result<Transaction, Error>;
    async fn find_by_id(&self, id: i64) -> Result<Transaction, Error>;
    async fn find_by_book(&self, book_id: i64, filters: &TransactionFilters, pagination: &Pagination) -> Result<(Vec<Transaction>, PageInfo), Error>;
    async fn update(&self, id: i64, transaction: &UpdateTransaction) -> Result<Transaction, Error>;
    async fn delete(&self, id: i64) -> Result<(), Error>;
}
```

## Implementation Details

### Double-Entry Validation
The kernel enforces double-entry integrity through validation rules:

```rust
impl TransactionService for DefaultTransactionService {
    fn validate_transaction(&self, transaction: &Transaction) -> Result<(), ValidationError> {
        // Ensure transaction has at least two splits
        if transaction.splits.len() < 2 {
            return Err(ValidationError::InsufficientSplits);
        }
        
        // Group splits by currency
        let mut currency_groups: HashMap<String, Vec<&Split>> = HashMap::new();
        for split in &transaction.splits {
            let currency_code = split.money.currency().code.clone();
            currency_groups.entry(currency_code).or_default().push(split);
        }
        
        // Validate each currency group separately
        for (currency, splits) in currency_groups {
            let money_service = self.money_service_provider.get_service();
            
            // Calculate total debits and credits
            let mut total_debits = Money::zero(money_service.get_currency(&currency).unwrap());
            let mut total_credits = Money::zero(money_service.get_currency(&currency).unwrap());
            
            for split in splits {
                match split.debit_credit {
                    DebitCredit::Debit => total_debits = total_debits.add(&split.money).unwrap(),
                    DebitCredit::Credit => total_credits = total_credits.add(&split.money).unwrap(),
                }
            }
            
            // Ensure debits equal credits for each currency
            if !total_debits.equals(&total_credits) {
                return Err(ValidationError::UnbalancedTransaction { 
                    currency, 
                    debits: total_debits, 
                    credits: total_credits 
                });
            }
        }
        
        Ok(())
    }
    
    async fn post_transaction(&self, id: i64) -> Result<Transaction, Error> {
        let transaction = self.get_transaction(id).await?;
        
        // Run pre-post hooks using the extension system
        let hook_registry = self.extension_service.hook_registry();
        hook_registry.run_before_post_hooks(&transaction).await?;
        
        // Validate transaction
        self.validate_transaction(&transaction)?;
        
        // Update transaction status
        let updated = self.transaction_repository
            .update_status(id, TransactionStatus::Posted)
            .await?;
            
        // Update account balances
        for split in &transaction.splits {
            self.account_balance_service
                .update_balance(split.account_id, &split.money, split.debit_credit)
                .await?;
        }
        
        // Run post-post hooks
        hook_registry.run_after_post_hooks(&updated).await?;
        
        // Publish event to the event bus
        self.extension_service.event_bus().publish(
            KernelEvent::TransactionPosted(updated.clone())
        ).await?;
        
        Ok(updated)
    }
}
```

### Balance Calculation
Account balances are calculated based on posted transactions:

```rust
impl AccountBalanceService for DefaultAccountBalanceService {
    async fn get_account_balance(&self, account_id: i64, as_of: Option<DateTime<Utc>>) -> Result<AccountBalance, Error> {
        let account = self.account_repository.find_by_id(account_id).await?;
        
        // Get all posted transactions affecting this account
        let filters = TransactionFilters {
            account_ids: vec![account_id],
            status: Some(TransactionStatus::Posted),
            end_date: as_of,
            ..Default::default()
        };
        
        let (transactions, _) = self.transaction_repository
            .find_by_book(account.book_id, &filters, &Pagination::all())
            .await?;
            
        // Calculate balance from transaction splits
        let money_service = self.money_service_provider.get_service();
        let currency = money_service.get_currency(&account.currency)
            .ok_or_else(|| Error::InvalidCurrency(account.currency.clone()))?;
            
        let mut balance = Money::zero(Rc::clone(&currency));
        
        for transaction in transactions {
            for split in transaction.splits {
                if split.account_id == account_id {
                    match split.debit_credit {
                        DebitCredit::Debit => {
                            if account.account_type.normal_balance() == DebitCredit::Debit {
                                balance = balance.add(&split.money)?;
                            } else {
                                balance = balance.subtract(&split.money)?;
                            }
                        },
                        DebitCredit::Credit => {
                            if account.account_type.normal_balance() == DebitCredit::Credit {
                                balance = balance.add(&split.money)?;
                            } else {
                                balance = balance.subtract(&split.money)?;
                            }
                        }
                    }
                }
            }
        }
        
        // Calculate pending balance from pending transactions
        let pending_filters = TransactionFilters {
            account_ids: vec![account_id],
            status: Some(TransactionStatus::Pending),
            end_date: as_of,
            ..Default::default()
        };
        
        let (pending_transactions, _) = self.transaction_repository
            .find_by_book(account.book_id, &pending_filters, &Pagination::all())
            .await?;
            
        let mut pending_balance = Money::zero(Rc::clone(&currency));
        
        // Similar calculation for pending transactions...
        
        // Run hooks for balance calculation
        let hook_registry = self.extension_service.hook_registry();
        
        // Run before balance calculation hooks
        hook_registry.run_before_balance_calculation_hooks(account_id, as_of).await?;
        
        let account_balance = AccountBalance {
            account_id,
            balance,
            pending_balance,
            available_balance: balance.add(&pending_balance)?,
            as_of: as_of.unwrap_or_else(Utc::now),
        };
        
        // Run after balance calculation hooks
        hook_registry.run_after_balance_calculation_hooks(&account_balance).await?;
        
        // Publish event
        self.extension_service.event_bus().publish(
            KernelEvent::BalanceCalculated(account_balance.clone())
        ).await?;
        
        Ok(account_balance)
    }
}
```

## Dependencies
- **Money Handling Component**: For financial calculations and currency management
- **Extension System Component**: For hooks, events, and Python integration
- **Database Layer**: For persistence of accounting data
- **API Layer**: For exposing services to clients

## Performance Considerations
- **Batch Operations**: Support for bulk transaction creation and processing
- **Caching**: Strategic caching of account balances and frequently accessed data
- **Pagination**: Efficient data retrieval for large datasets
- **Indexing**: Database indexing for common query patterns
- **Asynchronous Processing**: Non-blocking I/O for database operations

## Error Handling
The kernel uses a comprehensive error handling approach:

```rust
/// Error types for the accounting kernel
pub enum Error {
    /// Database errors
    Database(DatabaseError),
    
    /// Validation errors
    Validation(ValidationError),
    
    /// Not found errors
    NotFound { entity: String, id: i64 },
    
    /// Permission errors
    PermissionDenied { entity: String, id: i64 },
    
    /// Business rule violations
    BusinessRuleViolation(String),
    
    /// Money-related errors
    Money(MoneyError),
    
    /// Extension errors
    Extension(String),
    
    /// Invalid currency
    InvalidCurrency(String),
    
    /// Unexpected errors
    Internal(String),
}

/// Validation error types
pub enum ValidationError {
    /// Transaction is not balanced
    UnbalancedTransaction { 
        currency: String,
        debits: Money, 
        credits: Money 
    },
    
    /// Insufficient splits in transaction
    InsufficientSplits,
    
    /// Invalid account type for operation
    InvalidAccountType { account_id: i64, expected: Vec<AccountType> },
    
    /// Field validation error
    InvalidField { field: String, reason: String },
}
```

## Testing Approach
The kernel will be thoroughly tested using:

- **Unit Tests**: For individual functions and methods
- **Integration Tests**: For service interactions
- **Property-Based Tests**: For validating accounting rules
- **Benchmarks**: For performance-critical operations

## Security Considerations
- **Input Validation**: All inputs are validated before processing
- **Authorization**: Operations check for proper permissions
- **Audit Trail**: All financial operations are logged
- **Secure Defaults**: Conservative defaults for financial operations
- **Fail Secure**: On error, operations fail without side effects
