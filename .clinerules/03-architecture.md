# Architecture Guidelines

## Overview

Ratio employs a modular, layered architecture with clear separation of concerns. This document outlines the core architectural principles and patterns to follow when working on the project.

## System Architecture

Ratio uses a layered architecture with the following components:

```
CLI/TUI (Rust) ↔ gRPC API Layer (Rust) ↔ Accounting Kernel (Rust) ↔ PostgreSQL
                                           ↕
                                      Extensions (Python)
```

### Layer Responsibilities

1. **Terminal UI (TUI) Layer**
   - Handle user interaction through terminal interface
   - Render data in appropriate formats using tui-rs
   - Process user commands and inputs
   - Communicate with application logic via the API layer
   - Maintain current UI state and navigation

2. **API Layer**
   - Provide gRPC service definitions for core functionalities
   - Translate between API requests/responses and kernel operations
   - Handle authentication and authorization
   - Implement proper error handling and status codes
   - Provide API documentation and versioning

3. **Accounting Kernel**
   - Implement core double-entry bookkeeping logic
   - Enforce business rules and accounting principles
   - Maintain data integrity and validation
   - Provide extension points for Python modules
   - Handle all financial calculations and money operations

4. **Database Layer**
   - Store financial data with proper constraints
   - Enforce referential integrity
   - Optimize for common query patterns
   - Support transaction isolation for financial operations
   - Provide proper backup and recovery mechanisms

5. **Extension System**
   - Allow for Python-based extensions
   - Provide hook points for customization
   - Support plugin architecture for specialized reports and financial tools
   - Enable user-defined rules and automation

## Key Architectural Patterns

1. **Domain-Driven Design**
   - Model the domain based on accounting principles
   - Create a rich domain model with encapsulated business logic
   - Use aggregates, entities, and value objects appropriately
   - Separate domain logic from infrastructure concerns

2. **Dependency Injection**
   - Components should declare dependencies explicitly
   - Use trait objects to define interfaces
   - Test with mock implementations of dependencies
   - Avoid global state and singletons

3. **CQRS-Inspired Approach**
   - Separate read and write operations where appropriate
   - Optimize read paths for reporting performance
   - Ensure consistency in write operations

4. **Event-Based Communication**
   - Use events to notify across component boundaries
   - Enable loose coupling between components
   - Support extensibility through event hooks
   - Implement audit logging via events

5. **Repository Pattern**
   - Abstract data access behind repository interfaces
   - Use strongly typed queries with sqlx
   - Centralize data access logic
   - Support transaction management

## Implementation Considerations

### Error Handling

- Use rich error types with context
- Implement appropriate error conversion between layers
- Provide user-friendly error messages at UI level
- Ensure errors are logged with appropriate level and context

### Configuration Management

- Use a layered configuration approach
- Support environment variables, config files, and CLI options
- Provide sensible defaults
- Validate configuration at startup

### Performance Concerns

- Optimize for common financial operations
- Use appropriate caching strategies
- Be mindful of memory usage for large transaction sets
- Design for efficient database access patterns

### Security

- Follow the principle of least privilege
- Implement proper authentication and authorization
- Use secure defaults
- Protect sensitive financial information
- Implement audit logging for security-relevant operations

## Cross-Cutting Concerns

- **Logging**: Use structured logging with appropriate context
- **Metrics**: Collect performance metrics for key operations
- **Error Handling**: Consistent approach across all components
- **Validation**: Validate data at system boundaries
- **Internationalization**: Support for multiple currencies and locales

When implementing new features or making changes, ensure they align with these architectural principles and patterns.
