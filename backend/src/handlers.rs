use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{Next, from_fn},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthenticatedUser, RequireController};
use crate::closing::PeriodClosingEngine;
use crate::database::SaveVoucherOutcome;
use crate::dto::{
    ApiError, AuditLogQuery, BackupRequest, BackupResponse, BalanceSheetQuery,
    BalanceSheetResponse, ClosePeriodRequest, CreateVoucherRequest, DateRangeQuery, IdempotencyKey,
    IncomeStatementQuery, IncomeStatementResponse, PeriodStatusResponse, ReopenPeriodRequest,
    TrialBalanceResponse, VoidVoucherRequest, compute_request_hash,
};
use crate::ledger::AccountingError;
use crate::models::{AccountingPeriod, AuditLogEntry, JournalVoucher, VoucherLine, VoucherStatus};
use crate::state::AppState;

/// Health check handler returning service status and version.
pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": "0.1.0"
    }))
}

/// Creates a new journal voucher after validating all accounting invariants with LedgerEngine.
/// Persists atomically to the database upon validation success and captures author attribution.
/// Supports optional `Idempotency-Key` header for safe retries and duplicate prevention.
pub async fn create_voucher(
    user: AuthenticatedUser,
    idempotency_key: IdempotencyKey,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateVoucherRequest>,
) -> Result<(StatusCode, Json<JournalVoucher>), ApiError> {
    // 1. If idempotency key provided, check for existing idempotency record
    let idempotency_info = if let Some(ref key) = idempotency_key.0 {
        let hash =
            compute_request_hash(&req).map_err(|e| ApiError::InternalError(e.to_string()))?;
        let actor = user.claims.sub.clone();
        let endpoint = "/api/v1/vouchers".to_string();

        if let Some(record) = state.repo.get_idempotency_record(key, &actor, &endpoint)? {
            if record.request_hash == hash {
                let cached_voucher: JournalVoucher = serde_json::from_str(&record.response_body)
                    .map_err(|e| {
                        ApiError::InternalError(format!("Corrupt idempotency record: {e}"))
                    })?;
                return Ok((StatusCode::CREATED, Json(cached_voucher)));
            } else {
                return Err(ApiError::Conflict(format!(
                    "Idempotency key '{}' already used with a different request payload",
                    key
                )));
            }
        }
        Some((key.clone(), actor, endpoint, hash))
    } else {
        None
    };

    let voucher_id = Uuid::new_v4();

    let lines = req
        .lines
        .into_iter()
        .enumerate()
        .map(|(idx, l)| VoucherLine {
            id: Uuid::new_v4(),
            line_no: (idx + 1) as u32,
            account_code: l.account_code,
            debit: l.debit,
            credit: l.credit,
            line_desc: l.line_desc,
        })
        .collect();

    let voucher = JournalVoucher {
        id: voucher_id,
        voucher_no: req.voucher_no,
        voucher_date: req.voucher_date,
        description: req.description,
        invoice_ref: req.invoice_ref,
        status: VoucherStatus::Posted,
        created_by: user.claims.sub,
        approved_by: req.approved_by,
        voided_at: None,
        voided_by: None,
        void_reason: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        lines,
    };

    // 2. Invariant validation with LedgerEngine (THINK / CHECK)
    state.engine.validate_voucher(&voucher)?;

    // 3. Atomic persistence to SQLite with idempotency tracking (DO)
    let idempotency_param = idempotency_info
        .as_ref()
        .map(|(k, a, e, h)| (k.as_str(), a.as_str(), e.as_str(), h.as_str()));
    let outcome = state
        .repo
        .save_voucher_with_idempotency(&voucher, idempotency_param)?;

    let result_voucher = match outcome {
        SaveVoucherOutcome::Created(v) => v,
        SaveVoucherOutcome::Replayed(v) => v,
    };

    Ok((StatusCode::CREATED, Json(result_voucher)))
}

/// Retrieves a specific voucher by its UUID.
pub async fn get_voucher(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<JournalVoucher>, ApiError> {
    match state.repo.get_voucher(id)? {
        Some(voucher) => Ok(Json(voucher)),
        None => Err(ApiError::NotFound(format!(
            "Voucher with id '{}' not found",
            id
        ))),
    }
}

/// Voids an active journal voucher non-destructively per statutory audit protocol.
/// Excludes the voucher from subsequent reports while permanently preserving history.
///
/// Route: POST /api/v1/vouchers/{id}/void
pub async fn void_voucher_handler(
    RequireController(user): RequireController,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<VoidVoucherRequest>,
) -> Result<Json<JournalVoucher>, ApiError> {
    let actor = user.claims.sub;
    let voucher = state.repo.void_voucher(id, &actor, &req.reason, None)?;
    Ok(Json(voucher))
}

/// Queries immutable system and accounting audit trail records.
///
/// Route: GET /api/v1/audit-logs
pub async fn list_audit_logs_handler(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<Vec<AuditLogEntry>>, ApiError> {
    let logs = state.repo.list_audit_logs(&query)?;
    Ok(Json(logs))
}

/// Lists vouchers with optional date range filtering.
pub async fn list_vouchers(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<DateRangeQuery>,
) -> Result<Json<Vec<JournalVoucher>>, ApiError> {
    let vouchers = state.repo.list_vouchers(query.from_date, query.to_date)?;
    Ok(Json(vouchers))
}

/// Computes the statutory Trial Balance report (Bảng cân đối số phát sinh).
pub async fn get_trial_balance(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<TrialBalanceResponse>, ApiError> {
    let accounts = state.repo.compute_trial_balance()?;
    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;

    for row in &accounts {
        total_debit += row.total_debit;
        total_credit += row.total_credit;
    }

    let is_balanced = total_debit == total_credit;
    let as_of_date = chrono::Utc::now().date_naive();

    Ok(Json(TrialBalanceResponse {
        as_of_date,
        accounts,
        total_debit,
        total_credit,
        is_balanced,
    }))
}

// ============================================================================
// Phase Epsilon Handlers (Period Governance & Statutory Reports)
// ============================================================================

/// Handler for closing an accounting period (Khóa sổ kế toán).
/// Executes the 4-step closing sequence, persists the closing voucher atomically, and locks the period.
///
/// Route: POST /api/v1/periods/close
pub async fn close_period(
    RequireController(user): RequireController,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClosePeriodRequest>,
) -> Result<Json<PeriodStatusResponse>, ApiError> {
    if req.month < 1 || req.month > 12 {
        return Err(ApiError::BadRequest(format!(
            "Invalid month {}: month must be between 1 and 12",
            req.month
        )));
    }

    // 1. Check if period is already closed
    if state.repo.is_period_closed(req.year, req.month)? {
        return Err(AccountingError::PeriodAlreadyClosed {
            year: req.year,
            month: req.month,
        }
        .into());
    }

    // 2. Build closing voucher via 4-step closing engine
    let mut closing_voucher =
        PeriodClosingEngine::build_closing_voucher(&state.repo, req.year, req.month)?;
    closing_voucher.created_by = user.claims.sub;

    // 3. Atomically persist closing voucher and mark period closed
    state
        .repo
        .close_period(req.year, req.month, &closing_voucher)?;

    // 4. Return updated period status
    let period = AccountingPeriod {
        year: req.year,
        month: req.month,
        is_closed: true,
        closed_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    Ok(Json(period.into()))
}

/// Handler for reopening a closed accounting period (Mở lại sổ kế toán).
/// Atomically deletes/voids the closing voucher (KC-YYYY-MM) and marks the period unlocked.
///
/// Route: POST /api/v1/periods/reopen
pub async fn reopen_period(
    RequireController(user): RequireController,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReopenPeriodRequest>,
) -> Result<Json<PeriodStatusResponse>, ApiError> {
    if req.month < 1 || req.month > 12 {
        return Err(ApiError::BadRequest(format!(
            "Invalid month {}: month must be between 1 and 12",
            req.month
        )));
    }

    // 1. Check if period is closed
    if !state.repo.is_period_closed(req.year, req.month)? {
        return Err(AccountingError::PeriodNotClosed {
            year: req.year,
            month: req.month,
        }
        .into());
    }

    // 2. Atomically void closing voucher and unlock period
    state
        .repo
        .reopen_period(req.year, req.month, &user.claims.sub)?;

    let period = AccountingPeriod {
        year: req.year,
        month: req.month,
        is_closed: false,
        closed_at: None,
    };

    Ok(Json(period.into()))
}

/// Handler for listing all tracked accounting periods and their lock statuses.
///
/// Route: GET /api/v1/periods
pub async fn list_periods(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PeriodStatusResponse>>, ApiError> {
    let periods = state.repo.list_periods()?;
    let responses: Vec<PeriodStatusResponse> = periods.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

/// Handler for retrieving the statutory Balance Sheet report (Bảng cân đối kế toán).
/// Applies contra-asset rules deducting Accumulated Depreciation (TK 214) and Provisions (TK 229).
///
/// Route: GET /api/v1/reports/balance-sheet?as_of_date=YYYY-MM-DD
pub async fn get_balance_sheet(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<BalanceSheetQuery>,
) -> Result<Json<BalanceSheetResponse>, ApiError> {
    let as_of_date = query
        .as_of_date
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    let report = state.repo.compute_balance_sheet(as_of_date)?;
    Ok(Json(report.into()))
}

/// Handler for retrieving the statutory Income Statement / P&L (Báo cáo KQHĐKD).
/// Computes gross revenue, deductions, net revenue, COGS, operating profit, and net profit after tax.
///
/// Route: GET /api/v1/reports/income-statement?from_date=YYYY-MM-DD&to_date=YYYY-MM-DD
pub async fn get_income_statement(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<IncomeStatementQuery>,
) -> Result<Json<IncomeStatementResponse>, ApiError> {
    if query.from_date > query.to_date {
        return Err(ApiError::BadRequest(format!(
            "Invalid date range: from_date ({}) must be <= to_date ({})",
            query.from_date, query.to_date
        )));
    }

    let report = state
        .repo
        .compute_income_statement(query.from_date, query.to_date)?;
    Ok(Json(report.into()))
}

/// Creates an atomic live database backup snapshot via SQLite VACUUM INTO.
///
/// Route: POST /api/v1/system/backup
pub async fn create_backup(
    RequireController(user): RequireController,
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<BackupResponse>), ApiError> {
    let req: Option<BackupRequest> = if body.is_empty() {
        None
    } else {
        Some(
            serde_json::from_slice(&body)
                .map_err(|e| ApiError::BadRequest(format!("Invalid JSON payload: {}", e)))?,
        )
    };

    let filename = req
        .and_then(|r| r.filename)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "accounting_{}.db",
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            )
        });

    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(ApiError::BadRequest(
            "Invalid backup filename: directory traversal is forbidden".to_string(),
        ));
    }

    let backup_dir = &state.config.backup_dir;
    let backup_path = std::path::Path::new(backup_dir).join(&filename);
    let backup_path_str = backup_path.to_string_lossy().to_string();

    let file_size_bytes = state
        .repo
        .create_backup(&backup_path_str, &user.claims.sub)?;

    Ok((
        StatusCode::CREATED,
        Json(BackupResponse {
            status: "success".to_string(),
            backup_path: backup_path_str,
            file_size_bytes,
            created_at: chrono::Utc::now().to_rfc3339(),
        }),
    ))
}

/// Serves the OpenAPI 3.1 JSON specification.
///
/// Route: GET /api-docs/openapi.json
pub async fn openapi_json_handler() -> Json<serde_json::Value> {
    Json(crate::openapi::generate_openapi_spec())
}

/// Serves the interactive standalone Swagger UI HTML page.
///
/// Route: GET /swagger-ui
pub async fn swagger_ui_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(crate::openapi::swagger_ui_html())
}

/// Axum middleware for structured HTTP request/response diagnostic logging for Root Cause Analysis (RCA).
pub async fn request_logging_middleware(req: Request, next: Next) -> Response {
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    let response = next.run(req).await;

    let latency = start.elapsed();
    let status = response.status();

    if status.is_server_error() {
        tracing::error!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            latency_ms = latency.as_millis(),
            "HTTP request completed with 5xx server error"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            latency_ms = latency.as_millis(),
            "HTTP request completed with 4xx client error"
        );
    } else {
        tracing::info!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            latency_ms = latency.as_millis(),
            "HTTP request processed successfully"
        );
    }

    response
}

/// Constructs the Axum application Router with all accounting routes mounted.
pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { "Accounting Engine Online" }))
        .route("/health", get(health_check))
        // Vouchers
        .route("/api/v1/vouchers", post(create_voucher).get(list_vouchers))
        .route("/api/v1/vouchers/{id}", get(get_voucher))
        .route("/api/v1/vouchers/{id}/void", post(void_voucher_handler))
        // Period Governance
        .route("/api/v1/periods", get(list_periods))
        .route("/api/v1/periods/close", post(close_period))
        .route("/api/v1/periods/reopen", post(reopen_period))
        // Audit Logs & Forensic Trail
        .route("/api/v1/audit-logs", get(list_audit_logs_handler))
        // Statutory Reports
        .route("/api/v1/reports/trial-balance", get(get_trial_balance))
        .route("/api/v1/reports/balance-sheet", get(get_balance_sheet))
        .route(
            "/api/v1/reports/income-statement",
            get(get_income_statement),
        )
        // Disaster Recovery & System Backup
        .route("/api/v1/system/backup", post(create_backup))
        // Interactive OpenAPI 3.1 & Swagger UI
        .route("/api-docs/openapi.json", get(openapi_json_handler))
        .route("/swagger-ui", get(swagger_ui_handler))
        .layer(from_fn(request_logging_middleware))
        .with_state(state)
}
