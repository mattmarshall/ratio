# API Design Specification

## Overview
This document outlines the gRPC service definitions for Ratio, a personal finance application. The API is designed to provide a clean, strongly-typed interface between the frontend TUI and the backend accounting kernel.

## Architecture
The API follows a service-oriented architecture with Protocol Buffers as the interface definition language and gRPC as the communication protocol. This approach provides:

- **Strong typing**: Service contracts are well-defined and enforced at compile time
- **Efficient serialization**: Protocol Buffers offer compact binary serialization
- **Language agnosticism**: Services can be consumed by clients in multiple languages
- **Streaming capabilities**: For real-time updates and data feeds

## Project Structure

The Protocol Buffer definitions for the Ratio API are organized as follows:

```
ratio/
├── protos/               # Protocol Buffer definitions
│   ├── common.proto      # Common types used across services
│   ├── book.proto        # Book service definitions
│   ├── account.proto     # Account service definitions
│   ├── transaction.proto # Transaction service definitions
│   ├── scheduled.proto   # Scheduled transaction service definitions
│   ├── report.proto      # Report service definitions
│   └── rule.proto        # Rule service definitions
├── crates/
│   ├── ratio-api/        # gRPC service implementations
│   │   └── build.rs      # Compiles protos into Rust code
│   ├── ratio-common/     # Common types including generated proto code
│   └── ...
```

The generated code from the Protocol Buffer definitions will be used in:
- `ratio-api`: Service implementations
- `ratio-common`: Shared types for use across the codebase
- `ratio-tui`: Client stubs for the TUI to communicate with the services

## Core Services

### Common Types
Common types used across services include:
- Money representation
- Account types
- Transaction statuses
- Pagination support
- Error model

Location: `protos/common.proto`

### Book Service
Manages financial books, the top-level containers for a set of accounts.

Key operations:
- Create, read, update, delete books
- List books with pagination
- Get book summary with net worth calculation

Location: `protos/book.proto`

### Account Service
Manages financial accounts within a book.

Key operations:
- Create, read, update, delete accounts
- List accounts with filtering and pagination
- Reconcile accounts with statements
- Get account balances and history

Location: `protos/account.proto`

### Transaction Service
Manages financial transactions and their splits.

Key operations:
- Create, read, update, delete transactions
- List transactions with filtering and pagination
- Post and void transactions
- Attach documents to transactions

Location: `protos/transaction.proto`

### Scheduled Transaction Service
Manages recurring transactions.

Key operations:
- Create, read, update, delete scheduled transactions
- Generate transaction instances for a period
- Get upcoming instances
- Skip instances or create them on demand

Location: `protos/scheduled.proto`

### Report Service
Generates financial reports and visualizations.

Key operations:
- Generate income statements
- Generate balance sheets
- Generate cash flow reports
- Get net worth trends
- Get category spending analysis
- Get account balance trends

Location: `protos/report.proto`

### Rule Service
Manages custom rules for transaction processing and automation.

Key operations:
- Create, read, update, delete rules
- Toggle rule activation
- Test rules against transactions
- Batch run rules on transactions

Location: `protos/rule.proto`

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
