# Documentation Requirements

## Overview

Documentation is a core part of the Ratio project, treated as a first-class deliverable alongside code. This document outlines the documentation requirements and best practices to follow.

## Documentation Types

### 1. Specifications

Specifications in the `specs/` directory are the primary source of documentation and should be:
- Kept up-to-date as the project evolves
- Referenced in code comments
- Used as the foundation for implementation
- Reviewed alongside code changes

### 2. Code Documentation

- **Public API**: All public functions, types, and modules must be documented
- **Internal Components**: Document complex or non-obvious implementation details
- **Comments**: Focus on explaining "why" rather than "what"
- **Examples**: Include examples in documentation for common usage patterns

### 3. README Files

- Project-level README should provide:
  - Clear project overview
  - Installation instructions
  - Basic usage examples
  - Links to detailed documentation
  - Contribution guidelines
  
- Component-level READMEs should provide:
  - Component purpose and responsibility
  - Usage examples specific to the component
  - Implementation details relevant to developers
  - Links to related specifications

### 4. Architecture Documentation

- Document architectural decisions in specifications
- Create diagrams to illustrate component interactions
- Keep architectural documentation synchronized with implementation
- Document tradeoffs and alternatives considered

## Documentation Standards

### Markdown Guidelines

- Use consistent formatting across all Markdown files
- Structure documents with clear heading hierarchy
- Include a table of contents for longer documents
- Use code blocks with language identifiers for syntax highlighting
- Link to other documentation when referencing related concepts

### Code Documentation Style

For Rust code:
- Use rustdoc-compatible comments
- Document all public items
- Include examples where appropriate
- Document error conditions and handling
- Reference specifications where relevant

For Python code:
- Use Google-style docstrings
- Include type hints in docstrings and code
- Document parameters, return values, and exceptions
- Provide usage examples for complex functions

## Documentation Workflow

### When Implementing New Features

1. Start with the specification in `specs/`
2. Create or update code documentation as you implement
3. Update component READMEs if necessary
4. Ensure documentation is reviewed alongside code

### When Changing Existing Features

1. Update the specification to reflect changes
2. Update code documentation to match implementation
3. Update examples if necessary
4. Document any migration steps for users

### When Fixing Bugs

1. Document the root cause in the bug fix PR
2. Update documentation if the bug was due to unclear or incorrect documentation
3. Consider adding examples to prevent similar mistakes

## Working with Cline on Documentation

When asking Cline to help with documentation:

1. Reference existing documentation style and format
2. Specify which type of documentation you're creating/updating
3. Provide context about the feature or component being documented
4. Ask for specific documentation elements (examples, diagrams, etc.)

Example prompt:

```
Please help me update the documentation for the Transaction service according to our documentation standards. We've added new functionality for recurring transactions and need to:
1. Update the service's public API documentation
2. Add examples showing how to create different types of recurring transactions
3. Update the relevant specification in specs/features/transactions/
```

## Documentation Checklist

Before considering documentation complete, ensure:

- [ ] All public APIs are documented
- [ ] Specifications are up-to-date
- [ ] Examples cover common use cases
- [ ] Complex logic is explained
- [ ] README files are current
- [ ] Documentation builds without errors
- [ ] Links to other documentation work correctly
- [ ] Documentation follows the project's style guidelines

Good documentation is critical for both users and developers. It should be treated with the same level of care and attention as code.
