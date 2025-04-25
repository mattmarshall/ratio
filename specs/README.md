# Ratio Specifications

This directory contains detailed specifications for the Ratio project, a high-performance CLI/TUI personal finance application built with Rust and Python.

## Directory Structure

- **architecture/** - System-wide architectural decisions
  - [tech-stack.md](architecture/tech-stack.md) - Technology choices and rationale
  - [data-model.md](architecture/data-model.md) - Detailed database schema and relationships
  - [api-design.md](architecture/api-design.md) - Complete gRPC service definitions

- **features/** - Functional specifications by feature area
  - **accounts/** - Account management features
    - [account-tracking.md](features/accounts/account-tracking.md) - Account tracking and management
  - **transactions/** - Transaction-related features
    - [transaction-management.md](features/transactions/transaction-management.md) - Transaction creation and management
  - **scheduling/** - Scheduled transactions features
    - [transaction-scheduling.md](features/scheduling/transaction-scheduling.md) - Recurring transaction management

- **components/** - Technical component specifications
  - **kernel/** - Accounting kernel specifications
    - [accounting-kernel.md](components/kernel/accounting-kernel.md) - Core accounting engine
    - [money-handling.md](components/kernel/money-handling.md) - Financial calculations and currency support
    - [extension-system.md](components/kernel/extension-system.md) - Hook system and Python integration
  - **tui/** - Terminal UI specifications
    - [terminal-interface.md](components/tui/terminal-interface.md) - Terminal user interface

- **iterations/** - Work broken down by development iteration
  - [iteration-1-mvp.md](iterations/iteration-1-mvp.md) - Core accounting MVP

- **templates/** - Templates for creating new specifications
  - [feature-spec.md](templates/feature-spec.md) - Template for feature specifications
  - [component-spec.md](templates/component-spec.md) - Template for component specifications
  - [iteration-spec.md](templates/iteration-spec.md) - Template for iteration specifications

## Using These Specifications

### For Development Planning
1. Start with the [iteration-1-mvp.md](iterations/iteration-1-mvp.md) specification to understand the current development priorities.
2. Review the feature specifications for detailed requirements.
3. Consult the architecture documents for system-wide decisions.

### For Feature Implementation
1. Locate the relevant feature specification in the `features/` directory.
2. Review any related component specifications in the `components/` directory.
3. Refer to the architecture documents for context on how the feature fits into the overall system.

### For Adding New Specifications
1. Copy the appropriate template from the `templates/` directory.
2. Fill in the sections with detailed information.
3. Link the new specification in this README.

## Future Specifications to Add

The following specifications should be added to complete the documentation:

### Features
- Balance Optimization - Cash flow forecasting and balance management
- Liability Management - Debt tracking and management
- Data Visualization - Reporting and charting
- Extension System - Python module integration

### Components
- gRPC API Layer - API service implementation details
- Rules Engine - Custom rule processing and automation
- Python Extension System - Extension architecture and APIs

### Iterations
- Iteration 2 - Enhanced features
- Iteration 3 - Extended ecosystem

## Working with Cline

These specifications are designed to focus Cline's development efforts. When working with Cline, you can reference specific specifications to guide implementation:

1. For targeted feature development:
   ```
   Implement the account creation feature as specified in specs/features/accounts/account-tracking.md
   ```

2. For architectural guidance:
   ```
   Design the transaction validation system based on specs/components/kernel/accounting-kernel.md
   ```

3. For iteration planning:
   ```
   What should we focus on next given the priorities in specs/iterations/iteration-1-mvp.md?
