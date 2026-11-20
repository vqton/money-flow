# DEVELOPER GUIDELINES

This document outlines the development standards for the project, ensuring safety, maintainability, and strict adherence to financial integrity.

---

## R1. Rust Style Guide
- **Naming Conventions**: Use `snake_case` for modules, functions, variables, and constants. Use `PascalCase` for types, structs, enums, and traits.
- **Module Structure**: Keep modules logical. Deep hierarchies should be avoided unless necessary.
- **Error Handling**:
  - Use custom error types (using `thiserror`) for domain-specific errors.
  - Use `anyhow` for top-level application entry points and CLI tools where context wrapping is preferred over structured error handling.
- **Mandatory Documentation Template (Business-Critical)**:
  Every business-critical function (especially those modifying `LedgerEntry` or `Balance`) must follow this template:
  ```rust
  /// # VAS Rationale
  /// [Clearly explain the business rule, VAS principle, or compliance requirement applied here.]
  ///
  /// # Arguments
  /// [Description of arguments]
  ///
  /// # Returns
  /// [Description of return value]
  ///
  /// # Panics/Errors
  /// [Document potential errors or panics]
  ```
- **Tooling**:
  - `rustfmt`: Use standard configuration.
  - `clippy`: Enabled in CI with `deny(warnings)`.

## R2. Testing Strategy
- **Testing Pyramid**:
  - **Unit Tests**: Mandatory for all domain logic. **100% coverage required** for `LedgerEntry` and `Balance` calculation logic.
  - **Integration Tests**: Required for all database interactions and SQL execution to ensure ACID compliance.
  - **Property-based Tests**: Use `proptest` for all financial arithmetic to ensure robustness across ranges.
- **Coverage**: CI will fail if coverage for `LedgerEntry` and `Balance` is below 100%.

## R3. Financial Safety
- **No Floating Point**: Absolutely no `f32` or `f64` for financial values. Use `rust_decimal` for all currency calculations.
- **Audit Trail**: Every function that modifies a `LedgerEntry` or `Balance` must be marked with the `#[audit_trail]` attribute (or comment-based tracking if an attribute isn't applicable) to ensure traceability.

## R4. Documentation Standards
- **SQL Migrations**:
  - All migrations must be documented in the `migrations/` folder.
  - Each migration file must have a corresponding `README.md` explaining:
    - What the migration does (UP/DOWN actions).
    - Impact on ACID compliance.
- **Accountability**:
  - Every crate/module must include a `README.md` detailing:
    - **What**: Functionality provided.
    - **Why**: Rationale for existence.
    - **Side Effects**: Any dependencies or external impacts.

## R5. Code Review Checklist
All PRs must be verified against this 5-point checklist:
1. **Does it break ACID compliance?** (Check SQL and transactional code)
2. **Is the VAS standard clearly referenced?** (Are business-critical functions documented with the template?)
3. **Are tests updated?** (Do they meet coverage and property-based testing requirements?)
4. **Are decimals handled?** (Any floating-point usage identified?)
5. **Is the documentation updated?** (Are crates/modules/migrations updated?)
