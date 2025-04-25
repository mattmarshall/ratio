# Python Extension Guidelines

## Overview

Ratio uses Python for its extension system, allowing users to create custom reports, implement specialized financial tools, and automate workflows. This document outlines the guidelines for developing Python extensions for Ratio.

## Extension Architecture

Python extensions integrate with Ratio's core accounting kernel through a well-defined API:

```
Rust Accounting Kernel <--> PyO3 Bindings <--> Python Extensions
```

- The Rust kernel exposes functionality through PyO3 bindings
- Python extensions consume this API to extend functionality
- Extensions are loaded dynamically at runtime

## Extension Types

1. **Reports and Visualizations**
   - Custom financial reports
   - Data visualizations and charts
   - Export functionality to various formats

2. **Financial Analysis Tools**
   - Budget analysis
   - Investment calculations
   - Debt reduction strategies

3. **Automation Rules**
   - Transaction categorization
   - Scheduled operations
   - Alert and notification systems

4. **Data Import/Export**
   - Import from financial institutions
   - Export to accounting formats
   - Integration with external services

## Development Standards

### Python Version

- Use Python 3.9+ for all extensions
- Explicitly declare minimum Python version in setup.py or pyproject.toml
- Use type hints throughout

### Code Organization

- Follow a clear package structure:
  ```
  ratio_extension_name/
  ├── __init__.py        # Extension entry point
  ├── core.py            # Core functionality
  ├── models.py          # Data models
  ├── utils/             # Utility functions
  ├── tests/             # Test suite
  └── README.md          # Extension documentation
  ```

- Keep extension modules focused and single-purpose
- Use standard Python project structure

### Coding Style

- Follow PEP 8 style guidelines
- Use a consistent import style
- Format code with Black
- Use isort for import sorting
- Run flake8 for linting

### Dependency Management

- Minimize external dependencies
- Use a virtual environment for development
- Pin dependency versions for stability
- Document all dependencies and their purpose
- Prefer standard library solutions when available

### Documentation

- Document all public functions, classes, and methods
- Include usage examples in docstrings
- Provide a comprehensive README for each extension
- Document configuration options and parameters
- Document any assumptions or limitations

## API Integration

### Core API Usage

- Use the official Ratio Python API
- Do not bypass API to access data directly
- Respect API version compatibility
- Handle API errors gracefully
- Log API interactions for debugging

### Data Handling

- Use the `Money` class from ratio_common for all financial values
- Follow proper handling of currencies
- Validate all inputs before passing to the API
- Cache results appropriately for performance
- Be mindful of memory usage for large datasets

## Testing

- Write unit tests for all functionality
- Include integration tests with the Ratio API
- Use pytest as the testing framework
- Mock external dependencies and Ratio API for unit tests
- Test edge cases and error handling
- Aim for high test coverage

## Distribution

- Package extensions using setuptools or poetry
- Include proper metadata in setup.py or pyproject.toml
- Version extensions using semantic versioning
- Document installation instructions
- Include license information
- Publish to PyPI if appropriate

## Security Considerations

- Validate all user inputs
- Do not embed sensitive information in extension code
- Use secure defaults
- Follow least privilege principle when accessing data
- Document security considerations

## Working with Cline

When asking Cline to help with Python extensions:

1. Reference these guidelines for extension structure and standards
2. Specify which extension type you're working on
3. Provide context about how the extension will interact with Ratio's API
4. Request proper error handling and testing

Example prompt:

```
Please help me create a Python extension for Ratio that generates a custom budget report. The extension should:
1. Follow our Python extension guidelines
2. Use the Ratio API to fetch transaction data
3. Include proper error handling and comprehensive tests
4. Generate both text and HTML report formats
```

Always ensure extensions follow these guidelines and integrate smoothly with the Ratio core system.
