// RED tests for DB‑level period guard (P0‑4)

#[cfg(test)]
mod period_guard_tests {
    use super::*;
use crate::db::LedgerRepository;
use crate::ledger::AccountingError;
use rusqlite::params;
    use crate::ledger::LedgerEngine;
    use crate::models::{JournalVoucher, VoucherLine, VoucherStatus};
    use chrono::NaiveDate;
    use rusqlite::Connection;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_repo() -> LedgerRepository {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();
        repo
    }

    #[test]
    fn db_guard_blocks_insert_voucher_into_closed_period() {
        let repo = make_repo();
        // close period 2023‑05 via the repository API
        let closing_voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "KC-2023-05".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 5, 31).unwrap(),
            description: "closing".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-05-31T23:59:59Z".to_string(),
            lines: vec![],
        };
        repo.close_period(2023, 5, &closing_voucher).unwrap();
        // attempt to insert a voucher dated inside the closed period
        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-2023-05-15".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 5, 15).unwrap(),
            description: "test".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-05-15T12:00:00Z".to_string(),
            lines: vec![],
        };
        let err = repo.save_voucher(&v).unwrap_err();
        assert!(matches!(err, AccountingError::PeriodClosed { year: 2023, month: 5 }));
    }

    #[test]
    fn db_guard_blocks_insert_voucher_line_into_closed_period() {
        let repo = make_repo();
        // create a voucher in open period
        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-OPEN".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 4, 10).unwrap(),
            description: "open".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-04-10T12:00:00Z".to_string(),
            lines: vec![],
        };
        repo.save_voucher(&v).unwrap();
        // close the period of that voucher (2023‑04)
        let closing_voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "KC-2023-04".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 4, 30).unwrap(),
            description: "closing".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-04-30T23:59:59Z".to_string(),
            lines: vec![],
        };
        repo.close_period(2023, 4, &closing_voucher).unwrap();
        // attempt to insert a line for the existing voucher via raw SQL (still a DB write)
        let conn = repo.conn.lock().unwrap();
        let stmt = conn.prepare(
            "INSERT INTO voucher_lines (id, voucher_id, line_no, account_code, debit, credit, line_desc) VALUES (?, ?, ?, ?, ?, ?, ?)"
        ).unwrap();
        let err = stmt.execute(rusqlite::params![
            Uuid::new_v4().to_string(),
            v.id.to_string(),
            1,
            "1111",
            dec!(100).to_string(),
            dec!(0).to_string(),
            None::<String>
        ]).unwrap_err();
        assert!(err.to_string().contains("PERIOD_CLOSED"));
    }

    #[test]
    fn db_guard_blocks_status_change_in_closed_period() {
        let repo = make_repo();
        // create and post a voucher in open period
        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-POSTED".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 6, 5).unwrap(),
            description: "test".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-06-05T12:00:00Z".to_string(),
            lines: vec![],
        };
        repo.save_voucher(&v).unwrap();
        // close that period
        let closing_voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "KC-2023-06".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
            description: "closing".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-06-30T23:59:59Z".to_string(),
            lines: vec![],
        };
        repo.close_period(2023, 6, &closing_voucher).unwrap();
        // attempt to change status via raw UPDATE
        let conn = repo.conn.lock().unwrap();
        let err = conn.execute(
            "UPDATE vouchers SET status = 'VOIDED' WHERE voucher_no = ?1",
            rusqlite::params![v.voucher_no]
        ).unwrap_err();
        assert!(err.to_string().contains("PERIOD_CLOSED"));
    }

    #[test]
    fn db_guard_allows_writes_in_open_period() {
        let repo = make_repo();
        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-OK".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2023, 7, 1).unwrap(),
            description: "ok".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2023-07-01T12:00:00Z".to_string(),
            lines: vec![],
        };
        repo.save_voucher(&v).unwrap();
        // update status while period is open
        let conn = repo.conn.lock().unwrap();
        conn.execute("UPDATE vouchers SET status = 'VOIDED' WHERE voucher_no = ?1", rusqlite::params![v.voucher_no]).unwrap();
    }

    #[test]
    fn db_guard_allows_write_when_period_row_missing() {
        let repo = make_repo();
        // No entry in accounting_periods for 2025‑09 (implicitly open)
        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-MISSING".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2025, 9, 15).unwrap(),
            description: "missing period".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2025-09-15T12:00:00Z".to_string(),
            lines: vec![],
        };
        repo.save_voucher(&v).unwrap();
    }

    #[test]
    fn db_guard_closing_sequence_still_succeeds() {
        let repo = make_repo();
        // create a posting voucher in period 2022‑12
        let v = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "V-DEC".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2022, 12, 15).unwrap(),
            description: "dec".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2022-12-15T12:00:00Z".to_string(),
            lines: vec![],
        };
        repo.save_voucher(&v).unwrap();
        // run the standard closing routine for that month
        let closing_voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "KC-2022-12".to_string(),
            voucher_date: NaiveDate::from_ymd_opt(2022, 12, 31).unwrap(),
            description: "closing".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2022-12-31T23:59:59Z".to_string(),
            lines: vec![],
        };
        repo.close_period(2022, 12, &closing_voucher).unwrap();
    }

    #[test]
    fn db_guard_migration_from_baseline_adds_triggers() {
        // Initialise a fresh in‑memory DB and run init_schema (which executes migrations)
        let repo = make_repo();
        // Verify triggers exist
        let conn = repo.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='trigger' AND name LIKE 'trg_%' ORDER BY name").unwrap();
        let trigger_names: Vec<String> = stmt.query_map([], |row| row.get(0)).unwrap().map(|r| r.unwrap()).collect();
        assert!(trigger_names.contains(&"trg_vouchers_before_insert".to_string()));
        assert!(trigger_names.contains(&"trg_voucher_lines_before_insert".to_string()));
        assert!(trigger_names.contains(&"trg_vouchers_before_update_status".to_string()));
        // Verify user_version == 3
        let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        assert_eq!(version, 3);
        // Re‑open (init_schema again) – version should stay 3 and no error
        drop(conn);
        repo.init_schema().unwrap();
        let conn2 = repo.conn.lock().unwrap();
        let version2: i32 = conn2.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        assert_eq!(version2, 3);
    }
}
