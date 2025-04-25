# Working with Cline and Other LLMs

## Overview

Ratio is designed to be developed with assistance from AI tools like Cline and other Large Language Models (LLMs). This document provides guidance for effectively using LLMs when working on this project, with a focus on spec-driven development.

## Bootstrapping Cline for Ratio Work

When starting a new conversation with Cline about Ratio, provide context about the project:

```
You are assisting with the development of Ratio, a Rust/Python personal finance application that follows a spec-driven development approach. Ratio has a dedicated specs/ directory with architecture specs, feature specs, component specs, and iteration plans.

The project uses a double-entry bookkeeping system with Rust for the core accounting kernel and a terminal UI interface. All development should follow the patterns, architecture, and guidelines in the specs.
```

## Effective Prompting Patterns

### Reference Specifications

Always reference the relevant specifications when asking for implementation help:

```
Please help me implement the transaction scheduling feature according to the specs/features/scheduling/transaction-scheduling.md specification.
```

### Break Down Complex Tasks

For larger features, break down the work into manageable steps:

```
Let's implement the account tracking feature specified in specs/features/accounts/account-tracking.md. Let's start with:
1. First, define the data models
2. Then implement the core service functions
3. Finally, create the API endpoints

Let's begin with step 1.
```

### Start with Structure

Ask Cline to outline the approach before diving into implementation:

```
Based on specs/components/kernel/accounting-kernel.md, please outline the files we'll need to modify, the functions we'll need to create, and the overall approach for implementing the new account reconciliation feature.
```

### Test-Driven Development

Start with tests based on the specifications:

```
Before implementing the transaction validation logic, please help me write tests based on the requirements in specs/features/transactions/transaction-management.md. Focus particularly on the validation rules and edge cases.
```

## Task-Specific Prompting Strategies

### Feature Implementation

```
I need to implement the [feature] described in specs/features/[path]. Let's start by:
1. Understanding the key requirements and acceptance criteria
2. Planning the implementation approach
3. Creating the necessary models and services
4. Implementing the API layer
5. Adding UI components
6. Writing tests to ensure it meets the requirements
```

### Bug Fixing

```
I'm experiencing a bug where [description]. According to our specs, the behavior should be [expected behavior]. Here's the relevant code: [code]. Can you help me identify the issue and propose a fix that aligns with our architecture?
```

### Code Review

```
Please review this implementation of [feature] against its specification in specs/[path]. Identify any discrepancies or areas where the implementation doesn't meet the requirements or follow our architectural patterns.
```

### Refactoring

```
This [component/file] has grown too complex and needs refactoring. According to our architecture in specs/architecture/[path], we should [architectural principle]. Can you help restructure this code to better align with our architectural guidelines?
```

## Project-Specific Guidance

### Financial Calculations

When working with financial data, emphasize:

```
For this financial calculation, please ensure:
1. The Money type is used consistently (never floating-point)
2. Proper rounding is applied following our standards
3. Error cases are properly handled
4. Currency conversion follows the specifications
5. Property-based tests verify the mathematical correctness
```

### Database Operations

When implementing database-related functionality:

```
For this database operation, ensure:
1. Transactions maintain ACID properties
2. The double-entry constraints are enforced
3. Performance is considered for common query patterns
4. Error handling includes proper recovery paths
```

### User Interface

For TUI development:

```
When implementing this UI component, follow our TUI patterns by:
1. Using the standard widget library from tui-rs
2. Following our keyboard navigation conventions
3. Implementing proper error feedback
4. Ensuring responsive updates even with large datasets
```

## Troubleshooting LLM Assistance

### Cline Isn't Following the Specs

If Cline isn't properly following the specifications:

```
You seem to be implementing this differently than the specification describes. Please refer specifically to the [section] in specs/[path] which states: [quote from spec].
```

### Balancing Specs vs. Implementation Reality

When implementation realities differ from specifications:

```
I've encountered a challenge implementing [feature] as specified. The specification says [quote], but I've found that [issue]. What's the best way to address this while maintaining the intent of the specification?
```

### Managing Large Codebase Context

For complex tasks involving multiple components:

```
Let's approach this systematically:
1. First, let's examine the relevant specification in specs/[path]
2. Then, review the existing code in [files]
3. Next, identify the integration points with other components
4. Finally, implement the solution ensuring consistency with our architecture
```

## Best Practices Summary

1. **Always reference specific specifications** when requesting implementation help
2. **Break down complex tasks** into smaller, manageable pieces
3. **Ask for structure before implementation** to ensure alignment with architecture
4. **Begin with tests** to verify requirements are properly understood
5. **Review generated code** to ensure it follows project standards
6. **Update specifications** when implementation details diverge for good reasons
7. **Use the right level of abstraction** in your prompts (more detailed for specific code, higher-level for design)

Follow these guidelines to get the most effective assistance from Cline and other LLMs when working on the Ratio project.
