# Technology Stack Specification

## Overview
This document outlines the technology choices for Ratio, a high-performance CLI/TUI personal finance application. The stack is designed to provide optimal performance for core financial operations while maintaining extensibility for future features.

## Core Technologies

### Programming Languages
- **Rust**: Primary language for the core components, providing memory safety, performance, and concurrency
- **Python**: Used for the extension system, allowing for user customization and rapid development of specialized features

### Database
- **PostgreSQL 15+**: Primary data store, selected for:
  - Strong ACID compliance for financial data integrity
  - JSON/JSONB support for flexible data structures
  - Robust query optimization
  - Transaction support
  - Mature ecosystem

### UI Frameworks
- **TUI Framework**: tui-rs with crossterm for terminal-based user interface
  - Provides responsive, keyboard-driven interface
  - Supports complex layouts and custom widgets
  - Cross-platform compatibility
  - Enables low-level control for custom financial widgets

### API & Communication
- **gRPC**: For service definitions and client-server communication using Tonic
  - Strongly typed API with Protocol Buffers
  - Efficient binary serialization
  - Support for streaming for real-time updates
  - Code generation for multiple languages

### Development & Deployment
- **Docker**: For containerized development and deployment
  - Consistent environments across development and production
  - Easy setup for new contributors
  - Multi-stage builds with Alpine/distroless base images for production
  - Optimized container size and startup time
- **sqlx**: For database access and migrations management
  - Type-safe SQL in Rust with sqlx-cli for migrations
  - Compile-time verification of SQL queries
  - Connection pooling and transaction management
- **GitHub Actions**: For continuous integration and deployment
  - Automated testing and building
  - Docker image creation and optimization
  - Release management and versioning

## Component Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│                 │    │                 │    │                 │
│    CLI/TUI      │◄───┤   gRPC API      │◄───┤   Accounting    │
│    (Rust)       │    │   Layer (Rust)  │    │   Kernel (Rust) │
│                 │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                      ▲
                                                      │
                                                      ▼
┌─────────────────┐                          ┌─────────────────┐
│                 │                          │                 │
│   Extensions    │◄─────────────────────────┤   PostgreSQL    │
│   (Python)      │                          │   Database      │
│                 │                          │                 │
└─────────────────┘                          └─────────────────┘
```

## Project Structure

The application is organized as a Cargo workspace with multiple crates to promote modularity and clear separation of concerns:

```
ratio/
├── crates/
│   ├── ratio-kernel/    # Core accounting functionality
│   ├── ratio-api/       # gRPC service implementations
│   ├── ratio-tui/       # Terminal UI components
│   ├── ratio-common/    # Shared types and utilities
│   └── ratio/           # Main binary crate with subcommands
```

### Crate Responsibilities

- **ratio-common**: Contains shared types, utilities, and the Money type implementation used across all crates
- **ratio-kernel**: Implements the core accounting engine with domain models and business logic
- **ratio-api**: Provides gRPC services that expose kernel functionality
- **ratio-tui**: Implements the terminal user interface
- **ratio**: Main binary crate that serves as the entry point with subcommand architecture

### Binary Architecture

Ratio uses a single-binary approach with Git-like subcommands:

```
ratio                   # Main executable
├── account             # Account management subcommands
│   ├── create          # Create a new account
│   ├── list            # List accounts
│   └── ...
├── transaction         # Transaction subcommands
│   ├── add             # Add a new transaction
│   ├── search          # Search transactions
│   └── ...
├── report              # Reporting subcommands
├── schedule            # Scheduled transaction subcommands
└── server              # Run in server mode
```

This approach provides a unified CLI experience while maintaining modular code organization.

## Technology Choices Rationale

### Rust for Core Components
- **Memory Safety**: Prevents common bugs like null pointer dereferences and buffer overflows
- **Performance**: Near-C performance without manual memory management
- **Concurrency**: Safe concurrent programming with Rust's ownership model
- **Zero-Cost Abstractions**: High-level APIs without runtime overhead
- **Strong Typing**: Catch errors at compile time rather than runtime

### Python for Extensions
- **Rapid Development**: Fast prototyping and iteration for extensions
- **Rich Ecosystem**: Access to data science, finance, and machine learning libraries
- **Accessibility**: Lower barrier to entry for community contributors
- **Flexibility**: Dynamic typing for more flexible extension APIs

### PostgreSQL for Data Storage
- **Reliability**: Mature, proven database with strong consistency guarantees
- **Performance**: Efficient for both transactional and analytical workloads
- **Advanced Features**: Arrays, JSONB, full-text search, and more
- **Extensibility**: Custom types and functions when needed

### gRPC for Communication
- **Performance**: Efficient binary communication protocol
- **Strong Typing**: Defined service contracts with Protocol Buffers
- **Language Agnostic**: Client libraries for many languages
- **Streaming Support**: For real-time updates and data feeds

## Version Requirements

- Rust: 1.70+
- Python: 3.9+
- PostgreSQL: 15+
- Docker & Docker Compose: Latest stable

## Library Requirements

- **Core Libraries**:
  - sqlx: Database access with compile-time checked queries
  - tonic: gRPC implementation
  - tui-rs and crossterm: Terminal user interface
  - PyO3: Python extension system integration
  
- **Supporting Libraries**:
  - serde: Serialization and deserialization
  - tokio: Async runtime
  - tracing: Logging and instrumentation
  - chrono: Date and time handling

## Development Environment

The project includes a Docker Compose setup for consistent development:

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

## Future Technology Considerations

- **Web Dashboard**: Potential future component using a modern frontend framework
- **Mobile App**: Native mobile clients that consume the gRPC API
- **Cloud Deployment**: Infrastructure as code for cloud hosting
- **Machine Learning**: For transaction categorization and financial insights
