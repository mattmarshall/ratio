# Development Guide for Ratio

This guide outlines the development process for Ratio, with a focus on spec-driven development and best practices.

## Table of Contents
- [Development Guide for Ratio](#development-guide-for-ratio)
  - [Table of Contents](#table-of-contents)
  - [Development Philosophy](#development-philosophy)
  - [Spec-Driven Development](#spec-driven-development)
    - [Specification Types](#specification-types)
    - [Benefits of Spec-Driven Development](#benefits-of-spec-driven-development)
  - [Development Workflow](#development-workflow)
  - [Working with Specifications](#working-with-specifications)
    - [Creating a New Specification](#creating-a-new-specification)
    - [Updating Existing Specifications](#updating-existing-specifications)
    - [Specification Quality Checklist](#specification-quality-checklist)
  - [Implementation Guidelines](#implementation-guidelines)
    - [Code Organization](#code-organization)
    - [Code Style](#code-style)
    - [Implementation Checklist](#implementation-checklist)
  - [Testing Approach](#testing-approach)
    - [Test Types](#test-types)
    - [Testing Guidelines](#testing-guidelines)
  - [Working with Cline](#working-with-cline)
    - [Best Practices for LLM-Assisted Development](#best-practices-for-llm-assisted-development)
    - [Example Workflow with Cline](#example-workflow-with-cline)

## Development Philosophy

Ratio follows these core development principles:

1. **Spec-First Development**: Write specifications before code to ensure clear understanding
2. **Modular Architecture**: Build components with clear boundaries and well-defined interfaces
3. **Test-Driven Development**: Write tests alongside implementation
4. **Progressive Refinement**: Start with MVP features and refine based on feedback
5. **Documentation as Code**: Maintain specifications as a core part of the codebase

## Spec-Driven Development

Ratio uses a comprehensive specification system as the foundation for all development:

### Specification Types

1. **Architecture Specs**: System-wide technical decisions (tech stack, data model, API design)
   - Define overall system architecture and technology choices
   - Located in `specs/architecture/`
   - Focus on system-wide patterns and standards

2. **Feature Specs**: User-facing functional requirements and behaviors
   - Focus on what the user can do with the system
   - Include user stories, acceptance criteria, and functional requirements
   - Describe requirements from the user's perspective
   - Located in `specs/features/`
   - Example: Account tracking, transaction management, scheduling

3. **Component Specs**: Technical design of internal system components
   - Focus on how the system implements functionality
   - Detail internal APIs, data structures, and algorithms
   - Describe implementation details from a developer's perspective
   - Located in `specs/components/`
   - Can be hierarchical (breaking larger components into smaller ones)
   - Example: Accounting kernel, money handling, extension system

4. **Iteration Specs**: Work breakdown and planning for development phases
   - Define scope and timeline for development iterations
   - List features and technical tasks to be implemented
   - Include success criteria and risk mitigations
   - Located in `specs/iterations/`

### Benefits of Spec-Driven Development

- **Clarity**: Clear documentation of what we're building and why
- **Focus**: Reduced scope creep and feature bloat
- **Collaboration**: Easier communication among team members
- **LLM Assistance**: Specifications provide context for Cline and other AI tools
- **Quality**: Well-defined expectations lead to better implementations

## Development Workflow

The typical development workflow for Ratio follows these steps:

1. **Specification Creation**
   - Create a new specification using the appropriate template
   - For feature specs, focus on user stories and requirements
   - For component specs, focus on technical design and interfaces
   - Get feedback and iterate on the specification

2. **Implementation Planning**
   - Break down the work into manageable tasks
   - Create issues/tickets for each task
   - Assign tasks to iterations based on priority

3. **Implementation**
   - Write tests that verify the specification requirements
   - Implement the feature/component according to the specification
   - Document any deviations or decisions made during implementation

4. **Review**
   - Ensure the implementation meets the specification requirements
   - Update the specification if necessary to reflect actual implementation
   - Get code review from other team members

5. **Integration**
   - Merge changes into the main branch
   - Update any dependent components
   - Verify the integrated changes work as expected

## Working with Specifications

### Creating a New Specification

1. Choose the appropriate specification type (feature, component, architecture, iteration)
2. Copy the template from `specs/templates/` to the appropriate directory
3. Fill in all sections of the template with detailed information
4. Add the specification to the relevant listing in `specs/README.md`

### Updating Existing Specifications

1. When requirements change, update the specification first
2. Highlight changes in PRs when modifying specifications
3. Ensure code and specifications remain in sync
4. Use specifications as living documents that evolve with the project

### Specification Quality Checklist

Ensure your specifications:
- [ ] Are complete with all sections filled out
- [ ] Include clear user stories (for feature specs)
- [ ] Provide detailed technical design (for component specs)
- [ ] Define clear acceptance criteria
- [ ] Identify dependencies and constraints
- [ ] Document any open questions or decisions
- [ ] Link to related specifications

## Implementation Guidelines

When implementing features or components:

### Code Organization

- Follow the project structure defined in README.md
- Keep components modular and focused on a single responsibility
- Use clear, descriptive names for files, modules, and functions

### Code Style

- Follow Rust's official style guidelines
- Use consistent naming conventions
- Write clear comments explaining *why* not just *what*
- Document public APIs thoroughly

### Implementation Checklist

- [ ] All acceptance criteria from the specification are met
- [ ] Tests cover the functionality defined in the spec
- [ ] Code follows project style guidelines
- [ ] Documentation is updated (including inline docs)
- [ ] Any deviations from the spec are documented and justified

## Testing Approach

Ratio uses a comprehensive testing strategy:

### Test Types

- **Unit Tests**: Test individual functions and methods
- **Integration Tests**: Test interactions between components
- **Property-Based Tests**: Test invariants and properties of the system
- **End-to-End Tests**: Test complete user workflows

### Testing Guidelines

- Write tests alongside implementation, not after
- Use tests to verify specification requirements
- Aim for high test coverage, especially for critical components
- Use property-based tests for complex financial calculations
- Mock external dependencies when testing components in isolation

## Working with Cline

Ratio is designed to be developed with assistance from Cline and other LLM tools:

### Best Practices for LLM-Assisted Development

- Provide specifications as context when asking for implementation help
- Break down complex tasks into smaller, more manageable pieces
- Review and understand all generated code before committing
- Use LLMs for code generation, refactoring, and documentation
- See [CLINE.md](CLINE.md) for specific guidance on working with Cline

### Example Workflow with Cline

1. Share the relevant specification with Cline
2. Ask for implementation suggestions or code generation
3. Review, test, and refine the generated code
4. Document any insights or patterns for future reference

For detailed guidance on working with Cline specifically for this project, see [CLINE.md](CLINE.md).
