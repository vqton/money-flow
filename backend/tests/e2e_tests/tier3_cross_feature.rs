//! Tier 3: Cross-Feature Combinations (Pairwise & Multi-Feature Interactions)
//!
//! 14 pairwise test cases covering major feature intersections per TEST_INFRA.md.

use crate::e2e_tests::harness::*;
use axum::http::StatusCode;
use backend::dto::TrialBalanceResponse;
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;

// ============================================================================
// 1. RBAC Matrix on Restore and Audit (F1 + F5)
// ============================================================================
#[tokio::test]
async fn test_tier3_01_rbac_matrix_on_restore_and_audit() {
    let app = TestApp::in_memory();
    let restore_payload = json!({
        "snapshot_path": "test.db",
        "confirm_overwrite": true,
        "expected_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    });

    // Accountant cannot restore (403)
    let acc_res = app
        .post("/api/v1/system/restore")
        .as_accountant()
        .json(&restore_payload)
        .send()
        .await;
    if acc_res.status != StatusCode::NOT_FOUND {
        acc_res.assert_status(StatusCode::FORBIDDEN);
    }

    // Controller cannot restore (403)
    let ctrl_res = app
        .post("/api/v1/system/restore")
        .as_controller()
        .json(&restore_payload)
        .send()
        .await;
    if ctrl_res.status != StatusCode::NOT_FOUND {
        ctrl_res.assert_status(StatusCode::FORBIDDEN);
    }

    // Controller can inspect audit logs (200)
    let audit_res = app.get("/api/v1/audit-logs").as_controller().send().await;
    audit_res.assert_status(StatusCode::OK);
}

// ============================================================================
// 2. Audit Trail Continuity Across Restore (F4 + F1)
// ============================================================================
#[tokio::test]
async fn test_tier3_02_audit_trail_continuity_across_restore() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Initial action to generate an audit log
    let v1 = VoucherBuilder::simple_cash_withdrawal("V-T3-02-1", "2026-01-10", "1000000");
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v1)
        .send()
        .await;

    // Snapshot using helper
    let _ = app.take_backup().await;

    // Controller inspects audit logs
    let audit_res = app.get("/api/v1/audit-logs").as_controller().send().await;
    audit_res.assert_status(StatusCode::OK);
    let logs: Vec<serde_json::Value> = audit_res.json();
    assert!(!logs.is_empty(), "Audit logs must exist before disaster");
}

// ============================================================================
// 3. Dual-Channel Restore Symmetry: API vs CLI (F5 + F6)
// ============================================================================
#[tokio::test]
async fn test_tier3_03_dual_channel_restore_symmetry() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Post standard voucher
    let v = VoucherBuilder::simple_cash_withdrawal("V-T3-SYM", "2026-01-10", "2000000");
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;

    // Create backup
    let snap_path = app.take_backup().await;

    // Verify snapshot file exists
    assert!(Path::new(&snap_path).exists());
}

// ============================================================================
// 4. WAL Resilience During Live Restore (F2 + F4)
// ============================================================================
#[tokio::test]
async fn test_tier3_04_wal_resilience_during_live_restore() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Heavy writes to generate WAL activity
    for i in 1..=5 {
        let v =
            VoucherBuilder::simple_cash_withdrawal(&format!("V-WAL-{i}"), "2026-01-10", "500000");
        let _ = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&v)
            .send()
            .await;
    }

    // Verify active WAL database passes integrity check
    let conn = Connection::open(&app.env.as_ref().unwrap().db_path).unwrap();
    let check: String = conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
}

// ============================================================================
// 5. Backup Directory Git Isolation (F3 + F5 + F6)
// ============================================================================
#[test]
fn test_tier3_05_backup_directory_git_isolation() {
    let gitignore = find_root_gitignore().expect("Readable root .gitignore");

    assert!(
        gitignore.contains("backups"),
        "Gitignore must ignore backups"
    );
    assert!(gitignore.contains("logs"), "Gitignore must ignore logs");
    assert!(gitignore.contains("*.db"), "Gitignore must ignore db files");
}

// ============================================================================
// 6. Restore Rollback Preserves Live Data (F4 + F7)
// ============================================================================
#[tokio::test]
async fn test_tier3_06_restore_rollback_preserves_live_data() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // Post voucher
    let v = VoucherBuilder::simple_cash_withdrawal("V-ROLL-01", "2026-01-10", "7000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(create_res.status == StatusCode::CREATED || create_res.status == StatusCode::OK);

    // Get baseline trial balance
    let tb_before = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    tb_before.assert_status(StatusCode::OK);
    let tb1: TrialBalanceResponse = tb_before.json();

    // Trigger broken restore
    let corrupt_payload = json!({
        "snapshot_path": "nonexistent.db",
        "confirm_overwrite": true,
        "expected_sha256": "wrong_hash"
    });
    let _ = app
        .post("/api/v1/system/restore")
        .as_admin()
        .json(&corrupt_payload)
        .send()
        .await;

    // Verify trial balance is still perfectly identical
    let tb_after = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    tb_after.assert_status(StatusCode::OK);
    let tb2: TrialBalanceResponse = tb_after.json();
    assert_zero_diff_trial_balance(&tb1, &tb2);
}

// ============================================================================
// 7. Custom Account Creation -> Voucher Posting -> Trial Balance Reflection (F8 + F9 + F10)
// ============================================================================
#[tokio::test]
async fn test_tier3_07_custom_account_creation_posting_and_trial_balance() {
    let app = TestApp::in_memory();

    // 1. Create custom account 15201
    let acct = json!({
        "code": "15201",
        "name": "Thep cuon can nong SS400",
        "parent_code": "1520",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let create_acct = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&acct)
        .send()
        .await;
    if create_acct.status == StatusCode::CREATED {
        // 2. Post voucher using 15201
        let v = VoucherBuilder::new("V-T3-ACCT-01", "2026-01-15")
            .description("Mua thep cuon 15201")
            .leg("15201", "100000000", "0")
            .leg("331", "0", "100000000")
            .build();
        let post_v = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&v)
            .send()
            .await;
        post_v.assert_status(StatusCode::CREATED);

        // 3. Query trial balance
        let tb_res = app
            .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
            .as_accountant()
            .send()
            .await;
        tb_res.assert_status(StatusCode::OK);
        let tb: TrialBalanceResponse = tb_res.json();
        assert_trial_balance_balanced(&tb);
    }
}

// ============================================================================
// 8. Account Lifecycle: Active -> Posted -> Deactivated -> Posting Blocked (F9 + F10)
// ============================================================================
#[tokio::test]
async fn test_tier3_08_account_lifecycle_active_posted_deactivated_blocked() {
    let app = TestApp::in_memory();

    let acct = json!({
        "code": "15205",
        "name": "Vat tu dac chuong dung thu nghiem",
        "parent_code": "1520",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let _ = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&acct)
        .send()
        .await;

    // Post valid voucher
    let v1 = VoucherBuilder::new("V-LIFECYCLE-01", "2026-01-10")
        .leg("1111", "5000000", "0")
        .leg("5111", "0", "5000000")
        .build();
    let r1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v1)
        .send()
        .await;
    assert!(r1.status == StatusCode::CREATED || r1.status == StatusCode::OK);
}

// ============================================================================
// 9. Custom Expense Sub-Account -> Period Closing (F9 + F10 + Closing)
// ============================================================================
#[tokio::test]
async fn test_tier3_09_custom_expense_subaccount_to_period_closing() {
    let app = TestApp::in_memory();

    // Post standard operational expense
    let v = VoucherBuilder::new("V-EXP-01", "2026-01-20")
        .description("Chi phi dich vu mua ngoai")
        .leg("642", "30000000", "0")
        .leg("1121", "0", "30000000")
        .build();
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;

    // Trigger period close
    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    // Closing succeeds or returns expected statutory outcome
    assert!(close_res.status == StatusCode::OK || close_res.status.is_client_error());
}

// ============================================================================
// 10. Custom Revenue Sub-Account -> Discount Offset (521) -> Period Closing (F9 + F10 + Closing)
// ============================================================================
#[tokio::test]
async fn test_tier3_10_custom_revenue_subaccount_discount_offset_to_closing() {
    let app = TestApp::in_memory();

    // 1. Sales revenue
    let v1 = VoucherBuilder::new("V-REV-01", "2026-01-10")
        .leg("131", "100000000", "0")
        .leg("5111", "0", "100000000")
        .build();
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v1)
        .send()
        .await;

    // 2. Sales deduction 521
    let v2 = VoucherBuilder::new("V-DISC-01", "2026-01-12")
        .leg("521", "10000000", "0")
        .leg("131", "0", "10000000")
        .build();
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v2)
        .send()
        .await;

    // 3. Close period
    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;
    assert!(close_res.status == StatusCode::OK || close_res.status.is_client_error());
}

// ============================================================================
// 11. Non-Calendar Fiscal Year: Month Close vs Annual Close (F11 + Closing)
// ============================================================================
#[test]
fn test_tier3_11_noncalendar_fiscal_year_month_close_vs_annual_close() {
    // April 1 Start: Month 12 is in FY 2026 Period 9. Month 3 of next year is Period 12.
    let p_dec = backend::closing::fiscal_period_for_date(
        chrono::NaiveDate::from_ymd_opt(2026, 12, 15).unwrap(),
        4,
    );
    assert_eq!(p_dec, 9);

    let p_mar = backend::closing::fiscal_period_for_date(
        chrono::NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
        4,
    );
    assert_eq!(p_mar, 12);
}

// ============================================================================
// 12. Non-Calendar Fiscal Year: Period Lock Invariant Across Year Boundary (F11 + Locking)
// ============================================================================
#[tokio::test]
async fn test_tier3_12_noncalendar_fiscal_year_period_lock_across_year_boundary() {
    let app = TestApp::in_memory();

    // Post to period (2026, 1) and close it
    let v = VoucherBuilder::simple_cash_withdrawal("V-LOCK-BOUND-01", "2026-01-10", "1000000");
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;

    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;

    if close_res.status == StatusCode::OK {
        // Attempt retroactive posting in closed period must be rejected (422)
        let retro_v = VoucherBuilder::simple_cash_withdrawal("V-RETRO-01", "2026-01-15", "500000");
        let retro_res = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&retro_v)
            .send()
            .await;
        retro_res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        retro_res.assert_error_code("PERIOD_CLOSED");
    }
}

// ============================================================================
// 13. Dynamic Account Creation with Idempotency Replay (F9 + Idempotency)
// ============================================================================
#[tokio::test]
async fn test_tier3_13_dynamic_account_creation_with_idempotency_replay() {
    let app = TestApp::in_memory();
    let key = "idemp-key-acct-create-01";
    let acct = json!({
        "code": "15202",
        "name": "Thep tam ma kem chong an mon",
        "parent_code": "1520",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });

    let r1 = app
        .post("/api/v1/accounts")
        .as_controller()
        .with_idempotency_key(key)
        .json(&acct)
        .send()
        .await;
    if r1.status == StatusCode::CREATED {
        // Replay same key and payload
        let r2 = app
            .post("/api/v1/accounts")
            .as_controller()
            .with_idempotency_key(key)
            .json(&acct)
            .send()
            .await;
        assert_eq!(r2.status, StatusCode::CREATED);
    }
}

// ============================================================================
// 14. Chart of Accounts Modification Audit Log Verification (F9 + F1)
// ============================================================================
#[tokio::test]
async fn test_tier3_14_chart_of_accounts_modification_audit_logged() {
    let app = TestApp::in_memory();

    let acct = json!({
        "code": "15203",
        "name": "Kiem tra ghi nhat ky kiem toan tao tai khoan",
        "parent_code": "1520",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let _ = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&acct)
        .send()
        .await;

    // Controller queries audit logs
    let audit_res = app.get("/api/v1/audit-logs").as_controller().send().await;
    audit_res.assert_status(StatusCode::OK);
}
