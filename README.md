# Ratio

A high-performance CLI/TUI personal finance application built with Rust and Python for optimal family financial management.

## Etymology and Philosophy

The name "Ratio" draws from its Latin roots, where it meant not just "proportion" or "reason" but also "calculation", "account", and "reckoning" in ancient Roman finance. Romans would ask citizens to present their accounts in symmetrical tablets to achieve "parem rationem" — ensuring that credits and debits were accurately balanced. This concept of balance and symmetry in accounting reflects both the mathematical precision and ethical dimension of proper financial management that this application strives to embody.

## Overview

Ratio is an open-source personal finance tool inspired by GnuCash but focused on implementing only the necessary MVP features for effective household financial management. Built with a hybrid Rust/Python architecture, Ratio provides a fast, efficient CLI/TUI interface while allowing extensibility through Python modules.

For detailed design documentation, see the [specs directory](specs/README.md).

## Core Features

- **[Account Tracking](specs/features/accounts/account-tracking.md)**: Track multiple accounts (checking, savings, investments) in a unified interface
- **Liability Management**: Monitor debts, loans, and credit card balances
- **[Transaction Management](specs/features/transactions/transaction-management.md)**: Record and categorize all financial transactions
- **Balance Optimization**: Calculate required daily balances to meet all scheduled expenses while maximizing investments and high-yield savings
- **[Transaction Scheduling](specs/features/scheduling/transaction-scheduling.md)**: Set up recurring transactions for bills, subscriptions, and income
- **Double-Entry Bookkeeping**: Maintain accurate financial records with built-in validation
- **Data Visualization**: Terminal-based reports and charts for financial insights

## Technical Architecture

Ratio uses a modular, layered architecture with clear separation of concerns:

```
CLI/TUI (Rust) ↔ gRPC API Layer (Rust) ↔ Accounting Kernel (Rust) ↔ PostgreSQL
                                           ↕
                                      Extensions (Python)
```

### Core Components

- **[Accounting Kernel](specs/components/kernel/accounting-kernel.md)**: Core engine for managing books, accounts, and transactions
- **[Terminal UI](specs/components/tui/terminal-interface.md)**: User interface built with tui-rs and crossterm
- **API Layer**: gRPC services for communication between components
- **Extension System**: Python modules that hook into the accounting kernel via PyO3
- **Rules Engine**: System for defining custom accounting rules
- **PostgreSQL Database**: Primary data store with double-entry bookkeeping schema

For detailed technical specifications, see:
- [Technology Stack](specs/architecture/tech-stack.md)
- [Data Model](specs/architecture/data-model.md)
- [API Design](specs/architecture/api-design.md)

## Development Roadmap

Ratio is being developed in phases:

- **Phase 1: [MVP (Current Focus)](specs/iterations/iteration-1-mvp.md)** - Core accounting kernel, TUI, and essential features
- **Phase 2: Enhanced Features** - Subscription detection, receipt scanning, advanced forecasting
- **Phase 3: Extended Ecosystem** - Mobile app, investment tracking, tax preparation

See the [iteration plans](specs/iterations/) for detailed development roadmaps.

## Development

### Prerequisites
- Rust 1.70+
- Python 3.9+
- PostgreSQL 15+
- Docker & Docker Compose (for local development)

### Local Development Setup
```bash
# Clone repository
git clone https://github.com/yourusername/ratio.git
cd ratio

# Start the PostgreSQL container
docker-compose up -d

# Install dependencies
cargo build
pip install -r requirements.txt

# Run migrations
cargo run --bin migration

# Run development version
cargo run
```

### Docker Development Environment

The project includes a Docker Compose setup for local development:

```yaml
# docker-compose.yml
version: '3.8'
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_USER: ratio
      POSTGRES_PASSWORD: ratio_dev
      POSTGRES_DB: ratio_dev
    volumes:
      - ratio_pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"

volumes:
  ratio_pgdata:
```

## Project Structure

```
ratio/
├── specs/                  # Detailed specifications
├── src/                    # Rust source code
│   ├── kernel/             # Accounting kernel
│   ├── api/                # gRPC API implementation
│   ├── db/                 # Database interface
│   └── ui/                 # TUI components
├── python/                 # Python modules
├── protos/                 # Protocol buffer definitions
├── docker/                 # Docker configuration
└── tests/                  # Test suite
```

For detailed component specifications, see the [specs directory](specs/README.md).

## Documentation

Ratio uses a comprehensive specification system to document all aspects of the project:

- **Architecture Specs**: System-wide architectural decisions
- **Feature Specs**: Detailed requirements for user-facing features
- **Component Specs**: Technical design of system components
- **Iteration Plans**: Work breakdown for development phases

These specifications help guide development and ensure consistency across the codebase. See the [specs README](specs/README.md) for more information on how to use and contribute to the documentation.

## Development Workflow

When working on Ratio:

1. Start by reviewing the relevant specifications in the `specs/` directory
2. For new features, create a spec first following the templates in `specs/templates/`
3. Implement according to the specifications
4. Update specs as needed when design decisions change during implementation
5. Include links to relevant specs in PRs and commit messages

## Contributing

Contributions are welcome! This project is intended to be developed with assistance from LLM tools like Cline. When contributing:

1. Focus on modular design for easier AI-assisted development
2. Document design decisions clearly in the specs directory
3. Include comprehensive tests for all features
4. Follow the established code style guidelines

## License

MIT License - See LICENSE file for details
