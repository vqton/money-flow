use super::*;
use crate::ledger::LedgerEngine;
use crate::models::{JournalVoucher, VoucherLine, VoucherStatus};
use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
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
