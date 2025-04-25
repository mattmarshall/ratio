# API Design Specification

## Overview
This document outlines the gRPC service definitions for Ratio, a personal finance application. The API is designed to provide a clean, strongly-typed interface between the frontend TUI and the backend accounting kernel.

## Architecture
The API follows a service-oriented architecture with Protocol Buffers as the interface definition language and gRPC as the communication protocol. This approach provides:

- **Strong typing**: Service contracts are well-defined and enforced at compile time
- **Efficient serialization**: Protocol Buffers offer compact binary serialization
- **Language agnosticism**: Services can be consumed by clients in multiple languages
- **Streaming capabilities**: For real-time updates and data feeds

## Common Types

```protobuf
syntax = "proto3";

package ratio.common;

import "google/protobuf/timestamp.proto";

message Empty {}

message Error {
  int32 code = 1;
  string message = 2;
  string details = 3;
}

message Money {
  string currency = 1;
  int64 amount = 2;  // Amount in smallest currency unit (e.g., cents)
}

enum AccountType {
  ACCOUNT_TYPE_UNSPECIFIED = 0;
  ACCOUNT_TYPE_ASSET = 1;
  ACCOUNT_TYPE_LIABILITY = 2;
  ACCOUNT_TYPE_EQUITY = 3;
  ACCOUNT_TYPE_INCOME = 4;
  ACCOUNT_TYPE_EXPENSE = 5;
}

enum TransactionStatus {
  TRANSACTION_STATUS_UNSPECIFIED = 0;
  TRANSACTION_STATUS_PENDING = 1;
  TRANSACTION_STATUS_POSTED = 2;
  TRANSACTION_STATUS_VOIDED = 3;
}

// Pagination support
message PageRequest {
  int32 page_size = 1;
  string page_token = 2;
}

message PageResponse {
  string next_page_token = 1;
  int32 total_size = 2;
}
```

## Book Service
Manages financial books, the top-level containers for a set of accounts.

```protobuf
syntax = "proto3";

package ratio.book;

import "common.proto";
import "google/protobuf/timestamp.proto";

service BookService {
  rpc CreateBook(CreateBookRequest) returns (Book);
  rpc GetBook(GetBookRequest) returns (Book);
  rpc ListBooks(ListBooksRequest) returns (ListBooksResponse);
  rpc UpdateBook(UpdateBookRequest) returns (Book);
  rpc DeleteBook(DeleteBookRequest) returns (common.Empty);
  rpc GetBookSummary(GetBookSummaryRequest) returns (BookSummary);
}

message Book {
  int64 id = 1;
  string name = 2;
  string description = 3;
  string currency = 4;
  google.protobuf.Timestamp created_at = 5;
  google.protobuf.Timestamp updated_at = 6;
}

message CreateBookRequest {
  string name = 1;
  string description = 2;
  string currency = 3;
}

message GetBookRequest {
  int64 id = 1;
}

message ListBooksRequest {
  common.PageRequest pagination = 1;
}

message ListBooksResponse {
  repeated Book books = 1;
  common.PageResponse pagination = 2;
}

message UpdateBookRequest {
  int64 id = 1;
  string name = 2;
  string description = 3;
  string currency = 4;
}

message DeleteBookRequest {
  int64 id = 1;
}

message GetBookSummaryRequest {
  int64 id = 1;
}

message BookSummary {
  int64 id = 1;
  string name = 2;
  int32 account_count = 3;
  common.Money total_assets = 4;
  common.Money total_liabilities = 5;
  common.Money net_worth = 6;
}
```

## Account Service
Manages financial accounts within a book.

```protobuf
syntax = "proto3";

package ratio.account;

import "common.proto";
import "google/protobuf/timestamp.proto";

service AccountService {
  rpc CreateAccount(CreateAccountRequest) returns (Account);
  rpc GetAccount(GetAccountRequest) returns (Account);
  rpc ListAccounts(ListAccountsRequest) returns (ListAccountsResponse);
  rpc UpdateAccount(UpdateAccountRequest) returns (Account);
  rpc DeleteAccount(DeleteAccountRequest) returns (common.Empty);
  rpc ReconcileAccount(ReconcileAccountRequest) returns (ReconciliationResult);
  rpc GetAccountBalance(GetAccountBalanceRequest) returns (AccountBalance);
  rpc GetAccountHistory(GetAccountHistoryRequest) returns (stream AccountHistoryEntry);
}

message Account {
  int64 id = 1;
  int64 book_id = 2;
  string name = 3;
  common.AccountType type = 4;
  string code = 5;
  string description = 6;
  string currency = 7;
  int64 parent_id = 8;
  bool active = 9;
  google.protobuf.Timestamp created_at = 10;
  google.protobuf.Timestamp updated_at = 11;
}

message CreateAccountRequest {
  int64 book_id = 1;
  string name = 2;
  common.AccountType type = 3;
  string code = 4;
  string description = 5;
  string currency = 6;
  int64 parent_id = 7;
}

message GetAccountRequest {
  int64 id = 1;
}

message ListAccountsRequest {
  int64 book_id = 1;
  common.AccountType type = 2;
  bool include_inactive = 3;
  common.PageRequest pagination = 4;
}

message ListAccountsResponse {
  repeated Account accounts = 1;
  common.PageResponse pagination = 2;
}

message UpdateAccountRequest {
  int64 id = 1;
  string name = 2;
  string code = 3;
  string description = 4;
  string currency = 5;
  int64 parent_id = 6;
  bool active = 7;
}

message DeleteAccountRequest {
  int64 id = 1;
}

message ReconcileAccountRequest {
  int64 id = 1;
  common.Money ending_balance = 2;
  google.protobuf.Timestamp statement_date = 3;
  repeated int64 cleared_transaction_ids = 4;
}

message ReconciliationResult {
  bool success = 1;
  common.Money difference = 2;
  repeated int64 unreconciled_transaction_ids = 3;
}

message GetAccountBalanceRequest {
  int64 id = 1;
  google.protobuf.Timestamp as_of = 2;
}

message AccountBalance {
  int64 account_id = 1;
  common.Money balance = 2;
  common.Money pending_balance = 3;
  common.Money available_balance = 4;
  google.protobuf.Timestamp as_of = 5;
}

message GetAccountHistoryRequest {
  int64 id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
}

message AccountHistoryEntry {
  google.protobuf.Timestamp date = 1;
  common.Money balance = 2;
  int32 transaction_count = 3;
}
```

## Transaction Service
Manages financial transactions and their splits.

```protobuf
syntax = "proto3";

package ratio.transaction;

import "common.proto";
import "google/protobuf/timestamp.proto";

service TransactionService {
  rpc CreateTransaction(CreateTransactionRequest) returns (Transaction);
  rpc GetTransaction(GetTransactionRequest) returns (Transaction);
  rpc ListTransactions(ListTransactionsRequest) returns (ListTransactionsResponse);
  rpc UpdateTransaction(UpdateTransactionRequest) returns (Transaction);
  rpc DeleteTransaction(DeleteTransactionRequest) returns (common.Empty);
  rpc PostTransaction(PostTransactionRequest) returns (Transaction);
  rpc VoidTransaction(VoidTransactionRequest) returns (Transaction);
  rpc AttachDocument(AttachDocumentRequest) returns (Attachment);
  rpc GetAttachment(GetAttachmentRequest) returns (Attachment);
}

message Transaction {
  int64 id = 1;
  int64 book_id = 2;
  google.protobuf.Timestamp transaction_date = 3;
  google.protobuf.Timestamp post_date = 4;
  string description = 5;
  string reference = 6;
  common.TransactionStatus status = 7;
  repeated Split splits = 8;
  repeated Attachment attachments = 9;
  map<string, string> metadata = 10;
  google.protobuf.Timestamp created_at = 11;
  google.protobuf.Timestamp updated_at = 12;
}

message Split {
  int64 id = 1;
  int64 transaction_id = 2;
  int64 account_id = 3;
  common.Money amount = 4;
  string debit_credit = 5;  // "D" or "C"
  string memo = 6;
  bool reconciled = 7;
  google.protobuf.Timestamp reconciled_at = 8;
}

message Attachment {
  int64 id = 1;
  int64 transaction_id = 2;
  string name = 3;
  string file_path = 4;
  string file_type = 5;
  int64 file_size = 6;
  string content_hash = 7;
  map<string, string> metadata = 8;
  google.protobuf.Timestamp created_at = 9;
}

message CreateTransactionRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp transaction_date = 2;
  string description = 3;
  string reference = 4;
  repeated CreateSplitRequest splits = 5;
  map<string, string> metadata = 6;
  bool auto_post = 7;
}

message CreateSplitRequest {
  int64 account_id = 1;
  common.Money amount = 2;
  string debit_credit = 3;  // "D" or "C"
  string memo = 4;
}

message GetTransactionRequest {
  int64 id = 1;
}

message ListTransactionsRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  repeated int64 account_ids = 4;
  common.TransactionStatus status = 5;
  string search_query = 6;
  common.PageRequest pagination = 7;
}

message ListTransactionsResponse {
  repeated Transaction transactions = 1;
  common.PageResponse pagination = 2;
}

message UpdateTransactionRequest {
  int64 id = 1;
  google.protobuf.Timestamp transaction_date = 2;
  string description = 3;
  string reference = 4;
  repeated CreateSplitRequest splits = 5;
  map<string, string> metadata = 6;
}

message DeleteTransactionRequest {
  int64 id = 1;
}

message PostTransactionRequest {
  int64 id = 1;
}

message VoidTransactionRequest {
  int64 id = 1;
  string reason = 2;
}

message AttachDocumentRequest {
  int64 transaction_id = 1;
  string name = 2;
  bytes content = 3;
  string file_type = 4;
  map<string, string> metadata = 5;
}

message GetAttachmentRequest {
  int64 id = 1;
}
```

## Scheduled Transaction Service
Manages recurring transactions.

```protobuf
syntax = "proto3";

package ratio.scheduled;

import "common.proto";
import "google/protobuf/timestamp.proto";
import "transaction.proto";

service ScheduledTransactionService {
  rpc CreateScheduledTransaction(CreateScheduledTransactionRequest) returns (ScheduledTransaction);
  rpc GetScheduledTransaction(GetScheduledTransactionRequest) returns (ScheduledTransaction);
  rpc ListScheduledTransactions(ListScheduledTransactionsRequest) returns (ListScheduledTransactionsResponse);
  rpc UpdateScheduledTransaction(UpdateScheduledTransactionRequest) returns (ScheduledTransaction);
  rpc DeleteScheduledTransaction(DeleteScheduledTransactionRequest) returns (common.Empty);
  rpc GenerateInstancesForPeriod(GenerateInstancesRequest) returns (GenerateInstancesResponse);
  rpc GetUpcomingInstances(GetUpcomingInstancesRequest) returns (GetUpcomingInstancesResponse);
  rpc CreateInstanceNow(CreateInstanceNowRequest) returns (transaction.Transaction);
  rpc SkipNextInstance(SkipNextInstanceRequest) returns (ScheduledTransaction);
}

message ScheduledTransaction {
  int64 id = 1;
  int64 book_id = 2;
  string description = 3;
  string frequency = 4;  // DAILY, WEEKLY, MONTHLY, etc.
  FrequencyConfig frequency_config = 5;
  google.protobuf.Timestamp start_date = 6;
  google.protobuf.Timestamp end_date = 7;
  google.protobuf.Timestamp next_due_date = 8;
  TransactionTemplate template = 9;
  bool auto_post = 10;
  google.protobuf.Timestamp created_at = 11;
  google.protobuf.Timestamp updated_at = 12;
}

message FrequencyConfig {
  oneof config {
    DailyConfig daily = 1;
    WeeklyConfig weekly = 2;
    MonthlyConfig monthly = 3;
    YearlyConfig yearly = 4;
  }
}

message DailyConfig {
  int32 every_n_days = 1;
}

message WeeklyConfig {
  int32 every_n_weeks = 1;
  repeated int32 days_of_week = 2;  // 1-7, where 1 is Monday
}

message MonthlyConfig {
  int32 every_n_months = 1;
  oneof day_selection {
    int32 day_of_month = 2;  // 1-31
    RelativeDaySpec relative_day = 3;
  }
}

message RelativeDaySpec {
  enum Position {
    POSITION_UNSPECIFIED = 0;
    POSITION_FIRST = 1;
    POSITION_SECOND = 2;
    POSITION_THIRD = 3;
    POSITION_FOURTH = 4;
    POSITION_LAST = 5;
  }
  
  Position position = 1;
  int32 day_of_week = 2;  // 1-7, where 1 is Monday
}

message YearlyConfig {
  int32 every_n_years = 1;
  int32 month = 2;  // 1-12
  oneof day_selection {
    int32 day_of_month = 3;  // 1-31
    RelativeDaySpec relative_day = 4;
  }
}

message TransactionTemplate {
  string description = 1;
  string reference = 2;
  repeated transaction.CreateSplitRequest splits = 3;
  map<string, string> metadata = 4;
}

message CreateScheduledTransactionRequest {
  int64 book_id = 1;
  string description = 2;
  string frequency = 3;
  FrequencyConfig frequency_config = 4;
  google.protobuf.Timestamp start_date = 5;
  google.protobuf.Timestamp end_date = 6;
  TransactionTemplate template = 7;
  bool auto_post = 8;
}

message GetScheduledTransactionRequest {
  int64 id = 1;
}

message ListScheduledTransactionsRequest {
  int64 book_id = 1;
  bool include_inactive = 2;
  common.PageRequest pagination = 3;
}

message ListScheduledTransactionsResponse {
  repeated ScheduledTransaction scheduled_transactions = 1;
  common.PageResponse pagination = 2;
}

message UpdateScheduledTransactionRequest {
  int64 id = 1;
  string description = 2;
  string frequency = 3;
  FrequencyConfig frequency_config = 4;
  google.protobuf.Timestamp start_date = 5;
  google.protobuf.Timestamp end_date = 6;
  TransactionTemplate template = 7;
  bool auto_post = 8;
}

message DeleteScheduledTransactionRequest {
  int64 id = 1;
}

message GenerateInstancesRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  bool dry_run = 4;
}

message GenerateInstancesResponse {
  int32 instances_generated = 1;
  repeated transaction.Transaction transactions = 2;
}

message GetUpcomingInstancesRequest {
  int64 book_id = 1;
  int32 days_ahead = 2;
  bool group_by_date = 3;
}

message GetUpcomingInstancesResponse {
  message UpcomingInstance {
    int64 scheduled_transaction_id = 1;
    string description = 2;
    google.protobuf.Timestamp due_date = 3;
    TransactionTemplate template = 4;
  }
  
  message DateGroup {
    google.protobuf.Timestamp date = 1;
    repeated UpcomingInstance instances = 2;
  }
  
  oneof result {
    repeated UpcomingInstance instances = 1;
    repeated DateGroup date_groups = 2;
  }
}

message CreateInstanceNowRequest {
  int64 scheduled_transaction_id = 1;
  bool use_template_date = 2;
  bool auto_post = 3;
}

message SkipNextInstanceRequest {
  int64 scheduled_transaction_id = 1;
}
```

## Report Service
Generates financial reports and visualizations.

```protobuf
syntax = "proto3";

package ratio.report;

import "common.proto";
import "google/protobuf/timestamp.proto";

service ReportService {
  rpc GetIncomeStatement(IncomeStatementRequest) returns (IncomeStatementResponse);
  rpc GetBalanceSheet(BalanceSheetRequest) returns (BalanceSheetResponse);
  rpc GetCashFlow(CashFlowRequest) returns (CashFlowResponse);
  rpc GetNetWorthTrend(NetWorthTrendRequest) returns (NetWorthTrendResponse);
  rpc GetCategorySpending(CategorySpendingRequest) returns (CategorySpendingResponse);
  rpc GetAccountBalanceTrend(AccountBalanceTrendRequest) returns (AccountBalanceTrendResponse);
}

message IncomeStatementRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  string period = 4;  // MONTH, QUARTER, YEAR
  bool compare_previous_period = 5;
}

message IncomeStatementResponse {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  
  message LineItem {
    string name = 1;
    common.Money amount = 2;
    common.Money previous_amount = 3;
    double percent_change = 4;
    repeated LineItem sub_items = 5;
  }
  
  LineItem income = 4;
  LineItem expenses = 5;
  common.Money net_income = 6;
  common.Money previous_net_income = 7;
  double net_income_percent_change = 8;
}

message BalanceSheetRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp as_of = 2;
  bool compare_previous_period = 3;
}

message BalanceSheetResponse {
  int64 book_id = 1;
  google.protobuf.Timestamp as_of = 2;
  
  message LineItem {
    string name = 1;
    common.Money amount = 2;
    common.Money previous_amount = 3;
    double percent_change = 4;
    repeated LineItem sub_items = 5;
  }
  
  LineItem assets = 3;
  LineItem liabilities = 4;
  LineItem equity = 5;
  common.Money total_assets = 6;
  common.Money total_liabilities = 7;
  common.Money total_equity = 8;
  common.Money previous_total_assets = 9;
  common.Money previous_total_liabilities = 10;
  common.Money previous_total_equity = 11;
}

message CashFlowRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  string period = 4;  // MONTH, QUARTER, YEAR
}

message CashFlowResponse {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  
  message LineItem {
    string name = 1;
    common.Money amount = 2;
    repeated LineItem sub_items = 3;
  }
  
  LineItem operating_activities = 4;
  LineItem investing_activities = 5;
  LineItem financing_activities = 6;
  common.Money net_cash_flow = 7;
  common.Money beginning_cash = 8;
  common.Money ending_cash = 9;
}

message NetWorthTrendRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  string period = 4;  // MONTH, QUARTER, YEAR
}

message NetWorthTrendResponse {
  message DataPoint {
    google.protobuf.Timestamp date = 1;
    common.Money assets = 2;
    common.Money liabilities = 3;
    common.Money net_worth = 4;
  }
  
  repeated DataPoint data_points = 1;
}

message CategorySpendingRequest {
  int64 book_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  bool include_sub_categories = 4;
  int32 top_n = 5;
}

message CategorySpendingResponse {
  message CategoryTotal {
    int64 category_id = 1;
    string name = 2;
    common.Money amount = 3;
    double percentage = 4;
    repeated CategoryTotal sub_categories = 5;
  }
  
  repeated CategoryTotal categories = 1;
  common.Money total_spending = 2;
}

message AccountBalanceTrendRequest {
  int64 account_id = 1;
  google.protobuf.Timestamp start_date = 2;
  google.protobuf.Timestamp end_date = 3;
  string period = 4;  // DAY, WEEK, MONTH
}

message AccountBalanceTrendResponse {
  int64 account_id = 1;
  string account_name = 2;
  
  message DataPoint {
    google.protobuf.Timestamp date = 1;
    common.Money balance = 2;
  }
  
  repeated DataPoint data_points = 3;
}
```

## Rule Service
Manages custom rules for transaction processing and automation.

```protobuf
syntax = "proto3";

package ratio.rule;

import "common.proto";
import "google/protobuf/timestamp.proto";

service RuleService {
  rpc CreateRule(CreateRuleRequest) returns (Rule);
  rpc GetRule(GetRuleRequest) returns (Rule);
  rpc ListRules(ListRulesRequest) returns (ListRulesResponse);
  rpc UpdateRule(UpdateRuleRequest) returns (Rule);
  rpc DeleteRule(DeleteRuleRequest) returns (common.Empty);
  rpc ToggleRuleActive(ToggleRuleActiveRequest) returns (Rule);
  rpc TestRule(TestRuleRequest) returns (TestRuleResponse);
  rpc RunRulesBatch(RunRulesBatchRequest) returns (RunRulesBatchResponse);
}

message Rule {
  int64 id = 1;
  int64 book_id = 2;
  string name = 3;
  string description = 4;
  RuleCondition condition = 5;
  RuleAction action = 6;
  int32 priority = 7;
  bool active = 8;
  google.protobuf.Timestamp created_at = 9;
  google.protobuf.Timestamp updated_at = 10;
}

message RuleCondition {
  enum MatchType {
    MATCH_TYPE_UNSPECIFIED = 0;
    MATCH_TYPE_ALL = 1;
    MATCH_TYPE_ANY = 2;
  }
  
  MatchType match_type = 1;
  repeated Criterion criteria = 2;
  
  message Criterion {
    string field = 1;
    string operator = 2;
    string value = 3;
  }
}

message RuleAction {
  message SetField {
    string field = 1;
    string value = 2;
  }
  
  message AddSplit {
    int64 account_id = 1;
    common.Money amount = 2;
    string debit_credit = 3;
    string memo = 4;
  }
  
  message RemoveSplit {
    int64 account_id = 1;
  }
  
  repeated SetField set_fields = 1;
  repeated AddSplit add_splits = 2;
  repeated RemoveSplit remove_splits = 3;
  bool auto_post = 4;
}

message CreateRuleRequest {
  int64 book_id = 1;
  string name = 2;
  string description = 3;
  RuleCondition condition = 4;
  RuleAction action = 5;
  int32 priority = 6;
  bool active = 7;
}

message GetRuleRequest {
  int64 id = 1;
}

message ListRulesRequest {
  int64 book_id = 1;
  bool include_inactive = 2;
  common.PageRequest pagination = 3;
}

message ListRulesResponse {
  repeated Rule rules = 1;
  common.PageResponse pagination = 2;
}

message UpdateRuleRequest {
  int64 id = 1;
  string name = 2;
  string description = 3;
  RuleCondition condition = 4;
  RuleAction action = 5;
  int32 priority = 6;
}

message DeleteRuleRequest {
  int64 id = 1;
}

message ToggleRuleActiveRequest {
  int64 id = 1;
  bool active = 2;
}

message TestRuleRequest {
  Rule rule = 1;
  repeated int64 transaction_ids = 2;
}

message TestRuleResponse {
  message RuleMatch {
    int64 transaction_id = 1;
    bool matches = 2;
    string transaction_description = 3;
  }
  
  repeated RuleMatch matches = 1;
  int32 total_matched = 2;
  int32 total_tested = 3;
}

message RunRulesBatchRequest {
  int64 book_id = 1;
  repeated int64 rule_ids = 2;
  repeated int64 transaction_ids = 3;
  bool dry_run = 4;
}

message RunRulesBatchResponse {
  int32 transactions_processed = 1;
  int32 transactions_modified = 2;
  map<int64, int32> rule_match_counts = 3;
}
```

## Service Security
The gRPC services will be secured using:

1. **Authentication**:
   - Token-based authentication with JWT
   - Each request requires a valid token in the metadata

2. **Authorization**:
   - Role-based access control
   - Resource ownership validation

3. **Transport Security**:
   - TLS for all communication
   - Certificate validation

## Client Implementation Guidelines

### Rust Client
For the TUI interface, the Rust client will use the generated gRPC client stubs directly.

```rust
// Example Rust client usage
let mut client = BookServiceClient::connect("http://localhost:50051").await?;
let request = tonic::Request::new(CreateBookRequest {
    name: "Household Finances".to_string(),
    description: "Family budget and expenses".to_string(),
    currency: "USD".to_string(),
});
let response = client.create_book(request).await?;
let book = response.into_inner();
```

### Python Extensions
Python extensions can use the generated Python client stubs to interact with the core services.

```python
# Example Python extension
channel = grpc.insecure_channel('localhost:50051')
stub = book_pb2_grpc.BookServiceStub(channel)
response = stub.GetBook(book_pb2.GetBookRequest(id=1))
```

## Error Handling

The API uses a standard error model across all services:

1. **gRPC Status Codes**:
   - OK (0): Success
   - INVALID_ARGUMENT (3): Invalid request parameters
   - NOT_FOUND (5): Requested resource not found
   - ALREADY_EXISTS (6): Resource already exists
   - PERMISSION_DENIED (7): Insufficient permissions
   - UNAUTHENTICATED (16): Invalid authentication
   - INTERNAL (13): Internal server error

2. **Error Details**:
   The Error message provides additional context about the error.

## Versioning

The API will follow semantic versioning principles:

1. **Package Versioning**:
   - Major version changes in package name (ratio.v1, ratio.v2)
   - Breaking changes only in major version increments

2. **Backward Compatibility**:
   - Field additions are backward compatible
   - Required field changes are breaking changes
   - Service method additions are backward compatible

## Future Considerations

1. **Streaming Capabilities**:
   - Real-time updates for account balances
   - Streaming transactions for live monitoring

2. **Bulk Operations**:
   - Batch transaction creation and modification
   - Import/export functionality

3. **API Gateway**:
   - Potential REST gateway for web clients
   - GraphQL layer for more flexible queries
