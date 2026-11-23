use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::{AuthenticatedUser, RequireController};
use crate::dto::{ApiError, IdempotencyKey, compute_request_hash};
use crate::models::{Account, AccountClass, NormalBalance};
use crate::state::AppState;

/// Query parameters for listing accounts from the Chart of Accounts.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListAccountsQuery {
    /// Optional filter by account class (e.g. ASSET, LIABILITY, EQUITY, REVENUE, EXPENSE, CLOSING).
    pub class: Option<AccountClass>,
    /// Optional filter by active status.
    pub is_active: Option<bool>,
}

/// Request payload for creating a new account in the dynamic chart of accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    /// Account code (minimum 3 characters, e.g. "15201").
    pub code: String,
    /// Human-readable account name in Vietnamese or English.
    pub name: String,
    /// Parent account code (optional for root classes, required for subaccounts).
    pub parent_code: Option<String>,
    /// Accounting classification per Circular 99/2025/TT-BTC.
    pub class: AccountClass,
    /// Whether the account is active for journal entry postings (default: true).
    #[serde(default = "default_true")]
    pub is_active: bool,
    /// Normal balance indicator (DEBIT or CREDIT).
    pub normal_balance: NormalBalance,
}

fn default_true() -> bool {
    true
}

/// Lists accounts in the chart of accounts with optional filtering by class or active status.
///
/// Route: GET /api/v1/accounts
/// Accessible by: AuthenticatedUser
pub async fn list_accounts(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListAccountsQuery>,
) -> Result<Json<Vec<Account>>, ApiError> {
    let accounts = state.repo.list_accounts(query.class, query.is_active)?;
    Ok(Json(accounts))
}

/// Creates a new subaccount in the dynamic chart of accounts.
/// Validates code length >= 3, non-empty name, parent existence, and circular reference checks.
/// Records an immutable audit log entry (CREATE_ACCOUNT).
///
/// Route: POST /api/v1/accounts
/// Guarded by: RequireController
pub async fn create_account(
    RequireController(user): RequireController,
    idempotency_key: IdempotencyKey,
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<Account>), ApiError> {
    let req: CreateAccountRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("Invalid JSON payload: {e}")))?;

    // 1. Idempotency check if key provided
    let idempotency_info = if let Some(ref key) = idempotency_key.0 {
        let hash =
            compute_request_hash(&req).map_err(|e| ApiError::InternalError(e.to_string()))?;
        let actor = user.claims.sub.clone();
        let endpoint = "/api/v1/accounts".to_string();

        if let Some(record) = state.repo.get_idempotency_record(key, &actor, &endpoint)? {
            if record.request_hash == hash {
                let cached_account: Account =
                    serde_json::from_str(&record.response_body).map_err(|e| {
                        ApiError::InternalError(format!("Corrupt idempotency record: {e}"))
                    })?;
                return Ok((StatusCode::CREATED, Json(cached_account)));
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

    let trimmed_code = req.code.trim();
    if trimmed_code.len() < 3 {
        return Err(ApiError::BadRequest(
            "Account code must be at least 3 characters long".to_string(),
        ));
    }

    let trimmed_name = req.name.trim();
    if trimmed_name.is_empty() {
        return Err(ApiError::BadRequest(
            "Account name cannot be empty or whitespace".to_string(),
        ));
    }

    let parent_code = req
        .parent_code
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if parent_code.as_deref() == Some(trimmed_code) {
        return Err(ApiError::BadRequest(
            "Self-referential parent code is forbidden".to_string(),
        ));
    }

    let account = Account {
        code: trimmed_code.to_string(),
        name: trimmed_name.to_string(),
        parent_code,
        class: req.class,
        is_active: req.is_active,
        normal_balance: req.normal_balance,
    };

    let idempotency_param = idempotency_info
        .as_ref()
        .map(|(k, a, e, h)| (k.as_str(), a.as_str(), e.as_str(), h.as_str()));

    let created = state
        .repo
        .create_account(&account, &user.claims.sub, idempotency_param)?;
    Ok((StatusCode::CREATED, Json(created)))
}
