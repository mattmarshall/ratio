# Working with Cline on Ratio

This guide is designed to help you effectively use Cline (or other LLMs) when working on the Ratio project. It provides specific prompting patterns, examples, and best practices for spec-driven development with LLM assistance.

## Table of Contents
- [Bootstrapping Cline](#bootstrapping-cline)
- [Working with Specifications](#working-with-specifications)
- [Common Tasks](#common-tasks)
- [Effective Prompting Patterns](#effective-prompting-patterns)
- [Example Workflows](#example-workflows)
- [Troubleshooting](#troubleshooting)

## Bootstrapping Cline

When starting a new conversation with Cline, you can provide this bootstrapping context to orient the LLM to the project:

```
You are assisting with the development of Ratio, a Rust/Python personal finance application that follows a spec-driven development approach. Ratio has a dedicated specs/ directory with architecture specs, feature specs, component specs, and iteration plans.

The project uses a double-entry bookkeeping system with Rust for the core accounting kernel and a terminal UI interface. All development should follow the patterns, architecture, and guidelines in the specs.

Context about the project:
- High-performance CLI/TUI personal finance application
- Written in Rust (core) and Python (extensions)
- Uses PostgreSQL database with double-entry bookkeeping model
- Follows a clearly defined spec-driven development approach
- Terminal-based UI using tui-rs
- Strong typing and validation throughout

When implementing features, always refer to the relevant specification files in the specs/ directory.
```

Feel free to copy and adapt this context based on the specific task you're working on.

## Working with Specifications

### Referencing Specifications

When asking Cline to work on a feature or component, always reference the relevant specifications:

```
Please help me implement [feature] according to the specs/features/[feature].md specification.
```

This ensures Cline has the necessary context and follows the project's design decisions.

### Creating New Specifications

You can ask Cline to create new specifications following the project templates:

```
Please create a new feature specification for [feature name] following the template in specs/templates/feature-spec.md. The feature should [description of what the feature does].
```

### Updating Specifications

When implementation details diverge from the specification:

```
I've implemented [feature/component] differently than specified in [spec file]. Please help me update the specification to match the actual implementation while maintaining the original goals.
```

## Common Tasks

### Implementing a Feature

```
I'd like to implement the [feature name] feature as specified in specs/features/[path]. Please help me with the implementation, focusing on [specific aspect] first.
```

### Creating Tests

```
Based on the specification in specs/features/[path], please help me write comprehensive tests for the [feature/component] that verify all the requirements and edge cases.
```

### Code Review

```
Please review this implementation of [feature/component] against its specification in specs/[path]. Identify any discrepancies or areas where the implementation doesn't meet the requirements.
```

### Debugging

```
I'm having an issue with [problem description]. The implementation should follow specs/[path]. Here's the current code and the error I'm seeing. Can you help identify what's wrong?
```

## Effective Prompting Patterns

### Incremental Development

Break down implementation into manageable steps:

```
Let's implement the [feature] specified in specs/features/[path]. Let's start with:
1. First, define the data models
2. Then implement the core business logic
3. Finally, create the API endpoints

Let's begin with step 1.
```

### Start with Structure

Ask Cline to outline the approach before diving into implementation:

```
Based on specs/[path], please outline the files we'll need to modify, the functions we'll need to create, and the overall approach for implementing [feature].
```

### Test-Driven Development

Start with tests based on the specifications:

```
Before implementing [feature], please help me write tests based on the requirements in specs/features/[path]. Focus particularly on the acceptance criteria.
```

## Example Workflows

### Feature Implementation Workflow

1. **Understand the Specification**:
   ```
   I need to implement the account tracking feature described in specs/features/accounts/account-tracking.md. Can you help me understand the key components and requirements?
   ```

2. **Plan the Implementation**:
   ```
   Based on the account tracking specification, what files should we create or modify? Please outline the implementation approach.
   ```

3. **Create Core Models and Services**:
   ```
   Let's start implementing the account tracking feature by creating the Account model and AccountService as described in the spec.
   ```

4. **Implement the API Layer**:
   ```
   Now let's implement the gRPC service for accounts, following the API design in specs/architecture/api-design.md.
   ```

5. **Implement the UI Components**:
   ```
   Let's implement the TUI components for account management according to specs/components/tui/terminal-interface.md.
   ```

6. **Write Tests**:
   ```
   Please help me write tests for the account tracking implementation to ensure it meets all the requirements in the specification.
   ```

### Component Extension Workflow

1. **Review Existing Components**:
   ```
   I want to extend the accounting kernel component. Can you help me understand the current implementation in relation to specs/components/kernel/accounting-kernel.md?
   ```

2. **Specify Extension**:
   ```
   I need to add support for [new capability] to the accounting kernel. Can you help me update the spec to include this capability?
   ```

3. **Implement Extension**:
   ```
   Let's implement the [new capability] we added to the accounting kernel specification.
   ```

## Troubleshooting

### Cline Isn't Following the Specs

If Cline isn't properly following the specifications:

1. **Provide Explicit Context**:
   ```
   You seem to be implementing this differently than the specification describes. Please refer specifically to the [section] in specs/[path] which states: [quote from spec].
   ```

2. **Break Down the Task**:
   ```
   Let's focus solely on implementing [specific part] of the specification first, exactly as described in specs/[path].
   ```

3. **Reference the Architectural Decisions**:
   ```
   Please make sure your implementation follows the architectural patterns described in specs/architecture/[relevant file].
   ```

### Balancing Specs vs. Implementation Reality

Sometimes implementation realities differ from specifications:

```
I've encountered a challenge implementing [feature] as specified. The specification says [quote], but I've found that [issue]. What's the best way to address this while maintaining the intent of the specification?
```

---

Use this guide as a reference when working with Cline on the Ratio project. By following these patterns and best practices, you'll get more effective assistance and maintain consistency with the project's spec-driven approach.
