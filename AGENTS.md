# AGENTS.md: Developer Guide & Invariants for AI Coding Agents

## 1. Project One-Liner & Prime Directive
`money-flow` is an audit-grade double-entry accounting engine written in Rust (2024 edition) compliant with Vietnamese statutory accounting standards (Circular 99/2025/TT-BTC, Circular 91/2026/TT-BTC, Law 88/2015/QH13).  
**Prime Directive**: Monetary amounts must strictly be `rust_decimal::Decimal` (never float); closed accounting periods are immutable; posted records are never physically deleted.

## 2. Verified Commands
Run all cargo commands against `backend/Cargo.toml` (root `Cargo.toml` is an unlinked placeholder):
```powershell
# Run all tests (42 passing: 18 unit, 21 integration, 3 property suites) [EXECUTED]
cargo test --manifest-path backend/Cargo.toml

# Unit tests only [EXECUTED]
cargo test --manifest-path backend/Cargo.toml --lib

# API HTTP integration tests only [EXECUTED]
cargo test --manifest-path backend/Cargo.toml --test api_tests

# Property-based fuzzing tests only (1,100 iterations) [EXECUTED]
cargo test --manifest-path backend/Cargo.toml --test property_tests

# Clippy linter with zero warnings tolerance [EXECUTED]
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings

# Code formatting check [EXECUTED - flags unformatted lines; run cargo fmt to fix]
cargo fmt --manifest-path backend/Cargo.toml --check

# Production optimized binary build (3.47 MB target/release/backend.exe) [EXECUTED]
cargo build --manifest-path backend/Cargo.toml --release

# Run daemon on 127.0.0.1:3000 [NOT EXECUTED in automated runs]
cargo run --manifest-path backend/Cargo.toml
```

## 3. Architecture Boundaries
Strict layered clean architecture (`backend/src/`):
* `models.rs` & `ledger.rs` (Domain Layer): Pure entities and invariant checks. **Must NEVER import** `axum`, `rusqlite`, or any I/O crate.
* `closing.rs` (Application Workflow): Statutory 4-step closing engine. Relies on domain types and account balance maps; no direct database connection handles.
* `database.rs` (Persistence Layer): SQLite connection mutex (`Arc<Mutex<Connection>>`), parameterized DDL/DML, transaction management, and audit logging.
* `dto.rs` & `handlers.rs` (Interface Layer): Axum 0.8 routing, payload validation, status mapping, and OpenAPI specification. Imports domain, application, and persistence types.
* `config.rs`: 12-factor environment configuration loader (`HOST`, `PORT`, `DATABASE_PATH`, `BACKUP_DIR`, `LOG_LEVEL`).

## 4. Financial Safety Rules (Non-Negotiable)
1. **Zero-Float Guarantee**: Use `rust_decimal::Decimal` for all amounts and account balances. Never use `f32` or `f64`. Store in SQLite columns as `TEXT`.
2. **Double-Entry Invariants (`backend/src/ledger.rs`)**: Every voucher must have $\ge 2$ lines, amounts $\ge 0$, no dual debit/credit on a single line, no zero lines (`debit == 0 && credit == 0`), and strict zero-sum equilibrium: $\sum \text{Debits} == \sum \text{Credits} > 0$. Never bypass `validate_voucher()`.
3. **Non-Destructive Lifecycle**: Never physically `DELETE` vouchers or voucher lines. Voiding sets `status = 'VOIDED'`, records timestamp, actor, and reason.
4. **Audit Trail**: Every state mutation (`CREATE_VOUCHER`, `VOID_VOUCHER`, `PERIOD_CLOSE`, `PERIOD_REOPEN`, `BACKUP`) must execute an atomic insert into `audit_logs` inside the same database transaction.
5. **Period Lock Integrity**: Any attempt to post or void transactions within a closed period (`accounting_periods.is_closed == 1`) must fail with `422 Unprocessable Entity` (`PERIOD_CLOSED` / `CANNOT_VOID_IN_CLOSED_PERIOD`). Never weaken or bypass period lock checks.

## 5. Security Rules
* **Parameterized SQL Only**: Every SQL statement must use parameterized binding (`rusqlite::params![...]`). Never use string interpolation or `format!` for SQL.
* **Secrets Management**: No API keys, passwords, or credentials in code, tests, or documentation.
* **Explicit Auth Decision**: The API currently lacks authentication/RBAC middleware. Never add public endpoints without documenting an explicit auth requirement.
* **Synthetic Test Data**: Fixtures and tests must use synthetic data only; never commit real customer, employee, or corporate accounting data.

## 6. Coding Conventions
* **Style & Naming**: Format with `rustfmt`. Use `snake_case` for functions/modules, `PascalCase` for structs/enums.
* **Documentation**: Provide `///` doc comments on all public structs, enums, errors, and public functions.
* **Error Handling**: Use `thiserror::Error` for domain errors (`AccountingError`). Map all HTTP responses to structured JSON `ApiError` (`dto.rs`).
* **Logging**: Use `tracing::info!`, `warn!`, `error!`. Do not use `println!` or `eprintln!` in `backend/src/` (except test helpers).
* **No Unjustified Panics**: Prohibit `unwrap()` and `expect()` in non-test application code unless provably unreachable and accompanied by an explanatory comment.

## 7. Testing Rules
* Every money calculation or invariant change requires a unit test in `--lib`.
* Complex arithmetic (closing transfers, balances, trial balance) requires property-based fuzzing in `tests/property_tests.rs`.
* Every new or modified HTTP route requires an integration test case in `tests/api_tests.rs`.
* Always execute the full test suite (`cargo test --manifest-path backend/Cargo.toml`) before declaring work complete.

## 8. Change Protocol
1. **THINK**: Read relevant files, check existing invariants, and state a minimal plan.
2. **DO**: Make the smallest targeted change that solves the issue. Do not refactor unrelated modules. Do not add external dependencies without explicit technical justification.
3. **CHECK**: Run tests and clippy. Verify no regressions.

## 9. Do-Not-Touch / Caution List
* `backend/migrations/20261119000000_init.sql`: Abandoned PostgreSQL draft. The live system uses programmatic SQLite initialization in `database.rs:init_schema()`. Do not base new schema work on this file.
* `_verify/`: Untracked scratch validation workspace. Do not import or touch.
* `target/` & `backend/target/`: Compiler output directories.
* `.env*`: Local configuration files (never commit).

## 10. Known Gaps & Open Owner Questions
Before implementing solutions for these areas, ask the project owner:
1. **Identity & Auth**: Attribution (`created_by`, `actor`) is client-supplied; should we add JWT/mTLS middleware to enforce server-derived identity?
2. **SQLite Immutability Triggers**: Immutability is currently enforced by app code; should we install `BEFORE UPDATE/DELETE` triggers on `audit_logs`?
3. **Statutory Terminology**: Circular 99 renames Balance Sheet to "Statement of Financial Position"; should we rename `/api/v1/reports/balance-sheet`?
4. **Multi-Tenancy**: The app is single-tenant; what tenant resolution strategy (subdomain, header `X-Company-Id`) is desired for database-per-client?
5. **Foreign Currency**: Should we support non-VND accounting currencies per TT99 Art. 4?

## 11. Environment Notes
* **Platform**: Windows 10, PowerShell shell.
* **Toolchain Paths**: Rust toolchain located at `D:\rust\cargo\bin\rustc.exe`, Cargo home at `D:\rust\cargo`, repo at `D:\money-flow`.
* **User Working Style**: The owner prefers direct, verified implementation over repetitive clarifying questions for routine tasks. Ask questions only for irreversible architecture changes or statutory financial-rule modifications.

## 12. Definition of Done
A task is DONE only when:
1. `cargo test --manifest-path backend/Cargo.toml` passes 100% (42/42 tests).
2. `cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings` completes with 0 warnings.
3. No unjustified `unwrap()`/`expect()` introduced.
4. Double-entry invariants remain uncompromised.
5. Documentation updated if public interfaces or configurations changed.
6. Summary clearly lists all modified files and verification outputs.
