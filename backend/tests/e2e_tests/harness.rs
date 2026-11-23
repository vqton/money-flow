use axum::{
    body::{Body, Bytes},
    http::{Method, Request, StatusCode, header},
};
use backend::{
    auth::{Claims, Role, generate_test_token},
    config::{AppConfig, DEFAULT_TEST_JWT_SECRET},
    create_app,
    database::LedgerRepository,
    dto::{BalanceSheetResponse, IncomeStatementResponse, TrialBalanceResponse},
    ledger::LedgerEngine,
    models::{AuditLogEntry, JournalVoucher},
    state::AppState,
};
use jsonwebtoken::{EncodingKey, Header, encode};
use rusqlite::Connection;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tower::ServiceExt;
use uuid::Uuid;

/// Helper to format raw byte slices into lowercase hex strings without extra crates.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Helper to locate repository root LICENSE file.
pub fn find_license_content() -> Option<String> {
    let candidates = [
        PathBuf::from("LICENSE"),
        PathBuf::from("../LICENSE"),
        PathBuf::from("d:/money-flow/LICENSE"),
    ];
    candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
}

/// Helper to locate repository root .gitignore file.
pub fn find_root_gitignore() -> Option<String> {
    let candidates = [
        PathBuf::from("../.gitignore"),
        PathBuf::from("d:/money-flow/.gitignore"),
        PathBuf::from(".gitignore"),
    ];
    for p in &candidates {
        if let Ok(c) = std::fs::read_to_string(p)
            && (c.contains("backend/target") || c.contains("backend/logs"))
        {
            return Some(c);
        }
    }
    candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
}

/// RAII-managed isolated filesystem environment for E2E tests.
/// Automatically creates isolated temp directory, database path, backup dir, and log dir,
/// and cleans them up on Drop.
pub struct TestEnv {
    pub root_dir: PathBuf,
    pub db_path: PathBuf,
    pub backup_dir: PathBuf,
    pub log_dir: PathBuf,
    pub auto_cleanup: bool,
}

impl TestEnv {
    /// Initializes a new isolated temporary environment.
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        let root_dir = std::env::temp_dir().join(format!("mf_e2e_{id}"));
        let db_path = root_dir.join("accounting.db");
        let backup_dir = root_dir.join("backups");
        let log_dir = root_dir.join("logs");

        std::fs::create_dir_all(&root_dir).expect("Failed to create test root directory");
        std::fs::create_dir_all(&backup_dir).expect("Failed to create backup directory");
        std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

        Self {
            root_dir,
            db_path,
            backup_dir,
            log_dir,
            auto_cleanup: true,
        }
    }

    /// Retains the filesystem directory when TestEnv is dropped.
    pub fn retain_on_drop(&mut self) {
        self.auto_cleanup = false;
    }

    /// Creates an empty file in the test workspace.
    pub fn create_file(&self, relative_path: &str, content: &[u8]) -> PathBuf {
        let full_path = self.root_dir.join(relative_path);
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&full_path, content).expect("Failed to write test file");
        full_path
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        if self.auto_cleanup && std::fs::remove_dir_all(&self.root_dir).is_err() {
            eprintln!("[WARN] TestEnv failed to clean up {:?}", self.root_dir);
        }
    }
}

/// Test application instance encapsulating the Axum router and application state.
pub struct TestApp {
    pub router: axum::Router,
    pub state: Arc<AppState>,
    pub env: Option<TestEnv>,
}

impl TestApp {
    /// In-memory application for rapid API testing without persisting to disk.
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("In-memory SQLite connection failed");
        let repo = LedgerRepository::new(conn);
        repo.init_schema().expect("Schema initialization failed");
        let engine = LedgerEngine::new();
        let state = Arc::new(AppState::new(engine, repo));
        let router = create_app(state.clone());
        Self {
            router,
            state,
            env: None,
        }
    }

    /// File-backed application for backup/restore, WAL, and CLI execution drills.
    pub fn file_backed(env: TestEnv) -> Self {
        let config = AppConfig {
            database_path: env.db_path.to_str().unwrap().to_string(),
            backup_dir: env.backup_dir.to_str().unwrap().to_string(),
            log_dir: env.log_dir.to_str().unwrap().to_string(),
            ..Default::default()
        };
        let conn = Connection::open(&env.db_path).expect("Failed to open SQLite database");
        let repo = LedgerRepository::new(conn);
        repo.init_schema().expect("Schema initialization failed");
        let engine = LedgerEngine::new();
        let state = Arc::new(AppState::with_config(engine, repo, config));
        let router = create_app(state.clone());
        Self {
            router,
            state,
            env: Some(env),
        }
    }

    /// Custom configuration constructor allowing modifier functions.
    pub fn with_custom_config(env: TestEnv, config_modifier: impl FnOnce(&mut AppConfig)) -> Self {
        let mut config = AppConfig {
            database_path: env.db_path.to_str().unwrap().to_string(),
            backup_dir: env.backup_dir.to_str().unwrap().to_string(),
            log_dir: env.log_dir.to_str().unwrap().to_string(),
            ..Default::default()
        };
        config_modifier(&mut config);
        let conn = Connection::open(&env.db_path).expect("Failed to open SQLite database");
        let repo = LedgerRepository::new(conn);
        repo.init_schema().expect("Schema initialization failed");
        let engine = LedgerEngine::new();
        let state = Arc::new(AppState::with_config(engine, repo, config));
        let router = create_app(state.clone());
        Self {
            router,
            state,
            env: Some(env),
        }
    }

    /// Triggers a backup snapshot via POST /api/v1/system/backup and returns the snapshot path.
    pub async fn take_backup(&self) -> String {
        let res = self
            .post("/api/v1/system/backup")
            .as_controller()
            .send()
            .await;
        res.assert_success();
        let val = res.json_val();
        val.get("backup_path")
            .or_else(|| val.get("file_path"))
            .and_then(|v| v.as_str())
            .expect("Backup response must include backup_path or file_path")
            .to_string()
    }

    /// Initiates a GET request builder.
    pub fn get(&self, uri: &str) -> TestRequestBuilder {
        TestRequestBuilder::new(self.router.clone(), Method::GET, uri)
    }

    /// Initiates a POST request builder.
    pub fn post(&self, uri: &str) -> TestRequestBuilder {
        TestRequestBuilder::new(self.router.clone(), Method::POST, uri)
    }

    /// Initiates a PUT request builder.
    pub fn put(&self, uri: &str) -> TestRequestBuilder {
        TestRequestBuilder::new(self.router.clone(), Method::PUT, uri)
    }

    /// Initiates a DELETE request builder.
    pub fn delete(&self, uri: &str) -> TestRequestBuilder {
        TestRequestBuilder::new(self.router.clone(), Method::DELETE, uri)
    }
}

/// Fluent HTTP request builder for opaque-box API interactions.
pub struct TestRequestBuilder {
    router: axum::Router,
    method: Method,
    uri: String,
    token: Option<String>,
    idempotency_key: Option<String>,
    headers: Vec<(String, String)>,
    body: Body,
}

impl TestRequestBuilder {
    pub fn new(router: axum::Router, method: Method, uri: &str) -> Self {
        Self {
            router,
            method,
            uri: uri.to_string(),
            token: None,
            idempotency_key: None,
            headers: Vec::new(),
            body: Body::empty(),
        }
    }

    /// Authorizes request using an Accountant role token.
    pub fn as_accountant(mut self) -> Self {
        self.token = Some(accountant_token());
        self
    }

    /// Authorizes request using a Controller role token.
    pub fn as_controller(mut self) -> Self {
        self.token = Some(controller_token());
        self
    }

    /// Authorizes request using an Admin role token.
    pub fn as_admin(mut self) -> Self {
        self.token = Some(admin_token());
        self
    }

    /// Explicitly sends the request without any Authorization header.
    pub fn as_anonymous(mut self) -> Self {
        self.token = None;
        self
    }

    /// Sets a custom Bearer token.
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    /// Attaches an Idempotency-Key header.
    pub fn with_idempotency_key(mut self, key: &str) -> Self {
        self.idempotency_key = Some(key.to_string());
        self
    }

    /// Attaches a custom HTTP header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    /// Serializes a JSON payload and sets Content-Type to application/json.
    pub fn json<T: Serialize>(mut self, payload: &T) -> Self {
        let json_str = serde_json::to_string(payload).expect("Failed to serialize request JSON");
        self.body = Body::from(json_str);
        self.headers.push((
            header::CONTENT_TYPE.to_string(),
            "application/json".to_string(),
        ));
        self
    }

    /// Sets raw body bytes.
    pub fn raw_body(mut self, body: impl Into<Body>) -> Self {
        self.body = body.into();
        self
    }

    /// Executes the HTTP request against the Axum router and returns the response.
    pub async fn send(self) -> TestResponse {
        let mut builder = Request::builder().method(self.method).uri(self.uri);

        if let Some(token) = self.token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(key) = self.idempotency_key {
            builder = builder.header("Idempotency-Key", key);
        }
        for (k, v) in self.headers {
            builder = builder.header(k, v);
        }

        let req = builder
            .body(self.body)
            .expect("Failed to construct HTTP request");
        let res = self
            .router
            .oneshot(req)
            .await
            .expect("Request execution failed on Axum router");
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body");

        TestResponse { status, bytes }
    }
}

/// HTTP response wrapper providing ergonomic assertion and deserialization methods.
pub struct TestResponse {
    pub status: StatusCode,
    pub bytes: Bytes,
}

impl TestResponse {
    /// Asserts exact HTTP status code.
    pub fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(
            self.status,
            expected,
            "Expected status {expected}, but got {}. Response body: {}",
            self.status,
            self.text()
        );
        self
    }

    /// Asserts HTTP status code indicates success (2xx).
    pub fn assert_success(&self) -> &Self {
        assert!(
            self.status.is_success(),
            "Expected success status (2xx), but got {}. Response body: {}",
            self.status,
            self.text()
        );
        self
    }

    /// Deserializes response body JSON into type T.
    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.bytes).unwrap_or_else(|e| {
            panic!("Failed to deserialize response JSON ({e}): {}", self.text())
        })
    }

    /// Deserializes response body into generic serde_json::Value.
    pub fn json_val(&self) -> serde_json::Value {
        serde_json::from_slice(&self.bytes).unwrap_or_else(|e| {
            panic!(
                "Failed to parse response body as JSON Value ({e}): {}",
                self.text()
            )
        })
    }

    /// Returns response body as UTF-8 string.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).to_string()
    }

    /// Asserts that response JSON error code matches expected error string.
    pub fn assert_error_code(&self, expected_code: &str) {
        let val = self.json_val();
        let actual_code = val
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("Response JSON missing 'error' field: {val}"));
        assert_eq!(
            actual_code, expected_code,
            "Expected error code '{expected_code}', got '{actual_code}' in body: {val}"
        );
    }
}

// ---------------------------------------------------------------------------
// Authentication Tokens
// ---------------------------------------------------------------------------

pub fn accountant_token() -> String {
    generate_test_token(
        "acc_test_user",
        Role::Accountant,
        3600,
        DEFAULT_TEST_JWT_SECRET,
    )
}

pub fn controller_token() -> String {
    generate_test_token(
        "ctrl_test_user",
        Role::Controller,
        3600,
        DEFAULT_TEST_JWT_SECRET,
    )
}

pub fn admin_token() -> String {
    generate_test_token(
        "admin_test_user",
        Role::Admin,
        3600,
        DEFAULT_TEST_JWT_SECRET,
    )
}

/// Generates an expired JWT token (expired 3600 seconds ago).
pub fn expired_token() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;
    let claims = Claims {
        sub: "expired_user".to_string(),
        role: Role::Controller,
        exp: now.saturating_sub(3600),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(DEFAULT_TEST_JWT_SECRET.as_bytes()),
    )
    .expect("Failed to encode expired token")
}

/// Generates a token signed with an invalid/tampered secret key.
pub fn invalid_signature_token() -> String {
    let wrong_secret = "invalid-wrong-secret-key-32-bytes-long!!";
    generate_test_token("forged_user", Role::Admin, 3600, wrong_secret)
}

// ---------------------------------------------------------------------------
// CLI Runner
// ---------------------------------------------------------------------------

/// Helper for executing the compiled backend binary CLI subcommands.
pub struct CliRunner {
    pub bin_path: PathBuf,
}

impl CliRunner {
    pub fn new() -> Self {
        // Cargo automatically provides CARGO_BIN_EXE_backend for integration tests
        let bin = env!("CARGO_BIN_EXE_backend");
        Self {
            bin_path: PathBuf::from(bin),
        }
    }

    /// Invokes: backend.exe restore --snapshot <snapshot> --target <target>
    pub fn restore(&self, snapshot: &Path, target: &Path) -> std::process::Output {
        self.run_with_args(&[
            "restore",
            "--snapshot",
            snapshot.to_str().expect("Valid snapshot path"),
            "--target",
            target.to_str().expect("Valid target path"),
        ])
    }

    /// Invokes the backend binary with arbitrary custom argument slices with a safety timeout.
    pub fn run_with_args(&self, args: &[&str]) -> std::process::Output {
        let mut child = std::process::Command::new(&self.bin_path)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to execute backend process");

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(3);
        loop {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    return child.wait_with_output().expect("wait_with_output failed");
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        return child
                            .wait_with_output()
                            .expect("wait_with_output failed after kill");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => panic!("Error polling child process: {e}"),
            }
        }
    }
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Voucher Builder
// ---------------------------------------------------------------------------

/// Ergonomic double-entry zero-sum voucher JSON generator.
pub struct VoucherBuilder {
    voucher_no: String,
    voucher_date: String,
    description: String,
    invoice_ref: Option<String>,
    lines: Vec<serde_json::Value>,
}

impl VoucherBuilder {
    pub fn new(voucher_no: &str, voucher_date: &str) -> Self {
        Self {
            voucher_no: voucher_no.to_string(),
            voucher_date: voucher_date.to_string(),
            description: format!("Voucher {voucher_no}"),
            invoice_ref: None,
            lines: Vec::new(),
        }
    }

    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn invoice_ref(mut self, ref_no: &str) -> Self {
        self.invoice_ref = Some(ref_no.to_string());
        self
    }

    pub fn leg(mut self, account_code: &str, debit: &str, credit: &str) -> Self {
        self.lines.push(serde_json::json!({
            "account_code": account_code,
            "debit": debit,
            "credit": credit
        }));
        self
    }

    /// Builds a balanced cash withdrawal voucher (Debit 1111, Credit 1121).
    pub fn simple_cash_withdrawal(
        voucher_no: &str,
        date: &str,
        amount_vnd: &str,
    ) -> serde_json::Value {
        Self::new(voucher_no, date)
            .description("Rut tien gui ngan hang ve quy tien mat")
            .leg("1111", amount_vnd, "0")
            .leg("1121", "0", amount_vnd)
            .build()
    }

    /// Builds a standard sales invoice voucher with 10% VAT (Debit 131, Credit 5111, Credit 33311).
    pub fn sales_invoice(
        voucher_no: &str,
        date: &str,
        amount_vnd: &str,
        vat_vnd: &str,
    ) -> serde_json::Value {
        let amt: rust_decimal::Decimal = amount_vnd.parse().expect("Valid amount decimal");
        let vat: rust_decimal::Decimal = vat_vnd.parse().expect("Valid vat decimal");
        let total = (amt + vat).to_string();

        Self::new(voucher_no, date)
            .description("Doanh thu ban hang va thue GTGT phai nop")
            .invoice_ref("INV-E2E-001")
            .leg("131", &total, "0")
            .leg("5111", "0", amount_vnd)
            .leg("33311", "0", vat_vnd)
            .build()
    }

    /// Builds a standard materials purchase voucher with 10% VAT (Debit 152, Debit 1331, Credit 331).
    pub fn inventory_purchase(
        voucher_no: &str,
        date: &str,
        mat_vnd: &str,
        vat_vnd: &str,
    ) -> serde_json::Value {
        let mat: rust_decimal::Decimal = mat_vnd.parse().expect("Valid mat decimal");
        let vat: rust_decimal::Decimal = vat_vnd.parse().expect("Valid vat decimal");
        let total = (mat + vat).to_string();

        Self::new(voucher_no, date)
            .description("Mua nguyen vat lieu nhap kho co thue GTGT")
            .leg("152", mat_vnd, "0")
            .leg("1331", vat_vnd, "0")
            .leg("331", "0", &total)
            .build()
    }

    pub fn build(self) -> serde_json::Value {
        serde_json::json!({
            "voucher_no": self.voucher_no,
            "voucher_date": self.voucher_date,
            "description": self.description,
            "invoice_ref": self.invoice_ref,
            "lines": self.lines
        })
    }
}

// ---------------------------------------------------------------------------
// Zero-Diff & Equilibrium Assertions
// ---------------------------------------------------------------------------

/// Asserts that a Trial Balance is mathematically balanced: total debit == total credit.
pub fn assert_trial_balance_balanced(tb: &TrialBalanceResponse) {
    assert_eq!(
        tb.total_debit, tb.total_credit,
        "Trial balance debit ({}) != credit ({})",
        tb.total_debit, tb.total_credit
    );
    assert!(tb.is_balanced, "Trial balance marked as unbalanced");
}

/// Asserts zero discrepancy (0 VND) between pre-backup and post-restore Trial Balance reports.
pub fn assert_zero_diff_trial_balance(pre: &TrialBalanceResponse, post: &TrialBalanceResponse) {
    assert_eq!(
        pre.total_debit, post.total_debit,
        "Total debit discrepancy: pre={}, post={}",
        pre.total_debit, post.total_debit
    );
    assert_eq!(
        pre.total_credit, post.total_credit,
        "Total credit discrepancy: pre={}, post={}",
        pre.total_credit, post.total_credit
    );
    assert_eq!(
        pre.accounts.len(),
        post.accounts.len(),
        "Account count discrepancy: pre={}, post={}",
        pre.accounts.len(),
        post.accounts.len()
    );

    let pre_map: std::collections::HashMap<_, _> =
        pre.accounts.iter().map(|r| (&r.account_code, r)).collect();

    for post_row in &post.accounts {
        let pre_row = pre_map.get(&post_row.account_code).unwrap_or_else(|| {
            panic!(
                "Account {} exists in post-restore but missing in pre-backup",
                post_row.account_code
            )
        });
        assert_eq!(
            pre_row.total_debit, post_row.total_debit,
            "Debit variance on account {}",
            post_row.account_code
        );
        assert_eq!(
            pre_row.total_credit, post_row.total_credit,
            "Credit variance on account {}",
            post_row.account_code
        );
        assert_eq!(
            pre_row.net_balance, post_row.net_balance,
            "Net balance variance on account {}",
            post_row.account_code
        );
    }
}

/// Asserts that a Balance Sheet is mathematically balanced: Total Assets == Total Resources.
pub fn assert_balance_sheet_balanced(bs: &BalanceSheetResponse) {
    assert_eq!(
        bs.total_assets, bs.total_resources,
        "Balance sheet assets ({}) != resources ({})",
        bs.total_assets, bs.total_resources
    );
    assert!(
        bs.is_balanced,
        "Balance sheet report marked is_balanced=false"
    );
}

/// Asserts zero discrepancy (0 VND) between pre-backup and post-restore Balance Sheets.
pub fn assert_zero_diff_balance_sheet(pre: &BalanceSheetResponse, post: &BalanceSheetResponse) {
    assert_eq!(
        pre.total_assets, post.total_assets,
        "Total assets variance: pre={}, post={}",
        pre.total_assets, post.total_assets
    );
    assert_eq!(
        pre.total_resources, post.total_resources,
        "Total resources variance: pre={}, post={}",
        pre.total_resources, post.total_resources
    );
    assert_eq!(
        pre.current_assets, post.current_assets,
        "Current assets variance: pre={}, post={}",
        pre.current_assets, post.current_assets
    );
    assert_eq!(
        pre.non_current_assets, post.non_current_assets,
        "Non-current assets variance: pre={}, post={}",
        pre.non_current_assets, post.non_current_assets
    );
    assert_eq!(
        pre.accumulated_depreciation, post.accumulated_depreciation,
        "Accumulated depreciation variance: pre={}, post={}",
        pre.accumulated_depreciation, post.accumulated_depreciation
    );
    assert_eq!(
        pre.asset_provisions, post.asset_provisions,
        "Asset provisions variance: pre={}, post={}",
        pre.asset_provisions, post.asset_provisions
    );
    assert_eq!(
        pre.liabilities, post.liabilities,
        "Liabilities variance: pre={}, post={}",
        pre.liabilities, post.liabilities
    );
    assert_eq!(
        pre.equity, post.equity,
        "Equity variance: pre={}, post={}",
        pre.equity, post.equity
    );
}

/// Asserts zero discrepancy (0 VND) between pre-backup and post-restore Income Statements.
pub fn assert_zero_diff_income_statement(
    pre: &IncomeStatementResponse,
    post: &IncomeStatementResponse,
) {
    assert_eq!(
        pre.gross_revenue, post.gross_revenue,
        "Gross revenue variance: pre={}, post={}",
        pre.gross_revenue, post.gross_revenue
    );
    assert_eq!(
        pre.revenue_deductions, post.revenue_deductions,
        "Revenue deductions variance: pre={}, post={}",
        pre.revenue_deductions, post.revenue_deductions
    );
    assert_eq!(
        pre.net_revenue, post.net_revenue,
        "Net revenue variance: pre={}, post={}",
        pre.net_revenue, post.net_revenue
    );
    assert_eq!(
        pre.cost_of_goods_sold, post.cost_of_goods_sold,
        "Cost of goods sold variance: pre={}, post={}",
        pre.cost_of_goods_sold, post.cost_of_goods_sold
    );
    assert_eq!(
        pre.gross_profit, post.gross_profit,
        "Gross profit variance: pre={}, post={}",
        pre.gross_profit, post.gross_profit
    );
    assert_eq!(
        pre.net_profit_after_tax, post.net_profit_after_tax,
        "Net profit variance: pre={}, post={}",
        pre.net_profit_after_tax, post.net_profit_after_tax
    );
}

/// Asserts voucher list identity and zero discrepancy between pre and post states.
pub fn assert_zero_diff_voucher_counts(pre: &[JournalVoucher], post: &[JournalVoucher]) {
    assert_eq!(
        pre.len(),
        post.len(),
        "Voucher count mismatch: pre={}, post={}",
        pre.len(),
        post.len()
    );
    let pre_ids: std::collections::HashSet<_> = pre.iter().map(|v| &v.id).collect();
    for v in post {
        assert!(
            pre_ids.contains(&v.id),
            "Restored voucher id {} missing from pre-backup population",
            v.id
        );
    }
}

/// Asserts zero discrepancy in audit logs between pre and post states.
pub fn assert_zero_diff_audit_logs(pre: &[AuditLogEntry], post: &[AuditLogEntry]) {
    assert_eq!(
        pre.len(),
        post.len(),
        "Audit log count mismatch: pre={}, post={}",
        pre.len(),
        post.len()
    );
}
