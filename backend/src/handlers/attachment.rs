// Attachment HTTP handlers – upload, list, download.
use axum::{
    extract::{Path, Multipart, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde_json::json;
use std::path::PathBuf;
use std::fs;
use uuid::Uuid;
use chrono::Utc;
use crate::config::AppConfig;
use crate::db::attachment_repo::AttachmentRepo;
use crate::database::LedgerRepository;
use crate::models::VoucherAttachment;
use crate::ledger::AccountingError;

/// Helper to compute SHA‑256 of a byte slice.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// POST /api/v1/vouchers/{voucher_id}/attachments
/// Accepts a multipart form with a single file field named "file".
/// Returns 201 Created with attachment metadata JSON.
pub async fn upload_attachment(
    State(config): State<AppConfig>,
    State(repo): State<LedgerRepository>,
    Path(voucher_id): Path<Uuid>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Extract the file part.
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Failed to read file"}))),
        };
        // Compute hash and size.
        let sha256 = sha256_hex(&data);
        let file_size = data.len() as u64;
        // Store on disk.
        let dir = PathBuf::from(&config.attachment_dir);
        if let Err(e) = fs::create_dir_all(&dir) {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to create dir: {}", e)})));
        }
        let file_path = dir.join(&sha256);
        if let Err(e) = fs::write(&file_path, &data) {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to write file: {}", e)})));
        }
        // Persist metadata.
        let attachment_repo = AttachmentRepo::new(&repo);
        let uploaded_by = "system".to_string(); // In real code, extract from auth.
        match attachment_repo.add_attachment(
            voucher_id,
            file_name.clone(),
            content_type.clone(),
            file_size,
            sha256.clone(),
            uploaded_by,
        ) {
            Ok(att) => {
                return (
                    StatusCode::CREATED,
                    Json(json!({
                        "id": att.id.to_string(),
                        "voucher_id": att.voucher_id.to_string(),
                        "file_name": att.file_name,
                        "content_type": att.content_type,
                        "file_size": att.file_size,
                        "sha256_hash": att.sha256_hash,
                        "uploaded_at": att.uploaded_at,
                    })),
                );
            }
            Err(e) => {
                let (code, msg) = map_error(e);
                return (code, Json(json!({"error": msg})));
            }
        }
    }
    (StatusCode::BAD_REQUEST, Json(json!({"error": "No file field supplied"})))
}

/// GET /api/v1/vouchers/{voucher_id}/attachments – list metadata.
pub async fn list_attachments(
    State(repo): State<LedgerRepository>,
    Path(voucher_id): Path<Uuid>,
) -> impl IntoResponse {
    let attachment_repo = AttachmentRepo::new(&repo);
    match attachment_repo.list_by_voucher(voucher_id) {
        Ok(list) => (
            StatusCode::OK,
            Json(list.into_iter().map(|a| json!({
                "id": a.id.to_string(),
                "file_name": a.file_name,
                "content_type": a.content_type,
                "file_size": a.file_size,
                "sha256_hash": a.sha256_hash,
                "uploaded_at": a.uploaded_at,
            })).collect::<Vec<_>>()),
        ),
        Err(e) => {
            let (code, msg) = map_error(e);
            (code, Json(json!({"error": msg})))
        }
    }
}

/// GET /api/v1/vouchers/{voucher_id}/attachments/{att_id}/download
pub async fn download_attachment(
    State(config): State<AppConfig>,
    State(repo): State<LedgerRepository>,
    Path((voucher_id, att_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let attachment_repo = AttachmentRepo::new(&repo);
    match attachment_repo.get_attachment(att_id) {
        Ok(att) => {
            if att.voucher_id != voucher_id {
                return (StatusCode::BAD_REQUEST, Json(json!({"error": "Attachment does not belong to voucher"})));
            }
            let file_path = PathBuf::from(&config.attachment_dir).join(&att.sha256_hash);
            match fs::read(&file_path) {
                Ok(bytes) => {
                    let mut headers = HeaderMap::new();
                    headers.insert(axum::http::header::CONTENT_TYPE, att.content_type.parse().unwrap());
                    headers.insert(axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", att.file_name).parse().unwrap());
                    (headers, bytes)
                }
                Err(_) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found on disk"}))),
            }
        }
        Err(e) => {
            let (code, msg) = map_error(e);
            (code, Json(json!({"error": msg})))
        }
    }
}

fn map_error(err: AccountingError) -> (StatusCode, String) {
    match err {
        AccountingError::PeriodClosed => (StatusCode::UNPROCESSABLE_ENTITY, "Period is closed".to_string()),
        AccountingError::VoucherNotFound => (StatusCode::NOT_FOUND, "Voucher not found".to_string()),
        AccountingError::InvalidAttachmentSize => (StatusCode::PAYLOAD_TOO_LARGE, "Attachment exceeds maximum size".to_string()),
        AccountingError::AttachmentNotFound => (StatusCode::NOT_FOUND, "Attachment not found".to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", err)),
    }
}
