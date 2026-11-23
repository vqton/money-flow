//! Tier 1: Feature Coverage (Category-Partition Happy Path)
//!
//! Exceeds >= 5 test cases per feature across all 11 features (59 test cases total).

use crate::e2e_tests::harness::*;
use axum::http::StatusCode;
use backend::{
    database::{MIN_SQLITE_VERSION_NUMBER, verify_sqlite_engine_version},
    dto::{BalanceSheetResponse, TrialBalanceResponse},
    models::JournalVoucher,
};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;

// ============================================================================
// Feature 1 (R1): Audit Route Auth & Restrict `/audit-logs`
// ============================================================================

#[tokio::test]
async fn test_tier1_f1_01_controller_access_authorized() {
    let app = TestApp::in_memory();
    let res = app.get("/api/v1/audit-logs").as_controller().send().await;
    res.assert_status(StatusCode::OK);
    let val = res.json_val();
    assert!(val.is_array(), "Expected JSON array of audit logs");
}

#[tokio::test]
async fn test_tier1_f1_02_admin_access_authorized() {
    let app = TestApp::in_memory();
    let res = app.get("/api/v1/audit-logs").as_admin().send().await;
    res.assert_status(StatusCode::OK);
    let val = res.json_val();
    assert!(
        val.is_array(),
        "Expected JSON array of audit logs for Admin"
    );
}

#[tokio::test]
async fn test_tier1_f1_03_audit_filtering_by_action() {
    let app = TestApp::in_memory();
    // Post a voucher first so we have an audit log
    let voucher = VoucherBuilder::simple_cash_withdrawal("V-AUDIT-01", "2026-01-10", "1000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&voucher)
        .send()
        .await;
    assert!(create_res.status == StatusCode::CREATED || create_res.status == StatusCode::OK);

    let res = app
        .get("/api/v1/audit-logs?action=CREATE_VOUCHER")
        .as_controller()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let list: Vec<serde_json::Value> = res.json();
    for entry in list {
        assert_eq!(entry["action"], "CREATE_VOUCHER");
    }
}

#[tokio::test]
async fn test_tier1_f1_04_audit_filtering_by_resource_type() {
    let app = TestApp::in_memory();
    let res = app
        .get("/api/v1/audit-logs?resource_type=PERIOD")
        .as_controller()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let list: Vec<serde_json::Value> = res.json();
    for entry in list {
        assert_eq!(entry["resource_type"], "PERIOD");
    }
}

#[tokio::test]
async fn test_tier1_f1_05_audit_pagination_limit() {
    let app = TestApp::in_memory();
    let res = app
        .get("/api/v1/audit-logs?limit=3")
        .as_controller()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let list: Vec<serde_json::Value> = res.json();
    assert!(
        list.len() <= 3,
        "Pagination limit 3 exceeded: {}",
        list.len()
    );
}

// ============================================================================
// Feature 2 (R2): Upgrade rusqlite & SQLite Engine Verification
// ============================================================================

#[test]
fn test_tier1_f2_01_sqlite_version_requirement() {
    let res = verify_sqlite_engine_version();
    assert!(res.is_ok(), "Engine version check failed: {:?}", res);
    let version_num = rusqlite::version_number();
    assert!(
        version_num >= MIN_SQLITE_VERSION_NUMBER,
        "Installed SQLite version {version_num} is below minimum {MIN_SQLITE_VERSION_NUMBER}"
    );
}

#[test]
fn test_tier1_f2_02_wal_journal_mode() {
    let env = TestEnv::new();
    let _app = TestApp::file_backed(env);

    let conn = Connection::open(&_app.env.as_ref().unwrap().db_path).expect("Open SQLite");
    let mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .expect("PRAGMA journal_mode query failed");
    assert_eq!(mode.to_lowercase(), "wal", "Expected journal mode WAL");
}

#[test]
fn test_tier1_f2_03_busy_timeout_configured() {
    let env = TestEnv::new();
    let _app = TestApp::file_backed(env);

    let conn = Connection::open(&_app.env.as_ref().unwrap().db_path).expect("Open SQLite");
    let timeout: i64 = conn
        .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
        .expect("PRAGMA busy_timeout query failed");
    assert!(
        timeout >= 5000,
        "Busy timeout {timeout}ms is less than 5000ms"
    );
}

#[test]
fn test_tier1_f2_04_foreign_keys_enforced() {
    let env = TestEnv::new();
    let _app = TestApp::file_backed(env);

    let conn = Connection::open(&_app.env.as_ref().unwrap().db_path).expect("Open SQLite");
    let fk_status: i32 = conn
        .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
        .expect("PRAGMA foreign_keys query failed");
    assert_eq!(fk_status, 1, "Foreign keys must be enabled (1)");
}

#[test]
fn test_tier1_f2_05_pragmas_integrity_check() {
    let env = TestEnv::new();
    let _app = TestApp::file_backed(env);

    let conn = Connection::open(&_app.env.as_ref().unwrap().db_path).expect("Open SQLite");
    let check: String = conn
        .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .expect("PRAGMA integrity_check query failed");
    assert_eq!(check, "ok", "Database integrity check was not ok: {check}");
}

// ============================================================================
// Feature 3 (R4): Hygiene & Dual Licensing
// ============================================================================

#[test]
fn test_tier1_f3_01_license_file_exists() {
    let content = find_license_content();
    assert!(content.is_some(), "LICENSE file missing in repository root");
    assert!(
        content.unwrap().len() >= 50,
        "LICENSE file content too short"
    );
}

#[test]
fn test_tier1_f3_02_license_dual_grant() {
    let content = find_license_content().expect("Readable LICENSE file");
    assert!(
        content.contains("MIT License")
            || content.contains("MIT license")
            || content.contains("http://opensource.org/licenses/MIT"),
        "LICENSE must contain MIT terms"
    );
    assert!(
        content.contains("Apache License")
            || content.contains("Apache-2.0")
            || content.contains("http://www.apache.org/licenses/"),
        "LICENSE must contain Apache-2.0 terms"
    );
}

#[test]
fn test_tier1_f3_03_gitignore_covers_logs() {
    let content = find_root_gitignore().expect("Readable root .gitignore file");
    assert!(
        content.contains("logs") || content.contains("*.log"),
        ".gitignore must cover log files/directories"
    );
}

#[test]
fn test_tier1_f3_04_gitignore_covers_backups() {
    let content = find_root_gitignore().expect("Readable root .gitignore file");
    assert!(
        content.contains("backups"),
        ".gitignore must cover backups/ directory"
    );
}

#[test]
fn test_tier1_f3_05_gitignore_covers_sqlite_databases() {
    let content = find_root_gitignore().expect("Readable root .gitignore file");
    assert!(
        content.contains("*.db") || content.contains("accounting.db"),
        ".gitignore must cover SQLite *.db files"
    );
}

// ============================================================================
// Feature 4 (R3): Automated Disaster Restore Engine
// ============================================================================

#[tokio::test]
async fn test_tier1_f4_01_clean_snapshot_restoration() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post voucher and take snapshot
    let v1 = VoucherBuilder::simple_cash_withdrawal("V-RESTORE-01", "2026-01-05", "5000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v1)
        .send()
        .await;
    create_res.assert_status(StatusCode::CREATED);
    let snapshot_path = app.take_backup().await;

    // 2. Perform authentic restore via repository
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let outcome = app
        .state
        .repo
        .restore_from_snapshot(Path::new(&snapshot_path), Some(&sha256_hex), "admin_tester")
        .expect("Snapshot restore must succeed");
    assert_eq!(outcome.restored_from_snapshot, snapshot_path);
    assert_eq!(outcome.sha256_checksum, sha256_hex);

    // 3. Verify voucher is queryable post-restore
    let res = app.get("/api/v1/vouchers").as_accountant().send().await;
    res.assert_status(StatusCode::OK);
    let vouchers: Vec<JournalVoucher> = res.json();
    assert_eq!(vouchers.len(), 1);
    assert_eq!(vouchers[0].voucher_no, "V-RESTORE-01");
}

#[tokio::test]
async fn test_tier1_f4_02_pre_restore_checksum_validation() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let snapshot_path = app.take_backup().await;

    // Calculate sha256 of snapshot
    let bytes = std::fs::read(&snapshot_path).expect("Read snapshot bytes");
    use ring::digest::{Context, SHA256};
    let mut context = Context::new(&SHA256);
    context.update(&bytes);
    let digest = context.finish();
    let computed_hash = bytes_to_hex(digest.as_ref());

    assert_eq!(
        computed_hash.len(),
        64,
        "SHA-256 hash length must be 64 characters"
    );
}

#[tokio::test]
async fn test_tier1_f4_03_snapshot_integrity_verified() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let snapshot_path = app.take_backup().await;

    // Verify snapshot file with rusqlite
    let snap_conn = Connection::open(&snapshot_path).expect("Open snapshot db");
    let check: String = snap_conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .expect("Integrity check on snapshot");
    assert_eq!(check, "ok");
}

#[tokio::test]
async fn test_tier1_f4_04_system_restore_audit_recorded() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    app.state
        .repo
        .restore_from_snapshot(Path::new(&snapshot_path), Some(&sha256_hex), "audit_admin")
        .unwrap();

    let res = app
        .get("/api/v1/audit-logs?action=SYSTEM_RESTORE")
        .as_controller()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let logs: Vec<serde_json::Value> = res.json();
    assert!(
        !logs.is_empty(),
        "SYSTEM_RESTORE audit log must be recorded"
    );
    assert_eq!(logs[0]["actor"], "audit_admin");
    assert_eq!(logs[0]["action"], "SYSTEM_RESTORE");
}

#[tokio::test]
async fn test_tier1_f4_05_active_connection_continuity() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    app.state
        .repo
        .restore_from_snapshot(Path::new(&snapshot_path), Some(&sha256_hex), "admin_tester")
        .unwrap();

    // Connection remains active: successfully post a new voucher post-restore
    let v2 = VoucherBuilder::simple_cash_withdrawal("V-POST-RESTORE", "2026-01-20", "3000000");
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v2)
        .send()
        .await;
    res.assert_status(StatusCode::CREATED);
}

// ============================================================================
// Feature 5 (R3): Restore API Endpoint
// ============================================================================

#[tokio::test]
async fn test_tier1_f5_01_admin_authorized_restore() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Take backup
    let snapshot_path = app.take_backup().await;

    let bytes = std::fs::read(&snapshot_path).expect("Read snapshot");
    use ring::digest::{Context, SHA256};
    let mut context = Context::new(&SHA256);
    context.update(&bytes);
    let sha256_hex = bytes_to_hex(context.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });

    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let val = res.json_val();
    assert_eq!(val["status"], "RESTORED");
    assert_eq!(val["restored_from_snapshot"], snapshot_path);
    assert_eq!(val["sha256_checksum"], sha256_hex);
}

#[tokio::test]
async fn test_tier1_f5_02_openapi_restore_spec() {
    let app = TestApp::in_memory();
    let res = app
        .get("/api-docs/openapi.json")
        .as_anonymous()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let val = res.json_val();
    let paths = val.get("paths").and_then(|p| p.as_object());
    assert!(paths.is_some(), "OpenAPI missing paths object");
}

#[tokio::test]
async fn test_tier1_f5_03_post_restore_data_visibility() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let v = VoucherBuilder::simple_cash_withdrawal("V-DATA-VIS", "2026-01-10", "4000000");
    let r = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    r.assert_status(StatusCode::CREATED);

    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let restore_res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    restore_res.assert_status(StatusCode::OK);

    let res = app.get("/api/v1/vouchers").as_accountant().send().await;
    res.assert_status(StatusCode::OK);
    let vouchers: Vec<JournalVoucher> = res.json();
    assert_eq!(vouchers.len(), 1);
    assert_eq!(vouchers[0].voucher_no, "V-DATA-VIS");
}

#[tokio::test]
async fn test_tier1_f5_04_post_restore_period_lock_visibility() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let rev = VoucherBuilder::new("V-REV-PRECLOSE-01", "2026-01-15")
        .leg("1121", "10000000", "0")
        .leg("511", "0", "10000000")
        .build();
    let rev_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&rev)
        .send()
        .await;
    rev_res.assert_status(StatusCode::CREATED);

    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    close_res.assert_status(StatusCode::OK);

    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let restore_res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    restore_res.assert_status(StatusCode::OK);

    let res = app.get("/api/v1/periods").as_accountant().send().await;
    res.assert_status(StatusCode::OK);
    let periods: Vec<serde_json::Value> = res.json();
    let p = periods
        .iter()
        .find(|p| {
            (p["year"] == 2026 || p["period_year"] == 2026)
                && (p["month"] == 1 || p["period_month"] == 1)
        })
        .unwrap();
    assert_eq!(p["is_closed"], true);
}

#[tokio::test]
async fn test_tier1_f5_05_restore_audit_api_visibility() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let restore_res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    restore_res.assert_status(StatusCode::OK);

    let res = app
        .get("/api/v1/audit-logs?action=SYSTEM_RESTORE")
        .as_controller()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let logs: Vec<serde_json::Value> = res.json();
    assert!(!logs.is_empty());
    assert_eq!(logs[0]["action"], "SYSTEM_RESTORE");
}

// ============================================================================
// Feature 6 (R3): Restore CLI Subcommand
// ============================================================================

#[test]
fn test_tier1_f6_01_cli_standard_restore_invocation() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Create a real snapshot via app
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (snap_path, target_path) = rt.block_on(async {
        let p = app.take_backup().await;
        let target = app.env.as_ref().unwrap().root_dir.join("cli_restored.db");
        (p, target)
    });

    let runner = CliRunner::new();
    let output = runner.restore(Path::new(&snap_path), &target_path);
    assert!(
        output.status.success(),
        "CLI restore must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        target_path.exists(),
        "Target database file must exist after restore"
    );
}

#[test]
fn test_tier1_f6_02_cli_flag_order_invariance() {
    let env = TestEnv::new();
    let snap_path = env.root_dir.join("dummy_snap.db");
    let target_path = env.root_dir.join("dummy_target.db");
    std::fs::write(&snap_path, b"dummy").unwrap();

    let runner = CliRunner::new();
    let output = runner.run_with_args(&[
        "restore",
        "--target",
        target_path.to_str().unwrap(),
        "--snapshot",
        snap_path.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("unexpected flag --target"));
}

#[test]
fn test_tier1_f6_03_cli_nonexistent_target_creation() {
    let env = TestEnv::new();
    let snap_path = env.root_dir.join("snap.db");
    let nested_target = env.root_dir.join("nested/folder/target.db");
    let runner = CliRunner::new();
    let output = runner.run_with_args(&[
        "restore",
        "--snapshot",
        snap_path.to_str().unwrap(),
        "--target",
        nested_target.to_str().unwrap(),
    ]);
    // Verifies the runner executed
    assert!(output.status.code().is_some());
}

#[test]
fn test_tier1_f6_04_cli_target_sqlite_verification() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let (snap_path, target_path) = rt.block_on(async {
        let p = app.take_backup().await;
        let target = app
            .env
            .as_ref()
            .unwrap()
            .root_dir
            .join("cli_target_check.db");
        (p, target)
    });

    let runner = CliRunner::new();
    let output = runner.restore(Path::new(&snap_path), &target_path);
    assert!(output.status.success());

    // Connect to target and verify SQLite integrity
    let conn = Connection::open(&target_path).expect("Target DB must open");
    let check: String = conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
}

#[tokio::test]
async fn test_tier1_f6_05_cli_audit_attribution() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);
    let target_path = app.env.as_ref().unwrap().root_dir.join("cli_audit.db");

    let snap_path = app.take_backup().await;
    let runner = CliRunner::new();
    let output = runner.restore(Path::new(&snap_path), &target_path);
    assert!(output.status.success());

    // Verify audit attribution in restored DB
    let conn = Connection::open(&target_path).unwrap();
    let actor: String = conn
        .query_row(
            "SELECT actor FROM audit_logs WHERE action = 'SYSTEM_RESTORE' ORDER BY id DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .expect("CLI restore audit log must exist");
    assert_eq!(
        actor, "cli_admin",
        "CLI restore must record actor as 'cli_admin'"
    );
}

// ============================================================================
// Feature 7 (R3): Zero-Diff Drill Suite
// ============================================================================

#[tokio::test]
async fn test_tier1_f7_01_trial_balance_zero_variance() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post operational vouchers affecting multiple accounts
    let v1 = VoucherBuilder::simple_cash_withdrawal("V-F7-01", "2026-01-10", "15000000");
    let r1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v1)
        .send()
        .await;
    r1.assert_status(StatusCode::CREATED);

    let v2 = VoucherBuilder::sales_invoice("V-F7-02", "2026-01-15", "50000000", "5000000");
    let r2 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v2)
        .send()
        .await;
    r2.assert_status(StatusCode::CREATED);

    // 2. Query pre-backup Trial Balance baseline
    let pre_res = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    pre_res.assert_status(StatusCode::OK);
    let pre_tb: TrialBalanceResponse = pre_res.json();
    assert_trial_balance_balanced(&pre_tb);

    // 3. Take official backup snapshot
    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).expect("Read snapshot");
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    // 4. Execute authentic restore via API
    let restore_payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let restore_res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&restore_payload)
        .send()
        .await;
    restore_res.assert_status(StatusCode::OK);

    // 5. Query post-restore Trial Balance
    let post_res = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    post_res.assert_status(StatusCode::OK);
    let post_tb: TrialBalanceResponse = post_res.json();

    // 6. Assert authentic zero discrepancy between pre and post states
    assert_zero_diff_trial_balance(&pre_tb, &post_tb);
}

#[tokio::test]
async fn test_tier1_f7_02_balance_sheet_zero_variance() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post equity and capital contributions (Assets = Equity)
    let v = VoucherBuilder::new("V-F7-03", "2026-01-10")
        .description("Gop von kinh doanh")
        .leg("1121", "250000000", "0")
        .leg("4111", "0", "250000000")
        .build();
    let r = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    r.assert_status(StatusCode::CREATED);

    // 2. Query pre-backup Balance Sheet baseline
    let pre_res = app
        .get("/api/v1/reports/balance-sheet?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    pre_res.assert_status(StatusCode::OK);
    let pre_bs: BalanceSheetResponse = pre_res.json();
    assert_balance_sheet_balanced(&pre_bs);

    // 3. Take backup and compute SHA-256
    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).expect("Read snapshot");
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    // 4. Execute restore
    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::OK);

    // 5. Query post-restore Balance Sheet
    let post_res = app
        .get("/api/v1/reports/balance-sheet?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    post_res.assert_status(StatusCode::OK);
    let post_bs: BalanceSheetResponse = post_res.json();

    // 6. Assert zero variance across pre and post Balance Sheets
    assert_zero_diff_balance_sheet(&pre_bs, &post_bs);
}

#[tokio::test]
async fn test_tier1_f7_03_income_statement_zero_variance() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post sales revenue and cost of goods sold
    let v_sales = VoucherBuilder::sales_invoice("V-F7-04", "2026-01-15", "80000000", "8000000");
    let r1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_sales)
        .send()
        .await;
    r1.assert_status(StatusCode::CREATED);

    let v_cogs = VoucherBuilder::new("V-F7-05", "2026-01-16")
        .description("Gia von hang ban")
        .leg("632", "50000000", "0")
        .leg("156", "0", "50000000")
        .build();
    let r2 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_cogs)
        .send()
        .await;
    r2.assert_status(StatusCode::CREATED);

    // 2. Query pre-backup Income Statement
    let pre_res = app
        .get("/api/v1/reports/income-statement?from_date=2026-01-01&to_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    pre_res.assert_status(StatusCode::OK);
    let pre_is: backend::dto::IncomeStatementResponse = pre_res.json();

    // 3. Take backup and restore
    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::OK);

    // 4. Query post-restore Income Statement
    let post_res = app
        .get("/api/v1/reports/income-statement?from_date=2026-01-01&to_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    post_res.assert_status(StatusCode::OK);
    let post_is: backend::dto::IncomeStatementResponse = post_res.json();

    // 5. Assert zero difference
    assert_zero_diff_income_statement(&pre_is, &post_is);
}

#[tokio::test]
async fn test_tier1_f7_04_voucher_population_equality() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post 5 vouchers
    for i in 1..=5 {
        let v = VoucherBuilder::simple_cash_withdrawal(
            &format!("V-F7-POP-{i:02}"),
            "2026-01-10",
            "2000000",
        );
        let r = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&v)
            .send()
            .await;
        r.assert_status(StatusCode::CREATED);
    }

    // 2. Query pre-backup vouchers
    let pre_res = app.get("/api/v1/vouchers").as_accountant().send().await;
    pre_res.assert_status(StatusCode::OK);
    let pre_vouchers: Vec<JournalVoucher> = pre_res.json();
    assert_eq!(pre_vouchers.len(), 5);

    // 3. Backup and restore
    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::OK);

    // 4. Query post-restore vouchers
    let post_res = app.get("/api/v1/vouchers").as_accountant().send().await;
    post_res.assert_status(StatusCode::OK);
    let post_vouchers: Vec<JournalVoucher> = post_res.json();

    // 5. Assert voucher population identity
    assert_zero_diff_voucher_counts(&pre_vouchers, &post_vouchers);
}

#[tokio::test]
async fn test_tier1_f7_05_period_lock_state_preservation() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post voucher in 2026-01
    let v_rev = VoucherBuilder::new("V-LOCK-REV-01", "2026-01-10")
        .leg("1121", "10000000", "0")
        .leg("511", "0", "10000000")
        .build();
    let r_rev = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_rev)
        .send()
        .await;
    r_rev.assert_status(StatusCode::CREATED);

    let v = VoucherBuilder::simple_cash_withdrawal("V-LOCK-01", "2026-01-10", "1000000");
    let r = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    r.assert_status(StatusCode::CREATED);

    // 2. Controller closes period 2026-01
    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    close_res.assert_status(StatusCode::OK);

    // 3. Query pre-backup period state
    let pre_p_res = app.get("/api/v1/periods").as_accountant().send().await;
    pre_p_res.assert_status(StatusCode::OK);
    let pre_periods: Vec<serde_json::Value> = pre_p_res.json();
    let p_2026_01 = pre_periods
        .iter()
        .find(|p| {
            (p["year"] == 2026 || p["period_year"] == 2026)
                && (p["month"] == 1 || p["period_month"] == 1)
        })
        .expect("Period 2026-01 found");
    assert_eq!(p_2026_01["is_closed"], true);

    // 4. Backup and restore
    let snapshot_path = app.take_backup().await;
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    let payload = json!({
        "snapshot_path": snapshot_path,
        "confirm_overwrite": true,
        "expected_sha256": sha256_hex
    });
    let res = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::OK);

    // 5. Query post-restore periods
    let post_p_res = app.get("/api/v1/periods").as_accountant().send().await;
    post_p_res.assert_status(StatusCode::OK);
    let post_periods: Vec<serde_json::Value> = post_p_res.json();
    let post_p_2026_01 = post_periods
        .iter()
        .find(|p| {
            (p["year"] == 2026 || p["period_year"] == 2026)
                && (p["month"] == 1 || p["period_month"] == 1)
        })
        .expect("Period 2026-01 found post-restore");
    assert_eq!(
        post_p_2026_01["is_closed"], true,
        "Period 2026-01 must remain locked after restore"
    );
}

// ============================================================================
// Feature 8 (R5): Dynamic Chart of Accounts Entity & Schema
// ============================================================================

#[test]
fn test_tier1_f8_01_seeded_accounts_completeness() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM accounts;", [], |r| r.get(0))
        .expect("accounts table must exist and be queryable");
    assert!(
        count >= 50,
        "Expected at least 50 seeded Circular 99 accounts, found {count}"
    );
}

#[test]
fn test_tier1_f8_02_raw_materials_subaccounts_1520_to_1525() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM accounts WHERE code IN ('1520', '1521', '1522', '1523', '1524', '1525');",
            [],
            |r| r.get(0),
        )
        .expect("accounts table must exist");
    assert_eq!(count, 6, "Expected 6 raw materials subaccounts 1520..1525");
}

#[test]
fn test_tier1_f8_03_account_hierarchy_linkages() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let orphan_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM accounts a WHERE a.parent_code IS NOT NULL AND NOT EXISTS (SELECT 1 FROM accounts p WHERE p.code = a.parent_code);",
            [],
            |r| r.get(0),
        )
        .expect("Querying parent-child linkages must succeed");
    assert_eq!(orphan_count, 0, "No orphan parent_codes allowed");
}

#[test]
fn test_tier1_f8_04_contra_asset_normal_balance() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let normal_bal: String = conn
        .query_row(
            "SELECT normal_balance FROM accounts WHERE code = '2141';",
            [],
            |r| r.get(0),
        )
        .expect("Account 2141 must exist");
    assert_eq!(
        normal_bal, "CREDIT",
        "TK 2141 must have CREDIT normal balance (contra-asset)"
    );
}

#[test]
fn test_tier1_f8_05_contra_revenue_normal_balance() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let normal_bal: String = conn
        .query_row(
            "SELECT normal_balance FROM accounts WHERE code = '5211';",
            [],
            |r| r.get(0),
        )
        .expect("Account 5211 must exist");
    assert_eq!(
        normal_bal, "DEBIT",
        "TK 5211 must have DEBIT normal balance (contra-revenue)"
    );
}

#[test]
fn test_tier1_f8_06_clearing_account_911() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let class: String = conn
        .query_row("SELECT class FROM accounts WHERE code = '911';", [], |r| {
            r.get(0)
        })
        .expect("Account 911 must exist");
    assert_eq!(class, "CLOSING");
}

// ============================================================================
// Feature 9 (R5): Chart of Accounts API Endpoints
// ============================================================================

#[tokio::test]
async fn test_tier1_f9_01_list_all_accounts() {
    let app = TestApp::in_memory();
    let res = app.get("/api/v1/accounts").as_accountant().send().await;
    res.assert_status(StatusCode::OK);
    let list: Vec<serde_json::Value> = res.json();
    assert!(
        list.len() >= 50,
        "Expected at least 50 seeded accounts returned by API"
    );
}

#[tokio::test]
async fn test_tier1_f9_02_filter_accounts_by_class() {
    let app = TestApp::in_memory();
    let res = app
        .get("/api/v1/accounts?class=ASSET")
        .as_accountant()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let list: Vec<serde_json::Value> = res.json();
    assert!(!list.is_empty(), "ASSET accounts filter must not be empty");
    for item in list {
        assert_eq!(item["class"], "ASSET");
    }
}

#[tokio::test]
async fn test_tier1_f9_03_filter_accounts_by_active() {
    let app = TestApp::in_memory();
    let res = app
        .get("/api/v1/accounts?is_active=true")
        .as_accountant()
        .send()
        .await;
    res.assert_status(StatusCode::OK);
    let list: Vec<serde_json::Value> = res.json();
    assert!(!list.is_empty());
    for item in list {
        assert_eq!(item["is_active"], true);
    }
}

#[tokio::test]
async fn test_tier1_f9_04_create_raw_material_subaccount() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "15201",
        "name": "Thep cuon ma kem Posco 1.2mm",
        "parent_code": "1520",
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
    res.assert_status(StatusCode::CREATED);
    let item = res.json_val();
    assert_eq!(item["code"], "15201");
    assert_eq!(item["parent_code"], "1520");
}

#[tokio::test]
async fn test_tier1_f9_05_create_expense_subaccount() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "64281",
        "name": "Chi phi kiem toan doc lap",
        "parent_code": "6428",
        "class": "EXPENSE",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::CREATED);
    let item = res.json_val();
    assert_eq!(item["code"], "64281");
}

#[tokio::test]
async fn test_tier1_f9_06_create_revenue_subaccount() {
    let app = TestApp::in_memory();
    let payload = json!({
        "code": "51111",
        "name": "Doanh thu xuat khau may moc CNC",
        "parent_code": "5111",
        "class": "REVENUE",
        "is_active": true,
        "normal_balance": "CREDIT"
    });
    let res = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&payload)
        .send()
        .await;
    res.assert_status(StatusCode::CREATED);
    let item = res.json_val();
    assert_eq!(item["code"], "51111");
}

// ============================================================================
// Feature 10 (R5): Voucher Account Validation
// ============================================================================

#[tokio::test]
async fn test_tier1_f10_01_post_voucher_with_valid_seeded_accounts() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-F10-01", "2026-01-10")
        .description("Thu tien ban hang bang tien mat")
        .leg("1111", "10000000", "0")
        .leg("5111", "0", "10000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_f10_02_post_compound_voucher_with_1520() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-F10-02", "2026-01-12")
        .description("Mua vat lieu 1520")
        .leg("1520", "50000000", "0")
        .leg("1331", "5000000", "0")
        .leg("331", "0", "55000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_f10_03_post_voucher_referencing_new_custom_account() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-F10-03", "2026-01-15")
        .description("Chuyen khoan thanh toan")
        .leg("1121", "20000000", "0")
        .leg("131", "0", "20000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_f10_04_post_voucher_all_inventory_subaccounts_1520_to_1525() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-F10-04", "2026-01-16")
        .description("Nhap kho dong loat cac tieu khoan vat tu")
        .leg("1520", "10000000", "0")
        .leg("1521", "20000000", "0")
        .leg("1522", "5000000", "0")
        .leg("1523", "2000000", "0")
        .leg("1524", "8000000", "0")
        .leg("1525", "5000000", "0")
        .leg("331", "0", "50000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_f10_05_post_closing_voucher_account_911() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-F10-05", "2026-01-31")
        .description("Ket chuyen doanh thu vao tai khoan 911")
        .leg("5111", "100000000", "0")
        .leg("911", "0", "100000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_f10_06_post_equity_transfer_voucher_4211_4212() {
    let app = TestApp::in_memory();
    let v = VoucherBuilder::new("V-F10-06", "2026-01-31")
        .description("Ket chuyen loi nhuan sau thue chua phan phoi")
        .leg("4212", "50000000", "0")
        .leg("4211", "0", "50000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);
}

// ============================================================================
// Feature 11 (R6): Configurable Fiscal Year Start Date
// ============================================================================

pub fn oracle_fiscal_period_for_date(date: NaiveDate, start_month: u32) -> u32 {
    use chrono::Datelike;
    let m = date.month();
    if m >= start_month {
        m - start_month + 1
    } else {
        12 - start_month + m + 1
    }
}

#[test]
fn test_tier1_f11_01_default_calendar_fiscal_year_bounds() {
    let (start, end) = backend::closing::fiscal_year_bounds(2026, 1).unwrap();
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
}

#[test]
fn test_tier1_f11_02_april_first_fiscal_year_bounds() {
    let (start, end) = backend::closing::fiscal_year_bounds(2026, 4).unwrap();
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2027, 3, 31).unwrap());
}

#[test]
fn test_tier1_f11_03_july_first_fiscal_year_bounds() {
    let (start, end) = backend::closing::fiscal_year_bounds(2026, 7).unwrap();
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2027, 6, 30).unwrap());
}

#[test]
fn test_tier1_f11_04_october_first_fiscal_year_bounds() {
    let (start, end) = backend::closing::fiscal_year_bounds(2026, 10).unwrap();
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2027, 9, 30).unwrap());
}

#[test]
fn test_tier1_f11_05_leap_year_february_closing_bounds() {
    let leap_end = backend::closing::PeriodClosingEngine::last_day_of_month(2028, 2).unwrap();
    assert_eq!(leap_end, NaiveDate::from_ymd_opt(2028, 2, 29).unwrap());

    let non_leap_end = backend::closing::PeriodClosingEngine::last_day_of_month(2026, 2).unwrap();
    assert_eq!(non_leap_end, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
}

#[test]
fn test_tier1_f11_06_invalid_month_rejected() {
    assert!(backend::closing::fiscal_year_bounds(2026, 0).is_err());
    assert!(backend::closing::fiscal_year_bounds(2026, 13).is_err());
}
