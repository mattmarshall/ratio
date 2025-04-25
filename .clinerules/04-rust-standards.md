# Rust Coding Standards

## Overview

Ratio's core components are written in Rust, including the accounting kernel, gRPC API layer, and terminal UI. This document outlines the Rust coding standards to follow when working on the project.

## Code Organization

### Project Structure

Ratio follows a Cargo workspace structure with multiple crates:

```
ratio/
├── ratio-kernel/       # Core accounting functionality
├── ratio-api/          # gRPC service implementations
├── ratio-tui/          # Terminal user interface components
├── ratio-common/       # Shared types and utilities
└── ratio/              # Main binary crate
```

- Place code in the appropriate crate based on its responsibility
- Keep crate dependencies clean (e.g., the kernel should not depend on the TUI)
- Use feature flags to control optional functionality

### Module Structure

- Use clear, descriptive module names
- Organize modules to reflect domain concepts
- Keep public API surface minimal
- Prefer smaller modules with focused responsibility
- Use module-level documentation to explain purpose

## Code Style

### General Guidelines

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` with the project's configuration
- Run `clippy` regularly and address warnings
- Maintain 100% warning-free code

### Naming Conventions

- Types (structs, enums, traits): `PascalCase`
- Variables, functions, methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`
- Crates: `kebab-case` for package names, `snake_case` for import names

### Documentation

- Document all public items
- Use examples in documentation where appropriate
- Explain "why" not just "what"
- Document any invariants, pre-conditions, or post-conditions
- Add references to specifications where relevant

## Error Handling

- Create domain-specific error types using `thiserror`
- Provide context with error chain
- Ensure errors are properly propagated up the call stack
- Log errors at appropriate levels
- Use `anyhow` for errors that don't need to be exposed in public APIs

## Financial Calculations

- Use the `Money` type for all financial calculations
- Never use floating-point for financial values
- Follow proper rounding rules for financial calculations
- Ensure proper handling of different currencies
- Validate financial operations for correctness

## Testing

- Write tests for all business logic
- Use property-based testing for financial calculations
- Create integration tests for API endpoints
- Mock external dependencies in unit tests
- Test edge cases and error conditions

## Performance

- Profile performance-critical code paths
- Use benchmarks to validate performance improvements
- Be mindful of memory allocations in hot paths
- Optimize database queries for common operations
- Use async appropriately for I/O-bound operations

## Concurrency

- Prefer message passing over shared state
- Use strong types to prevent data races
- Implement `Send` and `Sync` with care
- Document thread safety assumptions
- Use appropriate synchronization primitives

## Dependencies

- Review dependencies carefully before adding them
- Prefer well-maintained, actively developed crates
- Consider vendoring small dependencies if necessary
- Track dependency versions and keep them updated
- Document why each dependency is needed

## Working with Cline

When asking Cline to help with Rust code:

1. Reference these guidelines for code style and organization
2. Specify which crate the code belongs to
3. Provide context about the surrounding code
4. Ask for proper error handling and testing

Example prompt:

```
Please help me implement the Transaction service in the ratio-kernel crate, following our Rust coding standards. 
The service should:
1. Use proper error handling with thiserror
2. Include comprehensive unit tests
3. Follow our financial calculation patterns with the Money type
