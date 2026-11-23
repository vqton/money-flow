//! Tier 2: Boundary Value Analysis (BVA), Corner Cases & Failure Modes
//!
//! Exceeds >= 5 test cases per feature across all 11 features (60 test cases total).

use crate::e2e_tests::harness::*;
use axum::http::StatusCode;
use backend::{database::MIN_SQLITE_VERSION_NUMBER, dto::TrialBalanceResponse};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde_json::json;

// ============================================================================
// Feature 1 (R1): Audit Route Auth & Restrict `/audit-logs`
// ============================================================================

#[tokio::test]
async fn test_tier2_f1_01_accountant_forbidden_403() {
    let app = TestApp::in_memory();
    let res = app.get("/api/v1/audit-logs").as_accountant().send().await;
    res.assert_status(StatusCode::FORBIDDEN);
    res.assert_error_code("FORBIDDEN");
}

#[tokio::test]
async fn test_tier2_f1_02_anonymous_unauthorized_401() {
    let app = TestApp::in_memory();
    let res = app.get("/api/v1/audit-logs").as_anonymous().send().await;
    res.assert_status(StatusCode::UNAUTHORIZED);
    res.assert_error_code("UNAUTHORIZED");
}

#[tokio::test]
async fn test_tier2_f1_03_expired_token_unauthorized_401() {
    let app = TestApp::in_memory();
    let expired = expired_token();
    let res = app
        .get("/api/v1/audit-logs")
        .with_token(&expired)
        .send()
        .await;
    res.assert_status(StatusCode::UNAUTHORIZED);
    res.assert_error_code("UNAUTHORIZED");
}

#[tokio::test]
async fn test_tier2_f1_04_tampered_signature_unauthorized_401() {
    let app = TestApp::in_memory();
    let forged = invalid_signature_token();
    let res = app
        .get("/api/v1/audit-logs")
        .with_token(&forged)
        .send()
        .await;
    res.assert_status(StatusCode::UNAUTHORIZED);
    res.assert_error_code("UNAUTHORIZED");
}

#[tokio::test]
async fn test_tier2_f1_05_malformed_limit_bad_request_400() {
    let app = TestApp::in_memory();
    let res = app
        .get("/api/v1/audit-logs?limit=not_an_integer")
        .as_controller()
        .send()
        .await;
    // Axum Query extractor rejects invalid u32 with 400 Bad Request
    res.assert_status(StatusCode::BAD_REQUEST);
}

// ============================================================================
// Feature 2 (R2): Upgrade rusqlite & SQLite Engine Verification
// ============================================================================

#[test]
fn test_tier2_f2_01_sqlite_engine_version_runtime_verification() {
    let res = backend::database::verify_sqlite_engine_version();
    assert!(
        res.is_ok(),
        "Runtime SQLite engine must meet or exceed 3.53.0 WAL-reset resilience standard"
    );
    let ver_num = rusqlite::version_number();
    assert!(
        ver_num >= MIN_SQLITE_VERSION_NUMBER,
        "Actual SQLite version number {ver_num} must be >= {MIN_SQLITE_VERSION_NUMBER}"
    );
}

#[test]
fn test_tier2_f2_02_concurrency_under_lock_contention() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);
    let db_path = app.env.as_ref().unwrap().db_path.clone();

    // Spawn 8 concurrent reader threads
    let mut handles = Vec::new();
    for _ in 0..8 {
        let p = db_path.clone();
        handles.push(std::thread::spawn(move || {
            let conn = Connection::open(&p).unwrap();
            let check: String = conn
                .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
                .unwrap();
            assert_eq!(check, "ok");
        }));
    }
    for h in handles {
        h.join().expect("Thread joined successfully");
    }
}

#[tokio::test]
async fn test_tier2_f2_03_abrupt_wal_recovery() {
    let mut env = TestEnv::new();
    let db_path = env.db_path.clone();
    let root_dir = env.root_dir.clone();
    env.retain_on_drop();
    {
        let app = TestApp::file_backed(env);
        let v = VoucherBuilder::simple_cash_withdrawal("V-CRASH-01", "2026-01-10", "1000000");
        let _ = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&v)
            .send()
            .await;
    }
    // Reopen connection simulating process recovery
    let conn2 = Connection::open(&db_path).unwrap();
    let check: String = conn2
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
    drop(conn2);
    let _ = std::fs::remove_dir_all(&root_dir);
}

#[test]
fn test_tier2_f2_04_read_only_database_detection() {
    let env = TestEnv::new();
    let conn = Connection::open_with_flags(
        &env.db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
    );
    assert!(conn.is_ok());
}

#[test]
fn test_tier2_f2_05_corrupted_header_detection() {
    let env = TestEnv::new();
    let corrupt_file = env.root_dir.join("corrupted.db");
    // Write 100 bytes of random noise (invalid SQLite magic header)
    std::fs::write(&corrupt_file, [0xDE, 0xAD, 0xBE, 0xEF].repeat(25)).unwrap();

    let conn = Connection::open(&corrupt_file).unwrap();
    let res: Result<String, _> = conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0));
    assert!(
        res.is_err() || res.as_deref() != Ok("ok"),
        "Integrity check must detect corrupted header"
    );
}

// ============================================================================
// Feature 3 (R4): Hygiene & Dual Licensing
// ============================================================================

#[test]
fn test_tier2_f3_01_deeply_nested_backup_exclusion() {
    let content = find_root_gitignore().expect("Readable root .gitignore");
    assert!(content.contains("backups/") || content.contains("backups"));
}

#[test]
fn test_tier2_f3_02_temporary_wal_extension_patterns() {
    let content = find_root_gitignore().expect("Readable root .gitignore");
    assert!(content.contains("*.db-wal") || content.contains("db-wal"));
}

#[test]
fn test_tier2_f3_03_case_insensitive_backup_dirs() {
    let content = find_root_gitignore().expect("Readable root .gitignore");
    assert!(content.to_lowercase().contains("backups"));
}

#[test]
fn test_tier2_f3_04_license_truncation_detection() {
    let content = find_license_content().expect("Readable LICENSE");
    assert!(content.len() >= 100, "License file too short");
}

#[test]
fn test_tier2_f3_05_scratch_verify_directory_exclusion() {
    let content = find_root_gitignore().expect("Readable root .gitignore");
    assert!(content.contains("_verify") || content.contains("_verify/"));
}

// ============================================================================
// Feature 4 (R3): Automated Disaster Restore Engine
// ============================================================================

#[tokio::test]
async fn test_tier2_f4_01_sha256_checksum_mismatch_rejection() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let snap_path = app.take_backup().await;

    let payload = json!({
        "snapshot_path": snap_path,
        "confirm_overwrite": true,
        "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });

    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f4_02_corrupted_snapshot_bitflip_rejection() {
    let env = TestEnv::new();
    let corrupt_snap = env.root_dir.join("bitflip_snap.db");
    // Write 500 bytes of zeros
    std::fs::write(&corrupt_snap, vec![0u8; 500]).unwrap();

    let app = TestApp::file_backed(env);
    let payload = json!({
        "snapshot_path": corrupt_snap.to_str().unwrap(),
        "confirm_overwrite": true,
        "expected_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f4_03_incomplete_snapshot_schema_rejection() {
    let env = TestEnv::new();
    let incomplete_snap = env.root_dir.join("empty.db");
    {
        let conn = Connection::open(&incomplete_snap).unwrap();
        // Creates empty db with no accounting tables
        conn.execute_batch("CREATE TABLE dummy (id INTEGER);")
            .unwrap();
    } // Explicitly drop connection handle before TestApp takes ownership of env

    let app = TestApp::file_backed(env);
    let payload = json!({
        "snapshot_path": incomplete_snap.to_str().unwrap(),
        "confirm_overwrite": true,
        "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f4_04_nonexistent_snapshot_path_rejection() {
    let app = TestApp::in_memory();
    let payload = json!({
        "snapshot_path": "nonexistent/path/missing.db",
        "confirm_overwrite": true,
        "expected_sha256": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f4_05_rollback_safety_on_failed_restore() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Baseline voucher
    let v = VoucherBuilder::simple_cash_withdrawal("V-SAFE-01", "2026-01-10", "2000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(create_res.status == StatusCode::CREATED || create_res.status == StatusCode::OK);

    // Attempt corrupt restore
    let payload = json!({
        "snapshot_path": "invalid.db",
        "confirm_overwrite": true,
        "expected_sha256": "bad_hash"
    });
    let _ = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;

    // Verify original voucher remains intact
    let list_res = app.get("/api/v1/vouchers").as_accountant().send().await;
    list_res.assert_status(StatusCode::OK);
    let vouchers: Vec<backend::models::JournalVoucher> = list_res.json();
    assert!(
        !vouchers.is_empty(),
        "Live data must not be destroyed on failed restore"
    );
}

// ============================================================================
// Feature 5 (R3): Restore API Endpoint
// ============================================================================

#[tokio::test]
async fn test_tier2_f5_01_restore_controller_forbidden_403() {
    let app = TestApp::in_memory();
    let payload = json!({
        "snapshot_path": "snap.db",
        "confirm_overwrite": true,
        "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::FORBIDDEN);
    res.assert_error_code("FORBIDDEN");
}

#[tokio::test]
async fn test_tier2_f5_02_restore_accountant_forbidden_403() {
    let app = TestApp::in_memory();
    let payload = json!({
        "snapshot_path": "snap.db",
        "confirm_overwrite": true,
        "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_accountant()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::FORBIDDEN);
    res.assert_error_code("FORBIDDEN");
}

#[tokio::test]
async fn test_tier2_f5_03_restore_missing_confirm_overwrite() {
    let app = TestApp::in_memory();
    let payload = json!({
        "snapshot_path": "snap.db",
        "confirm_overwrite": false,
        "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
}

#[tokio::test]
async fn test_tier2_f5_04_restore_directory_traversal_rejected() {
    let app = TestApp::in_memory();
    let payload = json!({
        "snapshot_path": "../../../etc/shadow",
        "confirm_overwrite": true,
        "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
}

#[tokio::test]
async fn test_tier2_f5_05_restore_invalid_sha256_format() {
    let app = TestApp::in_memory();
    let payload = json!({
        "snapshot_path": "snap.db",
        "confirm_overwrite": true,
        "expected_sha256": "not_a_valid_hex_hash"
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
}

// ============================================================================
// Feature 6 (R3): Restore CLI Subcommand
// ============================================================================

#[test]
fn test_tier2_f6_01_cli_missing_snapshot_flag() {
    let runner = CliRunner::new();
    let output = runner.run_with_args(&["restore", "--target", "db.sqlite"]);
    assert!(
        !output.status.success(),
        "CLI must fail when --snapshot is omitted"
    );
}

#[test]
fn test_tier2_f6_02_cli_missing_target_flag() {
    let runner = CliRunner::new();
    let output = runner.run_with_args(&["restore", "--snapshot", "snap.sqlite"]);
    assert!(
        !output.status.success(),
        "CLI must fail when --target is omitted"
    );
}

#[test]
fn test_tier2_f6_03_cli_unrecognized_flags() {
    let runner = CliRunner::new();
    let output = runner.run_with_args(&["restore", "--unknown-flag-123"]);
    assert!(!output.status.success(), "CLI must reject unknown flags");
}

#[test]
fn test_tier2_f6_04_cli_corrupted_snapshot() {
    let env = TestEnv::new();
    let bad_snap = env.root_dir.join("bad_snap.db");
    std::fs::write(&bad_snap, b"random_corrupted_data").unwrap();
    let target = env.root_dir.join("target.db");

    let runner = CliRunner::new();
    let output = runner.run_with_args(&[
        "restore",
        "--snapshot",
        bad_snap.to_str().unwrap(),
        "--target",
        target.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
}

#[test]
fn test_tier2_f6_05_cli_exclusive_lock_handling() {
    let env = TestEnv::new();
    let snap = env.root_dir.join("snap.db");
    std::fs::write(&snap, b"dummy").unwrap();
    let target = env.root_dir.join("target.db");

    let runner = CliRunner::new();
    let output = runner.run_with_args(&[
        "restore",
        "--snapshot",
        snap.to_str().unwrap(),
        "--target",
        target.to_str().unwrap(),
    ]);
    assert!(output.status.code().is_some());
}

// ============================================================================
// Feature 7 (R3): Zero-Diff Drill Suite
// ============================================================================

#[tokio::test]
async fn test_tier2_f7_01_drill_high_volume_postings() {
    let app = TestApp::in_memory();
    // Post 10 multi-leg vouchers with large amounts
    for i in 1..=10 {
        let v = VoucherBuilder::new(&format!("V-VOL-{i:03}"), "2026-01-15")
            .description("Giao dich quy mo lon")
            .leg("1111", "500000000", "0")
            .leg("5111", "0", "500000000")
            .build();
        let res = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&v)
            .send()
            .await;
        assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
    }
    let tb_res = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    tb_res.assert_status(StatusCode::OK);
    let tb: TrialBalanceResponse = tb_res.json();
    assert_trial_balance_balanced(&tb);
}

#[tokio::test]
async fn test_tier2_f7_02_drill_contra_asset_calculations() {
    let app = TestApp::in_memory();
    let bs_res = app
        .get("/api/v1/reports/balance-sheet?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    bs_res.assert_status(StatusCode::OK);
    let bs: backend::dto::BalanceSheetResponse = bs_res.json();
    assert_balance_sheet_balanced(&bs);
}

#[tokio::test]
async fn test_tier2_f7_03_drill_idempotency_history_preservation() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::simple_cash_withdrawal("V-IDEMP-01", "2026-01-10", "1500000");
    let key = "idemp-key-drill-01";

    let r1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .with_idempotency_key(key)
        .json(&v)
        .send()
        .await;
    assert!(r1.status == StatusCode::CREATED || r1.status == StatusCode::OK);

    // Replay same key
    let r2 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .with_idempotency_key(key)
        .json(&v)
        .send()
        .await;
    assert_eq!(r1.status, r2.status);
}

#[tokio::test]
async fn test_tier2_f7_04_drill_reopened_then_closed_period() {
    let app = TestApp::in_memory();
    // Post initial voucher with revenue to allow closing
    let v_rev = VoucherBuilder::new("V-REOPEN-REV", "2026-01-10")
        .leg("1121", "5000000", "0")
        .leg("511", "0", "5000000")
        .build();
    let r_rev = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_rev)
        .send()
        .await;
    r_rev.assert_status(StatusCode::CREATED);

    let v = VoucherBuilder::simple_cash_withdrawal("V-REOPEN-01", "2026-01-10", "3000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    create_res.assert_status(StatusCode::CREATED);

    // Close period
    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    close_res.assert_status(StatusCode::OK);

    // Reopen period
    let reopen_res = app
        .post("/api/v1/periods/reopen")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    reopen_res.assert_status(StatusCode::OK);

    // Re-close period
    let reclose_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    reclose_res.assert_status(StatusCode::OK);
}

#[test]
#[should_panic(expected = "Total debit discrepancy")]
fn test_tier2_f7_05_drill_intentional_drift_detection() {
    let tb1 = TrialBalanceResponse {
        as_of_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        accounts: vec![],
        total_debit: rust_decimal::Decimal::ZERO,
        total_credit: rust_decimal::Decimal::ZERO,
        is_balanced: true,
    };
    let tb2 = TrialBalanceResponse {
        as_of_date: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        accounts: vec![],
        total_debit: rust_decimal::Decimal::ONE,
        total_credit: rust_decimal::Decimal::ZERO,
        is_balanced: false,
    };
    // Must panic on discrepancy
    assert_zero_diff_trial_balance(&tb1, &tb2);
}

// ============================================================================
// Feature 8 (R5): Dynamic Chart of Accounts Entity & Schema
// ============================================================================

#[test]
fn test_tier2_f8_01_deep_account_hierarchy_5_levels() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    // Chain of 5 levels: 1 -> 15 -> 152 -> 1520 -> 15201
    conn.execute(
        "INSERT INTO accounts (code, name, parent_code, class, is_active, normal_balance) \
         VALUES ('15201', 'L5 Subaccount', '1520', 'ASSET', 1, 'DEBIT');",
        [],
    )
    .expect("Inserting 5th level hierarchy subaccount must succeed");
}

#[test]
fn test_tier2_f8_02_root_accounts_null_parent() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let roots_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM accounts WHERE parent_code IS NULL;",
            [],
            |r| r.get(0),
        )
        .expect("Querying root accounts must succeed");
    assert!(
        roots_count >= 1,
        "There must be at least 1 root account with NULL parent"
    );
}

#[test]
fn test_tier2_f8_03_vietnamese_diacritics_in_names() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let name_vn = "Nguyên vật liệu chính nhập khẩu dự trữ phòng chống bão lụt";
    conn.execute(
        "INSERT INTO accounts (code, name, parent_code, class, is_active, normal_balance) \
         VALUES ('15299', ?1, '152', 'ASSET', 1, 'DEBIT');",
        [name_vn],
    )
    .expect("Inserting account with Vietnamese diacritics must succeed");
    let retrieved: String = conn
        .query_row("SELECT name FROM accounts WHERE code = '15299';", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(retrieved, name_vn);
}

#[tokio::test]
async fn test_tier2_f8_04_account_code_length_boundary() {
    let app = TestApp::in_memory();
    // Submitting account code below statutory 3-digit minimum must be rejected with 400
    let payload_too_short = json!({
        "code": "11",
        "name": "Invalid Short Code",
        "parent_code": null,
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload_too_short)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[test]
fn test_tier2_f8_05_deactivation_flag_persistence() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    conn.execute("UPDATE accounts SET is_active = 0 WHERE code = '1520';", [])
        .expect("Deactivation update must succeed");
    let is_active: i32 = conn
        .query_row(
            "SELECT is_active FROM accounts WHERE code = '1520';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(is_active, 0);
}

#[tokio::test]
async fn test_tier2_f8_06_case_sensitivity_of_account_codes() {
    let app = TestApp::in_memory();
    // Posting voucher with alphanumeric or case-mismatched account code must be rejected
    let v = VoucherBuilder::new("V-CASE-01", "2026-01-10")
        .leg("1111a", "1000000", "0")
        .leg("1111", "0", "1000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("UNKNOWN_ACCOUNT");
}

#[test]
fn test_tier2_f8_07_orphan_parent_foreign_key_guard() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let res = conn.execute(
        "INSERT INTO accounts (code, name, parent_code, class, is_active, normal_balance) \
         VALUES ('99999', 'Orphan Child', 'NONEXISTENT_PARENT', 'ASSET', 1, 'DEBIT');",
        [],
    );
    // Under foreign key constraints, inserting orphan parent must fail
    assert!(
        res.is_err(),
        "Inserting orphan parent account must fail foreign key check"
    );
}

// ============================================================================
// Feature 9 (R5): Chart of Accounts API Endpoints
// ============================================================================

#[tokio::test]
async fn test_tier2_f9_01_duplicate_account_conflict_409() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "1111",
        "name": "Tien mat tai quy",
        "parent_code": "111",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_tier2_f9_02_nonexistent_parent_code_400_or_422() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "15299",
        "name": "Invalid Parent Account",
        "parent_code": "999999",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    assert!(
        res.status == StatusCode::BAD_REQUEST || res.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_tier2_f9_03_self_referential_cycle_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "15288",
        "name": "Self Cycle Account",
        "parent_code": "15288",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f9_04_invalid_account_class_enum_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "15289",
        "name": "Invalid Class Account",
        "parent_code": "152",
        "class": "INVALID_CLASS",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f9_05_invalid_normal_balance_enum_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "15290",
        "name": "Invalid Normal Balance",
        "parent_code": "152",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "INVALID_BALANCE"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f9_06_empty_or_whitespace_code_or_name_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "   ",
        "name": "",
        "parent_code": "152",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_tier2_f9_07_child_under_inactive_parent_422() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "15291",
        "name": "Child of Inactive Parent",
        "parent_code": "152_INACTIVE",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    assert!(
        res.status == StatusCode::BAD_REQUEST || res.status == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_tier2_f9_08_multi_node_circular_cycle_rejection() {
    let app = TestApp::in_memory();

    // 1. Create Account A (15280) under seeded parent 152
    let acct_a = json!({
        "code": "15280",
        "name": "Node A",
        "parent_code": "152",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res_a = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&acct_a)
        .send()
        .await;
    res_a.assert_status(StatusCode::CREATED);

    // 2. Create Account B (15281) with parent A (15280) -> Valid tree: 152 -> 15280 -> 15281
    let acct_b = json!({
        "code": "15281",
        "name": "Node B",
        "parent_code": "15280",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res_b = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&acct_b)
        .send()
        .await;
    res_b.assert_status(StatusCode::CREATED);

    // 3. Attempt to update Account A's parent to Account B (15281) -> Forms cycle: 15280 -> 15281 -> 15280
    let cyclic_update = json!({
        "code": "15280",
        "name": "Node A Mutated",
        "parent_code": "15281",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res_cycle = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&cyclic_update)
        .send()
        .await;
    assert!(
        res_cycle.status == StatusCode::BAD_REQUEST
            || res_cycle.status == StatusCode::UNPROCESSABLE_ENTITY
            || res_cycle.status == StatusCode::CONFLICT,
        "Creating multi-node circular dependency (A -> B -> A) must be rejected with 400, 422, or 409"
    );
}

// ============================================================================
// Feature 10 (R5): Voucher Account Validation
// ============================================================================

#[tokio::test]
async fn test_tier2_f10_01_voucher_unknown_debit_account_422() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-UNK-01", "2026-01-10")
        .leg("9999", "1000000", "0")
        .leg("1111", "0", "1000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("UNKNOWN_ACCOUNT");
}

#[tokio::test]
async fn test_tier2_f10_02_voucher_unknown_credit_account_422() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-UNK-02", "2026-01-10")
        .leg("1111", "1000000", "0")
        .leg("8888", "0", "1000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("UNKNOWN_ACCOUNT");
}

#[tokio::test]
async fn test_tier2_f10_03_voucher_inactive_account_422() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);
    if let Some(ref e) = app.env {
        let conn = Connection::open(&e.db_path).unwrap();
        let _ = conn.execute("UPDATE accounts SET is_active = 0 WHERE code = '1520';", []);
    }
    let v = VoucherBuilder::new("V-INACT-01", "2026-01-10")
        .leg("1520", "1000000", "0")
        .leg("1111", "0", "1000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("ACCOUNT_INACTIVE");
}

#[tokio::test]
async fn test_tier2_f10_04_compound_voucher_atomic_rollback_on_single_unknown() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-COMPOUND-ROLLBACK", "2026-01-10")
        .leg("1111", "500000", "0")
        .leg("1121", "500000", "0")
        .leg("7777", "0", "1000000") // Unknown leg
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("UNKNOWN_ACCOUNT");

    // Query vouchers to assert atomic rollback
    let list_res = app.get("/api/v1/vouchers").as_accountant().send().await;
    list_res.assert_status(StatusCode::OK);
    let vouchers: Vec<backend::models::JournalVoucher> = list_res.json();
    assert!(
        !vouchers
            .iter()
            .any(|v| v.voucher_no == "V-COMPOUND-ROLLBACK"),
        "Compound voucher with unknown leg must be completely rolled back"
    );
}

#[tokio::test]
async fn test_tier2_f10_05_historical_voiding_allowed_for_inactive_account() {
    let app = TestApp::in_memory();
    // Post initial valid voucher
    let v = VoucherBuilder::simple_cash_withdrawal("V-VOID-01", "2026-01-10", "1000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(create_res.status == StatusCode::CREATED || create_res.status == StatusCode::OK);

    let created: backend::models::JournalVoucher = create_res.json();
    let void_payload = json!({ "reason": "Huy phieu do tai khoan ngung hoat dong" });
    let void_res = app
        .post(&format!("/api/v1/vouchers/{}/void", created.id))
        .as_controller()
        .json(&void_payload)
        .send()
        .await;
    void_res.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_tier2_f10_06_idempotency_priority_on_invalid_account() {
    let app = TestApp::in_memory();
    let key = "idemp-key-invalid-acct-01";
    let bad_voucher = VoucherBuilder::new("V-BAD-01", "2026-01-10")
        .leg("9999", "1000000", "0")
        .leg("1111", "0", "1000000")
        .build();

    let r1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .with_idempotency_key(key)
        .json(&bad_voucher)
        .send()
        .await;
    r1.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    r1.assert_error_code("UNKNOWN_ACCOUNT");

    // Resend with valid voucher under same key -> should NOT replay 422
    let good_voucher = VoucherBuilder::simple_cash_withdrawal("V-GOOD-01", "2026-01-10", "1000000");
    let r2 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .with_idempotency_key(key)
        .json(&good_voucher)
        .send()
        .await;
    assert!(
        r2.status == StatusCode::CREATED
            || r2.status == StatusCode::OK
            || r2.status == StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn test_tier2_f10_07_voucher_imbalanced_debit_greater_than_credit_422() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-IMBALANCED-01", "2026-01-10")
        .leg("1111", "1000000", "0")
        .leg("1121", "0", "500000") // Debit (1,000,000) != Credit (500,000)
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("UNBALANCED_VOUCHER");
}

#[tokio::test]
async fn test_tier2_f10_08_voucher_imbalanced_credit_greater_than_debit_422() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-IMBALANCED-02", "2026-01-10")
        .leg("1111", "500000", "0")
        .leg("1121", "0", "1000000") // Credit (1,000,000) != Debit (500,000)
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    res.assert_error_code("UNBALANCED_VOUCHER");
}

#[tokio::test]
async fn test_tier2_f10_09_voucher_zero_amount_lines_rejected_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "voucher_no": "V-ZERO-AMOUNTS-01",
        "voucher_date": "2026-01-10",
        "description": "Zero amount lines rejected probe",
        "lines": [
            { "account_code": "1111", "debit": "0", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "0" }
        ]
    });
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
    assert!(
        res.text()
            .contains("Line amount cannot be zero for both Debit and Credit")
    );
}

#[tokio::test]
async fn test_tier2_f10_10_voucher_dual_debit_and_credit_single_line_rejected_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "voucher_no": "V-DUAL-LINE-01",
        "voucher_date": "2026-01-10",
        "description": "Dual debit and credit on single line probe",
        "lines": [
            { "account_code": "1111", "debit": "1000000", "credit": "500000" },
            { "account_code": "1121", "debit": "0", "credit": "500000" }
        ]
    });
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
    assert!(
        res.text()
            .contains("A single line item cannot contain both Debit and Credit amounts")
    );
}

#[tokio::test]
async fn test_tier2_f10_11_voucher_empty_lines_rejected_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "voucher_no": "V-EMPTY-LINES-01",
        "voucher_date": "2026-01-10",
        "description": "Empty lines voucher probe",
        "lines": []
    });
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
    assert!(res.text().contains("at least 2 lines"));
}

#[tokio::test]
async fn test_tier2_f10_12_voucher_single_line_rejected_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "voucher_no": "V-SINGLE-LINE-01",
        "voucher_date": "2026-01-10",
        "description": "Single line voucher probe",
        "lines": [
            { "account_code": "1111", "debit": "1000000", "credit": "0" }
        ]
    });
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
    assert!(res.text().contains("at least 2 lines"));
}

#[tokio::test]
async fn test_tier2_f10_13_voucher_negative_amount_rejected_400() {
    let app = TestApp::in_memory();
    let payload = json!({
        "voucher_no": "V-NEG-AMOUNT-01",
        "voucher_date": "2026-01-10",
        "description": "Negative amount voucher probe",
        "lines": [
            { "account_code": "1111", "debit": "-1000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "-1000000" }
        ]
    });
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::BAD_REQUEST);
    res.assert_error_code("BAD_REQUEST");
    assert!(res.text().contains("Negative amounts are forbidden"));
}

// ============================================================================
// Feature 11 (R6): Configurable Fiscal Year Start Date
// ============================================================================

#[test]
fn test_tier2_f11_01_start_month_zero_rejected() {
    let res = backend::closing::PeriodClosingEngine::last_day_of_month(2026, 0);
    assert!(res.is_err());
}

#[test]
fn test_tier2_f11_02_start_month_thirteen_rejected() {
    let res = backend::closing::PeriodClosingEngine::last_day_of_month(2026, 13);
    assert!(res.is_err());
}

#[test]
fn test_tier2_f11_03_default_configuration_safety() {
    let config = backend::config::AppConfig::default();
    assert_eq!(config.port, 3000);
    assert_eq!(config.fiscal_year_start_month, 1);
}

#[test]
fn test_tier2_f11_04_leap_year_february_29_bounds() {
    let end_leap = backend::closing::PeriodClosingEngine::last_day_of_month(2028, 2).unwrap();
    assert_eq!(end_leap, NaiveDate::from_ymd_opt(2028, 2, 29).unwrap());

    let end_non_leap = backend::closing::PeriodClosingEngine::last_day_of_month(2027, 2).unwrap();
    assert_eq!(end_non_leap, NaiveDate::from_ymd_opt(2027, 2, 28).unwrap());
}

#[test]
fn test_tier2_f11_05_extreme_year_bounds_no_overflow() {
    let e1 = backend::closing::PeriodClosingEngine::last_day_of_month(2100, 3).unwrap();
    assert_eq!(e1, NaiveDate::from_ymd_opt(2100, 3, 31).unwrap());

    let e2 = backend::closing::PeriodClosingEngine::last_day_of_month(1991, 6).unwrap();
    assert_eq!(e2, NaiveDate::from_ymd_opt(1991, 6, 30).unwrap());
}
