use super::error::ApiError;
use axum::{extract::FromRequestParts, http::request::Parts};
use serde::Serialize;

/// Validates an `Idempotency-Key` header value:
/// - Must not be empty or whitespace-only (1..=255 characters after trimming)
/// - Must contain only printable ASCII characters (32..=126)
pub fn validate_idempotency_key(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Idempotency-Key header cannot be empty or whitespace-only".to_string());
    }
    if trimmed.len() > 255 {
        return Err("Idempotency-Key header length exceeds 255 characters".to_string());
    }
    if !trimmed.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
        return Err(
            "Idempotency-Key header must contain only printable ASCII characters".to_string(),
        );
    }
    Ok(trimmed.to_string())
}

/// Extractor for the optional `Idempotency-Key` HTTP header.
/// Enforces length 1..=255 and printable ASCII characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey(pub Option<String>);

impl<S> FromRequestParts<S> for IdempotencyKey
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.headers.get("idempotency-key") {
            None => Ok(IdempotencyKey(None)),
            Some(val) => {
                let s = val.to_str().map_err(|_| {
                    ApiError::BadRequest(
                        "Idempotency-Key header contains non-ASCII characters".to_string(),
                    )
                })?;
                validate_idempotency_key(s)
                    .map(|k| IdempotencyKey(Some(k)))
                    .map_err(ApiError::BadRequest)
            }
        }
    }
}

/// Computes a canonical SHA-256 request payload hash as a 64-character lowercase hex string.
pub fn compute_request_hash<T: Serialize>(req: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(req)?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let mut hash_hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write;
        let _ = write!(hash_hex, "{:02x}", byte);
    }
    Ok(hash_hex)
}
