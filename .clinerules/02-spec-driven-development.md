# Spec-Driven Development Guidelines

## Overview

Ratio uses a comprehensive specification system as the foundation for all development. This document outlines the approach to spec-driven development that should be followed when working with Cline on this project.

## Specification Types

1. **Architecture Specs** (`specs/architecture/`)
   - Define system-wide technical decisions
   - Include tech stack choices, data model, API design
   - Focus on system-wide patterns and standards

2. **Feature Specs** (`specs/features/`)
   - Define user-facing functional requirements and behaviors
   - Focus on what the user can do with the system
   - Include user stories, acceptance criteria, and functional requirements
   - Describe requirements from the user's perspective

3. **Component Specs** (`specs/components/`)
   - Define technical design of internal system components
   - Focus on how the system implements functionality
   - Detail internal APIs, data structures, and algorithms
   - Can be hierarchical (breaking larger components into smaller ones)

4. **Iteration Specs** (`specs/iterations/`)
   - Define work breakdown and planning for development phases
   - List features and technical tasks to be implemented
   - Include success criteria and risk mitigations

## Development Workflow

When implementing features in Ratio, always follow this workflow:

1. **Specification Creation/Review**
   - Start by reviewing the existing specification in the specs/ directory
   - If no specification exists, create one using the appropriate template
   - For feature specs, focus on user stories and requirements
   - For component specs, focus on technical design and interfaces

2. **Implementation Planning**
   - Break down the work into manageable tasks
   - Create a development plan based on the specification
   - Identify dependencies and potential challenges

3. **Implementation**
   - Write tests that verify the specification requirements
   - Implement the feature/component according to the specification
   - Document any deviations or decisions made during implementation

4. **Review**
   - Ensure the implementation meets the specification requirements
   - Update the specification if necessary to reflect actual implementation

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

### Specification Quality Checklist

Ensure specifications:
- Are complete with all sections filled out
- Include clear user stories (for feature specs)
- Provide detailed technical design (for component specs)
- Define clear acceptance criteria
- Identify dependencies and constraints
- Document any open questions or decisions
- Link to related specifications

## Working with Cline on Spec-Driven Development

When asking Cline for assistance:

1. **Reference the Relevant Specification**
   ```
   Please help me implement [feature] according to the specs/features/[feature].md specification.
   ```

2. **Break Down Implementation**
   ```
   Let's implement the [feature] specified in specs/features/[path]. Let's start with:
   1. First, define the data models
   2. Then implement the core business logic
   3. Finally, create the API endpoints
   ```

3. **Start with Structure**
   ```
   Based on specs/[path], please outline the files we'll need to modify, the functions we'll need to create, and the overall approach for implementing [feature].
   ```

4. **Test-Driven Development**
   ```
   Before implementing [feature], please help me write tests based on the requirements in specs/features/[path]. Focus particularly on the acceptance criteria.
   ```

Always ensure Cline's implementations are aligned with the specifications and follow the project's established patterns and principles.
