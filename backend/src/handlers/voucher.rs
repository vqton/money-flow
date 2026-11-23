use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthenticatedUser, RequireController};
use crate::database::SaveVoucherOutcome;
use crate::dto::{
    ApiError, AuditLogQuery, CreateVoucherRequest, DateRangeQuery, IdempotencyKey,
    VoidVoucherRequest, compute_request_hash,
};
use crate::models::{AuditLogEntry, JournalVoucher, VoucherLine, VoucherStatus};
use crate::state::AppState;
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
    state
        .engine
        .validate_voucher_with_validator(&voucher, &state.repo)?;

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
/// Restricted to Controller or Admin role.
///
/// Route: GET /api/v1/audit-logs
pub async fn list_audit_logs_handler(
    RequireController(_): RequireController,
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
