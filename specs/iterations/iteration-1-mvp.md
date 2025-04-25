# Iteration Specification: 1 - Core Accounting MVP

## Overview
This first iteration will establish the foundation of Ratio by implementing the core accounting kernel, basic TUI interface, initial gRPC API, and PostgreSQL schema. The focus is on creating a functional double-entry bookkeeping system with essential reporting capabilities that can serve as the foundation for future development.

## Timeline
- Start Date: TBD
- End Date: TBD
- Duration: 8 weeks

## Goals
- Establish a functional double-entry accounting system
- Create a usable terminal-based interface
- Define and implement the core data model
- Implement basic reporting capabilities
- Provide a solid foundation for future iterations

## Features Included
- [ ] [Account Tracking](../features/accounts/account-tracking.md) - Create, manage, and view multiple account types
- [ ] [Transaction Management](../features/transactions/transaction-management.md) - Record and categorize financial transactions
- [ ] [Transaction Scheduling](../features/scheduling/transaction-scheduling.md) - Set up recurring transactions for bills and income
- [ ] Double-Entry Bookkeeping - Maintain accurate financial records with built-in validation
- [ ] Basic Reports - Implement essential financial reporting (balance sheet, income statement)

## Technical Tasks

### Setup and Infrastructure
- [ ] Create project structure and repository organization
- [ ] Set up development environment with Docker Compose
- [ ] Configure CI/CD pipeline for testing and deployment
- [ ] Set up logging and metrics collection

### Database Implementation
- [ ] Define and implement PostgreSQL schema
- [ ] Implement database migrations using sqlx-cli
- [ ] Set up data access layer with strongly-typed SQL
- [ ] Create database connectivity and connection pool management

### Core Accounting Kernel
- [ ] Implement [Accounting Kernel](../components/kernel/accounting-kernel.md) architecture
- [ ] Create domain models (Books, Accounts, Transactions, Splits)
- [ ] Implement service layer with business logic
- [ ] Build validation system for double-entry integrity
- [ ] Develop extension point system for future customization

### API Layer
- [ ] Define gRPC service definitions for core services
- [ ] Implement API services with proper error handling
- [ ] Create serialization/deserialization for API messages
- [ ] Set up authentication and authorization
- [ ] Implement pagination for large result sets

### Terminal User Interface
- [ ] Implement basic [Terminal Interface](../components/tui/terminal-interface.md) framework
- [ ] Create account management screens
- [ ] Build transaction entry and management interface
- [ ] Develop reporting screens
- [ ] Implement keyboard navigation and shortcuts

### Testing
- [ ] Create comprehensive unit test suite
- [ ] Implement integration tests for core workflows
- [ ] Develop property-based tests for accounting rules
- [ ] Set up end-to-end testing scenarios

## Architecture Focus
- **Data Model Integrity**: Ensuring the database schema correctly implements double-entry accounting principles
- **API Design**: Creating a clean, consistent API that will support future expansion
- **Separation of Concerns**: Clear boundaries between kernel, API, and UI layers
- **Performance Foundations**: Establishing patterns for efficient data access and processing
- **Extensibility**: Building hooks and extension points for future features

## Testing Strategy
- **Unit Testing**: All core business logic will have comprehensive unit tests
- **Property-Based Testing**: Using property tests to validate invariants of the accounting system
- **Integration Testing**: Testing interactions between components
- **Manual Testing**: Exploratory testing of the TUI workflows

## Definition of Done
- [ ] All planned features are implemented and tested
- [ ] Code passes all automated tests with >80% coverage
- [ ] Documentation is updated to reflect implemented features
- [ ] Database migrations work correctly
- [ ] TUI can perform basic accounting operations
- [ ] Double-entry validation ensures data integrity
- [ ] Basic reports generate correct financial statements
- [ ] Performance meets specified requirements
- [ ] All critical bugs are resolved

## Success Metrics
- **Functionality**: All core accounting operations work correctly and maintain integrity
- **Usability**: Users can complete basic tasks in the TUI without confusion
- **Performance**: 
  - Transaction creation completes in <200ms
  - Report generation completes in <1s for books with up to 10,000 transactions
  - UI remains responsive during all operations
- **Reliability**: No data corruption or loss in any test scenario
- **Coverage**: >80% test coverage for critical components

## Risks and Mitigations
- **Risk**: Complex accounting rules leading to logical errors
  - **Mitigation**: Thorough testing, including property-based tests for accounting invariants
  
- **Risk**: Performance issues with large transaction volumes
  - **Mitigation**: Early performance testing with realistic data volumes, database indexing
  
- **Risk**: Terminal UI limitations affecting usability
  - **Mitigation**: Early usability testing, fall back to simpler UI patterns if needed
  
- **Risk**: Scope creep delaying the MVP
  - **Mitigation**: Strict prioritization, defer non-essential features to later iterations

## Dependencies
- **External**: 
  - Rust 1.70+ with async support
  - PostgreSQL 15+
  - Docker and Docker Compose for development
  
- **Internal**:
  - Database schema must be defined before service implementation
  - Core models must be implemented before services
  - API layer depends on the accounting kernel
  - TUI depends on the API layer

## Post-MVP Evaluation
After completing the MVP, we will evaluate:
- Which features were most challenging to implement
- Performance bottlenecks that need to be addressed
- User feedback on the TUI workflow
- Technical debt that should be addressed before Iteration 2
- Any architectural changes needed based on implementation experience

This evaluation will help shape the priorities and approach for Iteration 2.
