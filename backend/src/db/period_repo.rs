//! Period lock management and period closing/reopening lifecycle.

use crate::database::LedgerRepository;
use crate::ledger::AccountingError;
use crate::models::{AccountingPeriod, AuditLogEntry, JournalVoucher};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl LedgerRepository {
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
}
