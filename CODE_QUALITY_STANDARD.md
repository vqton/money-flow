# Code Quality & Documentation Standard

This document outlines the coding standards, documentation requirements, and design principles for the Rust accounting project located in `d:\money-flow`.

## 1. Core Principles

### 1.1 Maintainability
- **Module Structure**: Follow standard Rust project layout (`src/` for source, `tests/` for integration tests). Keep modules small, focused, and loosely coupled.
- **Dependency Management**: Minimize dependencies. Clearly justify any new dependency in `Cargo.toml`.
- **Formatting**: Strictly adhere to `rustfmt`.

### 1.2 Auditability
- **Immutable Transaction Logs**: All financial state changes must be recorded as immutable entries. Do not overwrite or modify existing transaction records.
- **Data Consistency**: Ensure all database operations are wrapped in ACID-compliant transactions.
- **Traceability**: Every transaction must have a unique identifier, timestamp, and metadata to trace its origin and actor.

### 1.3 Safety
- **Type Safety**: Never use `f64` for currency calculations. Use integer representations of the smallest currency unit (e.g., cents) or specialized decimal libraries (e.g., `rust_decimal`).
- **Database Safety**: Use parameterized queries (`rusqlite`) to prevent SQL injection.
- **Error Handling**: Use explicit `Result<T, E>` types for all fallible operations. Never use `panic!` for expected application errors.

## 2. Documentation Requirements

- **Code Comments**: All public functions and structs must be documented with `///` doc comments.
- **Architectural Decision Records (ADRs)**: Significant architectural choices must be documented in a `docs/adr/` directory using Markdown.
- **README**: The root `README.md` must be kept updated with setup instructions, key architectural overview, and contribution guidelines.

## 3. Testing Strategy

- **TDD (Test-Driven Development)**: Prefer writing tests before implementing new business logic.
- **Coverage**: Aim for high coverage on financial calculation modules.
- **Integration Testing**: Use the `tests/` directory to test full transaction workflows against a test SQLite database.
