# Contributing to Ratio

Thank you for your interest in contributing to Ratio! This document provides guidelines and instructions for contributing to the project.

## Table of Contents
- [Contributing to Ratio](#contributing-to-ratio)
  - [Table of Contents](#table-of-contents)
  - [Code of Conduct](#code-of-conduct)
  - [How to Contribute](#how-to-contribute)
  - [Development Process](#development-process)
  - [Pull Request Process](#pull-request-process)
    - [Pull Request Checklist](#pull-request-checklist)
  - [Coding Standards](#coding-standards)
    - [Rust Guidelines](#rust-guidelines)
    - [Python Guidelines](#python-guidelines)
    - [General Guidelines](#general-guidelines)
  - [Commit Message Guidelines](#commit-message-guidelines)
    - [Format](#format)
    - [Types](#types)
    - [Example](#example)
  - [Issue Reporting Guidelines](#issue-reporting-guidelines)
  - [Documentation](#documentation)
  - [Community](#community)

## Code of Conduct

We are committed to providing a friendly, safe, and welcoming environment for all contributors. By participating in this project, you agree to abide by our code of conduct:

- Be respectful and inclusive
- Exercise empathy and kindness
- Be open to constructive feedback
- Focus on what is best for the community
- Show courtesy and respect towards other community members

## How to Contribute

There are many ways to contribute to Ratio:

- Implementing new features
- Fixing bugs
- Improving documentation
- Writing tests
- Reviewing pull requests
- Reporting issues
- Suggesting enhancements

For detailed information on the development process, see [DEVELOPING.md](DEVELOPING.md).

## Development Process

1. **Fork the Repository**: Create your own fork of the project
2. **Create a Branch**: Create a branch for your changes
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Implement Changes**: Make your changes following the [development guide](DEVELOPING.md)
4. **Run Tests**: Ensure all tests pass
   ```bash
   cargo test
   ```
5. **Submit a Pull Request**: Push your changes and create a pull request

## Pull Request Process

1. **Follow the Template**: Use the provided pull request template
2. **Link Related Issues**: Reference any related issues
3. **Update Documentation**: Ensure documentation is updated
4. **Include Tests**: Add tests for new features or fixes
5. **Request Review**: Request review from maintainers
6. **Respond to Feedback**: Address review comments promptly
7. **Continuous Integration**: Ensure CI checks pass

### Pull Request Checklist

- [ ] I have read the [CONTRIBUTING.md](CONTRIBUTING.md) document
- [ ] I have updated the documentation as needed
- [ ] I have added tests that prove my fix/feature works
- [ ] I have updated specs to match implementation (if applicable)
- [ ] New and existing tests pass with my changes
- [ ] My changes follow the coding standards for this project
- [ ] I have linked any related issues

## Coding Standards

### Rust Guidelines

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use [Rustfmt](https://github.com/rust-lang/rustfmt) for code formatting
- Run [Clippy](https://github.com/rust-lang/rust-clippy) to catch common mistakes
- Add documentation comments to public APIs
- Write idiomatic Rust code

### Python Guidelines

- Follow [PEP 8](https://www.python.org/dev/peps/pep-0008/) style guide
- Use [Black](https://github.com/psf/black) for code formatting
- Document functions and classes with docstrings
- Use type hints where appropriate

### General Guidelines

- Write clear, descriptive variable and function names
- Keep functions small and focused on a single responsibility
- Comment complex logic but avoid obvious comments
- Prioritize readability and maintainability
- Follow the principle of least surprise

## Commit Message Guidelines

We follow a structured commit message format to make the project history clear and to enable automated changelog generation.

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- **feat**: A new feature
- **fix**: A bug fix
- **docs**: Documentation changes
- **style**: Changes that don't affect code functionality (formatting, etc.)
- **refactor**: Code changes that neither fix bugs nor add features
- **perf**: Performance improvements
- **test**: Adding or correcting tests
- **chore**: Changes to build process or auxiliary tools

### Example

```
feat(transactions): add recurring transaction support

Implement the ability to schedule recurring transactions with various
frequency options. This allows users to automate regular bills and income.

Closes #123
```

## Issue Reporting Guidelines

When reporting issues, please use the provided issue templates and include:

1. **Clear Title**: Concise description of the issue
2. **Detailed Description**: What happened, what you expected to happen
3. **Reproduction Steps**: Specific steps to reproduce the issue
4. **Environment Details**: OS, Rust version, etc.
5. **Additional Context**: Logs, screenshots, etc.

## Documentation

Good documentation is as important as good code. When contributing:

- Update relevant specification documents in the `specs/` directory
- Add inline documentation for new code
- Update README or other guide files for user-facing changes
- Consider creating diagrams for complex systems

See [DEVELOPING.md](DEVELOPING.md) for details on our spec-driven development approach.

## Community

- **Discussions**: Use GitHub Discussions for questions and ideas
- **Issues**: Use GitHub Issues for bugs and feature requests
- **Pull Requests**: Contribute code through GitHub Pull Requests
- **Code of Conduct**: Follow our Code of Conduct in all community interactions

---

Thank you for contributing to Ratio! Your efforts help make this project better for everyone.
