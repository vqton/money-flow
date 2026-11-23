//! Voucher read queries: get single voucher and list vouchers with filtering.

use crate::database::LedgerRepository;
use crate::ledger::AccountingError;
use crate::models::{JournalVoucher, VoucherLine, VoucherStatus};
use chrono::NaiveDate;
use rusqlite::params;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

impl LedgerRepository {
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
}
