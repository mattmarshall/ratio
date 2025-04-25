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
- **TUI Framework**: Either tui-rs or cursive for terminal-based user interface
  - Provides responsive, keyboard-driven interface
  - Supports complex layouts and custom widgets
  - Cross-platform compatibility

### API & Communication
- **gRPC**: For service definitions and client-server communication
  - Strongly typed API with Protocol Buffers
  - Efficient binary serialization
  - Support for streaming for real-time updates
  - Code generation for multiple languages

### Development & Deployment
- **Docker**: For containerized development and deployment
  - Consistent environments across development and production
  - Easy setup for new contributors
- **sqlx-cli**: For database migrations management
  - Type-safe SQL in Rust
  - Compile-time verification of SQL queries

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
