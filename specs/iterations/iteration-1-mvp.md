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
  - [ ] Set up Cargo workspace with the following crates:
    - `ratio-kernel`: Core accounting functionality
    - `ratio-api`: gRPC service implementations
    - `ratio-tui`: Terminal user interface components
    - `ratio-common`: Shared types and utilities
    - `ratio`: Main binary crate that dispatches to TUI and other modes
  - [ ] Configure binary architecture with single `ratio` executable (Git-like subcommand approach)
  - [ ] Set up shared dependencies and workspace-level configuration
  - [ ] Implement initial module structure for each crate
  - [ ] Create central Money type in common crate

- [ ] Set up development environment with Docker Compose
  - [ ] Create development Docker Compose file with PostgreSQL service
  - [ ] Add configuration for database initialization and persistence
  - [ ] Configure network settings for local development
  - [ ] Create scripts for common development tasks

- [ ] Configure CI/CD pipeline for testing and deployment
  - [ ] Set up GitHub Actions workflow for automated testing
  - [ ] Create multi-stage Dockerfile for efficient and secure builds
  - [ ] Configure Docker image building and optimization using Alpine/distroless base images
  - [ ] Implement binary optimization for reduced size and improved startup time
  - [ ] Set up artifact publishing and versioning

- [ ] Set up logging and metrics collection
  - [ ] Configure structured logging with different levels
  - [ ] Implement metrics collection points for core operations
  - [ ] Set up performance benchmarking framework

### Database Implementation
- [ ] Define and implement PostgreSQL schema
  - [ ] Design tables with proper constraints for financial integrity
  - [ ] Implement double-entry bookkeeping structure
  - [ ] Create indexes for common query patterns
  - [ ] Set up foreign key relationships
  - [ ] Configure storage for Money type (BIGINT amount, VARCHAR currency_code)

- [ ] Implement database migrations using sqlx-cli
  - [ ] Create initial schema migration
  - [ ] Set up migration versioning
  - [ ] Configure migration testing
  - [ ] Document migration strategy

- [ ] Set up data access layer with strongly-typed SQL using sqlx
  - [ ] Implement repository interfaces for core entities
  - [ ] Create strongly-typed query methods
  - [ ] Establish connection pooling strategy
  - [ ] Implement transaction handling

- [ ] Create database connectivity and connection pool management
  - [ ] Configure connection parameters and timeouts
  - [ ] Implement connection pooling with idle timeouts
  - [ ] Create health check mechanism
  - [ ] Implement retry logic for connection issues

### Core Accounting Kernel
- [ ] Implement [Accounting Kernel](../components/kernel/accounting-kernel.md) architecture
  - [ ] Create core kernel module structure
  - [ ] Implement domain-driven design patterns
  - [ ] Define service interfaces and traits
  - [ ] Set up error handling and result types

- [ ] Create domain models (Books, Accounts, Transactions, Splits)
  - [ ] Implement Money type for financial calculations
    ```rust
    // Currency definition with precision information
    pub struct Currency {
        pub code: String,        // ISO 4217 code (e.g., "USD")
        pub name: String,        // Human-readable name (e.g., "US Dollar")
        pub symbol: String,      // Currency symbol (e.g., "$")
        pub decimal_places: u8,  // Number of decimal places (e.g., 2 for USD)
        pub rounding_method: RoundingMethod, // Default rounding method
    }

    // Money value with currency association
    pub struct Money {
        amount: i64,            // Scaled integer amount
        currency: Rc<Currency>, // Reference to currency definition
    }

    // Rounding methods for financial calculations
    pub enum RoundingMethod {
        RoundHalfUp,
        RoundHalfDown,
        RoundDown,
        RoundUp,
        Bankers, // Round to nearest even number (common in finance)
    }
    ```
  - [ ] Implement Book, Account, Transaction, and Split models
  - [ ] Create account hierarchy and typing system
  - [ ] Implement proper relationship between entities

- [ ] Implement service layer with business logic
  - [ ] BookService for managing financial books
  - [ ] AccountService for account management and balance calculation
  - [ ] TransactionService for processing financial transactions
  - [ ] ValidationService for ensuring financial integrity

- [ ] Build validation system for double-entry integrity
  - [ ] Implement transaction balance validation
  - [ ] Create account type validation rules
  - [ ] Build validation pipeline for financial operations
  - [ ] Implement comprehensive error reporting

- [ ] Develop extension point system for future customization
  - [ ] Create hook system for key operations
  - [ ] Implement basic PyO3 integration for Python extensions
  - [ ] Set up event system for notifications and triggers
  - [ ] Document extension points for future development

### API Layer
- [ ] Define gRPC service definitions for core services
  - [ ] Create Protocol Buffer definitions for core entities
  - [ ] Define service interfaces for Books, Accounts, and Transactions
  - [ ] Implement pagination and filtering options
  - [ ] Design error codes and status responses

- [ ] Implement API services with Tonic
  - [ ] Create service implementations that connect to the kernel
  - [ ] Implement proper error mapping and status codes
  - [ ] Set up proper concurrency handling
  - [ ] Implement request validation

- [ ] Create serialization/deserialization for API messages
  - [ ] Implement conversions between domain models and Protocol Buffer messages
  - [ ] Handle Money type serialization properly
  - [ ] Create validation for incoming messages
  - [ ] Implement proper error handling for conversion failures

- [ ] Set up authentication and authorization
  - [ ] Create authentication middleware
  - [ ] Implement basic authorization mechanisms
  - [ ] Set up secure credential storage
  - [ ] Create audit logging for sensitive operations

- [ ] Implement pagination for large result sets
  - [ ] Create standardized pagination approach
  - [ ] Implement cursor-based pagination for efficiency
  - [ ] Add filtering capabilities for all collection endpoints
  - [ ] Create sorting options for result sets

### Terminal User Interface
- [ ] Implement basic [Terminal Interface](../components/tui/terminal-interface.md) framework with tui-rs and crossterm
  - [ ] Create application state management
  - [ ] Implement event loop and input handling
  - [ ] Design common UI components and widgets
  - [ ] Create layout management system
  - [ ] Implement theming support

- [ ] Create account management screens
  - [ ] Build account list view with balance information
  - [ ] Implement account creation and editing forms
  - [ ] Create account details view
  - [ ] Implement account hierarchical view

- [ ] Build transaction entry and management interface
  - [ ] Create transaction register view
  - [ ] Implement transaction entry form with validation
  - [ ] Build split management interface
  - [ ] Implement search and filtering capabilities

- [ ] Develop reporting screens
  - [ ] Create balance sheet report
  - [ ] Implement income statement report
  - [ ] Build cash flow reporting
  - [ ] Implement data visualization widgets

- [ ] Implement keyboard navigation and shortcuts
  - [ ] Create vim-like navigation for power users
  - [ ] Implement comprehensive keyboard shortcuts
  - [ ] Build help system with keybinding information
  - [ ] Create navigational breadcrumbs

### Testing
- [ ] Create comprehensive unit test suite
  - [ ] Design test structure with fixtures and mocks
  - [ ] Implement unit tests for all services and models
  - [ ] Create tests for Money type and financial calculations
  - [ ] Set up test helpers and utilities

- [ ] Implement integration tests for core workflows
  - [ ] Test database interaction with test containers
  - [ ] Create integration tests for API endpoints
  - [ ] Test kernel integration with other components
  - [ ] Implement cross-component integration testing

- [ ] Develop property-based tests for accounting rules
  - [ ] Create property tests for double-entry validation
  - [ ] Implement transaction integrity tests
  - [ ] Test currency conversion and rounding
  - [ ] Create randomized test scenarios

- [ ] Set up end-to-end testing scenarios
  - [ ] Create test harness for full system testing
  - [ ] Implement common user workflows as tests
  - [ ] Test UI interactions with automated tools
  - [ ] Create performance and load testing scenarios

## Architecture Focus
- **Data Model Integrity**: Ensuring the database schema correctly implements double-entry accounting principles
- **API Design**: Creating a clean, consistent API that will support future expansion
- **Separation of Concerns**: Clear boundaries between kernel, API, and UI layers
- **Performance Foundations**: Establishing patterns for efficient data access and processing
- **Extensibility**: Building hooks and extension points for future features
- **Binary Distribution**: Creating an efficient binary architecture with a single entry point
- **Financial Accuracy**: Ensuring precise financial calculations with the Money type

## Testing Strategy
- **Unit Testing**: All core business logic will have comprehensive unit tests with a target of >80% code coverage
- **Property-Based Testing**: Using property tests to validate invariants of the accounting system, especially for financial calculations
- **Integration Testing**: Testing interactions between components using test containers for database testing
- **Manual Testing**: Exploratory testing of the TUI workflows with documented test plans
- **CI Testing**: Automated testing on each commit using GitHub Actions
- **Performance Testing**: Benchmarking key operations to ensure they meet performance targets

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
  - Docker image size under 20MB
  - Application startup time under 1 second
- **Reliability**: No data corruption or loss in any test scenario
- **Coverage**: >80% test coverage for critical components
- **Build Performance**: CI/CD pipeline completes within 10 minutes

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
  - GitHub Actions for CI/CD
  
- **Internal**:
  - Database schema must be defined before service implementation
  - Core models must be implemented before services
  - API layer depends on the accounting kernel
  - TUI depends on the API layer
  
- **Libraries**:
  - sqlx for database access
  - tonic for gRPC implementation
  - tui-rs and crossterm for terminal interface
  - PyO3 for Python extension system integration

## Post-MVP Evaluation
After completing the MVP, we will evaluate:
- Which features were most challenging to implement
- Performance bottlenecks that need to be addressed
- User feedback on the TUI workflow
- Technical debt that should be addressed before Iteration 2
- Any architectural changes needed based on implementation experience
- Extension system effectiveness and improvement areas
- Docker image size and optimization opportunities

This evaluation will help shape the priorities and approach for Iteration 2.
