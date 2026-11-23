use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

use crate::auth::{RequireAdmin, RequireController};
use crate::dto::{ApiError, BackupRequest, BackupResponse, RestoreRequest, RestoreResponse};
use crate::ledger::AccountingError;
use crate::state::AppState;
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

/// Executes an authorized disaster recovery restore of the live database from a snapshot.
///
/// Route: POST /api/v1/system/restore
/// Guarded by: RequireAdmin
pub async fn restore_backup_handler(
    RequireAdmin(user): RequireAdmin,
    State(state): State<Arc<AppState>>,
    Json(req): Json<RestoreRequest>,
) -> Result<(StatusCode, Json<RestoreResponse>), ApiError> {
    // Dual-secret confirmation check 1: confirm_overwrite must be true
    if !req.confirm_overwrite {
        return Err(ApiError::BadRequest(
            "Dual-secret confirmation failed: 'confirm_overwrite' must be explicitly set to true"
                .to_string(),
        ));
    }

    // Dual-secret confirmation check 2: expected_sha256 must be provided
    if req.expected_sha256.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Dual-secret confirmation failed: 'expected_sha256' is required".to_string(),
        ));
    }

    // Format validation: SHA-256 must be 64 hexadecimal characters
    let trimmed_sha = req.expected_sha256.trim();
    if trimmed_sha.len() != 64 || !trimmed_sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::BadRequest(
            "Invalid SHA-256 checksum format: must be 64 hexadecimal characters".to_string(),
        ));
    }

    // Path safety validation: forbid directory traversal
    if req.snapshot_path.contains("..") {
        return Err(ApiError::BadRequest(
            "Invalid snapshot path: directory traversal is forbidden".to_string(),
        ));
    }

    let snapshot_path = std::path::Path::new(&req.snapshot_path);

    let outcome = state
        .repo
        .restore_from_snapshot(snapshot_path, Some(trimmed_sha), &user.claims.sub)
        .map_err(|err| match err {
            AccountingError::DatabaseError(msg) => ApiError::BadRequest(msg),
            other => ApiError::from(other),
        })?;

    Ok((
        StatusCode::OK,
        Json(RestoreResponse {
            status: "RESTORED".to_string(),
            timestamp: outcome.timestamp,
            restored_from_snapshot: outcome.restored_from_snapshot,
            sha256_checksum: outcome.sha256_checksum,
        }),
    ))
}
