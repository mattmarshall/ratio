# Ratio Project Overview

## Project Description

Ratio is an open-source personal finance tool inspired by GnuCash but focused on implementing only the necessary features for effective household financial management. Built with a hybrid Rust/Python architecture, Ratio provides a fast, efficient CLI/TUI interface while allowing extensibility through Python modules.

## Etymology and Philosophy

The name "Ratio" draws from its Latin roots, where it meant not just "proportion" or "reason" but also "calculation", "account", and "reckoning" in ancient Roman finance. Romans would ask citizens to present their accounts in symmetrical tablets to achieve "parem rationem" — ensuring that credits and debits were accurately balanced. This concept of balance and symmetry in accounting reflects both the mathematical precision and ethical dimension of proper financial management that this application strives to embody.

## Core Development Principles

1. **Spec-First Development**: Write specifications before code to ensure clear understanding
2. **Modular Architecture**: Build components with clear boundaries and well-defined interfaces
3. **Test-Driven Development**: Write tests alongside implementation
4. **Progressive Refinement**: Start with MVP features and refine based on feedback
5. **Documentation as Code**: Maintain specifications as a core part of the codebase

## Key Components

- **Accounting Kernel**: Core double-entry bookkeeping system written in Rust
- **gRPC API Layer**: Service definitions connecting the kernel to the UI
- **Terminal UI (TUI)**: User interface built with tui-rs and crossterm
- **Extension System**: Python extension capabilities
- **PostgreSQL Database**: Persistence layer with proper financial integrity

## Current Focus

The project is currently in Phase 1 (MVP) which includes implementing:
- Core accounting kernel
- Basic TUI interface
- Initial gRPC API
- PostgreSQL schema
- Essential financial management features

When working with Cline on this project, always reference the appropriate specifications in the specs/ directory and adhere to the development principles outlined above.
