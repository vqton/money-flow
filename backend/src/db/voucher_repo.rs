//! Voucher persistence, idempotency tracking, and voiding.

use crate::database::{LedgerRepository, SaveVoucherOutcome};
use crate::ledger::AccountingError;
use crate::models::{AuditLogEntry, JournalVoucher};
use chrono::{Datelike, NaiveDate};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl LedgerRepository {
    /// Internal helper to insert voucher header and lines within an active transaction.
    pub(crate) fn insert_voucher_tx(
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

    /// Persists a complete JournalVoucher (header + lines) with optional idempotency tracking.
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
}
