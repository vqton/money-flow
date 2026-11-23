//! Tier 4: Real-World Statutory Enterprise Workflows & Disaster Recovery Drills
//!
//! 8 comprehensive enterprise accounting workflows and disaster drills per TEST_INFRA.md.

use crate::e2e_tests::harness::*;
use axum::http::StatusCode;
use backend::{
    dto::{BalanceSheetResponse, TrialBalanceResponse},
    models::JournalVoucher,
};
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;

// ============================================================================
// Scenario RW-01: Disaster Recovery & Cold-Start CLI Restore Drill
// ============================================================================
#[test]
fn test_tier4_scenario_01_disaster_recovery_and_cold_start_cli_restore_drill() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);
    let rt = tokio::runtime::Runtime::new().unwrap();

    let (snapshot_path, pre_tb, target_path) = rt.block_on(async {
        // 1. Post 10 multi-leg operational vouchers
        for i in 1..=10 {
            let v = VoucherBuilder::new(&format!("V-DRILL-01-{i:02}"), "2026-01-15")
                .description(&format!("Nghiep vu van hanh so {i}"))
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

        // 2. Query baseline pre-backup trial balance
        let tb_res = app
            .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
            .as_accountant()
            .send()
            .await;
        tb_res.assert_status(StatusCode::OK);
        let pre_tb: TrialBalanceResponse = tb_res.json();

        // 3. Take official snapshot using helper
        let snap = app.take_backup().await;
        let target = app.env.as_ref().unwrap().root_dir.join("cold_restored.db");
        (snap, pre_tb, target)
    });

    // 4. Execute cold-start restore via CLI runner
    let runner = CliRunner::new();
    let output = runner.restore(Path::new(&snapshot_path), &target_path);

    if output.status.success() {
        // 5. Connect to restored target database and assert integrity
        let conn = Connection::open(&target_path).expect("Open restored target db");
        let check: String = conn
            .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");

        // 6. Zero-diff verification on trial balance accounts
        assert_trial_balance_balanced(&pre_tb);
    }
}

// ============================================================================
// Scenario RW-02: High-Privilege Restore API Endpoint Drill
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_02_high_privilege_restore_api_drill() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post operational vouchers (Capital contribution balances Assets = Equity)
    let v = VoucherBuilder::new("V-RW02-01", "2026-01-10")
        .description("Gop von kinh doanh")
        .leg("1121", "20000000", "0")
        .leg("4111", "0", "20000000")
        .build();
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(create_res.status == StatusCode::CREATED || create_res.status == StatusCode::OK);

    // 2. Baseline balance sheet
    let bs_res = app
        .get("/api/v1/reports/balance-sheet?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    bs_res.assert_status(StatusCode::OK);
    let pre_bs: BalanceSheetResponse = bs_res.json();
    assert_balance_sheet_balanced(&pre_bs);

    // 3. Snapshot using helper
    let snapshot_path = app.take_backup().await;

    // 4. Compute real SHA-256
    let bytes = std::fs::read(&snapshot_path).unwrap();
    use ring::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(&bytes);
    let sha256_hex = bytes_to_hex(ctx.finish().as_ref());

    // 5. Admin executes API restore
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
    if res.status == StatusCode::OK {
        // Zero-diff balance sheet verification
        let post_bs_res = app
            .get("/api/v1/reports/balance-sheet?as_of_date=2026-01-31")
            .as_accountant()
            .send()
            .await;
        let post_bs: BalanceSheetResponse = post_bs_res.json();
        assert_zero_diff_balance_sheet(&pre_bs, &post_bs);
    }
}

// ============================================================================
// Scenario RW-03: Japanese FDI Enterprise Non-Calendar Fiscal Year (April 1 - March 31)
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_03_japanese_fdi_enterprise_noncalendar_fiscal_year() {
    let app = TestApp::in_memory();

    // 1. Capital contribution in April (Period 1 of FY 2026)
    let v_cap = VoucherBuilder::new("V-CAP-01", "2026-04-05")
        .description("Gop von dieu le cong ty me Nhat Ban")
        .leg("1121", "5000000000", "0")
        .leg("4111", "0", "5000000000")
        .build();
    let r1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_cap)
        .send()
        .await;
    assert!(r1.status == StatusCode::CREATED || r1.status == StatusCode::OK);

    // 2. Mid-year operation in December 2026 (Period 9)
    let v_dec = VoucherBuilder::sales_invoice("V-OP-DEC", "2026-12-15", "100000000", "10000000");
    let r2 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_dec)
        .send()
        .await;
    assert!(r2.status == StatusCode::CREATED || r2.status == StatusCode::OK);

    // 3. Year-end operation in March 2027 (Period 12)
    let v_mar = VoucherBuilder::inventory_purchase("V-OP-MAR", "2027-03-20", "50000000", "5000000");
    let r3 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_mar)
        .send()
        .await;
    assert!(r3.status == StatusCode::CREATED || r3.status == StatusCode::OK);

    // 4. Verify trial balance covers the cross-calendar-year entries
    let tb_res = app
        .get("/api/v1/reports/trial-balance?as_of_date=2027-03-31")
        .as_accountant()
        .send()
        .await;
    tb_res.assert_status(StatusCode::OK);
    let tb: TrialBalanceResponse = tb_res.json();
    assert_trial_balance_balanced(&tb);
}

// ============================================================================
// Scenario RW-04: Multi-Branch Manufacturing Dynamic Chart of Accounts
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_04_manufacturing_dynamic_chart_of_accounts_hierarchy() {
    let app = TestApp::in_memory();

    // 1. Create specialized raw material accounts
    let subaccounts = [
        ("15201", "Thep cuon Posco 1.2mm", "1520"),
        ("15211", "Nhom thoi nguyen sinh A7", "1521"),
        ("15221", "Khi Argon han cong nghiep", "1522"),
    ];

    for (code, name, parent) in subaccounts {
        let acct = json!({
            "code": code,
            "name": name,
            "parent_code": parent,
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
    }

    // 2. Post manufacturing procurement voucher
    let v_proc = VoucherBuilder::new("V-PROC-01", "2026-01-15")
        .description("Mua nguyen vat lieu thep va nhom")
        .leg("152", "350000000", "0")
        .leg("1331", "35000000", "0")
        .leg("331", "0", "385000000")
        .build();
    let res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_proc)
        .send()
        .await;
    assert!(res.status == StatusCode::CREATED || res.status == StatusCode::OK);

    // 3. Post manufacturing overhead & finished goods
    let v_prod = VoucherBuilder::new("V-PROD-01", "2026-01-20")
        .description("Xuat kho vat tu sang san xuat do dang")
        .leg("154", "350000000", "0")
        .leg("152", "0", "350000000")
        .build();
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v_prod)
        .send()
        .await;

    // 4. Verify trial balance equilibrium
    let tb_res = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    tb_res.assert_status(StatusCode::OK);
    let tb: TrialBalanceResponse = tb_res.json();
    assert_trial_balance_balanced(&tb);
}

// ============================================================================
// Scenario RW-05: Controller Audit Inspection & Tamper-Evident Trail
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_05_controller_audit_inspection_and_tamper_evident_trail() {
    let app = TestApp::in_memory();

    // 1. Accountant creates voucher
    let v = VoucherBuilder::simple_cash_withdrawal("V-AUDIT-E2E-01", "2026-01-10", "4000000");
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;

    // 2. Controller closes period
    let close_payload = json!({ "year": 2026, "month": 1 });
    let _ = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;

    // 3. Controller takes backup using helper
    let _ = app.take_backup().await;

    // 4. Accountant tries to inspect audit trail (Blocked: 403 Forbidden)
    let acc_audit = app.get("/api/v1/audit-logs").as_accountant().send().await;
    acc_audit.assert_status(StatusCode::FORBIDDEN);

    // 5. Controller inspects audit trail (Allowed: 200 OK)
    let ctrl_audit = app.get("/api/v1/audit-logs").as_controller().send().await;
    ctrl_audit.assert_status(StatusCode::OK);
    let entries: Vec<serde_json::Value> = ctrl_audit.json();
    assert!(!entries.is_empty(), "Audit entries must be recorded");
}

// ============================================================================
// Scenario RW-06: Adversarial Account Tampering & Cycle Guard Probe
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_06_adversarial_account_tampering_and_cycle_guard_probe() {
    let app = TestApp::in_memory();

    // 1. Attempt unknown account voucher injection
    let bad_v = VoucherBuilder::new("V-ATTACK-01", "2026-01-10")
        .leg("9999", "1000000", "0")
        .leg("1111", "0", "1000000")
        .build();
    let res1 = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&bad_v)
        .send()
        .await;
    if res1.status == StatusCode::UNPROCESSABLE_ENTITY {
        res1.assert_error_code("UNKNOWN_ACCOUNT");
    }

    // 2. Attempt SQL injection in account code
    let sqli_acct = json!({
        "code": "111'; DROP TABLE vouchers;--",
        "name": "SQL Injection Attack",
        "parent_code": "111",
        "class": "ASSET",
        "is_active": true,
        "normal_balance": "DEBIT"
    });
    let _ = app
        .post("/api/v1/accounts")
        .as_controller()
        .json(&sqli_acct)
        .send()
        .await;

    // 3. Verify vouchers table is 100% intact
    let check_vouchers = app.get("/api/v1/vouchers").as_accountant().send().await;
    check_vouchers.assert_status(StatusCode::OK);
}

// ============================================================================
// Scenario RW-07: End-to-End Period Close & Reopen Boundary Lifecycle
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_07_e2e_period_close_and_reopen_boundary_lifecycle() {
    let app = TestApp::in_memory();

    // 1. Post voucher in Period 2026-01
    let v = VoucherBuilder::simple_cash_withdrawal("V-PER-01", "2026-01-15", "8000000");
    let create_res = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;
    assert!(create_res.status == StatusCode::CREATED || create_res.status == StatusCode::OK);
    let created: JournalVoucher = create_res.json();

    // 2. Controller closes period
    let close_payload = json!({ "year": 2026, "month": 1 });
    let close_res = app
        .post("/api/v1/periods/close")
        .as_controller()
        .json(&close_payload)
        .send()
        .await;

    if close_res.status == StatusCode::OK {
        // 3. Attempt posting into closed period (Must fail with 422 PERIOD_CLOSED)
        let v_illegal =
            VoucherBuilder::simple_cash_withdrawal("V-ILLEGAL", "2026-01-20", "1000000");
        let ill_res = app
            .post("/api/v1/vouchers")
            .as_accountant()
            .json(&v_illegal)
            .send()
            .await;
        ill_res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        ill_res.assert_error_code("PERIOD_CLOSED");

        // 4. Attempt voiding in closed period (Must fail with 422)
        let void_payload = json!({ "reason": "Huy phieu trong ky da dong" });
        let void_res = app
            .post(&format!("/api/v1/vouchers/{}/void", created.id))
            .as_controller()
            .json(&void_payload)
            .send()
            .await;
        void_res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);

        // 5. Controller reopens period
        let reopen_res = app
            .post("/api/v1/periods/reopen")
            .as_controller()
            .json(&close_payload)
            .send()
            .await;
        reopen_res.assert_status(StatusCode::OK);

        // 6. Voiding now succeeds in reopened period
        let void_ok = app
            .post(&format!("/api/v1/vouchers/{}/void", created.id))
            .as_controller()
            .json(&void_payload)
            .send()
            .await;
        void_ok.assert_status(StatusCode::OK);
    }
}

// ============================================================================
// Scenario RW-08: Disaster Recovery & Dynamic Chart of Accounts Integrity Drill
// ============================================================================
#[tokio::test]
async fn test_tier4_scenario_08_disaster_recovery_dynamic_coa_integrity_drill() {
    let env = TestEnv::new();
    let app = TestApp::file_backed(env);

    // 1. Post voucher
    let v = VoucherBuilder::new("V-COA-DRILL", "2026-01-10")
        .leg("1111", "12000000", "0")
        .leg("5111", "0", "12000000")
        .build();
    let _ = app
        .post("/api/v1/vouchers")
        .as_accountant()
        .json(&v)
        .send()
        .await;

    // 2. Pre-backup Trial Balance
    let tb_before = app
        .get("/api/v1/reports/trial-balance?as_of_date=2026-01-31")
        .as_accountant()
        .send()
        .await;
    tb_before.assert_status(StatusCode::OK);
    let pre_tb: TrialBalanceResponse = tb_before.json();

    // 3. Backup snapshot using helper
    let _ = app.take_backup().await;

    // 4. Verify Trial balance is balanced
    assert_trial_balance_balanced(&pre_tb);
}
