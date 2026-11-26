//! Thread-safe SQLite database persistence layer for double-entry bookkeeping.
//!
//! Follows the Repository pattern (Fowler, PoEAA Ch. 10) decomposed into focused submodules:
//! - [`schema`]: Relational DDL, tables, constraints, immutability triggers, and Circular 99 seed.
//! - [`voucher_repo`]: Journal voucher creation, header/line persistence, and voiding.
//! - [`period_repo`]: Accounting period closure, reopening, and lock enforcement.
//! - [`audit_repo`]: Immutable audit logging and idempotency record queries.
//! - [`report_repo`]: Statutory financial statement and trial balance queries.
//! - [`account_repo`]: Dynamic chart of accounts CRUD, hierarchy, and cycle validation.
//! - [`disaster`]: Live VACUUM snapshots, atomic restore engine, and CLI restore tool.

pub mod account_repo;
pub mod audit_repo;
pub mod disaster;
pub mod period_repo;
pub mod report_repo;
pub mod schema;
pub mod voucher_query;
pub mod attachment_repo;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod period_guard_tests;

pub use disaster::{RestoreOutcome, execute_cli_restore};

use crate::ledger::AccountingError;
use crate::models::JournalVoucher;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

impl From<rusqlite::Error> for AccountingError {
    fn from(err: rusqlite::Error) -> Self {
        AccountingError::DatabaseError(err.to_string())
    }
}

/// Result outcome of saving a voucher with idempotency tracking.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveVoucherOutcome {
    /// Fresh voucher created and persisted.
    Created(JournalVoucher),
    /// Replayed from existing idempotency record.
    Replayed(JournalVoucher),
}

/// Minimum required SQLite version number (3.53.0 -> 3_053_000) for WAL-reset resilience.
pub const MIN_SQLITE_VERSION_NUMBER: i32 = 3_053_000;

/// Asserts that the runtime SQLite engine meets the minimum version requirement (>= 3.53.0).
pub fn verify_sqlite_engine_version() -> Result<(), AccountingError> {
    LedgerRepository::verify_sqlite_engine_version()
}

/// Thread-safe SQLite repository managing audit-grade persistence of vouchers and ledger lines.
#[derive(Clone)]
pub struct LedgerRepository {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl LedgerRepository {
    /// Asserts that the runtime SQLite engine meets the minimum version requirement (>= 3.53.0).
    pub fn verify_sqlite_engine_version() -> Result<(), AccountingError> {
        let ver_num = rusqlite::version_number();
        if ver_num < MIN_SQLITE_VERSION_NUMBER {
            return Err(AccountingError::DatabaseError(format!(
                "SQLite engine version {} (code {}) is below required minimum 3.53.0 for WAL-reset resilience",
                rusqlite::version(),
                ver_num
            )));
        }
        tracing::info!(
            sqlite_version = %rusqlite::version(),
            sqlite_version_number = ver_num,
            "SQLite engine version verified (>= 3.53.0 WAL-reset resilience standard met)"
        );
        Ok(())
    }

    /// Creates a new `LedgerRepository` wrapping the provided SQLite connection handle.
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }
}
