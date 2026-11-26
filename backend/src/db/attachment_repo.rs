// Attachment repository – handles persistence of VoucherAttachment metadata.
use crate::database::LedgerRepository;
use crate::ledger::AccountingError;
use crate::models::{VoucherAttachment, VoucherStatus};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;
use chrono::Utc;

pub struct AttachmentRepo<'a> {
    repo: &'a LedgerRepository,
}

impl<'a> AttachmentRepo<'a> {
    pub fn new(repo: &'a LedgerRepository) -> Self {
        Self { repo }
    }

    /// Insert a new attachment record after validating business rules.
    /// * voucher must exist and be in POSTED status.
    /// * the accounting period for the voucher date must be open.
    /// * file size must be <= MAX_ATTACHMENT_SIZE (10 MiB).
    pub fn add_attachment(
        &self,
        voucher_id: Uuid,
        file_name: String,
        content_type: String,
        file_size: u64,
        sha256_hash: String,
        uploaded_by: String,
    ) -> Result<VoucherAttachment, AccountingError> {
        const MAX_ATTACHMENT_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB
        if file_size > MAX_ATTACHMENT_SIZE {
            return Err(AccountingError::InvalidAttachmentSize);
        }
        let conn = self.repo.conn.lock().unwrap();
        // Verify voucher exists, status, and period lock.
        let (status, voucher_date): (String, String) = conn
            .query_row(
                "SELECT status, voucher_date FROM vouchers WHERE id = ?1",
                params![voucher_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(AccountingError::VoucherNotFound)?;
        if status != VoucherStatus::Posted.to_string() {
            return Err(AccountingError::PeriodClosed); // reuse error code for simplicity
        }
        // Period lock check – assume accounting periods table stores is_closed per month.
        let (year, month) = {
            let dt = chrono::NaiveDate::parse_from_str(&voucher_date, "%Y-%m-%d")
                .map_err(|_| AccountingError::InvalidDate)?;
            (dt.year(), dt.month())
        };
        let is_closed: i64 = conn
            .query_row(
                "SELECT is_closed FROM accounting_periods WHERE year = ?1 AND month = ?2",
                params![year, month as i64],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if is_closed != 0 {
            return Err(AccountingError::PeriodClosed);
        }
        // Insert attachment metadata.
        let att_id = Uuid::new_v4();
        let uploaded_at = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO voucher_attachments (id, voucher_id, file_name, content_type, file_size, sha256_hash, uploaded_by, uploaded_at) \
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                att_id.to_string(),
                voucher_id.to_string(),
                file_name,
                content_type,
                file_size as i64,
                sha256_hash,
                uploaded_by,
                uploaded_at,
            ],
        )?;
        Ok(VoucherAttachment {
            id: att_id,
            voucher_id,
            file_name: file_name.clone(),
            content_type: content_type.clone(),
            file_size,
            sha256_hash,
            uploaded_by,
            uploaded_at,
        })
    }

    /// List all attachments for a given voucher.
    pub fn list_by_voucher(
        &self,
        voucher_id: Uuid,
    ) -> Result<Vec<VoucherAttachment>, AccountingError> {
        let conn = self.repo.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, voucher_id, file_name, content_type, file_size, sha256_hash, uploaded_by, uploaded_at \
            FROM voucher_attachments WHERE voucher_id = ?1",
        )?;
        let rows = stmt.query_map(params![voucher_id.to_string()], |row| {
            Ok(VoucherAttachment {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
                voucher_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                file_name: row.get(2)?,
                content_type: row.get(3)?,
                file_size: row.get::<_, i64>(4)? as u64,
                sha256_hash: row.get(5)?,
                uploaded_by: row.get(6)?,
                uploaded_at: row.get(7)?,
            })
        })?;
        let mut vec = Vec::new();
        for r in rows { vec.push(r?); }
        Ok(vec)
    }

    /// Retrieve a specific attachment by its ID.
    pub fn get_attachment(
        &self,
        att_id: Uuid,
    ) -> Result<VoucherAttachment, AccountingError> {
        let conn = self.repo.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT id, voucher_id, file_name, content_type, file_size, sha256_hash, uploaded_by, uploaded_at \
            FROM voucher_attachments WHERE id = ?1",
            params![att_id.to_string()],
            |row| {
                Ok(VoucherAttachment {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
                    voucher_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    file_name: row.get(2)?,
                    content_type: row.get(3)?,
                    file_size: row.get::<_, i64>(4)? as u64,
                    sha256_hash: row.get(5)?,
                    uploaded_by: row.get(6)?,
                    uploaded_at: row.get(7)?,
                })
            },
        )
        .optional()?;
        row.ok_or(AccountingError::AttachmentNotFound)
    }
}

// Unit tests for the repository – these run against an in‑memory DB.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::LedgerRepository;
    use rusqlite::Connection;
    use uuid::Uuid;

    fn setup_repo() -> LedgerRepository {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();
        // Insert a test voucher (POSTED) for attachment linkage.
        let voucher_id = Uuid::new_v4();
        repo.conn.lock().unwrap().execute(
            "INSERT INTO vouchers (id, voucher_no, voucher_date, description, status, created_by, created_at) \
            VALUES (?1, 'TEST-001', '2025-06-15', 'test', 'POSTED', 'tester', datetime('now'))",
            params![voucher_id.to_string()],
        ).unwrap();
        // Ensure period is open.
        repo.conn.lock().unwrap().execute(
            "INSERT INTO accounting_periods (year, month, is_closed) VALUES (2025, 6, 0)",
            [],
        ).unwrap();
        repo
    }

    #[test]
    fn insert_and_fetch_attachment() {
        let repo = setup_repo();
        let attachment_repo = AttachmentRepo::new(&repo);
        let voucher_id = repo.conn.lock().unwrap().query_row(
            "SELECT id FROM vouchers LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        ).unwrap();
        let voucher_uuid = Uuid::parse_str(&voucher_id).unwrap();
        let att = attachment_repo
            .add_attachment(
                voucher_uuid,
                "doc.pdf".into(),
                "application/pdf".into(),
                1024,
                "a".repeat(64),
                "alice".into(),
            )
            .expect("insert attachment");
        // Fetch directly by ID.
        let fetched = attachment_repo.get_attachment(att.id).expect("get attachment");
        assert_eq!(att, fetched);
        // List by voucher.
        let list = attachment_repo.list_by_voucher(voucher_uuid).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], att);
    }

    #[test]
    fn reject_closed_period() {
        let repo = setup_repo();
        // close the period for the voucher's month
        repo.conn.lock().unwrap().execute(
            "UPDATE accounting_periods SET is_closed = 1 WHERE year = 2025 AND month = 6",
            [],
        ).unwrap();
        let attachment_repo = AttachmentRepo::new(&repo);
        let voucher_id = repo.conn.lock().unwrap().query_row(
            "SELECT id FROM vouchers LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        ).unwrap();
        let voucher_uuid = Uuid::parse_str(&voucher_id).unwrap();
        let err = attachment_repo
            .add_attachment(
                voucher_uuid,
                "doc.pdf".into(),
                "application/pdf".into(),
                1024,
                "b".repeat(64),
                "bob".into(),
            )
            .unwrap_err();
        matches!(err, AccountingError::PeriodClosed);
    }
}
