//! SQLite database schema initialization, DDL migrations, and table definitions.

use crate::database::LedgerRepository;
use crate::ledger::AccountingError;
use rusqlite::params;

impl LedgerRepository {
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
                status TEXT NOT NULL DEFAULT 'DRAFT',
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
            WHEN NOT (
              -- Allowed Transition 1: VOID an active POSTED voucher
              (OLD.status = 'POSTED'
               AND NEW.status = 'VOIDED'
               AND NEW.id IS OLD.id
               AND NEW.voucher_no IS OLD.voucher_no
               AND NEW.voucher_date IS OLD.voucher_date
               AND NEW.description IS OLD.description
               AND NEW.invoice_ref IS OLD.invoice_ref
               AND NEW.created_by IS OLD.created_by
               AND NEW.approved_by IS OLD.approved_by
               AND NEW.created_at IS OLD.created_at)
              OR
              -- Allowed Transition 2: APPROVE a DRAFT voucher to POSTED
              (OLD.status = 'DRAFT'
               AND NEW.status = 'POSTED'
               AND NEW.id IS OLD.id
               AND NEW.voucher_no IS OLD.voucher_no
               AND NEW.voucher_date IS OLD.voucher_date
               AND NEW.description IS OLD.description
               AND NEW.invoice_ref IS OLD.invoice_ref
               AND NEW.created_by IS OLD.created_by
               AND NEW.voided_at IS NULL
               AND NEW.voided_by IS NULL
               AND NEW.void_reason IS NULL
               AND NEW.created_at IS OLD.created_at)
            )
            BEGIN
                SELECT RAISE(ABORT, 'vouchers are immutable: only voiding transitions (status, voided_at, voided_by, void_reason) of POSTED vouchers or DRAFT -> POSTED approval are permitted');
            END;
            ",
        )?;

        Ok(())
    }
}
