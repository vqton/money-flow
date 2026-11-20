use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use backend::{
    auth::{Role, generate_test_token},
    config::DEFAULT_TEST_JWT_SECRET,
    create_app,
    database::LedgerRepository,
    dto::{
        BalanceSheetResponse, IncomeStatementResponse, PeriodStatusResponse, TrialBalanceResponse,
    },
    ledger::LedgerEngine,
    models::{AuditLogEntry, JournalVoucher, VoucherStatus},
    state::AppState,
};
use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

/// Helper function to create an in-memory test application instance.
fn setup_test_app() -> axum::Router {
    let conn = Connection::open_in_memory().expect("In-memory SQLite connection failed");
    let repo = LedgerRepository::new(conn);
    repo.init_schema().expect("Schema initialization failed");
    let engine = LedgerEngine::new();
    let state = Arc::new(AppState::new(engine, repo));
    create_app(state)
}

fn test_token(sub: &str, role: Role) -> String {
    generate_test_token(sub, role, 3600, DEFAULT_TEST_JWT_SECRET)
}

fn accountant_token() -> String {
    test_token("accountant_lan", Role::Accountant)
}

fn controller_token() -> String {
    test_token("chief_controller", Role::Controller)
}

#[tokio::test]
async fn test_api_health_check() {
    let app = setup_test_app();

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json_val["status"], "ok");
    assert_eq!(json_val["version"], "0.1.0");
}

#[tokio::test]
async fn test_api_create_valid_voucher_and_persistence() {
    let app = setup_test_app();

    // 1. Post a valid balanced voucher (Cash withdrawal: Nợ 1111 / Có 1121)
    let payload = json!({
        "voucher_no": "PT-2026-0001",
        "voucher_date": "2026-01-15",
        "description": "Rút tiền gửi ngân hàng về quỹ tiền mặt",
        "invoice_ref": "HD-00123",
        "lines": [
            {
                "account_code": "1111",
                "debit": "15000000",
                "credit": "0",
                "line_desc": "Thu tiền mặt"
            },
            {
                "account_code": "1121",
                "debit": "0",
                "credit": "15000000",
                "line_desc": "Giảm tiền gửi ngân hàng VCB"
            }
        ]
    });

    let post_req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let post_res = app.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::CREATED);

    let post_bytes = axum::body::to_bytes(post_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_voucher: JournalVoucher = serde_json::from_slice(&post_bytes).unwrap();

    assert_eq!(created_voucher.voucher_no, "PT-2026-0001");
    assert_eq!(
        created_voucher.voucher_date,
        NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
    );
    assert_eq!(created_voucher.lines.len(), 2);
    assert_eq!(created_voucher.lines[0].account_code, "1111");
    assert_eq!(created_voucher.lines[0].debit, dec!(15000000));
    assert_eq!(created_voucher.lines[1].account_code, "1121");
    assert_eq!(created_voucher.lines[1].credit, dec!(15000000));

    // 2. Fetch the created voucher by ID via GET /api/v1/vouchers/{id}
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/vouchers/{}", created_voucher.id))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let get_res = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);

    let get_bytes = axum::body::to_bytes(get_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let fetched_voucher: JournalVoucher = serde_json::from_slice(&get_bytes).unwrap();

    assert_eq!(fetched_voucher.id, created_voucher.id);
    assert_eq!(fetched_voucher.voucher_no, "PT-2026-0001");
    assert_eq!(fetched_voucher.invoice_ref, Some("HD-00123".to_string()));
    assert_eq!(fetched_voucher.lines.len(), 2);
}

#[tokio::test]
async fn test_api_create_unbalanced_voucher_rejected() {
    let app = setup_test_app();

    // Mismatched debit (1,000,000) != credit (999,000) -> 1,000 VND variance
    let payload = json!({
        "voucher_no": "PKT-ERR-001",
        "voucher_date": "2026-01-15",
        "description": "Lệch Nợ Có",
        "lines": [
            {
                "account_code": "1111",
                "debit": "1000000",
                "credit": "0"
            },
            {
                "account_code": "5111",
                "debit": "0",
                "credit": "999000"
            }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(error_body["error"], "UNBALANCED_VOUCHER");
    let msg = error_body["message"].as_str().unwrap();
    assert!(msg.contains("total debit (1000000) != total credit (999000)"));
}

#[tokio::test]
async fn test_api_create_negative_amount_rejected() {
    let app = setup_test_app();

    let payload = json!({
        "voucher_no": "PKT-ERR-NEG",
        "voucher_date": "2026-01-15",
        "description": "Số tiền âm vi phạm nguyên tắc kế toán",
        "lines": [
            {
                "account_code": "1111",
                "debit": "-500000",
                "credit": "0"
            },
            {
                "account_code": "1121",
                "debit": "0",
                "credit": "-500000"
            }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(error_body["error"], "BAD_REQUEST");
    let msg = error_body["message"].as_str().unwrap();
    assert!(msg.contains("Negative amounts are forbidden"));
}

#[tokio::test]
async fn test_api_get_voucher_not_found() {
    let app = setup_test_app();
    let random_id = Uuid::new_v4();

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/vouchers/{}", random_id))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(error_body["error"], "NOT_FOUND");
    let msg = error_body["message"].as_str().unwrap();
    assert!(msg.contains(&random_id.to_string()));
}

#[tokio::test]
async fn test_api_date_range_filtering() {
    let app = setup_test_app();

    // Helper to post voucher
    async fn post_v(app: &axum::Router, no: &str, date: &str) {
        let payload = json!({
            "voucher_no": no,
            "voucher_date": date,
            "description": format!("Transaction on {}", date),
            "lines": [
                { "account_code": "1111", "debit": "1000", "credit": "0" },
                { "account_code": "1121", "debit": "0", "credit": "1000" }
            ]
        });

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/vouchers")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", accountant_token()),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    // Insert 3 vouchers on different dates
    post_v(&app, "V-2026-01-10", "2026-01-10").await;
    post_v(&app, "V-2026-01-20", "2026-01-20").await;
    post_v(&app, "V-2026-02-05", "2026-02-05").await;

    // 1. Query with date filter [2026-01-15, 2026-01-25] -> Expect only V-2026-01-20
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers?from_date=2026-01-15&to_date=2026-01-25")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Vec<JournalVoucher> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].voucher_no, "V-2026-01-20");

    // 2. Query with from_date only [>= 2026-01-15] -> Expect 2 vouchers
    let req2 = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers?from_date=2026-01-15")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let list2: Vec<JournalVoucher> = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(list2.len(), 2);

    // 3. Query all vouchers
    let req3 = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let res3 = app.clone().oneshot(req3).await.unwrap();
    let bytes3 = axum::body::to_bytes(res3.into_body(), usize::MAX)
        .await
        .unwrap();
    let list3: Vec<JournalVoucher> = serde_json::from_slice(&bytes3).unwrap();
    assert_eq!(list3.len(), 3);
}

#[tokio::test]
async fn test_api_trial_balance_calculation_and_balance_verification() {
    let app = setup_test_app();

    // 1. Initially empty trial balance
    let req_init = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/trial-balance")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let res_init = app.clone().oneshot(req_init).await.unwrap();
    assert_eq!(res_init.status(), StatusCode::OK);
    let bytes_init = axum::body::to_bytes(res_init.into_body(), usize::MAX)
        .await
        .unwrap();
    let tb_init: TrialBalanceResponse = serde_json::from_slice(&bytes_init).unwrap();
    assert!(tb_init.accounts.is_empty());
    assert_eq!(tb_init.total_debit, Decimal::ZERO);
    assert_eq!(tb_init.total_credit, Decimal::ZERO);
    assert!(tb_init.is_balanced);

    // 2. Post compound purchase voucher (1-to-many):
    let p1 = json!({
        "voucher_no": "PNK-001",
        "voucher_date": "2026-01-10",
        "description": "Nhập kho nguyên vật liệu",
        "invoice_ref": "HD-VAT-901",
        "lines": [
            { "account_code": "152", "debit": "50000000", "credit": "0" },
            { "account_code": "1331", "debit": "5000000", "credit": "0" },
            { "account_code": "331", "debit": "0", "credit": "55000000" }
        ]
    });

    let post_req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p1.to_string()))
        .unwrap();

    let post_res1 = app.clone().oneshot(post_req1).await.unwrap();
    assert_eq!(post_res1.status(), StatusCode::CREATED);

    // 3. Post bank payment voucher
    let p2 = json!({
        "voucher_no": "UNC-001",
        "voucher_date": "2026-01-12",
        "description": "Thanh toán tiền hàng qua ngân hàng VCB",
        "lines": [
            { "account_code": "331", "debit": "55000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "55000000" }
        ]
    });

    let post_req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p2.to_string()))
        .unwrap();

    let post_res2 = app.clone().oneshot(post_req2).await.unwrap();
    assert_eq!(post_res2.status(), StatusCode::CREATED);

    // 4. Query Trial Balance
    let tb_req = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/trial-balance")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let tb_res = app.clone().oneshot(tb_req).await.unwrap();
    assert_eq!(tb_res.status(), StatusCode::OK);

    let tb_bytes = axum::body::to_bytes(tb_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let tb: TrialBalanceResponse = serde_json::from_slice(&tb_bytes).unwrap();

    assert!(tb.is_balanced);
    assert_eq!(tb.total_debit, dec!(110000000));
    assert_eq!(tb.total_credit, dec!(110000000));
    assert_eq!(tb.total_debit, tb.total_credit);

    assert_eq!(tb.accounts.len(), 4);
    assert_eq!(tb.accounts[0].account_code, "1121");
    assert_eq!(tb.accounts[0].total_debit, dec!(0));
    assert_eq!(tb.accounts[0].total_credit, dec!(55000000));
    assert_eq!(tb.accounts[0].net_balance, dec!(-55000000));

    assert_eq!(tb.accounts[1].account_code, "1331");
    assert_eq!(tb.accounts[1].total_debit, dec!(5000000));
    assert_eq!(tb.accounts[1].total_credit, dec!(0));
    assert_eq!(tb.accounts[1].net_balance, dec!(5000000));

    assert_eq!(tb.accounts[2].account_code, "152");
    assert_eq!(tb.accounts[2].total_debit, dec!(50000000));
    assert_eq!(tb.accounts[2].total_credit, dec!(0));
    assert_eq!(tb.accounts[2].net_balance, dec!(50000000));

    assert_eq!(tb.accounts[3].account_code, "331");
    assert_eq!(tb.accounts[3].total_debit, dec!(55000000));
    assert_eq!(tb.accounts[3].total_credit, dec!(55000000));
    assert_eq!(tb.accounts[3].net_balance, dec!(0));

    let sum_net: Decimal = tb.accounts.iter().map(|a| a.net_balance).sum();
    assert_eq!(sum_net, Decimal::ZERO);
}

#[tokio::test]
async fn test_api_single_line_voucher_rejected() {
    let app = setup_test_app();

    let payload = json!({
        "voucher_no": "PKT-SINGLE",
        "voucher_date": "2026-01-15",
        "description": "Voucher thiếu định khoản đối ứng",
        "lines": [
            { "account_code": "1111", "debit": "500000", "credit": "0" }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(error_body["error"], "BAD_REQUEST");
    assert!(
        error_body["message"]
            .as_str()
            .unwrap()
            .contains("at least 2 lines")
    );
}

#[tokio::test]
async fn test_api_duplicate_voucher_no_rejected() {
    let app = setup_test_app();

    let payload = json!({
        "voucher_no": "PKT-DUP-01",
        "voucher_date": "2026-01-15",
        "description": "First voucher",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "1000" }
        ]
    });

    // First post should succeed
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    // Second post with duplicate voucher_no should fail
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::BAD_REQUEST);

    let bytes = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let error_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(error_body["error"], "BAD_REQUEST");
    assert!(
        error_body["message"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );
}

// ============================================================================
// Phase Epsilon Integration Tests (Circular 99 / Period Closing & Reports)
// ============================================================================

#[tokio::test]
async fn test_api_period_closing_sequence_balances_911_and_transfers_to_4212() {
    let app = setup_test_app();

    // 1. Post Gross Revenue (Nợ 1121 / Có 5111: 120,000,000)
    let p_rev = json!({
        "voucher_no": "REV-001",
        "voucher_date": "2026-01-10",
        "description": "Doanh thu ban hang",
        "lines": [
            { "account_code": "1121", "debit": "120000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "120000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_rev.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 2. Post Revenue Deduction (Nợ 5211 / Có 1121: 20,000,000)
    let p_ded = json!({
        "voucher_no": "DED-001",
        "voucher_date": "2026-01-12",
        "description": "Chiet khau thuong mai",
        "lines": [
            { "account_code": "5211", "debit": "20000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "20000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_ded.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 3. Post COGS (Nợ 632 / Có 1561: 50,000,000)
    let p_cogs = json!({
        "voucher_no": "COGS-001",
        "voucher_date": "2026-01-15",
        "description": "Gia von hang ban",
        "lines": [
            { "account_code": "632", "debit": "50000000", "credit": "0" },
            { "account_code": "1561", "debit": "0", "credit": "50000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_cogs.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 4. Post Admin Expense (Nợ 642 / Có 1121: 10,000,000)
    let p_adm = json!({
        "voucher_no": "ADM-001",
        "voucher_date": "2026-01-18",
        "description": "Chi phi quan ly",
        "lines": [
            { "account_code": "642", "debit": "10000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "10000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_adm.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 5. Post Other Expense (Nợ 811 / Có 1121: 5,000,000)
    let p_oth = json!({
        "voucher_no": "OTH-001",
        "voucher_date": "2026-01-20",
        "description": "Chi phi khac",
        "lines": [
            { "account_code": "811", "debit": "5000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "5000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_oth.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 6. Execute Period Closing: POST /api/v1/periods/close (Controller privileged)
    let close_req_body = json!({ "year": 2026, "month": 1 });
    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(close_req_body.to_string()))
        .unwrap();

    let close_res = app.clone().oneshot(close_req).await.unwrap();
    assert_eq!(close_res.status(), StatusCode::OK);

    let close_bytes = axum::body::to_bytes(close_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_res: serde_json::Value = serde_json::from_slice(&close_bytes).unwrap();
    assert_eq!(status_res["year"], 2026);
    assert_eq!(status_res["month"], 1);
    assert_eq!(status_res["is_closed"], true);

    // 7. Verify Trial Balance: Account 911 balance must be ZERO, Account 4212 must have 35M credit
    let tb_req = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/trial-balance")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let tb_res = app.clone().oneshot(tb_req).await.unwrap();
    assert_eq!(tb_res.status(), StatusCode::OK);
    let tb_bytes = axum::body::to_bytes(tb_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let tb: TrialBalanceResponse = serde_json::from_slice(&tb_bytes).unwrap();

    assert!(tb.is_balanced);

    if let Some(row_911) = tb.accounts.iter().find(|a| a.account_code == "911") {
        assert_eq!(row_911.net_balance, Decimal::ZERO);
    }

    let row_4212 = tb
        .accounts
        .iter()
        .find(|a| a.account_code == "4212")
        .expect("Account 4212 must exist");
    assert_eq!(row_4212.total_credit, dec!(35000000));
    assert_eq!(row_4212.net_balance, dec!(-35000000));
}

#[tokio::test]
async fn test_api_period_lock_enforcement_blocks_retroactive_postings() {
    let app = setup_test_app();

    // 1. Post valid transaction in Jan 2026
    let p1 = json!({
        "voucher_no": "V-JAN-01",
        "voucher_date": "2026-01-10",
        "description": "Initial Jan transaction",
        "lines": [
            { "account_code": "1111", "debit": "10000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "10000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p1.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 2. Close Jan 2026 (Controller privileged)
    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    // 3. Attempt retroactive posting in Jan 2026 -> Must return 422 Unprocessable Entity
    let p_retro = json!({
        "voucher_no": "V-JAN-LATE",
        "voucher_date": "2026-01-20",
        "description": "Retroactive posting attempt",
        "lines": [
            { "account_code": "1111", "debit": "5000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "5000000" }
        ]
    });
    let req_retro = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_retro.to_string()))
        .unwrap();

    let res_retro = app.clone().oneshot(req_retro).await.unwrap();
    assert_eq!(res_retro.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let bytes = axum::body::to_bytes(res_retro.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_json["error"], "PERIOD_CLOSED");

    // 4. Posting in open month Feb 2026 must SUCCEED
    let p_feb = json!({
        "voucher_no": "V-FEB-01",
        "voucher_date": "2026-02-05",
        "description": "Normal Feb transaction",
        "lines": [
            { "account_code": "1111", "debit": "2000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "2000000" }
        ]
    });
    let req_feb = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_feb.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req_feb).await.unwrap().status(),
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn test_api_period_reopening_protocol() {
    let app = setup_test_app();

    // 1. Post Jan transaction & close period
    let p1 = json!({
        "voucher_no": "V-JAN-01",
        "voucher_date": "2026-01-10",
        "description": "Jan sales",
        "lines": [
            { "account_code": "1111", "debit": "50000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "50000000" }
        ]
    });
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p1.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req1).await.unwrap().status(),
        StatusCode::CREATED
    );

    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    // 2. Reopen Jan 2026: POST /api/v1/periods/reopen (Controller privileged)
    let reopen_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/reopen")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "year": 2026, "month": 1, "reason": "Auditor adjustment" }).to_string(),
        ))
        .unwrap();

    let reopen_res = app.clone().oneshot(reopen_req).await.unwrap();
    assert_eq!(reopen_res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(reopen_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status_json["is_closed"], false);

    // 3. Post adjustment voucher into Jan 2026 -> Must now SUCCEED
    let p_adj = json!({
        "voucher_no": "V-JAN-ADJ",
        "voucher_date": "2026-01-25",
        "description": "Auditor adjustment entry",
        "lines": [
            { "account_code": "642", "debit": "5000000", "credit": "0" },
            { "account_code": "1111", "debit": "0", "credit": "5000000" }
        ]
    });
    let req_adj = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p_adj.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req_adj).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 4. Re-close Jan 2026 -> Net profit is now 45M (50M - 5M)
    let reclose_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(reclose_req).await.unwrap().status(),
        StatusCode::OK
    );

    let tb_req = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/trial-balance")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let tb_res = app.clone().oneshot(tb_req).await.unwrap();
    let tb_bytes = axum::body::to_bytes(tb_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let tb: TrialBalanceResponse = serde_json::from_slice(&tb_bytes).unwrap();
    let row_4212 = tb
        .accounts
        .iter()
        .find(|a| a.account_code == "4212")
        .unwrap();
    assert_eq!(row_4212.total_credit, dec!(45000000));
}

#[tokio::test]
async fn test_api_balance_sheet_contra_asset_identity() {
    let app = setup_test_app();

    // 1. Initial capital: Cash 1111 = 500M / Equity 4111 = 500M
    let p1 = json!({
        "voucher_no": "CAP-001",
        "voucher_date": "2026-01-01",
        "description": "Von gop ban dau",
        "lines": [
            { "account_code": "1111", "debit": "500000000", "credit": "0" },
            { "account_code": "4111", "debit": "0", "credit": "500000000" }
        ]
    });
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p1.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req1).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 2. Buy Fixed Asset: TK 2111 = 200M / Cash 1111 = 200M
    let p2 = json!({
        "voucher_no": "FA-001",
        "voucher_date": "2026-01-05",
        "description": "Mua may moc thiet bi",
        "lines": [
            { "account_code": "2111", "debit": "200000000", "credit": "0" },
            { "account_code": "1111", "debit": "0", "credit": "200000000" }
        ]
    });
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p2.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req2).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 3. Record Depreciation (TK 2141 credit 50M) & Provision (TK 2291 credit 10M)
    let p3 = json!({
        "voucher_no": "DEP-001",
        "voucher_date": "2026-01-20",
        "description": "Trich khau hao va du phong",
        "lines": [
            { "account_code": "642", "debit": "60000000", "credit": "0" },
            { "account_code": "2141", "debit": "0", "credit": "50000000" },
            { "account_code": "2291", "debit": "0", "credit": "10000000" }
        ]
    });
    let req3 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p3.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req3).await.unwrap().status(),
        StatusCode::CREATED
    );

    // 4. Close Jan 2026 period (Controller privileged)
    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    // 5. Query Balance Sheet: GET /api/v1/reports/balance-sheet?as_of_date=2026-01-31
    let bs_req = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/balance-sheet?as_of_date=2026-01-31")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let bs_res = app.clone().oneshot(bs_req).await.unwrap();
    assert_eq!(bs_res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(bs_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let bs: BalanceSheetResponse = serde_json::from_slice(&bytes).unwrap();

    // Verify contra-asset deductions
    assert_eq!(bs.current_assets, dec!(300000000));
    assert_eq!(bs.non_current_assets, dec!(200000000));
    assert_eq!(bs.accumulated_depreciation, dec!(50000000));
    assert_eq!(bs.asset_provisions, dec!(10000000));

    // Total Assets = 300M + 200M - 50M - 10M = 440,000,000
    assert_eq!(bs.total_assets, dec!(440000000));
    // Total Resources = Liabilities (0) + Equity (500M - 60M = 440M) = 440,000,000
    assert_eq!(bs.total_resources, dec!(440000000));
    assert!(bs.is_balanced);
}

#[tokio::test]
async fn test_api_income_statement_calculations() {
    let app = setup_test_app();

    // Post sample transactions
    // Gross Rev: 120M
    let p1 = json!({
        "voucher_no": "P1", "voucher_date": "2026-01-05", "description": "Doanh thu",
        "lines": [
            { "account_code": "1121", "debit": "120000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "120000000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p1.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    // Deduction: 20M
    let p2 = json!({
        "voucher_no": "P2", "voucher_date": "2026-01-08", "description": "Giam tru DT",
        "lines": [
            { "account_code": "5211", "debit": "20000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "20000000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p2.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    // COGS: 50M
    let p3 = json!({
        "voucher_no": "P3", "voucher_date": "2026-01-10", "description": "Gia von",
        "lines": [
            { "account_code": "632", "debit": "50000000", "credit": "0" },
            { "account_code": "1561", "debit": "0", "credit": "50000000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p3.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    // Financial Income 515: 5M
    let p4 = json!({
        "voucher_no": "P4", "voucher_date": "2026-01-15", "description": "Lai tien gui",
        "lines": [
            { "account_code": "1121", "debit": "5000000", "credit": "0" },
            { "account_code": "515", "debit": "0", "credit": "5000000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p4.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    // Admin Expense: 10M
    let p5 = json!({
        "voucher_no": "P5", "voucher_date": "2026-01-20", "description": "Chi phi QLDN",
        "lines": [
            { "account_code": "642", "debit": "10000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "10000000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p5.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    // Other Expense: 5M
    let p6 = json!({
        "voucher_no": "P6", "voucher_date": "2026-01-22", "description": "Chi phi khac",
        "lines": [
            { "account_code": "811", "debit": "5000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "5000000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p6.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    // Query Income Statement
    let is_req = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/income-statement?from_date=2026-01-01&to_date=2026-01-31")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let is_res = app.clone().oneshot(is_req).await.unwrap();
    assert_eq!(is_res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(is_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let is: IncomeStatementResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(is.gross_revenue, dec!(120000000));
    assert_eq!(is.revenue_deductions, dec!(20000000));
    assert_eq!(is.net_revenue, dec!(100000000));
    assert_eq!(is.cost_of_goods_sold, dec!(50000000));
    assert_eq!(is.gross_profit, dec!(50000000));
    assert_eq!(is.financial_income, dec!(5000000));
    assert_eq!(is.administrative_expenses, dec!(10000000));
    assert_eq!(is.operating_profit, dec!(45000000));
    assert_eq!(is.other_expenses, dec!(5000000));
    assert_eq!(is.net_profit_before_tax, dec!(40000000));
    assert_eq!(is.net_profit_after_tax, dec!(40000000));

    // Close period and verify income statement is invariant (Controller privileged)
    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    let is_req2 = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/income-statement?from_date=2026-01-01&to_date=2026-01-31")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let is_res2 = app.clone().oneshot(is_req2).await.unwrap();
    assert_eq!(is_res2.status(), StatusCode::OK);

    let bytes2 = axum::body::to_bytes(is_res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let is2: IncomeStatementResponse = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(is2.net_profit_after_tax, dec!(40000000));
}

#[tokio::test]
async fn test_api_list_periods() {
    let app = setup_test_app();

    // Initially no closed periods
    let req1 = Request::builder()
        .method("GET")
        .uri("/api/v1/periods")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let list1: Vec<PeriodStatusResponse> = serde_json::from_slice(&bytes1).unwrap();
    assert!(list1.is_empty());

    // Post transaction and close Jan 2026
    let p = json!({
        "voucher_no": "V-INIT", "voucher_date": "2026-01-10", "description": "Init",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "1000" }
        ]
    });
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", accountant_token())
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(p.to_string()))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    // List periods again
    let req2 = Request::builder()
        .method("GET")
        .uri("/api/v1/periods")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let list2: Vec<PeriodStatusResponse> = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(list2.len(), 1);
    assert_eq!(list2[0].year, 2026);
    assert_eq!(list2[0].month, 1);
    assert!(list2[0].is_closed);
}

#[tokio::test]
async fn test_api_system_backup_default_and_custom() {
    let temp_backup_dir = std::env::temp_dir().join(format!("api_backup_test_{}", Uuid::new_v4()));
    let conn = Connection::open_in_memory().expect("In-memory SQLite connection failed");
    let repo = LedgerRepository::new(conn);
    repo.init_schema().expect("Schema initialization failed");
    let engine = LedgerEngine::new();
    let config = backend::config::AppConfig {
        backup_dir: temp_backup_dir.to_str().unwrap().to_string(),
        ..Default::default()
    };
    let state = Arc::new(AppState::with_config(engine, repo, config));
    let app = create_app(state);

    // 1. POST /api/v1/system/backup with empty payload -> defaults to timestamped filename (Controller privileged)
    let req1 = Request::builder()
        .method("POST")
        .uri("/api/v1/system/backup")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::empty())
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp1: backend::dto::BackupResponse = serde_json::from_slice(&bytes1).unwrap();
    assert_eq!(resp1.status, "success");
    assert!(resp1.file_size_bytes > 0);
    assert!(std::path::Path::new(&resp1.backup_path).exists());

    // 2. POST /api/v1/system/backup with custom filename (Controller privileged)
    let custom_name = "custom_snapshot.db";
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/system/backup")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "filename": custom_name }).to_string()))
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CREATED);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp2: backend::dto::BackupResponse = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(resp2.status, "success");
    assert!(resp2.backup_path.ends_with(custom_name));
    assert!(std::path::Path::new(&resp2.backup_path).exists());

    // 3. POST /api/v1/system/backup with directory traversal attack -> 400 Bad Request
    let malicious = "../../../windows/system32/evil.db";
    let req3 = Request::builder()
        .method("POST")
        .uri("/api/v1/system/backup")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "filename": malicious }).to_string()))
        .unwrap();
    let res3 = app.clone().oneshot(req3).await.unwrap();
    assert_eq!(res3.status(), StatusCode::BAD_REQUEST);

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_backup_dir);
}

#[tokio::test]
async fn test_api_openapi_spec_and_swagger_ui() {
    let app = setup_test_app();

    // GET /api-docs/openapi.json (Public, unauthenticated)
    let req1 = Request::builder()
        .method("GET")
        .uri("/api-docs/openapi.json")
        .body(Body::empty())
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let spec: serde_json::Value = serde_json::from_slice(&bytes1).unwrap();
    assert_eq!(spec["openapi"], "3.1.0");
    assert_eq!(spec["info"]["title"], "VAS Enterprise Accounting API");
    assert!(spec["paths"]["/api/v1/system/backup"].is_object());

    // GET /swagger-ui (Public, unauthenticated)
    let req2 = Request::builder()
        .method("GET")
        .uri("/swagger-ui")
        .body(Body::empty())
        .unwrap();
    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(bytes2.to_vec()).unwrap();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("/api-docs/openapi.json"));
}

#[tokio::test]
async fn test_api_voucher_user_attribution() {
    let app = setup_test_app();

    // Post voucher with explicit token sub ("accountant_lan")
    let payload = json!({
        "voucher_no": "PKT-ATTR-01",
        "voucher_date": "2026-01-15",
        "description": "Office rent payment with attribution",
        "invoice_ref": "INV-RENT-01",
        "created_by": "ignored_body_user",
        "approved_by": "chief_acc_hung",
        "lines": [
            { "account_code": "642", "debit": "20000000", "credit": "0", "line_desc": "Rent expense" },
            { "account_code": "1121", "debit": "0", "credit": "20000000", "line_desc": "Bank transfer" }
        ]
    });

    let post_req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let post_res = app.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(post_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(voucher.status, VoucherStatus::Posted);
    assert_eq!(voucher.created_by, "accountant_lan");
    assert_eq!(voucher.approved_by, Some("chief_acc_hung".to_string()));
    assert_eq!(voucher.lines.len(), 2);
    assert_eq!(voucher.lines[0].line_no, 1);
    assert_eq!(voucher.lines[1].line_no, 2);

    // Fetch by ID and verify persistence
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/vouchers/{}", voucher.id))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();

    let get_res = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_bytes = axum::body::to_bytes(get_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let fetched: JournalVoucher = serde_json::from_slice(&get_bytes).unwrap();

    assert_eq!(fetched.created_by, "accountant_lan");
    assert_eq!(fetched.approved_by, Some("chief_acc_hung".to_string()));
    assert_eq!(fetched.lines[0].line_no, 1);
    assert_eq!(fetched.lines[1].line_no, 2);
}

#[tokio::test]
async fn test_api_void_voucher_and_report_exclusion() {
    let app = setup_test_app();

    // 1. Post voucher
    let payload = json!({
        "voucher_no": "PKT-VOID-TEST",
        "voucher_date": "2026-01-18",
        "description": "Erroneous posting to be voided",
        "lines": [
            { "account_code": "1111", "debit": "7500000", "credit": "0" },
            { "account_code": "711", "debit": "0", "credit": "7500000" }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    // 2. Verify voucher is present in Trial Balance
    let tb_req = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/trial-balance")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let tb_res = app.clone().oneshot(tb_req).await.unwrap();
    let tb_bytes = axum::body::to_bytes(tb_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let tb_val: TrialBalanceResponse = serde_json::from_slice(&tb_bytes).unwrap();
    assert_eq!(tb_val.total_debit, dec!(7500000));

    // 3. Void the voucher: POST /api/v1/vouchers/{id}/void (Controller token with sub: cfo_minh)
    let cfo_token = test_token("cfo_minh", Role::Controller);
    let void_payload = json!({
        "reason": "Customer payment was already deposited via bank transfer",
        "actor": "spoofed_actor"
    });
    let void_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/vouchers/{}/void", created.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", cfo_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(void_payload.to_string()))
        .unwrap();
    let void_res = app.clone().oneshot(void_req).await.unwrap();
    assert_eq!(void_res.status(), StatusCode::OK);
    let void_bytes = axum::body::to_bytes(void_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voided_voucher: JournalVoucher = serde_json::from_slice(&void_bytes).unwrap();

    assert_eq!(voided_voucher.status, VoucherStatus::Voided);
    assert_eq!(voided_voucher.voided_by, Some("cfo_minh".to_string()));
    assert_eq!(
        voided_voucher.void_reason,
        Some("Customer payment was already deposited via bank transfer".to_string())
    );
    assert!(voided_voucher.voided_at.is_some());

    // 4. Verify Trial Balance strictly excludes the voided voucher
    let tb_req2 = Request::builder()
        .method("GET")
        .uri("/api/v1/reports/trial-balance")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let tb_res2 = app.clone().oneshot(tb_req2).await.unwrap();
    let tb_bytes2 = axum::body::to_bytes(tb_res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let tb_val2: TrialBalanceResponse = serde_json::from_slice(&tb_bytes2).unwrap();
    assert_eq!(tb_val2.total_debit, dec!(0));
    assert!(tb_val2.accounts.is_empty());

    // 5. Verify list_vouchers excludes voided voucher
    let list_req = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let list_res = app.clone().oneshot(list_req).await.unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_vouchers: Vec<JournalVoucher> = serde_json::from_slice(&list_bytes).unwrap();
    assert!(list_vouchers.is_empty());

    // 6. Direct lookup by ID still returns the voided voucher for historical audit
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/vouchers/{}", created.id))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .body(Body::empty())
        .unwrap();
    let get_res = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_bytes = axum::body::to_bytes(get_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let fetched: JournalVoucher = serde_json::from_slice(&get_bytes).unwrap();
    assert_eq!(fetched.status, VoucherStatus::Voided);

    // 7. Attempting to void again returns 400 VOUCHER_ALREADY_VOIDED
    let void_again_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/vouchers/{}/void", created.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", cfo_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(void_payload.to_string()))
        .unwrap();
    let void_again_res = app.oneshot(void_again_req).await.unwrap();
    assert_eq!(void_again_res.status(), StatusCode::BAD_REQUEST);
    let err_bytes = axum::body::to_bytes(void_again_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_val: serde_json::Value = serde_json::from_slice(&err_bytes).unwrap();
    assert_eq!(err_val["error"], "VOUCHER_ALREADY_VOIDED");
}

#[tokio::test]
async fn test_api_cannot_void_in_closed_period() {
    let app = setup_test_app();

    // 1. Post Jan voucher & close Jan
    let payload = json!({
        "voucher_no": "V-CLOSED-TEST",
        "voucher_date": "2026-01-10",
        "description": "Jan transaction",
        "lines": [
            { "account_code": "1111", "debit": "1000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "1000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    // 2. Attempt to void voucher in closed period -> HTTP 422 CANNOT_VOID_IN_CLOSED_PERIOD
    let void_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/vouchers/{}/void", voucher.id))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", controller_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "reason": "Retroactive change" }).to_string(),
        ))
        .unwrap();

    let void_res = app.oneshot(void_req).await.unwrap();
    assert_eq!(void_res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let err_bytes = axum::body::to_bytes(void_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&err_bytes).unwrap();
    assert_eq!(err_json["error"], "CANNOT_VOID_IN_CLOSED_PERIOD");
}

#[tokio::test]
async fn test_api_audit_log_capture() {
    let app = setup_test_app();

    let acc_token = test_token("accountant_lan", Role::Accountant);
    let ctrl_token = test_token("chief_acc_hung", Role::Controller);

    // 1. CREATE_VOUCHER event
    let payload = json!({
        "voucher_no": "PKT-LOG-01",
        "voucher_date": "2026-01-10",
        "description": "Log generation test",
        "lines": [
            { "account_code": "1111", "debit": "10000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "10000000" }
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, format!("Bearer {}", acc_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    // 2. VOID_VOUCHER event (actor derived from token sub)
    let void_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/vouchers/{}/void", created.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", ctrl_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "reason": "Test voiding" }).to_string()))
        .unwrap();
    let void_res = app.clone().oneshot(void_req).await.unwrap();
    assert_eq!(void_res.status(), StatusCode::OK);

    // 3. PERIOD_CLOSE event
    let p2 = json!({
        "voucher_no": "PKT-LOG-02",
        "voucher_date": "2026-01-12",
        "description": "Sales",
        "lines": [
            { "account_code": "1111", "debit": "20000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "20000000" }
        ]
    });
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, format!("Bearer {}", acc_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(p2.to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req2).await.unwrap().status(),
        StatusCode::CREATED
    );

    let close_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(header::AUTHORIZATION, format!("Bearer {}", ctrl_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(close_req).await.unwrap().status(),
        StatusCode::OK
    );

    // 4. PERIOD_REOPEN event
    let reopen_req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/reopen")
        .header(header::AUTHORIZATION, format!("Bearer {}", ctrl_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(reopen_req).await.unwrap().status(),
        StatusCode::OK
    );

    // 5. Query GET /api/v1/audit-logs
    let log_req = Request::builder()
        .method("GET")
        .uri("/api/v1/audit-logs")
        .header(header::AUTHORIZATION, format!("Bearer {}", acc_token))
        .body(Body::empty())
        .unwrap();
    let log_res = app.clone().oneshot(log_req).await.unwrap();
    assert_eq!(log_res.status(), StatusCode::OK);
    let log_bytes = axum::body::to_bytes(log_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let logs: Vec<AuditLogEntry> = serde_json::from_slice(&log_bytes).unwrap();

    let actions: Vec<&str> = logs.iter().map(|l| l.action.as_str()).collect();
    assert!(actions.contains(&"PERIOD_REOPEN"));
    assert!(actions.contains(&"PERIOD_CLOSE"));
    assert!(actions.contains(&"VOID_VOUCHER"));
    assert!(actions.contains(&"CREATE_VOUCHER"));

    // 6. Query with filter ?action=VOID_VOUCHER
    let filtered_req = Request::builder()
        .method("GET")
        .uri("/api/v1/audit-logs?action=VOID_VOUCHER")
        .header(header::AUTHORIZATION, format!("Bearer {}", acc_token))
        .body(Body::empty())
        .unwrap();
    let filtered_res = app.oneshot(filtered_req).await.unwrap();
    assert_eq!(filtered_res.status(), StatusCode::OK);
    let f_bytes = axum::body::to_bytes(filtered_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let f_logs: Vec<AuditLogEntry> = serde_json::from_slice(&f_bytes).unwrap();
    assert_eq!(f_logs.len(), 1);
    assert_eq!(f_logs[0].action, "VOID_VOUCHER");
    assert_eq!(f_logs[0].actor, "chief_acc_hung");
    assert_eq!(f_logs[0].details, Some("Test voiding".to_string()));
}

// ============================================================================
// Phase 1 Modernization Tests (R3 Auth, RBAC & Server-Derived Attribution)
// ============================================================================

#[tokio::test]
async fn test_api_auth_missing_token_returns_401() {
    let app = setup_test_app();

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"], "UNAUTHORIZED");
    assert!(
        err["message"]
            .as_str()
            .unwrap()
            .contains("Missing Authorization header")
    );
}

#[tokio::test]
async fn test_api_auth_malformed_header_returns_401() {
    let app = setup_test_app();

    // 1. Non-Bearer scheme
    let req1 = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, "Basic dXNlcjpwYXNz")
        .body(Body::empty())
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::UNAUTHORIZED);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let err1: serde_json::Value = serde_json::from_slice(&bytes1).unwrap();
    assert_eq!(err1["error"], "UNAUTHORIZED");

    // 2. Empty Bearer token
    let req2 = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, "Bearer    ")
        .body(Body::empty())
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::UNAUTHORIZED);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let err2: serde_json::Value = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(err2["error"], "UNAUTHORIZED");
    assert!(err2["message"].as_str().unwrap().contains("Token is empty"));

    // 3. Corrupt token string
    let req3 = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, "Bearer not.a.valid.jwt")
        .body(Body::empty())
        .unwrap();

    let res3 = app.oneshot(req3).await.unwrap();
    assert_eq!(res3.status(), StatusCode::UNAUTHORIZED);
    let bytes3 = axum::body::to_bytes(res3.into_body(), usize::MAX)
        .await
        .unwrap();
    let err3: serde_json::Value = serde_json::from_slice(&bytes3).unwrap();
    assert_eq!(err3["error"], "UNAUTHORIZED");
    assert!(err3["message"].as_str().unwrap().contains("Invalid token"));
}

#[tokio::test]
async fn test_api_auth_invalid_signature_returns_401() {
    let app = setup_test_app();

    // Token signed with untrusted secret
    let forged_token = generate_test_token(
        "forger",
        Role::Controller,
        3600,
        "untrusted-secret-key-at-least-32-bytes-long",
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, format!("Bearer {forged_token}"))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"], "UNAUTHORIZED");
    assert!(err["message"].as_str().unwrap().contains("Invalid token"));
}

#[tokio::test]
async fn test_api_auth_expired_token_returns_401() {
    let app = setup_test_app();

    // Token expired 1 hour ago
    let expired_token = generate_test_token(
        "expired_user",
        Role::Controller,
        -3600,
        DEFAULT_TEST_JWT_SECRET,
    );

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/vouchers")
        .header(header::AUTHORIZATION, format!("Bearer {expired_token}"))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"], "UNAUTHORIZED");
    assert_eq!(err["message"], "Token has expired");
}

#[tokio::test]
async fn test_api_rbac_accountant_forbidden_on_void() {
    let app = setup_test_app();

    // 1. Create a voucher
    let payload = json!({
        "voucher_no": "PKT-RBAC-01",
        "voucher_date": "2026-01-15",
        "description": "RBAC void test",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "1000" }
        ]
    });
    let post_req = Request::builder()
        .method("POST")
        .uri("/api/v1/vouchers")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let post_res = app.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(post_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    // 2. Attempt void with Accountant token -> 403 Forbidden
    let void_req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/vouchers/{}/void", voucher.id))
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "reason": "Unauthorized void attempt" }).to_string(),
        ))
        .unwrap();
    let void_res = app.oneshot(void_req).await.unwrap();
    assert_eq!(void_res.status(), StatusCode::FORBIDDEN);

    let err_bytes = axum::body::to_bytes(void_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&err_bytes).unwrap();
    assert_eq!(err["error"], "FORBIDDEN");
    assert!(
        err["message"]
            .as_str()
            .unwrap()
            .contains("Controller role required")
    );
}

#[tokio::test]
async fn test_api_rbac_accountant_forbidden_on_close() {
    let app = setup_test_app();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/close")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"], "FORBIDDEN");
}

#[tokio::test]
async fn test_api_rbac_accountant_forbidden_on_reopen() {
    let app = setup_test_app();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/periods/reopen")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"], "FORBIDDEN");
}

#[tokio::test]
async fn test_api_rbac_accountant_forbidden_on_backup() {
    let app = setup_test_app();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/backup")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", accountant_token()),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err["error"], "FORBIDDEN");
}

#[tokio::test]
async fn test_api_rbac_controller_allowed_on_privileged_ops() {
    let temp_backup_dir =
        std::env::temp_dir().join(format!("api_rbac_ctrl_test_{}", Uuid::new_v4()));
    let conn = Connection::open_in_memory().expect("In-memory SQLite connection failed");
    let repo = LedgerRepository::new(conn);
    repo.init_schema().expect("Schema initialization failed");
    let engine = LedgerEngine::new();
    let config = backend::config::AppConfig {
        backup_dir: temp_backup_dir.to_str().unwrap().to_string(),
        ..Default::default()
    };
    let state = Arc::new(AppState::with_config(engine, repo, config));
    let app = create_app(state);

    let ctrl_tok = controller_token();

    // 1. Post a revenue voucher so period closing has positive activity
    let p_rev = json!({
        "voucher_no": "PKT-REV-01",
        "voucher_date": "2026-01-10",
        "description": "Revenue for period",
        "lines": [
            { "account_code": "1111", "debit": "10000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "10000" }
        ]
    });
    let post_rev_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(p_rev.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_rev_res.status(), StatusCode::CREATED);

    // 2. Post a separate voucher to test voiding
    let payload = json!({
        "voucher_no": "PKT-CTRL-01",
        "voucher_date": "2026-01-15",
        "description": "Controller test voucher to void",
        "lines": [
            { "account_code": "1111", "debit": "5000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "5000" }
        ]
    });
    let post_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_res.status(), StatusCode::CREATED);
    let v_bytes = axum::body::to_bytes(post_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&v_bytes).unwrap();

    // 3. Controller can void
    let void_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/vouchers/{}/void", voucher.id))
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "reason": "Controller void test" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(void_res.status(), StatusCode::OK);

    // 4. Controller can close period
    let close_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/periods/close")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(close_res.status(), StatusCode::OK);

    // 5. Controller can reopen period
    let reopen_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/periods/reopen")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "year": 2026, "month": 1 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reopen_res.status(), StatusCode::OK);

    // 6. Controller can perform system backup
    let backup_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/backup")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(backup_res.status(), StatusCode::CREATED);

    let _ = std::fs::remove_dir_all(temp_backup_dir);
}

#[tokio::test]
async fn test_api_voucher_created_by_derived_from_jwt_sub_ignores_body() {
    let app = setup_test_app();

    // Attacker tries to set created_by to "admin" or "cfo" in the JSON payload
    let token = test_token("verified_accountant_quang", Role::Accountant);
    let payload = json!({
        "voucher_no": "PKT-SPOOF-01",
        "voucher_date": "2026-01-15",
        "description": "Anti-spoofing creation test",
        "created_by": "malicious_spoofed_admin",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "1000" }
        ]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    // Must be derived from token sub, NOT the JSON payload
    assert_eq!(voucher.created_by, "verified_accountant_quang");

    // Verify audit log also captures verified sub
    let log_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/audit-logs?resource_id={}",
                    voucher.voucher_no
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(log_res.status(), StatusCode::OK);
    let log_bytes = axum::body::to_bytes(log_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let logs: Vec<AuditLogEntry> = serde_json::from_slice(&log_bytes).unwrap();
    assert!(!logs.is_empty());
    assert_eq!(logs[0].actor, "verified_accountant_quang");
}

#[tokio::test]
async fn test_api_void_actor_derived_from_jwt_sub_ignores_body() {
    let app = setup_test_app();

    let acc_token = test_token("staff_user", Role::Accountant);
    let ctrl_token = test_token("verified_controller_linh", Role::Controller);

    // 1. Post voucher
    let payload = json!({
        "voucher_no": "PKT-SPOOF-02",
        "voucher_date": "2026-01-15",
        "description": "Anti-spoofing void test",
        "lines": [
            { "account_code": "1111", "debit": "2000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "2000" }
        ]
    });
    let post_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {acc_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_res.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(post_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&bytes).unwrap();

    // 2. Void voucher with spoofed actor in JSON body
    let void_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/vouchers/{}/void", voucher.id))
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "reason": "Test void", "actor": "fake_cfo_identity" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(void_res.status(), StatusCode::OK);
    let void_bytes = axum::body::to_bytes(void_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voided: JournalVoucher = serde_json::from_slice(&void_bytes).unwrap();

    // Must be derived from controller token sub
    assert_eq!(
        voided.voided_by,
        Some("verified_controller_linh".to_string())
    );

    // Verify audit log also has verified actor
    let log_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/audit-logs?action=VOID_VOUCHER")
                .header(header::AUTHORIZATION, format!("Bearer {acc_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(log_res.status(), StatusCode::OK);
    let log_bytes = axum::body::to_bytes(log_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let logs: Vec<AuditLogEntry> = serde_json::from_slice(&log_bytes).unwrap();
    assert_eq!(logs[0].actor, "verified_controller_linh");
}

#[tokio::test]
async fn test_api_public_endpoints_accessible_without_auth() {
    let app = setup_test_app();

    // Root GET /
    let res_root = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_root.status(), StatusCode::OK);

    // Health GET /health
    let res_health = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_health.status(), StatusCode::OK);

    // OpenAPI JSON GET /api-docs/openapi.json
    let res_openapi = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_openapi.status(), StatusCode::OK);

    // Swagger UI GET /swagger-ui
    let res_swagger = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/swagger-ui")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_swagger.status(), StatusCode::OK);
}

// ============================================================================
// Phase 2 Option A: Idempotency-Key Integration Test Suite (AC1–AC6 + Edges)
// ============================================================================

/// AC1: First request with valid Idempotency-Key creates voucher,
/// creates audit log, and returns 201 Created.
#[tokio::test]
async fn test_api_idempotency_ac1_first_request_success() {
    let app = setup_test_app();
    let token = accountant_token();

    let payload = json!({
        "voucher_no": "PKT-IDEMP-AC1",
        "voucher_date": "2026-02-15",
        "description": "Idempotency AC1 initial post",
        "lines": [
            { "account_code": "1111", "debit": "10000000", "credit": "0", "line_desc": "Debit cash" },
            { "account_code": "5111", "debit": "0", "credit": "10000000", "line_desc": "Credit revenue" }
        ]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "ac1-key-0001")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let voucher: JournalVoucher = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(voucher.voucher_no, "PKT-IDEMP-AC1");

    // Verify voucher exists
    let list_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let vouchers: Vec<JournalVoucher> = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(vouchers.len(), 1);

    // Verify exactly one CREATE_VOUCHER audit log was captured
    let audit_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/audit-logs?action=CREATE_VOUCHER")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_bytes = axum::body::to_bytes(audit_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let audits: Vec<AuditLogEntry> = serde_json::from_slice(&audit_bytes).unwrap();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].resource_id.as_deref(), Some("PKT-IDEMP-AC1"));
}

/// AC2: Retry with same key and identical payload returns cached 201 Created
/// without duplicate vouchers or duplicate audit logs.
#[tokio::test]
async fn test_api_idempotency_ac2_retry_identical_payload_returns_cached_201() {
    let app = setup_test_app();
    let token = accountant_token();

    let payload = json!({
        "voucher_no": "PKT-IDEMP-AC2",
        "voucher_date": "2026-02-15",
        "description": "Idempotency AC2 replay test",
        "lines": [
            { "account_code": "1111", "debit": "25000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "25000000" }
        ]
    });

    // Request 1
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "ac2-key-replay")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let v1: JournalVoucher = serde_json::from_slice(&bytes1).unwrap();

    // Request 2 (Retry with same key and identical payload)
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "ac2-key-replay")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::CREATED);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let v2: JournalVoucher = serde_json::from_slice(&bytes2).unwrap();

    // Vouchers must be identical (including ID and voucher_no)
    assert_eq!(v1.id, v2.id);
    assert_eq!(v1.voucher_no, v2.voucher_no);

    // Verify vouchers table contains exactly 1 row (no duplicate)
    let list_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let vouchers: Vec<JournalVoucher> = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(vouchers.len(), 1);

    // Verify audit logs contains exactly 1 CREATE_VOUCHER entry (no duplicate audit log)
    let audit_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/audit-logs?action=CREATE_VOUCHER")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_bytes = axum::body::to_bytes(audit_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let audits: Vec<AuditLogEntry> = serde_json::from_slice(&audit_bytes).unwrap();
    assert_eq!(audits.len(), 1);
}

/// AC3: Reuse of key with mismatched payload returns 409 Conflict
/// without modifying vouchers or audit logs.
#[tokio::test]
async fn test_api_idempotency_ac3_retry_mismatched_payload_returns_409_conflict() {
    let app = setup_test_app();
    let token = accountant_token();

    let payload1 = json!({
        "voucher_no": "PKT-IDEMP-AC3",
        "voucher_date": "2026-02-15",
        "description": "Original transaction",
        "lines": [
            { "account_code": "1111", "debit": "10000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "10000000" }
        ]
    });

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "ac3-conflict-key")
                .body(Body::from(payload1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    // Request 2 with same key but altered payload (amount changed to 20,000,000)
    let payload2 = json!({
        "voucher_no": "PKT-IDEMP-AC3",
        "voucher_date": "2026-02-15",
        "description": "Altered conflicting transaction",
        "lines": [
            { "account_code": "1111", "debit": "20000000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "20000000" }
        ]
    });

    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "ac3-conflict-key")
                .body(Body::from(payload2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::CONFLICT);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(err_json["error"], "CONFLICT");

    // Verify voucher table still has only 1 voucher
    let list_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let vouchers: Vec<JournalVoucher> = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(vouchers.len(), 1);
    assert_eq!(vouchers[0].description, "Original transaction");

    // Verify audit logs has only 1 entry
    let audit_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/audit-logs?action=CREATE_VOUCHER")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_bytes = axum::body::to_bytes(audit_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let audits: Vec<AuditLogEntry> = serde_json::from_slice(&audit_bytes).unwrap();
    assert_eq!(audits.len(), 1);
}

/// AC4: Requests with invalid Idempotency-Key (empty, whitespace-only, >255 chars) return 400 Bad Request.
#[tokio::test]
async fn test_api_idempotency_ac4_invalid_key_formats_rejected_with_400() {
    let app = setup_test_app();
    let token = accountant_token();

    let payload = json!({
        "voucher_no": "PKT-IDEMP-AC4",
        "voucher_date": "2026-02-15",
        "description": "Invalid key test",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "1000" }
        ]
    });

    let overlong_key = "a".repeat(256);
    let invalid_keys = vec![
        "",            // Empty
        "   ",         // Whitespace only
        "   \t   ",    // Whitespace only (tabs/spaces)
        &overlong_key, // Overlong (> 255 chars)
    ];

    for key in invalid_keys {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vouchers")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("Idempotency-Key", key)
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "Failed for key: {:?}",
            key
        );
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let err: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(err["error"], "BAD_REQUEST");
    }

    // Non-ASCII byte rejected with 400
    let res_non_ascii = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    "Idempotency-Key",
                    axum::http::HeaderValue::from_bytes(b"bad\x80key").unwrap(),
                )
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_non_ascii.status(), StatusCode::BAD_REQUEST);

    // Boundary valid keys: 1 character and 255 characters
    let valid_len_1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "x")
                .body(Body::from(
                    json!({
                        "voucher_no": "PKT-IDEMP-AC4-B1",
                        "voucher_date": "2026-02-15",
                        "description": "Boundary 1 char",
                        "lines": [
                            { "account_code": "1111", "debit": "1000", "credit": "0" },
                            { "account_code": "1121", "debit": "0", "credit": "1000" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid_len_1.status(), StatusCode::CREATED);

    let valid_len_255 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "k".repeat(255))
                .body(Body::from(
                    json!({
                        "voucher_no": "PKT-IDEMP-AC4-B255",
                        "voucher_date": "2026-02-15",
                        "description": "Boundary 255 chars",
                        "lines": [
                            { "account_code": "1111", "debit": "1000", "credit": "0" },
                            { "account_code": "1121", "debit": "0", "credit": "1000" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid_len_255.status(), StatusCode::CREATED);
}

/// AC5: Requests without Idempotency-Key header succeed normally without error.
#[tokio::test]
async fn test_api_idempotency_ac5_omitted_key_succeeds_normally() {
    let app = setup_test_app();
    let token = accountant_token();

    let payload1 = json!({
        "voucher_no": "PKT-NO-IDEMP-1",
        "voucher_date": "2026-02-15",
        "description": "No header request 1",
        "lines": [
            { "account_code": "1111", "debit": "5000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "5000" }
        ]
    });

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);

    let payload2 = json!({
        "voucher_no": "PKT-NO-IDEMP-2",
        "voucher_date": "2026-02-15",
        "description": "No header request 2",
        "lines": [
            { "account_code": "1111", "debit": "7000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "7000" }
        ]
    });

    let res2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::CREATED);
}

/// AC6: Two distinct authenticated actors (sub) using the same Idempotency-Key do not collide.
#[tokio::test]
async fn test_api_idempotency_ac6_multi_actor_isolation_no_collision() {
    let app = setup_test_app();
    let token_lan = test_token("accountant_lan", Role::Accountant);
    let token_hoa = test_token("accountant_hoa", Role::Accountant);
    let shared_key = "shared-batch-key-999";

    let payload_lan = json!({
        "voucher_no": "PKT-LAN-001",
        "voucher_date": "2026-02-15",
        "description": "Lan's voucher with shared key",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "1000" }
        ]
    });

    let payload_hoa = json!({
        "voucher_no": "PKT-HOA-001",
        "voucher_date": "2026-02-15",
        "description": "Hoa's voucher with same shared key",
        "lines": [
            { "account_code": "1111", "debit": "2000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "2000" }
        ]
    });

    // Lan posts with shared key -> 201 Created
    let res_lan = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token_lan}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", shared_key)
                .body(Body::from(payload_lan.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_lan.status(), StatusCode::CREATED);

    // Hoa posts with identical shared key -> MUST NOT return 409 Conflict; must succeed 201 Created
    let res_hoa = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token_hoa}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", shared_key)
                .body(Body::from(payload_hoa.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_hoa.status(), StatusCode::CREATED);

    // Retrying for Lan returns Lan's cached voucher
    let retry_lan = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token_lan}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", shared_key)
                .body(Body::from(payload_lan.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry_lan.status(), StatusCode::CREATED);
    let bytes_lan = axum::body::to_bytes(retry_lan.into_body(), usize::MAX)
        .await
        .unwrap();
    let v_lan: JournalVoucher = serde_json::from_slice(&bytes_lan).unwrap();
    assert_eq!(v_lan.voucher_no, "PKT-LAN-001");
    assert_eq!(v_lan.created_by, "accountant_lan");

    // Retrying for Hoa returns Hoa's cached voucher
    let retry_hoa = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token_hoa}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", shared_key)
                .body(Body::from(payload_hoa.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry_hoa.status(), StatusCode::CREATED);
    let bytes_hoa = axum::body::to_bytes(retry_hoa.into_body(), usize::MAX)
        .await
        .unwrap();
    let v_hoa: JournalVoucher = serde_json::from_slice(&bytes_hoa).unwrap();
    assert_eq!(v_hoa.voucher_no, "PKT-HOA-001");
    assert_eq!(v_hoa.created_by, "accountant_hoa");

    // Total vouchers must be 2
    let list_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {token_lan}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let vouchers: Vec<JournalVoucher> = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(vouchers.len(), 2);
}

/// Edge Case: Closed period error fails with 422 and does NOT store an idempotency record.
/// Retrying with same key after period is opened succeeds.
#[tokio::test]
async fn test_api_idempotency_closed_period_failure_leaves_no_record() {
    let app = setup_test_app();
    let ctrl_tok = controller_token();
    let acc_tok = accountant_token();

    // 0. Post initial revenue voucher in 2026-03 so period closing can compute 911 transfers
    let p_init = json!({
        "voucher_no": "PKT-INIT-03",
        "voucher_date": "2026-03-01",
        "description": "Initial revenue for closing",
        "lines": [
            { "account_code": "1111", "debit": "5000000", "credit": "0" },
            { "account_code": "5111", "debit": "0", "credit": "5000000" }
        ]
    });
    let init_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {acc_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(p_init.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(init_res.status(), StatusCode::CREATED);

    // 1. Close period 2026-03
    let close_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/periods/close")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "year": 2026, "month": 3 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(close_res.status(), StatusCode::OK);

    let key = "period-locked-key-01";
    let payload = json!({
        "voucher_no": "PKT-LOCKED-01",
        "voucher_date": "2026-03-15",
        "description": "Attempt in closed period",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "1000" }
        ]
    });

    // 2. Attempt post with Idempotency-Key in closed period -> 422
    let post_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {acc_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 3. Reopen period 2026-03
    let reopen_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/periods/reopen")
                .header(header::AUTHORIZATION, format!("Bearer {ctrl_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "year": 2026, "month": 3, "reason": "Audit correction" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reopen_res.status(), StatusCode::OK);

    // 4. Retry with exact same Idempotency-Key -> now succeeds 201 Created
    let retry_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {acc_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry_res.status(), StatusCode::CREATED);
}

/// Edge Case: Unbalanced voucher fails with 422 and does NOT store an idempotency record.
/// Retrying with same key and corrected payload succeeds.
#[tokio::test]
async fn test_api_idempotency_unbalanced_voucher_leaves_no_record() {
    let app = setup_test_app();
    let acc_tok = accountant_token();
    let key = "unbalanced-key-01";

    let invalid_payload = json!({
        "voucher_no": "PKT-UNBAL-01",
        "voucher_date": "2026-02-15",
        "description": "Unbalanced transaction",
        "lines": [
            { "account_code": "1111", "debit": "1000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "2000" }
        ]
    });

    // 1. Post unbalanced voucher -> 422
    let post_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {acc_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(invalid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Post corrected balanced voucher with SAME key -> succeeds 201 Created
    let valid_payload = json!({
        "voucher_no": "PKT-BAL-01",
        "voucher_date": "2026-02-15",
        "description": "Corrected balanced transaction",
        "lines": [
            { "account_code": "1111", "debit": "2000", "credit": "0" },
            { "account_code": "1121", "debit": "0", "credit": "2000" }
        ]
    });

    let retry_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/vouchers")
                .header(header::AUTHORIZATION, format!("Bearer {acc_tok}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", key)
                .body(Body::from(valid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry_res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_api_log_dir_configuration_and_file_creation() {
    let temp_log_dir =
        std::env::temp_dir().join(format!("mf_api_log_test_{}", uuid::Uuid::new_v4()));
    let temp_log_str = temp_log_dir.to_str().unwrap().to_string();

    // 1. Verify AppConfig from_env loading respects LOG_DIR
    let config = backend::config::AppConfig::from_env_with_fallback(|k| {
        if k == "LOG_DIR" {
            Some(temp_log_str.clone())
        } else {
            None
        }
    })
    .expect("Config loading must succeed");

    assert_eq!(config.log_dir, temp_log_str);

    // 2. Verify rolling file appender creates directory and daily rolling file
    let appender = tracing_appender::rolling::daily(&config.log_dir, "money-flow.log");
    let (mut non_blocking, guard) = tracing_appender::non_blocking(appender);

    use std::io::Write;
    non_blocking
        .write_all(b"2026-11-20T09:30:00Z INFO [api_test] Verification log line\n")
        .expect("Write to rolling appender failed");
    non_blocking.flush().expect("Flush failed");

    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(guard);

    assert!(temp_log_dir.exists(), "Log directory must be created");
    let entries = std::fs::read_dir(&temp_log_dir).expect("Read log dir failed");
    let has_log = entries.filter_map(|e| e.ok()).any(|e| {
        e.file_name()
            .to_string_lossy()
            .starts_with("money-flow.log")
    });
    assert!(has_log, "money-flow.log file must exist in LOG_DIR");

    // 3. Verify startup error persistent writing in configured LOG_DIR
    backend::diagnostics::log_startup_error(
        &config.log_dir,
        "Integration Test Startup Error",
        "Synthetic port collision test",
        "Port 3000 is occupied by test runner",
        "Set PORT=3001",
    );

    let startup_log = temp_log_dir.join("startup_error.log");
    assert!(
        startup_log.exists(),
        "startup_error.log must exist in LOG_DIR"
    );
    let startup_content = std::fs::read_to_string(&startup_log).expect("Read startup log failed");
    assert!(startup_content.contains("Integration Test Startup Error"));
    assert!(startup_content.contains("Synthetic port collision test"));

    let _ = std::fs::remove_dir_all(&temp_log_dir);
}
