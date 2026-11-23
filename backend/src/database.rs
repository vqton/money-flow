use crate::dto::TrialBalanceRow;
use crate::ledger::{AccountValidator, AccountingError};
use crate::models::{
    Account, AccountClass, AccountingPeriod, AuditLogEntry, BalanceSheetReport, IdempotencyRecord,
    IncomeStatementReport, JournalVoucher, NormalBalance, VoucherLine, VoucherStatus,
};
use chrono::{Datelike, NaiveDate};
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

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
    conn: Arc<Mutex<Connection>>,
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

    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    /// Initializes the relational double-entry schema in SQLite.
    /// Foreign key constraints, period tracking, and performance indexes are enforced.
    pub fn init_schema(&self) -> Result<(), AccountingError> {
        Self::verify_sqlite_engine_version()?;

        let conn = self.conn.lock().unwrap();

        // Configure SQLite pragmas for high concurrency, durability, and foreign key integrity.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        // 0. Accounts Table (Dynamic Chart of Accounts per Circular 99/2025/TT-BTC)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                code TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                parent_code TEXT REFERENCES accounts(code),
                class TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                normal_balance TEXT NOT NULL
            );",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_parent ON accounts(parent_code);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_class ON accounts(class);",
            [],
        )?;

        // Seed Statutory Circular 99 Accounts
        for acct in crate::catalog::STATUTORY_ACCOUNTS {
            conn.execute(
                "INSERT OR IGNORE INTO accounts (code, name, parent_code, class, is_active, normal_balance) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                params![
                    acct.code,
                    acct.name,
                    acct.parent_code,
                    acct.class.to_string(),
                    if acct.is_active { 1 } else { 0 },
                    acct.normal_balance.to_string(),
                ],
            )?;
        }

        // 1. Voucher Header Table (Chứng từ kế toán) with User Attribution and Lifecycle Audit Columns
        conn.execute(
            "CREATE TABLE IF NOT EXISTS vouchers (
                id TEXT PRIMARY KEY,
                voucher_no TEXT NOT NULL,
                voucher_date TEXT NOT NULL,
                description TEXT NOT NULL,
                invoice_ref TEXT,
                status TEXT NOT NULL DEFAULT 'POSTED',
                created_by TEXT NOT NULL DEFAULT 'system',
                approved_by TEXT,
                voided_at TEXT,
                voided_by TEXT,
                void_reason TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
            [],
        )?;

        // Forward-compatible schema migration: add missing columns if opening an older database file
        let _ = conn.execute(
            "ALTER TABLE vouchers ADD COLUMN status TEXT NOT NULL DEFAULT 'POSTED';",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE vouchers ADD COLUMN created_by TEXT NOT NULL DEFAULT 'system';",
            [],
        );
        let _ = conn.execute("ALTER TABLE vouchers ADD COLUMN approved_by TEXT;", []);
        let _ = conn.execute("ALTER TABLE vouchers ADD COLUMN voided_at TEXT;", []);
        let _ = conn.execute("ALTER TABLE vouchers ADD COLUMN voided_by TEXT;", []);
        let _ = conn.execute("ALTER TABLE vouchers ADD COLUMN void_reason TEXT;", []);
        let _ = conn.execute(
            "ALTER TABLE vouchers ADD COLUMN created_at TEXT NOT NULL DEFAULT (datetime('now'));",
            [],
        );

        // 2. Voucher Lines Table (Các định khoản Nợ / Có) with Explicit Leg Sequence Order
        conn.execute(
            "CREATE TABLE IF NOT EXISTS voucher_lines (
                id TEXT PRIMARY KEY,
                voucher_id TEXT NOT NULL REFERENCES vouchers(id) ON DELETE CASCADE,
                line_no INTEGER NOT NULL DEFAULT 1,
                account_code TEXT NOT NULL,
                debit TEXT NOT NULL,
                credit TEXT NOT NULL,
                line_desc TEXT
            );",
            [],
        )?;

        let _ = conn.execute(
            "ALTER TABLE voucher_lines ADD COLUMN line_no INTEGER NOT NULL DEFAULT 1;",
            [],
        );

        // 3. Accounting Periods Table (Period Lock & Statutory Status)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS accounting_periods (
                year INTEGER NOT NULL,
                month INTEGER NOT NULL,
                is_closed INTEGER NOT NULL DEFAULT 0,
                closed_at TEXT,
                PRIMARY KEY (year, month)
            );",
            [],
        )?;

        // 4. Immutable Audit Logs Table (Sổ nhật ký kiểm toán hệ thống)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                details TEXT,
                client_ip TEXT
            );",
            [],
        )?;

        // 5. Performance & Audit Indices
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_vouchers_no_posted ON vouchers(voucher_no) WHERE status = 'POSTED';",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vouchers_date ON vouchers(voucher_date);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vouchers_status ON vouchers(status);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_lines_account ON voucher_lines(account_code);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_lines_voucher ON voucher_lines(voucher_id);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_periods_closed ON accounting_periods(is_closed);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_logs(actor);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_logs(action);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp);",
            [],
        )?;

        // 6. Idempotency Records Table (API Request Deduplication & Replay)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS idempotency_records (
                key TEXT NOT NULL,
                actor TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                request_hash TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                response_body TEXT NOT NULL,
                resource_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (key, actor, endpoint)
            );",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_idempotency_actor ON idempotency_records(actor);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_idempotency_created_at ON idempotency_records(created_at);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_idempotency_resource ON idempotency_records(resource_id);",
            [],
        )?;

        // 7. Database Immutability Triggers (Financial Integrity & Non-Destructive Lifecycle)
        conn.execute_batch(
            "
            -- Audit logs: Strict append-only; updates and deletes prohibited
            CREATE TRIGGER IF NOT EXISTS trg_audit_logs_no_update
            BEFORE UPDATE ON audit_logs
            BEGIN
                SELECT RAISE(ABORT, 'audit_logs are immutable: UPDATE not permitted');
            END;

            CREATE TRIGGER IF NOT EXISTS trg_audit_logs_no_delete
            BEFORE DELETE ON audit_logs
            BEGIN
                SELECT RAISE(ABORT, 'audit_logs are immutable: DELETE not permitted');
            END;

            -- Idempotency records: Strict write-once; updates and deletes prohibited
            CREATE TRIGGER IF NOT EXISTS trg_idempotency_records_no_update
            BEFORE UPDATE ON idempotency_records
            BEGIN
                SELECT RAISE(ABORT, 'idempotency_records are immutable: UPDATE not permitted');
            END;

            CREATE TRIGGER IF NOT EXISTS trg_idempotency_records_no_delete
            BEFORE DELETE ON idempotency_records
            BEGIN
                SELECT RAISE(ABORT, 'idempotency_records are immutable: DELETE not permitted');
            END;

            -- Voucher lines: Immutable transaction legs; updates and deletes prohibited
            CREATE TRIGGER IF NOT EXISTS trg_voucher_lines_no_delete
            BEFORE DELETE ON voucher_lines
            BEGIN
                SELECT RAISE(ABORT, 'voucher_lines are immutable: DELETE not permitted');
            END;

            CREATE TRIGGER IF NOT EXISTS trg_voucher_lines_no_update
            BEFORE UPDATE ON voucher_lines
            BEGIN
                SELECT RAISE(ABORT, 'voucher_lines are immutable: UPDATE not permitted');
            END;

            -- Vouchers: No physical deletion; updates restricted exclusively to voiding transitions
            CREATE TRIGGER IF NOT EXISTS trg_vouchers_no_delete
            BEFORE DELETE ON vouchers
            BEGIN
                SELECT RAISE(ABORT, 'vouchers are immutable: DELETE not permitted');
            END;

            CREATE TRIGGER IF NOT EXISTS trg_vouchers_protect_update
            BEFORE UPDATE ON vouchers
            FOR EACH ROW
            WHEN OLD.status != 'POSTED'
              OR NEW.status != 'VOIDED'
              OR NEW.id IS NOT OLD.id
              OR NEW.voucher_no IS NOT OLD.voucher_no
              OR NEW.voucher_date IS NOT OLD.voucher_date
              OR NEW.description IS NOT OLD.description
              OR NEW.invoice_ref IS NOT OLD.invoice_ref
              OR NEW.created_by IS NOT OLD.created_by
              OR NEW.approved_by IS NOT OLD.approved_by
              OR NEW.created_at IS NOT OLD.created_at
            BEGIN
                SELECT RAISE(ABORT, 'vouchers are immutable: only voiding transitions (status, voided_at, voided_by, void_reason) of POSTED vouchers are permitted');
            END;
            ",
        )?;

        Ok(())
    }

    /// Internal helper to record an immutable audit log entry within an active transaction.
    fn record_audit_log_tx(
        tx: &rusqlite::Transaction,
        entry: &AuditLogEntry,
    ) -> Result<(), rusqlite::Error> {
        tx.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.id.to_string(),
                entry.timestamp,
                entry.actor,
                entry.action,
                entry.resource_type,
                entry.resource_id,
                entry.details,
                entry.client_ip,
            ],
        )?;
        Ok(())
    }

    /// Internal helper to insert voucher header and lines within an active transaction.
    fn insert_voucher_tx(
        tx: &rusqlite::Transaction,
        voucher: &JournalVoucher,
    ) -> Result<(), rusqlite::Error> {
        tx.execute(
            "INSERT INTO vouchers (id, voucher_no, voucher_date, description, invoice_ref,
                                   status, created_by, approved_by, voided_at, voided_by, void_reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                voucher.id.to_string(),
                voucher.voucher_no,
                voucher.voucher_date.to_string(),
                voucher.description,
                voucher.invoice_ref,
                voucher.status.to_string(),
                voucher.created_by,
                voucher.approved_by,
                voucher.voided_at,
                voucher.voided_by,
                voucher.void_reason,
                voucher.created_at,
            ],
        )?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO voucher_lines (id, voucher_id, line_no, account_code, debit, credit, line_desc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;

            for line in &voucher.lines {
                stmt.execute(params![
                    line.id.to_string(),
                    voucher.id.to_string(),
                    line.line_no,
                    line.account_code,
                    line.debit.to_string(),
                    line.credit.to_string(),
                    line.line_desc,
                ])?;
            }
        }

        Ok(())
    }

    /// Retrieves an idempotency record by key, actor, and endpoint.
    pub fn get_idempotency_record(
        &self,
        key: &str,
        actor: &str,
        endpoint: &str,
    ) -> Result<Option<IdempotencyRecord>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, actor, endpoint, request_hash, status_code, response_body, resource_id, created_at
             FROM idempotency_records
             WHERE key = ?1 AND actor = ?2 AND endpoint = ?3",
        )?;
        let record = stmt
            .query_row(params![key, actor, endpoint], |row| {
                let status_raw: i64 = row.get(4)?;
                Ok(IdempotencyRecord {
                    key: row.get(0)?,
                    actor: row.get(1)?,
                    endpoint: row.get(2)?,
                    request_hash: row.get(3)?,
                    status_code: status_raw as u16,
                    response_body: row.get(5)?,
                    resource_id: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .optional()?;
        Ok(record)
    }

    /// Persists a complete JournalVoucher (header + lines) with optional idempotency tracking.
    /// Within a single ACID transaction:
    /// 1. Checks if an idempotency record exists for (key, actor, endpoint):
    ///    - If matching hash: returns Replayed(voucher).
    ///    - If mismatched hash: returns Err(AccountingError::IdempotencyConflict).
    /// 2. Enforces period lock.
    /// 3. Inserts voucher header and legs.
    /// 4. Records CREATE_VOUCHER audit log.
    /// 5. Inserts idempotency record if key was provided.
    /// 6. Commits transaction and returns Created(voucher).
    pub fn save_voucher_with_idempotency(
        &self,
        voucher: &JournalVoucher,
        idempotency: Option<(&str, &str, &str, &str)>, // (key, actor, endpoint, request_hash)
    ) -> Result<SaveVoucherOutcome, AccountingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        if let Some((key, actor, endpoint, request_hash)) = idempotency {
            let existing: Option<(String, i64, String)> = tx
                .query_row(
                    "SELECT request_hash, status_code, response_body
                     FROM idempotency_records
                     WHERE key = ?1 AND actor = ?2 AND endpoint = ?3",
                    params![key, actor, endpoint],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;

            if let Some((saved_hash, _status, body)) = existing {
                if saved_hash == request_hash {
                    let replayed_voucher: JournalVoucher =
                        serde_json::from_str(&body).map_err(|e| {
                            AccountingError::DatabaseError(format!("Corrupt idempotency body: {e}"))
                        })?;
                    return Ok(SaveVoucherOutcome::Replayed(replayed_voucher));
                } else {
                    return Err(AccountingError::IdempotencyConflict {
                        key: key.to_string(),
                    });
                }
            }
        }

        let year = voucher.voucher_date.year();
        let month = voucher.voucher_date.month();

        // Enforce period lock
        let is_closed: bool = tx
            .query_row(
                "SELECT is_closed FROM accounting_periods WHERE year = ?1 AND month = ?2",
                params![year, month],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);

        if is_closed {
            return Err(AccountingError::PeriodClosed { year, month });
        }

        // Validate all leg account codes against accounts table
        for line in &voucher.lines {
            let is_active: Option<i32> = tx
                .query_row(
                    "SELECT is_active FROM accounts WHERE code = ?1",
                    params![line.account_code],
                    |r| r.get(0),
                )
                .optional()?;
            match is_active {
                None => {
                    return Err(AccountingError::UnknownAccount {
                        account_code: line.account_code.clone(),
                    });
                }
                Some(0) => {
                    return Err(AccountingError::InactiveAccount {
                        account_code: line.account_code.clone(),
                    });
                }
                Some(_) => {}
            }
        }

        Self::insert_voucher_tx(&tx, voucher)?;

        let audit = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: voucher.created_by.clone(),
            action: "CREATE_VOUCHER".to_string(),
            resource_type: "VOUCHER".to_string(),
            resource_id: Some(voucher.voucher_no.clone()),
            details: Some(format!(
                "Created voucher {} with {} lines",
                voucher.voucher_no,
                voucher.lines.len()
            )),
            client_ip: None,
        };
        Self::record_audit_log_tx(&tx, &audit)?;

        if let Some((key, actor, endpoint, request_hash)) = idempotency {
            let response_body = serde_json::to_string(voucher).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to serialize voucher response: {e}"))
            })?;
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO idempotency_records (key, actor, endpoint, request_hash, status_code, response_body, resource_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    key,
                    actor,
                    endpoint,
                    request_hash,
                    201i64,
                    response_body,
                    voucher.id.to_string(),
                    now,
                ],
            )?;
        }

        tx.commit()?;
        Ok(SaveVoucherOutcome::Created(voucher.clone()))
    }

    /// Persists a complete JournalVoucher (header + all debit/credit lines)
    /// within a single ACID transaction. Rejects posting if the period is closed.
    /// Also records an immutable CREATE_VOUCHER audit trail event.
    /// Delegates to `save_voucher_with_idempotency(voucher, None)`.
    pub fn save_voucher(&self, voucher: &JournalVoucher) -> Result<(), AccountingError> {
        self.save_voucher_with_idempotency(voucher, None)?;
        Ok(())
    }

    /// Checks whether an accounting period (year, month) is closed.
    pub fn is_period_closed(&self, year: i32, month: u32) -> Result<bool, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let is_closed: bool = conn
            .query_row(
                "SELECT is_closed FROM accounting_periods WHERE year = ?1 AND month = ?2",
                params![year, month],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);

        Ok(is_closed)
    }

    /// Atomically persists the closing voucher, marks the accounting period as closed,
    /// and records an immutable PERIOD_CLOSE audit trail event.
    pub fn close_period(
        &self,
        year: i32,
        month: u32,
        closing_voucher: &JournalVoucher,
    ) -> Result<(), AccountingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Verify period is not already closed
        let is_closed: bool = tx
            .query_row(
                "SELECT is_closed FROM accounting_periods WHERE year = ?1 AND month = ?2",
                params![year, month],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);

        if is_closed {
            return Err(AccountingError::PeriodAlreadyClosed { year, month });
        }

        // Persist closing voucher
        Self::insert_voucher_tx(&tx, closing_voucher)?;

        // Record closed period
        let closed_at = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO accounting_periods (year, month, is_closed, closed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(year, month) DO UPDATE SET
                 is_closed = 1,
                 closed_at = excluded.closed_at",
            params![year, month, closed_at],
        )?;

        let audit = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: closed_at,
            actor: closing_voucher.created_by.clone(),
            action: "PERIOD_CLOSE".to_string(),
            resource_type: "PERIOD".to_string(),
            resource_id: Some(format!("{year}-{month:02}")),
            details: Some(format!(
                "Closed period {year}-{month:02} with voucher {}",
                closing_voucher.voucher_no
            )),
            client_ip: None,
        };
        Self::record_audit_log_tx(&tx, &audit)?;

        tx.commit()?;
        Ok(())
    }

    /// Atomically reopens a closed accounting period via statutory non-destructive voiding:
    /// marks the closing voucher as VOIDED (preserving historical audit trails),
    /// resets `is_closed` to false, and records a PERIOD_REOPEN audit event.
    pub fn reopen_period(&self, year: i32, month: u32, actor: &str) -> Result<(), AccountingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let is_closed: bool = tx
            .query_row(
                "SELECT is_closed FROM accounting_periods WHERE year = ?1 AND month = ?2",
                params![year, month],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);

        if !is_closed {
            return Err(AccountingError::PeriodNotClosed { year, month });
        }

        let closing_voucher_no = format!("KC-{year}-{month:02}");
        let now = chrono::Utc::now().to_rfc3339();

        // Non-destructive voiding of the closing voucher
        tx.execute(
            "UPDATE vouchers
             SET status = 'VOIDED',
                 voided_at = ?1,
                 voided_by = ?2,
                 void_reason = 'Period reopened'
             WHERE voucher_no = ?3 AND status = 'POSTED'",
            params![now, actor, closing_voucher_no],
        )?;

        // Set period as open
        tx.execute(
            "UPDATE accounting_periods
             SET is_closed = 0, closed_at = NULL
             WHERE year = ?1 AND month = ?2",
            params![year, month],
        )?;

        let audit = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: now,
            actor: actor.to_string(),
            action: "PERIOD_REOPEN".to_string(),
            resource_type: "PERIOD".to_string(),
            resource_id: Some(format!("{year}-{month:02}")),
            details: Some(format!("Reopened period {year}-{month:02}")),
            client_ip: None,
        };
        Self::record_audit_log_tx(&tx, &audit)?;

        tx.commit()?;
        Ok(())
    }

    /// Non-destructively voids an active journal voucher per statutory audit requirements.
    /// Checks that the voucher exists, has not already been voided, and belongs to an open accounting period.
    /// Marks `status = 'VOIDED'`, records metadata, and writes a VOID_VOUCHER audit log entry.
    pub fn void_voucher(
        &self,
        id: Uuid,
        actor: &str,
        reason: &str,
        client_ip: Option<&str>,
    ) -> Result<JournalVoucher, AccountingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. Fetch voucher header
        let (voucher_no, date_str, current_status): (String, String, String) = tx
            .query_row(
                "SELECT voucher_no, voucher_date, status FROM vouchers WHERE id = ?1",
                params![id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| AccountingError::VoucherNotFound(id.to_string()))?;

        // 2. Check if already voided
        if current_status == "VOIDED" {
            return Err(AccountingError::VoucherAlreadyVoided { voucher_no });
        }

        let voucher_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
            AccountingError::DatabaseError(format!("Corrupt voucher date in database: {e}"))
        })?;

        // 3. Check period lock
        let year = voucher_date.year();
        let month = voucher_date.month();
        let is_closed: bool = tx
            .query_row(
                "SELECT is_closed FROM accounting_periods WHERE year = ?1 AND month = ?2",
                params![year, month],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);

        if is_closed {
            return Err(AccountingError::CannotVoidInClosedPeriod {
                voucher_no,
                year,
                month,
            });
        }

        // 4. Update status to VOIDED
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE vouchers
             SET status = 'VOIDED',
                 voided_at = ?1,
                 voided_by = ?2,
                 void_reason = ?3
             WHERE id = ?4",
            params![now, actor, reason, id.to_string()],
        )?;

        // 5. Audit log
        let audit = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: now,
            actor: actor.to_string(),
            action: "VOID_VOUCHER".to_string(),
            resource_type: "VOUCHER".to_string(),
            resource_id: Some(voucher_no),
            details: Some(reason.to_string()),
            client_ip: client_ip.map(|s| s.to_string()),
        };
        Self::record_audit_log_tx(&tx, &audit)?;

        tx.commit()?;
        drop(conn);

        // 6. Reload updated voucher
        self.get_voucher(id)?
            .ok_or_else(|| AccountingError::VoucherNotFound(id.to_string()))
    }

    /// Queries the immutable audit trail with optional filtering and pagination.
    pub fn list_audit_logs(
        &self,
        query: &crate::dto::AuditLogQuery,
    ) -> Result<Vec<AuditLogEntry>, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut sql =
            "SELECT id, timestamp, actor, action, resource_type, resource_id, details, client_ip
                       FROM audit_logs WHERE 1=1"
                .to_string();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(ref actor) = query.actor {
            sql.push_str(" AND actor = ?");
            params_vec.push(rusqlite::types::Value::Text(actor.clone()));
        }
        if let Some(ref action) = query.action {
            sql.push_str(" AND action = ?");
            params_vec.push(rusqlite::types::Value::Text(action.clone()));
        }
        if let Some(ref r_type) = query.resource_type {
            sql.push_str(" AND resource_type = ?");
            params_vec.push(rusqlite::types::Value::Text(r_type.clone()));
        }
        if let Some(ref r_id) = query.resource_id {
            sql.push_str(" AND resource_id = ?");
            params_vec.push(rusqlite::types::Value::Text(r_id.clone()));
        }
        if let Some(ref from_d) = query.from_date {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(rusqlite::types::Value::Text(from_d.clone()));
        }
        if let Some(ref to_d) = query.to_date {
            sql.push_str(" AND timestamp <= ?");
            params_vec.push(rusqlite::types::Value::Text(to_d.clone()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        let limit = query.limit.unwrap_or(100).min(1000);
        sql.push_str(&format!(" LIMIT {}", limit));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            let id_str: String = row.get(0)?;
            let id = Uuid::parse_str(&id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let timestamp: String = row.get(1)?;
            let actor: String = row.get(2)?;
            let action: String = row.get(3)?;
            let resource_type: String = row.get(4)?;
            let resource_id: Option<String> = row.get(5)?;
            let details: Option<String> = row.get(6)?;
            let client_ip: Option<String> = row.get(7)?;

            Ok(AuditLogEntry {
                id,
                timestamp,
                actor,
                action,
                resource_type,
                resource_id,
                details,
                client_ip,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    /// Lists all accounting periods and their lock statuses sorted chronologically.
    pub fn list_periods(&self) -> Result<Vec<AccountingPeriod>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT year, month, is_closed, closed_at
             FROM accounting_periods
             ORDER BY year ASC, month ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let year: i32 = row.get(0)?;
            let month: u32 = row.get(1)?;
            let is_closed: bool = row.get(2)?;
            let closed_at: Option<String> = row.get(3)?;

            Ok(AccountingPeriod {
                year,
                month,
                is_closed,
                closed_at,
            })
        })?;

        let mut periods = Vec::new();
        for r in rows {
            periods.push(r?);
        }
        Ok(periods)
    }

    /// Retrieves account balances across a specified date range, excluding closing vouchers and voided entries.
    pub fn get_account_balances_for_range(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<BTreeMap<String, (Decimal, Decimal)>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.voucher_date >= ?1
               AND v.voucher_date <= ?2
               AND v.voucher_no NOT LIKE 'KC-%'
               AND v.status = 'POSTED'
             ORDER BY l.account_code ASC",
        )?;

        let from_str = from_date.to_string();
        let to_str = to_date.to_string();

        let rows = stmt.query_map(params![from_str, to_str], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        let mut map = BTreeMap::new();
        for r in rows {
            let (account_code, debit, credit) = r?;
            let entry = map
                .entry(account_code)
                .or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        Ok(map)
    }

    /// Computes the statutory Balance Sheet as of a specified date, strictly enforcing
    /// the contra-asset deduction formula under Circular 99/2025/TT-BTC and isolating active POSTED vouchers.
    pub fn compute_balance_sheet(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<BalanceSheetReport, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.voucher_date <= ?1
               AND v.status = 'POSTED'",
        )?;

        let as_of_str = as_of_date.to_string();
        let rows = stmt.query_map(params![as_of_str], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        let mut balances: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
        for r in rows {
            let (account_code, debit, credit) = r?;
            let entry = balances
                .entry(account_code)
                .or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        let mut current_assets = Decimal::ZERO;
        let mut non_current_assets = Decimal::ZERO;
        let mut accumulated_depreciation = Decimal::ZERO;
        let mut asset_provisions = Decimal::ZERO;
        let mut liabilities = Decimal::ZERO;
        let mut equity = Decimal::ZERO;

        for (code, (debit, credit)) in &balances {
            if code.starts_with("214") {
                accumulated_depreciation += credit - debit;
            } else if code.starts_with("229") {
                asset_provisions += credit - debit;
            } else if code.starts_with('1') {
                current_assets += debit - credit;
            } else if code.starts_with('2') {
                non_current_assets += debit - credit;
            } else if code.starts_with('3') {
                liabilities += credit - debit;
            } else if code.starts_with('4') {
                equity += credit - debit;
            }
        }

        let total_assets =
            current_assets + non_current_assets - (accumulated_depreciation + asset_provisions);
        let total_resources = liabilities + equity;
        let is_balanced = total_assets == total_resources;

        Ok(BalanceSheetReport {
            as_of_date,
            current_assets,
            non_current_assets,
            accumulated_depreciation,
            asset_provisions,
            total_assets,
            liabilities,
            equity,
            total_resources,
            is_balanced,
        })
    }

    /// Computes the statutory Income Statement across a specified date range, isolating active POSTED vouchers.
    pub fn compute_income_statement(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<IncomeStatementReport, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.voucher_date >= ?1
               AND v.voucher_date <= ?2
               AND v.voucher_no NOT LIKE 'KC-%'
               AND v.status = 'POSTED'",
        )?;

        let from_str = from_date.to_string();
        let to_str = to_date.to_string();

        let rows = stmt.query_map(params![from_str, to_str], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        let mut gross_revenue = Decimal::ZERO;
        let mut revenue_deductions = Decimal::ZERO;
        let mut cost_of_goods_sold = Decimal::ZERO;
        let mut financial_income = Decimal::ZERO;
        let mut financial_expenses = Decimal::ZERO;
        let mut selling_expenses = Decimal::ZERO;
        let mut administrative_expenses = Decimal::ZERO;
        let mut other_income = Decimal::ZERO;
        let mut other_expenses = Decimal::ZERO;
        let mut corporate_tax = Decimal::ZERO;

        for r in rows {
            let (code, debit, credit) = r?;
            if code.starts_with("511") {
                gross_revenue += credit - debit;
            } else if code.starts_with("521") {
                revenue_deductions += debit - credit;
            } else if code.starts_with("632") {
                cost_of_goods_sold += debit - credit;
            } else if code.starts_with("515") {
                financial_income += credit - debit;
            } else if code.starts_with("635") {
                financial_expenses += debit - credit;
            } else if code.starts_with("641") {
                selling_expenses += debit - credit;
            } else if code.starts_with("642") {
                administrative_expenses += debit - credit;
            } else if code.starts_with("711") {
                other_income += credit - debit;
            } else if code.starts_with("811") {
                other_expenses += debit - credit;
            } else if code.starts_with("821") {
                corporate_tax += debit - credit;
            }
        }

        let net_revenue = gross_revenue - revenue_deductions;
        let gross_profit = net_revenue - cost_of_goods_sold;
        let operating_profit = gross_profit + financial_income
            - financial_expenses
            - selling_expenses
            - administrative_expenses;
        let net_profit_before_tax = operating_profit + (other_income - other_expenses);
        let net_profit_after_tax = net_profit_before_tax - corporate_tax;

        Ok(IncomeStatementReport {
            from_date,
            to_date,
            gross_revenue,
            revenue_deductions,
            net_revenue,
            cost_of_goods_sold,
            gross_profit,
            financial_income,
            financial_expenses,
            selling_expenses,
            administrative_expenses,
            operating_profit,
            other_income,
            other_expenses,
            net_profit_before_tax,
            corporate_tax,
            net_profit_after_tax,
        })
    }

    /// Retrieves a single voucher with all its constituent lines and audit attribution metadata by ID.
    pub fn get_voucher(&self, id: Uuid) -> Result<Option<JournalVoucher>, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let header = conn.query_row(
            "SELECT id, voucher_no, voucher_date, description, invoice_ref,
                    status, created_by, approved_by, voided_at, voided_by, void_reason, created_at
             FROM vouchers WHERE id = ?1",
            params![id.to_string()],
            |row| {
                let id_str: String = row.get(0)?;
                let voucher_no: String = row.get(1)?;
                let date_str: String = row.get(2)?;
                let description: String = row.get(3)?;
                let invoice_ref: Option<String> = row.get(4)?;
                let status_str: String = row.get(5)?;
                let created_by: String = row.get(6)?;
                let approved_by: Option<String> = row.get(7)?;
                let voided_at: Option<String> = row.get(8)?;
                let voided_by: Option<String> = row.get(9)?;
                let void_reason: Option<String> = row.get(10)?;
                let created_at: String = row.get(11)?;

                let parsed_id = Uuid::parse_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let parsed_date =
                    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                let status = VoucherStatus::from_str(&status_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?;

                Ok((
                    parsed_id,
                    voucher_no,
                    parsed_date,
                    description,
                    invoice_ref,
                    status,
                    created_by,
                    approved_by,
                    voided_at,
                    voided_by,
                    void_reason,
                    created_at,
                ))
            },
        );

        let (
            voucher_id,
            voucher_no,
            voucher_date,
            description,
            invoice_ref,
            status,
            created_by,
            approved_by,
            voided_at,
            voided_by,
            void_reason,
            created_at,
        ) = match header {
            Ok(data) => data,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(AccountingError::from(e)),
        };

        let mut line_stmt = conn.prepare(
            "SELECT id, line_no, account_code, debit, credit, line_desc
             FROM voucher_lines WHERE voucher_id = ?1
             ORDER BY line_no ASC, rowid ASC",
        )?;

        let line_iter = line_stmt.query_map(params![voucher_id.to_string()], |row| {
            let line_id_str: String = row.get(0)?;
            let line_no: u32 = row.get(1)?;
            let account_code: String = row.get(2)?;
            let debit_str: String = row.get(3)?;
            let credit_str: String = row.get(4)?;
            let line_desc: Option<String> = row.get(5)?;

            let line_id = Uuid::parse_str(&line_id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok(VoucherLine {
                id: line_id,
                line_no,
                account_code,
                debit,
                credit,
                line_desc,
            })
        })?;

        let mut lines = Vec::new();
        for line_res in line_iter {
            lines.push(line_res?);
        }

        Ok(Some(JournalVoucher {
            id: voucher_id,
            voucher_no,
            voucher_date,
            description,
            invoice_ref,
            status,
            created_by,
            approved_by,
            voided_at,
            voided_by,
            void_reason,
            created_at,
            lines,
        }))
    }

    /// Retrieves active POSTED historical vouchers within an optional date range [from_date, to_date].
    pub fn list_vouchers(
        &self,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
    ) -> Result<Vec<JournalVoucher>, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let from_str = from_date.map(|d| d.to_string());
        let to_str = to_date.map(|d| d.to_string());

        let mut stmt = conn.prepare(
            "SELECT id, voucher_no, voucher_date, description, invoice_ref,
                    status, created_by, approved_by, voided_at, voided_by, void_reason, created_at
             FROM vouchers
             WHERE status = 'POSTED'
               AND (?1 IS NULL OR voucher_date >= ?1)
               AND (?2 IS NULL OR voucher_date <= ?2)
             ORDER BY voucher_date ASC, voucher_no ASC",
        )?;

        let header_rows = stmt.query_map(params![from_str, to_str], |row| {
            let id_str: String = row.get(0)?;
            let voucher_no: String = row.get(1)?;
            let date_str: String = row.get(2)?;
            let description: String = row.get(3)?;
            let invoice_ref: Option<String> = row.get(4)?;
            let status_str: String = row.get(5)?;
            let created_by: String = row.get(6)?;
            let approved_by: Option<String> = row.get(7)?;
            let voided_at: Option<String> = row.get(8)?;
            let voided_by: Option<String> = row.get(9)?;
            let void_reason: Option<String> = row.get(10)?;
            let created_at: String = row.get(11)?;

            let parsed_id = Uuid::parse_str(&id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let parsed_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let status = VoucherStatus::from_str(&status_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;

            Ok((
                parsed_id,
                voucher_no,
                parsed_date,
                description,
                invoice_ref,
                status,
                created_by,
                approved_by,
                voided_at,
                voided_by,
                void_reason,
                created_at,
            ))
        })?;

        let mut headers = Vec::new();
        for h in header_rows {
            headers.push(h?);
        }

        let mut vouchers = Vec::with_capacity(headers.len());
        let mut line_stmt = conn.prepare(
            "SELECT id, line_no, account_code, debit, credit, line_desc
             FROM voucher_lines WHERE voucher_id = ?1
             ORDER BY line_no ASC, rowid ASC",
        )?;

        for (
            v_id,
            v_no,
            v_date,
            desc,
            inv_ref,
            status,
            created_by,
            approved_by,
            voided_at,
            voided_by,
            void_reason,
            created_at,
        ) in headers
        {
            let line_iter = line_stmt.query_map(params![v_id.to_string()], |row| {
                let line_id_str: String = row.get(0)?;
                let line_no: u32 = row.get(1)?;
                let account_code: String = row.get(2)?;
                let debit_str: String = row.get(3)?;
                let credit_str: String = row.get(4)?;
                let line_desc: Option<String> = row.get(5)?;

                let line_id = Uuid::parse_str(&line_id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let debit = Decimal::from_str(&debit_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let credit = Decimal::from_str(&credit_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(VoucherLine {
                    id: line_id,
                    line_no,
                    account_code,
                    debit,
                    credit,
                    line_desc,
                })
            })?;

            let mut lines = Vec::new();
            for l in line_iter {
                lines.push(l?);
            }

            vouchers.push(JournalVoucher {
                id: v_id,
                voucher_no: v_no,
                voucher_date: v_date,
                description: desc,
                invoice_ref: inv_ref,
                status,
                created_by,
                approved_by,
                voided_at,
                voided_by,
                void_reason,
                created_at,
                lines,
            });
        }

        Ok(vouchers)
    }

    /// Computes the statutory Trial Balance across all active POSTED vouchers.
    pub fn compute_trial_balance(&self) -> Result<Vec<TrialBalanceRow>, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.status = 'POSTED'
             ORDER BY l.account_code ASC",
        )?;

        let mut map: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();

        let rows = stmt.query_map([], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        for r in rows {
            let (account_code, debit, credit) = r?;
            let entry = map
                .entry(account_code)
                .or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        let trial_balance_rows = map
            .into_iter()
            .map(|(account_code, (total_debit, total_credit))| {
                let net_balance = total_debit - total_credit;
                TrialBalanceRow {
                    account_code,
                    total_debit,
                    total_credit,
                    net_balance,
                }
            })
            .collect();

        Ok(trial_balance_rows)
    }

    /// Creates an atomic live SQLite snapshot using `VACUUM INTO ?1`.
    /// Handles stale existing files, ensures parent directories exist,
    /// records a SYSTEM_BACKUP audit log event, and returns the backup file size in bytes.
    pub fn create_backup(
        &self,
        destination_path: &str,
        actor: &str,
    ) -> Result<u64, AccountingError> {
        if destination_path.trim().is_empty() {
            return Err(AccountingError::DatabaseError(
                "Destination path cannot be empty".to_string(),
            ));
        }

        let dest = std::path::Path::new(destination_path);
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to create backup directory: {}", e))
            })?;
        }

        if dest.exists() {
            std::fs::remove_file(dest).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to remove stale backup file: {}", e))
            })?;
        }

        let conn = self.conn.lock().unwrap();
        conn.execute("VACUUM INTO ?1;", params![destination_path])?;

        let metadata = std::fs::metadata(destination_path).map_err(|e| {
            AccountingError::DatabaseError(format!("Failed to read backup file metadata: {}", e))
        })?;

        let file_size_bytes = metadata.len();

        // Record immutable audit event for backup
        let _ = conn.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                chrono::Utc::now().to_rfc3339(),
                actor,
                "SYSTEM_BACKUP",
                "SYSTEM",
                destination_path,
                format!("Created live backup snapshot ({file_size_bytes} bytes)"),
                Option::<String>::None,
            ],
        );

        Ok(file_size_bytes)
    }

    /// Restores the active database from a point-in-time snapshot.
    ///
    /// Pre-validates file existence, computes SHA-256 (asserting expected match if provided),
    /// executes PRAGMA integrity_check in an isolated read-only connection, and verifies
    /// required tables (vouchers, voucher_lines, audit_logs).
    ///
    /// Uses rollback protection: creates a temporary backup of the live database before restoring.
    /// If restore fails, the live database is restored from the rollback backup.
    ///
    /// On success, checkpoints WAL, enforces foreign keys, and appends a SYSTEM_RESTORE audit log entry.
    pub fn restore_from_snapshot(
        &self,
        snapshot_path: &std::path::Path,
        expected_sha256: Option<&str>,
        actor: &str,
    ) -> Result<RestoreOutcome, AccountingError> {
        // 1. Snapshot existence and file validation
        if !snapshot_path.exists() || !snapshot_path.is_file() {
            return Err(AccountingError::DatabaseError(format!(
                "Snapshot file does not exist or is not a regular file: {}",
                snapshot_path.display()
            )));
        }

        // 2. Compute SHA-256 checksum using ring
        let bytes = std::fs::read(snapshot_path).map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to read snapshot file '{}': {}",
                snapshot_path.display(),
                e
            ))
        })?;

        let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
        let computed_sha256: String = digest
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        // 3. Expected SHA-256 case-insensitive match (if specified)
        if let Some(expected) = expected_sha256 {
            let trimmed = expected.trim();
            if !trimmed.is_empty() && !computed_sha256.eq_ignore_ascii_case(trimmed) {
                return Err(AccountingError::DatabaseError(format!(
                    "Snapshot SHA-256 checksum mismatch: expected '{}', computed '{}'",
                    trimmed, computed_sha256
                )));
            }
        }

        // 4. Isolated read-only connection for integrity_check and catalog check
        let snap_conn = Connection::open_with_flags(
            snapshot_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to open snapshot for pre-validation: {}",
                e
            ))
        })?;

        let integrity_check: Result<String, rusqlite::Error> =
            snap_conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0));
        match integrity_check {
            Ok(ref check) if check == "ok" => {}
            Ok(other) => {
                return Err(AccountingError::DatabaseError(format!(
                    "Snapshot SQLite integrity check failed: {}",
                    other
                )));
            }
            Err(e) => {
                return Err(AccountingError::DatabaseError(format!(
                    "Failed to execute PRAGMA integrity_check on snapshot: {}",
                    e
                )));
            }
        }

        // 5. Schema verification for essential tables
        let table_count: i64 = snap_conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('vouchers', 'voucher_lines', 'audit_logs')",
                [],
                |r| r.get(0),
            )
            .map_err(|e| {
                AccountingError::DatabaseError(format!(
                    "Failed to verify schema in snapshot: {}",
                    e
                ))
            })?;

        if table_count < 3 {
            return Err(AccountingError::DatabaseError(format!(
                "Snapshot schema is missing required tables (found {} of 3: vouchers, voucher_lines, audit_logs)",
                table_count
            )));
        }

        drop(snap_conn);

        // 6. Active connection restore with rollback protection
        let mut conn = self.conn.lock().unwrap();

        let temp_rollback = std::env::temp_dir().join(format!("rollback_{}.db", Uuid::new_v4()));
        conn.backup(
            rusqlite::MAIN_DB,
            &temp_rollback,
            rusqlite::backup::NO_PROGRESS,
        )
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to create rollback checkpoint before restore: {}",
                e
            ))
        })?;

        if let Err(restore_err) = conn.restore(
            rusqlite::MAIN_DB,
            snapshot_path,
            rusqlite::backup::NO_PROGRESS,
        ) {
            // Roll back to the original database state
            let _ = conn.restore(
                rusqlite::MAIN_DB,
                &temp_rollback,
                rusqlite::backup::NO_PROGRESS,
            );
            let _ = std::fs::remove_file(&temp_rollback);
            return Err(AccountingError::DatabaseError(format!(
                "Database restore failed, rolled back to original state: {}",
                restore_err
            )));
        }

        // Clean up rollback checkpoint file upon successful restore
        let _ = std::fs::remove_file(&temp_rollback);

        // 7. Post-restore pragmas and audit trail logging
        let _ = conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA wal_checkpoint(TRUNCATE);",
        );

        let timestamp = chrono::Utc::now().to_rfc3339();
        let snapshot_path_str = snapshot_path.to_string_lossy().to_string();
        let details = format!(
            "Restored database from snapshot '{}' (SHA-256: {})",
            snapshot_path_str, computed_sha256
        );

        let _ = conn.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                timestamp,
                actor,
                "SYSTEM_RESTORE",
                "SYSTEM",
                snapshot_path_str,
                details,
                Option::<String>::None,
            ],
        );

        Ok(RestoreOutcome {
            restored_from_snapshot: snapshot_path_str,
            sha256_checksum: computed_sha256,
            timestamp,
        })
    }

    /// Lists accounts from the chart of accounts, optionally filtered by class or active status.
    pub fn list_accounts(
        &self,
        class: Option<AccountClass>,
        is_active: Option<bool>,
    ) -> Result<Vec<Account>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = "SELECT code, name, parent_code, class, is_active, normal_balance FROM accounts WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(c) = class {
            sql.push_str(" AND class = ?");
            params_vec.push(Box::new(c.to_string()));
        }
        if let Some(a) = is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(if a { 1 } else { 0 }));
        }
        sql.push_str(" ORDER BY code ASC");

        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| &**b).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_slice.as_slice(), |r| {
            let class_str: String = r.get(3)?;
            let normal_str: String = r.get(5)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                class_str,
                r.get::<_, i32>(4)? != 0,
                normal_str,
            ))
        })?;

        let mut accounts = Vec::new();
        for r in rows {
            let (code, name, parent_code, class_str, is_active, normal_str) = r?;
            let class = class_str.parse().unwrap_or(AccountClass::Asset);
            let normal_balance = normal_str.parse().unwrap_or(NormalBalance::Debit);
            accounts.push(Account {
                code,
                name,
                parent_code,
                class,
                is_active,
                normal_balance,
            });
        }
        Ok(accounts)
    }

    /// Retrieves an account by its code.
    pub fn get_account(&self, code: &str) -> Result<Option<Account>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let row: Option<(String, String, Option<String>, String, i32, String)> = conn
            .query_row(
                "SELECT code, name, parent_code, class, is_active, normal_balance FROM accounts WHERE code = ?1",
                params![code],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()?;

        match row {
            Some((code, name, parent_code, class_str, is_active, normal_str)) => {
                let class = class_str.parse().unwrap_or(AccountClass::Asset);
                let normal_balance = normal_str.parse().unwrap_or(NormalBalance::Debit);
                Ok(Some(Account {
                    code,
                    name,
                    parent_code,
                    class,
                    is_active: is_active != 0,
                    normal_balance,
                }))
            }
            None => Ok(None),
        }
    }

    /// Creates a new account in the chart of accounts with duplicate, parent, and cycle validation.
    /// Also records an immutable CREATE_ACCOUNT audit trail event atomically.
    pub fn create_account(
        &self,
        account: &Account,
        actor: &str,
        idempotency: Option<(&str, &str, &str, &str)>,
    ) -> Result<Account, AccountingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. Check duplicate code (409 Conflict)
        let exists: i64 = tx.query_row(
            "SELECT count(*) FROM accounts WHERE code = ?1",
            params![account.code],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Err(AccountingError::DuplicateAccount(format!(
                "Account code {} already exists",
                account.code
            )));
        }

        // 2. Validate parent if present
        if let Some(ref parent) = account.parent_code {
            if parent == &account.code {
                return Err(AccountingError::CircularAccountReference(format!(
                    "Self-referential account parent: {}",
                    account.code
                )));
            }
            let parent_row: Option<(i32, Option<String>)> = tx
                .query_row(
                    "SELECT is_active, parent_code FROM accounts WHERE code = ?1",
                    params![parent],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;

            match parent_row {
                None => {
                    return Err(AccountingError::ParentAccountNotFound(format!(
                        "Parent account {} does not exist",
                        parent
                    )));
                }
                Some((0, _)) => {
                    return Err(AccountingError::ParentAccountInactive(format!(
                        "Parent account {} is inactive",
                        parent
                    )));
                }
                Some((_, next_parent)) => {
                    // Walk ancestor chain to detect cycle
                    let mut curr_parent = next_parent;
                    let mut visited = std::collections::HashSet::new();
                    visited.insert(parent.clone());
                    while let Some(ancestor) = curr_parent {
                        if ancestor == account.code {
                            return Err(AccountingError::CircularAccountReference(format!(
                                "Circular reference detected between {} and {}",
                                account.code, ancestor
                            )));
                        }
                        if !visited.insert(ancestor.clone()) {
                            break;
                        }
                        let anc_parent: Option<Option<String>> = tx
                            .query_row(
                                "SELECT parent_code FROM accounts WHERE code = ?1",
                                params![ancestor],
                                |r| r.get(0),
                            )
                            .optional()?;
                        curr_parent = anc_parent.flatten();
                    }
                }
            }
        }

        // 3. Insert into accounts
        tx.execute(
            "INSERT INTO accounts (code, name, parent_code, class, is_active, normal_balance) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                account.code,
                account.name,
                account.parent_code,
                account.class.to_string(),
                if account.is_active { 1 } else { 0 },
                account.normal_balance.to_string(),
            ],
        )?;

        // 4. Audit log entry
        let audit_payload = serde_json::to_string(account).unwrap_or_default();
        tx.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details) \
             VALUES (?1, ?2, ?3, 'CREATE_ACCOUNT', 'ACCOUNT', ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                chrono::Utc::now().to_rfc3339(),
                actor,
                account.code,
                audit_payload,
            ],
        )?;

        // 5. Idempotency record if key provided
        if let Some((key, actor_key, endpoint, request_hash)) = idempotency {
            let response_body = serde_json::to_string(account).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to serialize account response: {e}"))
            })?;
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO idempotency_records (key, actor, endpoint, request_hash, status_code, response_body, resource_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    key,
                    actor_key,
                    endpoint,
                    request_hash,
                    201i64,
                    response_body,
                    account.code,
                    now,
                ],
            )?;
        }

        tx.commit()?;
        Ok(account.clone())
    }
}

impl AccountValidator for LedgerRepository {
    fn validate_account(&self, code: &str) -> Result<(), AccountingError> {
        let conn = self.conn.lock().unwrap();
        let row: Option<i32> = conn
            .query_row(
                "SELECT is_active FROM accounts WHERE code = ?1",
                params![code],
                |r| r.get(0),
            )
            .optional()?;
        match row {
            None => Err(AccountingError::UnknownAccount {
                account_code: code.to_string(),
            }),
            Some(0) => Err(AccountingError::InactiveAccount {
                account_code: code.to_string(),
            }),
            Some(_) => Ok(()),
        }
    }
}

/// Result outcome metadata from a successfully executed database restore operation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RestoreOutcome {
    /// Absolute or relative path of the snapshot file restored.
    pub restored_from_snapshot: String,
    /// Computed hex-encoded SHA-256 checksum of the restored snapshot file.
    pub sha256_checksum: String,
    /// RFC-3339 UTC timestamp when the restore operation committed.
    pub timestamp: String,
}

/// Public helper function to execute an offline / CLI database restore from `snapshot` into `target`.
/// Ensures target directory exists, validates snapshot integrity and schema, and restores into target.
pub fn execute_cli_restore(
    snapshot: &std::path::Path,
    target: &std::path::Path,
    actor: &str,
) -> Result<RestoreOutcome, AccountingError> {
    if !snapshot.exists() || !snapshot.is_file() {
        return Err(AccountingError::DatabaseError(format!(
            "Snapshot file does not exist: {}",
            snapshot.display()
        )));
    }

    // Compute and verify snapshot SHA-256
    let bytes = std::fs::read(snapshot).map_err(|e| {
        AccountingError::DatabaseError(format!(
            "Failed to read snapshot file '{}': {}",
            snapshot.display(),
            e
        ))
    })?;

    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let computed_sha256: String = digest
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    // Verify snapshot integrity and schema
    let snap_conn = Connection::open_with_flags(
        snapshot,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        AccountingError::DatabaseError(format!("Failed to open snapshot for pre-validation: {}", e))
    })?;

    let integrity_check: Result<String, rusqlite::Error> =
        snap_conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0));
    match integrity_check {
        Ok(ref check) if check == "ok" => {}
        Ok(other) => {
            return Err(AccountingError::DatabaseError(format!(
                "Snapshot SQLite integrity check failed: {}",
                other
            )));
        }
        Err(e) => {
            return Err(AccountingError::DatabaseError(format!(
                "Failed to execute PRAGMA integrity_check on snapshot: {}",
                e
            )));
        }
    }

    let table_count: i64 = snap_conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('vouchers', 'voucher_lines', 'audit_logs')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to verify schema in snapshot: {}",
                e
            ))
        })?;

    if table_count < 3 {
        return Err(AccountingError::DatabaseError(format!(
            "Snapshot schema is missing required tables (found {} of 3: vouchers, voucher_lines, audit_logs)",
            table_count
        )));
    }
    drop(snap_conn);

    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| {
            AccountingError::DatabaseError(format!("Failed to create target directory: {}", e))
        })?;
    }

    // Restore to target file via direct copy, ensuring zero-byte discrepancy
    std::fs::copy(snapshot, target).map_err(|e| {
        AccountingError::DatabaseError(format!(
            "Failed to copy snapshot to target '{}': {}",
            target.display(),
            e
        ))
    })?;

    // Verify restored target database is valid SQLite
    let target_conn = Connection::open_with_flags(
        target,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        AccountingError::DatabaseError(format!("Failed to open restored target database: {}", e))
    })?;
    let target_check: String = target_conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to verify integrity of restored target database: {}",
                e
            ))
        })?;
    if target_check != "ok" {
        return Err(AccountingError::DatabaseError(format!(
            "Restored target database integrity check failed: {}",
            target_check
        )));
    }

    let timestamp = chrono::Utc::now().to_rfc3339();
    let snapshot_path_str = snapshot.to_string_lossy().to_string();
    let details = format!(
        "Restored database from snapshot '{}' (SHA-256: {}) via CLI",
        snapshot_path_str, computed_sha256
    );

    let _ = target_conn.execute(
        "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            uuid::Uuid::new_v4().to_string(),
            timestamp,
            actor,
            "SYSTEM_RESTORE",
            "SYSTEM",
            snapshot_path_str,
            details,
            Option::<String>::None,
        ],
    );
    drop(target_conn);

    Ok(RestoreOutcome {
        restored_from_snapshot: snapshot_path_str,
        sha256_checksum: computed_sha256,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::LedgerEngine;
    use crate::models::{JournalVoucher, VoucherLine, VoucherStatus};
    use chrono::NaiveDate;
    use rusqlite::Connection;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[test]
    fn test_sqlite_engine_version_wal_resilience() {
        assert!(
            rusqlite::version_number() >= MIN_SQLITE_VERSION_NUMBER,
            "SQLite engine version {} ({}) must be >= 3.53.0 ({})",
            rusqlite::version(),
            rusqlite::version_number(),
            MIN_SQLITE_VERSION_NUMBER
        );
        assert!(LedgerRepository::verify_sqlite_engine_version().is_ok());
        assert!(verify_sqlite_engine_version().is_ok());
    }

    #[test]
    fn test_init_and_save_compound_voucher() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        let engine = LedgerEngine::new();

        let voucher_id = Uuid::new_v4();
        let voucher = JournalVoucher {
            id: voucher_id,
            voucher_no: "PNK-2026-0001".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "Nhập kho nguyên vật liệu từ Công ty XYZ".to_string(),
            invoice_ref: Some("HD-XYZ-991".to_string()),
            status: VoucherStatus::Posted,
            created_by: "accountant_lan".to_string(),
            approved_by: Some("chief_acc_hung".to_string()),
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-10T08:30:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "152".to_string(),
                    debit: dec!(50000000),
                    credit: dec!(0),
                    line_desc: Some("Nguyên vật liệu A".to_string()),
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1331".to_string(),
                    debit: dec!(5000000),
                    credit: dec!(0),
                    line_desc: Some("Thuế GTGT đầu vào 10%".to_string()),
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 3,
                    account_code: "331".to_string(),
                    debit: dec!(0),
                    credit: dec!(55000000),
                    line_desc: Some("Phải trả NCC XYZ".to_string()),
                },
            ],
        };

        assert!(engine.validate_voucher(&voucher).is_ok());
        repo.save_voucher(&voucher).unwrap();

        let retrieved = repo
            .get_voucher(voucher_id)
            .unwrap()
            .expect("Voucher must exist in database");
        assert_eq!(retrieved.voucher_no, "PNK-2026-0001");
        assert_eq!(
            retrieved.voucher_date,
            NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()
        );
        assert_eq!(retrieved.status, VoucherStatus::Posted);
        assert_eq!(retrieved.created_by, "accountant_lan");
        assert_eq!(retrieved.approved_by, Some("chief_acc_hung".to_string()));
        assert_eq!(retrieved.lines.len(), 3);
        assert_eq!(retrieved.lines[0].line_no, 1);
        assert_eq!(retrieved.lines[0].account_code, "152");
        assert_eq!(retrieved.lines[0].debit, dec!(50000000));
        assert_eq!(retrieved.lines[2].line_no, 3);
        assert_eq!(retrieved.lines[2].account_code, "331");
        assert_eq!(retrieved.lines[2].credit, dec!(55000000));
    }

    #[test]
    fn test_list_vouchers_with_date_filter() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        let v1 = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-001".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "Jan 10 transaction".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-10T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(100),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(100),
                    line_desc: None,
                },
            ],
        };

        let v2 = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-002".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
            description: "Jan 20 transaction".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-20T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(200),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(200),
                    line_desc: None,
                },
            ],
        };

        let v3 = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-003".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
            description: "Feb 5 transaction".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-02-05T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(300),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(300),
                    line_desc: None,
                },
            ],
        };

        repo.save_voucher(&v1).unwrap();
        repo.save_voucher(&v2).unwrap();
        repo.save_voucher(&v3).unwrap();

        // 1. All vouchers
        let all = repo.list_vouchers(None, None).unwrap();
        assert_eq!(all.len(), 3);

        // 2. Filter Jan 15 to Jan 25 -> only v2
        let filtered = repo
            .list_vouchers(
                Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()),
                Some(NaiveDate::from_ymd_opt(2026, 1, 25).unwrap()),
            )
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].voucher_no, "V-002");

        // 3. Filter from Jan 15 onwards -> v2 and v3
        let from_jan15 = repo
            .list_vouchers(Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()), None)
            .unwrap();
        assert_eq!(from_jan15.len(), 2);
    }

    #[test]
    fn test_compute_trial_balance() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        let empty_tb = repo.compute_trial_balance().unwrap();
        assert!(empty_tb.is_empty());

        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-001".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "Materials purchase".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-10T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "152".to_string(),
                    debit: dec!(1000),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1331".to_string(),
                    debit: dec!(100),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 3,
                    account_code: "331".to_string(),
                    debit: dec!(0),
                    credit: dec!(1100),
                    line_desc: None,
                },
            ],
        };
        repo.save_voucher(&v).unwrap();

        let tb = repo.compute_trial_balance().unwrap();
        assert_eq!(tb.len(), 3);
        assert_eq!(tb[0].account_code, "1331");
        assert_eq!(tb[0].total_debit, dec!(100));
        assert_eq!(tb[0].total_credit, dec!(0));
        assert_eq!(tb[0].net_balance, dec!(100));

        assert_eq!(tb[1].account_code, "152");
        assert_eq!(tb[1].total_debit, dec!(1000));
        assert_eq!(tb[1].total_credit, dec!(0));
        assert_eq!(tb[1].net_balance, dec!(1000));

        assert_eq!(tb[2].account_code, "331");
        assert_eq!(tb[2].total_debit, dec!(0));
        assert_eq!(tb[2].total_credit, dec!(1100));
        assert_eq!(tb[2].net_balance, dec!(-1100));
    }

    #[test]
    fn test_period_lock_and_lifecycle() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        assert!(!repo.is_period_closed(2026, 1).unwrap());

        let closing_voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "KC-2026-01".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
            description: "Closing voucher".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-31T23:59:59Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "911".to_string(),
                    debit: dec!(1000),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "4212".to_string(),
                    debit: dec!(0),
                    credit: dec!(1000),
                    line_desc: None,
                },
            ],
        };

        repo.close_period(2026, 1, &closing_voucher).unwrap();
        assert!(repo.is_period_closed(2026, 1).unwrap());

        // Attempting to close again returns PeriodAlreadyClosed
        assert_eq!(
            repo.close_period(2026, 1, &closing_voucher).unwrap_err(),
            AccountingError::PeriodAlreadyClosed {
                year: 2026,
                month: 1
            }
        );

        // Attempting to post into closed period returns PeriodClosed
        let v_late = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-LATE".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "Late voucher".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(100),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(100),
                    line_desc: None,
                },
            ],
        };
        assert_eq!(
            repo.save_voucher(&v_late).unwrap_err(),
            AccountingError::PeriodClosed {
                year: 2026,
                month: 1
            }
        );

        // Reopen period
        repo.reopen_period(2026, 1, "chief_controller").unwrap();
        assert!(!repo.is_period_closed(2026, 1).unwrap());

        // Verify closing voucher is now VOIDED and still present
        let voided_kc = repo.get_voucher(closing_voucher.id).unwrap().unwrap();
        assert_eq!(voided_kc.status, VoucherStatus::Voided);
        assert_eq!(voided_kc.void_reason, Some("Period reopened".to_string()));
        assert_eq!(voided_kc.voided_by, Some("chief_controller".to_string()));

        // Reopen again returns PeriodNotClosed
        assert_eq!(
            repo.reopen_period(2026, 1, "chief_controller").unwrap_err(),
            AccountingError::PeriodNotClosed {
                year: 2026,
                month: 1
            }
        );

        // Now posting into period succeeds
        repo.save_voucher(&v_late).unwrap();
    }

    #[test]
    fn test_void_voucher_and_audit_log_capture() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        let v_id = Uuid::new_v4();
        let v = JournalVoucher {
            id: v_id,
            voucher_no: "V-AUDIT-01".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "To be voided".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "accountant_lan".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-10T08:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(5000),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(5000),
                    line_desc: None,
                },
            ],
        };

        repo.save_voucher(&v).unwrap();

        // Trial balance includes it
        let tb = repo.compute_trial_balance().unwrap();
        assert_eq!(tb.len(), 2);

        // Void the voucher
        let voided = repo
            .void_voucher(
                v_id,
                "cfo_minh",
                "Duplicate transaction entry",
                Some("127.0.0.1"),
            )
            .unwrap();
        assert_eq!(voided.status, VoucherStatus::Voided);
        assert_eq!(voided.voided_by, Some("cfo_minh".to_string()));
        assert_eq!(
            voided.void_reason,
            Some("Duplicate transaction entry".to_string())
        );

        // Trial balance excludes it!
        let tb_after = repo.compute_trial_balance().unwrap();
        assert!(tb_after.is_empty());

        // Cannot void again
        assert_eq!(
            repo.void_voucher(v_id, "cfo_minh", "Again", None)
                .unwrap_err(),
            AccountingError::VoucherAlreadyVoided {
                voucher_no: "V-AUDIT-01".to_string()
            }
        );

        // Audit logs recorded CREATE_VOUCHER and VOID_VOUCHER
        let logs = repo
            .list_audit_logs(&crate::dto::AuditLogQuery::default())
            .unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].action, "VOID_VOUCHER");
        assert_eq!(logs[0].actor, "cfo_minh");
        assert_eq!(logs[0].client_ip, Some("127.0.0.1".to_string()));
        assert_eq!(logs[1].action, "CREATE_VOUCHER");
        assert_eq!(logs[1].actor, "accountant_lan");
    }

    #[test]
    fn test_sqlite_pragmas_and_backup() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        // Verify pragmas
        {
            let conn_guard = repo.conn.lock().unwrap();
            let busy_timeout: i64 = conn_guard
                .query_row("PRAGMA busy_timeout;", [], |r| r.get(0))
                .unwrap();
            assert_eq!(busy_timeout, 5000);

            let foreign_keys: i64 = conn_guard
                .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
                .unwrap();
            assert_eq!(foreign_keys, 1);
        }

        // Test create_backup to temp dir
        let temp_dir = std::env::temp_dir().join(format!("test_backup_{}", Uuid::new_v4()));
        let backup_file = temp_dir.join("snapshot.db");
        let backup_path = backup_file.to_str().unwrap();

        let size = repo
            .create_backup(backup_path, "chief_controller")
            .expect("create_backup should succeed");
        assert!(size > 0);
        assert!(backup_file.exists());

        // Test stale file handling (overwriting existing file)
        let size2 = repo
            .create_backup(backup_path, "chief_controller")
            .expect("create_backup overwrite should succeed");
        assert!(size2 > 0);

        // Verify audit log captured SYSTEM_BACKUP
        let logs = repo
            .list_audit_logs(&crate::dto::AuditLogQuery {
                action: Some("SYSTEM_BACKUP".to_string()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].action, "SYSTEM_BACKUP");

        // Clean up
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_sqlite_immutability_triggers() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        let v_id = Uuid::new_v4();
        let voucher = JournalVoucher {
            id: v_id,
            voucher_no: "TRG-001".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "Trigger immutability test".to_string(),
            invoice_ref: Some("INV-001".to_string()),
            status: VoucherStatus::Posted,
            created_by: "accountant_a".to_string(),
            approved_by: Some("manager_b".to_string()),
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T08:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(1000),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(1000),
                    line_desc: None,
                },
            ],
        };
        repo.save_voucher(&voucher).unwrap();

        let raw = repo.conn.lock().unwrap();

        // 1. Audit logs: Raw UPDATE must fail
        let err = raw
            .execute("UPDATE audit_logs SET actor = 'attacker'", [])
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("audit_logs are immutable: UPDATE not permitted")
        );

        // 2. Audit logs: Raw DELETE must fail
        let err = raw.execute("DELETE FROM audit_logs", []).unwrap_err();
        assert!(
            err.to_string()
                .contains("audit_logs are immutable: DELETE not permitted")
        );

        // 3. Voucher lines: Raw DELETE must fail
        let err = raw.execute("DELETE FROM voucher_lines", []).unwrap_err();
        assert!(
            err.to_string()
                .contains("voucher_lines are immutable: DELETE not permitted")
        );

        // 4. Voucher lines: Raw UPDATE must fail
        let err = raw
            .execute("UPDATE voucher_lines SET debit = '9999'", [])
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("voucher_lines are immutable: UPDATE not permitted")
        );

        // 5. Vouchers: Raw DELETE must fail
        let err = raw
            .execute("DELETE FROM vouchers WHERE id = ?1", [v_id.to_string()])
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("vouchers are immutable: DELETE not permitted")
        );

        // 6. Vouchers: Raw UPDATE of description on POSTED voucher must fail
        let err = raw
            .execute(
                "UPDATE vouchers SET description = 'tampered' WHERE id = ?1",
                [v_id.to_string()],
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("vouchers are immutable: only voiding transitions")
        );

        // 7. Vouchers: Raw UPDATE altering date during void must fail
        let err = raw
            .execute(
                "UPDATE vouchers SET status = 'VOIDED', voucher_date = '2026-02-01' WHERE id = ?1",
                [v_id.to_string()],
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("vouchers are immutable: only voiding transitions")
        );

        drop(raw);

        // 8. Permitted application flow: void_voucher succeeds
        let voided = repo
            .void_voucher(v_id, "controller_c", "Valid void test", None)
            .unwrap();
        assert_eq!(voided.status, VoucherStatus::Voided);

        // 9. Vouchers: Raw UPDATE on already VOIDED voucher must fail
        let raw2 = repo.conn.lock().unwrap();
        let err = raw2
            .execute(
                "UPDATE vouchers SET void_reason = 'tampered again' WHERE id = ?1",
                [v_id.to_string()],
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("vouchers are immutable: only voiding transitions")
        );
    }

    #[test]
    fn test_idempotency_schema_and_immutability_triggers() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        // 1. Insert an idempotency record directly
        let raw = repo.conn.lock().unwrap();
        raw.execute(
            "INSERT INTO idempotency_records (key, actor, endpoint, request_hash, status_code, response_body, resource_id, created_at)
             VALUES ('key1', 'user1', '/api/v1/vouchers', 'hash1', 201, '{}', 'res1', '2026-02-15T00:00:00Z')",
            [],
        )
        .unwrap();

        // 2. Immutability: Raw UPDATE must fail
        let err_update = raw
            .execute(
                "UPDATE idempotency_records SET request_hash = 'tampered'",
                [],
            )
            .unwrap_err();
        assert!(
            err_update
                .to_string()
                .contains("idempotency_records are immutable: UPDATE not permitted")
        );

        // 3. Immutability: Raw DELETE must fail
        let err_delete = raw
            .execute("DELETE FROM idempotency_records", [])
            .unwrap_err();
        assert!(
            err_delete
                .to_string()
                .contains("idempotency_records are immutable: DELETE not permitted")
        );
    }

    #[test]
    fn test_save_voucher_with_idempotency_lifecycle() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        let v1_id = Uuid::new_v4();
        let voucher1 = JournalVoucher {
            id: v1_id,
            voucher_no: "PKT-UNIT-01".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
            description: "Unit test voucher".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "accountant_lan".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-02-10T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: Decimal::from(1000),
                    credit: Decimal::ZERO,
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: Decimal::ZERO,
                    credit: Decimal::from(1000),
                    line_desc: None,
                },
            ],
        };

        // 1. First call: Created
        let outcome1 = repo
            .save_voucher_with_idempotency(
                &voucher1,
                Some((
                    "key-unit-1",
                    "accountant_lan",
                    "/api/v1/vouchers",
                    "hash-abc",
                )),
            )
            .unwrap();
        assert!(matches!(outcome1, SaveVoucherOutcome::Created(_)));

        // Record exists in DB
        let rec = repo
            .get_idempotency_record("key-unit-1", "accountant_lan", "/api/v1/vouchers")
            .unwrap()
            .expect("Idempotency record should exist");
        assert_eq!(rec.request_hash, "hash-abc");
        assert_eq!(rec.status_code, 201);

        // 2. Replay with identical key and hash: returns Replayed with original voucher
        let outcome2 = repo
            .save_voucher_with_idempotency(
                &voucher1,
                Some((
                    "key-unit-1",
                    "accountant_lan",
                    "/api/v1/vouchers",
                    "hash-abc",
                )),
            )
            .unwrap();
        match outcome2 {
            SaveVoucherOutcome::Replayed(replayed) => {
                assert_eq!(replayed.id, v1_id);
                assert_eq!(replayed.voucher_no, "PKT-UNIT-01");
            }
            _ => panic!("Expected Replayed outcome"),
        }

        // Verify no duplicate voucher in DB
        let vouchers = repo.list_vouchers(None, None).unwrap();
        assert_eq!(vouchers.len(), 1);

        // 3. Conflict with same key but different hash: returns AccountingError::IdempotencyConflict
        let res_conflict = repo.save_voucher_with_idempotency(
            &voucher1,
            Some((
                "key-unit-1",
                "accountant_lan",
                "/api/v1/vouchers",
                "hash-DIFFERENT",
            )),
        );
        assert_eq!(
            res_conflict.unwrap_err(),
            AccountingError::IdempotencyConflict {
                key: "key-unit-1".to_string()
            }
        );

        // 4. Multi-actor isolation: different actor with SAME key succeeds
        let v2_id = Uuid::new_v4();
        let voucher2 = JournalVoucher {
            id: v2_id,
            voucher_no: "PKT-UNIT-02".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
            description: "Second actor voucher".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "accountant_hoa".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-02-10T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: Decimal::from(2000),
                    credit: Decimal::ZERO,
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: Decimal::ZERO,
                    credit: Decimal::from(2000),
                    line_desc: None,
                },
            ],
        };
        let outcome_hoa = repo
            .save_voucher_with_idempotency(
                &voucher2,
                Some((
                    "key-unit-1",
                    "accountant_hoa",
                    "/api/v1/vouchers",
                    "hash-hoa",
                )),
            )
            .unwrap();
        assert!(matches!(outcome_hoa, SaveVoucherOutcome::Created(_)));

        // Both records exist
        let rec_lan = repo
            .get_idempotency_record("key-unit-1", "accountant_lan", "/api/v1/vouchers")
            .unwrap();
        let rec_hoa = repo
            .get_idempotency_record("key-unit-1", "accountant_hoa", "/api/v1/vouchers")
            .unwrap();
        assert!(rec_lan.is_some());
        assert!(rec_hoa.is_some());
    }

    #[test]
    fn test_save_voucher_with_idempotency_closed_period_rollback() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        // Close period 2026-04 manually in DB
        {
            let raw = repo.conn.lock().unwrap();
            raw.execute(
                "INSERT INTO accounting_periods (year, month, is_closed) VALUES (2026, 4, 1)",
                [],
            )
            .unwrap();
        }

        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "PKT-CLOSED-01".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
            description: "In closed period".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "accountant_lan".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-04-15T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: Decimal::from(1000),
                    credit: Decimal::ZERO,
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: Decimal::ZERO,
                    credit: Decimal::from(1000),
                    line_desc: None,
                },
            ],
        };

        let err = repo
            .save_voucher_with_idempotency(
                &voucher,
                Some((
                    "locked-key",
                    "accountant_lan",
                    "/api/v1/vouchers",
                    "hash-lock",
                )),
            )
            .unwrap_err();
        assert!(matches!(
            err,
            AccountingError::PeriodClosed {
                year: 2026,
                month: 4
            }
        ));

        // Ensure no idempotency record was created
        let rec = repo
            .get_idempotency_record("locked-key", "accountant_lan", "/api/v1/vouchers")
            .unwrap();
        assert!(rec.is_none());

        // Ensure no voucher in DB
        let vouchers = repo.list_vouchers(None, None).unwrap();
        assert!(vouchers.is_empty());
    }

    #[test]
    fn test_restore_from_snapshot_lifecycle_and_rollback() {
        let temp_dir = std::env::temp_dir().join(format!("test_restore_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let snap_file = temp_dir.join("test_snap.db");

        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        // 1. Post a baseline voucher
        let v1_id = Uuid::new_v4();
        let voucher1 = JournalVoucher {
            id: v1_id,
            voucher_no: "PKT-SNAP-01".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            description: "Baseline voucher before snapshot".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "accountant_lan".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-10T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: Decimal::from(5000),
                    credit: Decimal::ZERO,
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: Decimal::ZERO,
                    credit: Decimal::from(5000),
                    line_desc: None,
                },
            ],
        };
        repo.save_voucher(&voucher1).unwrap();

        // 2. Create backup snapshot
        let snap_str = snap_file.to_string_lossy().to_string();
        repo.create_backup(&snap_str, "admin").unwrap();
        assert!(snap_file.exists());

        // 3. Post a second voucher post-backup (rogue data)
        let v2_id = Uuid::new_v4();
        let voucher2 = JournalVoucher {
            id: v2_id,
            voucher_no: "PKT-ROGUE-02".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "Rogue voucher after snapshot".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "rogue_actor".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T00:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: Decimal::from(1000),
                    credit: Decimal::ZERO,
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: Decimal::ZERO,
                    credit: Decimal::from(1000),
                    line_desc: None,
                },
            ],
        };
        repo.save_voucher(&voucher2).unwrap();
        assert_eq!(repo.list_vouchers(None, None).unwrap().len(), 2);

        // 4. Test SHA-256 mismatch rejection (leaves active database intact)
        let err_hash = repo
            .restore_from_snapshot(
                &snap_file,
                Some("0000000000000000000000000000000000000000000000000000000000000000"),
                "admin",
            )
            .unwrap_err();
        assert!(err_hash.to_string().contains("checksum mismatch"));
        assert_eq!(repo.list_vouchers(None, None).unwrap().len(), 2);

        // 5. Test successful restore from snapshot
        let outcome = repo
            .restore_from_snapshot(&snap_file, None, "system_admin")
            .unwrap();
        assert_eq!(outcome.sha256_checksum.len(), 64);
        assert_eq!(outcome.restored_from_snapshot, snap_str);

        // Verify state is restored: voucher 2 is gone, voucher 1 is present
        let restored_vouchers = repo.list_vouchers(None, None).unwrap();
        assert_eq!(restored_vouchers.len(), 1);
        assert_eq!(restored_vouchers[0].voucher_no, "PKT-SNAP-01");

        // Verify audit log has SYSTEM_RESTORE
        let logs = repo
            .list_audit_logs(&crate::dto::AuditLogQuery::default())
            .unwrap();
        let has_restore_log = logs
            .iter()
            .any(|l| l.action == "SYSTEM_RESTORE" && l.actor == "system_admin");
        assert!(has_restore_log, "SYSTEM_RESTORE audit log must be recorded");

        // 6. Test execute_cli_restore
        let target_file = temp_dir.join("cli_target.db");
        let cli_outcome = execute_cli_restore(&snap_file, &target_file, "cli_admin").unwrap();
        assert_eq!(cli_outcome.sha256_checksum, outcome.sha256_checksum);
        assert!(target_file.exists());
        // Verify audit log in cli_target.db
        let target_conn = Connection::open(&target_file).unwrap();
        let target_actor: String = target_conn
            .query_row(
                "SELECT actor FROM audit_logs WHERE action = 'SYSTEM_RESTORE' ORDER BY id DESC LIMIT 1;",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(target_actor, "cli_admin");

        let target_vouchers_count: i64 = target_conn
            .query_row("SELECT count(*) FROM vouchers;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(target_vouchers_count, 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
