# Testing Strategy: Financial Accounting System

## Overview
This document outlines the rigorous testing philosophy required for the financial accounting platform. Our mandate is to guarantee 100% data integrity, financial precision, and system availability. There is no tolerance for rounding errors, race conditions, or unverified deployments.

## 1. The Testing Pyramid
We strictly adhere to the testing pyramid to maintain speed and reliability.

- **Unit Tests (70%)**: Focus on core business logic, algebraic properties, and individual function contracts. These tests must run in milliseconds.
- **Integration Tests (25%)**: Focus on interactions between modules, including database layer, API handlers, and domain service boundaries.
- **End-to-End Tests (5%)**: Focus on high-level user workflows (e.g., "Full Invoice Lifecycle"). Expensive to maintain; kept to a minimum.

### The Critical Path
Before any deployment, the "Critical Path Suite" must pass. This suite includes:
1.  All property-based tests for arithmetic.
2.  All database migration integrity tests.
3.  All integration tests for ledger and reconciliation logic.

## 2. Financial Arithmetic Testing
For financial calculations, we utilize property-based testing (via `proptest`).

- **Requirement**: All calculations must use arbitrary-precision types or `Decimal` libraries (`rust_decimal`). Never use `f32`/`f64` for money.
- **Strategy**: Define properties (e.g., `a + b = b + a`, `(a + b) - b = a`). Run thousands of iterations with random inputs to ensure no precision loss in edge cases (e.g., zero balances, maximum currency values, negative sums).

## 3. Database & ACID Testing
Financial data requires strict atomicity, consistency, isolation, and durability.

- **SQL Migrations**: All migrations must be tested against a temporary, production-like database instance. Each migration must have a "forward" and "backward" test to ensure schema stability.
- **Concurrency/Race Conditions**: Ledger updates are subject to concurrent access. We implement integration tests that explicitly attempt to violate ACID properties (e.g., initiating concurrent withdrawals from the same account) using `tokio::spawn` and shared test resources to verify database locking/transaction isolation levels work as intended.

## 4. Mocking & External I/O
Real-world side effects are prohibited in the test environment.

- **Policy**: No test shall reach out to production APIs (E-Invoice, Tax, Banking Gateways).
- **Strategy**: Define trait boundaries for all external I/O. During testing, inject mock implementations that simulate successful responses, network failures, timeouts, and malformed payloads.

## 5. Automation & CI/CD
Our CI/CD pipeline enforces the testing strategy.

- **PR Level (Fast)**: Runs unit tests and critical path integration tests. Execution time limit: 5 minutes.
- **Release Level (Comprehensive)**: Runs the full test suite, including property-based tests and slower integration tests.
- **Coverage Thresholds**:
    - Minimum branch coverage: 90%.
    - Minimum line coverage for core financial modules: 100%.

---
*This document serves as the authoritative guide for all testing efforts in this codebase. Non-compliance is a blocking issue for Pull Requests.*
