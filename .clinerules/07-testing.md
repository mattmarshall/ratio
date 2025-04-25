# Testing Standards

## Overview

Ratio employs a comprehensive testing strategy to ensure the reliability, correctness, and performance of the application. This document outlines the testing standards and approaches to follow when developing for the project.

## Testing Philosophy

- Testing is an integral part of the development process, not an afterthought
- Write tests alongside implementation code
- Test both the happy path and edge cases
- Tests serve as documentation of expected behavior
- Maintain a high level of test coverage for critical components

## Test Types

### Unit Tests

- Focus on testing individual functions, methods, and classes in isolation
- Mock dependencies to isolate the code under test
- Keep unit tests fast and focused
- Use test doubles (mocks, stubs, fakes) to control test conditions
- Verify behavior, not implementation details when possible

### Integration Tests

- Test interactions between components
- Verify that components work together correctly
- Use test containers for database and external service testing
- Focus on API contracts and component interactions
- Test realistic scenarios across component boundaries

### Property-Based Tests

- Use property-based testing for mathematical and financial calculations
- Define invariants and properties that should hold true for all inputs
- Automatically generate test cases using tools like proptest (Rust) or hypothesis (Python)
- Particularly important for Money type operations and accounting rules
- Include edge cases like currency conversion, rounding, and large numbers

### End-to-End Tests

- Verify complete user workflows
- Test the application as a whole
- Focus on critical business flows
- Include both happy path and error scenarios
- Validate that all components work together correctly

## Testing Tools

### Rust Testing

- Use the built-in `#[test]` attribute for unit tests
- Organize tests in a `tests` module or separate test files
- Use `proptest` for property-based testing
- Consider `mockall` for mocking in unit tests
- Use `rstest` for parameterized tests where appropriate

### Python Testing

- Use pytest as the testing framework
- Use pytest fixtures for test setup
- Use hypothesis for property-based testing
- Use pytest-mock for mocking
- Organize tests in a consistent directory structure

## Test Coverage

- Aim for >80% code coverage for critical components
- Focus on testing business logic thoroughly
- Ensure all error paths are tested
- Don't sacrifice test quality for coverage metrics
- Regularly review coverage reports to identify gaps

## Test Data Management

- Create reusable test fixtures and factories
- Use realistic but simplified test data
- Avoid hardcoding test values when possible
- Reset test state between tests
- Isolate tests from each other

## Financial Testing Specific Guidelines

- Verify that double-entry constraints are maintained
- Test currency conversion and money calculations thoroughly
- Include tests for financial edge cases (e.g., rounding, large transactions)
- Verify that financial reports produce correct results
- Test all accounting rules and constraints

## Testing Workflow

1. **Specification Review**
   - Begin by understanding the requirements from specifications
   - Identify the key behaviors to test
   - Note edge cases and constraints

2. **Test Planning**
   - Determine appropriate test types (unit, integration, etc.)
   - Plan test scenarios and cases
   - Consider test data requirements

3. **Test Implementation**
   - Write tests before or alongside implementation
   - Start with happy path tests, then add edge cases
   - Include negative tests for error conditions

4. **Test Execution**
   - Run tests frequently during development
   - Run the full test suite before submitting changes
   - Fix failing tests before continuing development

5. **Test Review**
   - Review tests as part of code review
   - Ensure tests are comprehensive and readable
   - Check that tests clearly specify expected behavior

## Working with Cline on Testing

When asking Cline to help with testing:

1. Reference these guidelines for test structure and approach
2. Specify which type of test you're working on
3. Provide context about the component or feature being tested
4. Include relevant specifications to ensure tests verify requirements

Example prompt:

```
Please help me create comprehensive unit tests for the Transaction service in ratio-kernel, following our testing standards. The tests should:
1. Verify the service correctly implements the requirements in specs/features/transactions/transaction-management.md
2. Include property-based tests for financial calculations
3. Test error conditions and edge cases
4. Mock dependencies appropriately
```

## Testing Checklist

Before considering testing complete, ensure:

- [ ] All requirements from specifications are covered by tests
- [ ] Happy path and error cases are tested
- [ ] Performance-critical components have benchmark tests
- [ ] Tests are readable and well-organized
- [ ] Tests run reliably without flakiness
- [ ] Test coverage meets the target for the component
- [ ] Financial calculations are thoroughly tested, including edge cases
