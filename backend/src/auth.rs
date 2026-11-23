use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::dto::ApiError;
use crate::state::AppState;

/// User roles for Role-Based Access Control (RBAC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Standard accountant role: can create vouchers, query data, run statutory reports.
    #[serde(alias = "Accountant", alias = "ACCOUNTANT")]
    Accountant,
    /// Privileged controller role: can void vouchers, close/reopen periods, create backups.
    #[serde(alias = "Controller", alias = "CONTROLLER")]
    Controller,
    /// Administrative role: superuser access to all accounting and system operations.
    #[serde(alias = "Admin", alias = "ADMIN")]
    Admin,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Accountant => write!(f, "accountant"),
            Role::Controller => write!(f, "controller"),
            Role::Admin => write!(f, "admin"),
        }
    }
}

/// JSON Web Token standard payload claims.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// Subject (unique user identifier / username / actor identity).
    pub sub: String,
    /// User RBAC role.
    pub role: Role,
    /// Expiration time (UNIX epoch seconds).
    pub exp: usize,
}

/// Extractor representing a successfully authenticated HTTP caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedUser {
    /// Decoded and verified JWT claims.
    pub claims: Claims,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<AppState>::from_ref(state);

        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".to_string()))?
            .to_str()
            .map_err(|_| {
                ApiError::Unauthorized("Invalid Authorization header format".to_string())
            })?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .or_else(|| auth_header.strip_prefix("bearer "))
            .ok_or_else(|| {
                ApiError::Unauthorized(
                    "Invalid Authorization header scheme: must start with 'Bearer '".to_string(),
                )
            })?
            .trim();

        if token.is_empty() {
            return Err(ApiError::Unauthorized("Token is empty".to_string()));
        }

        let claims = decode_jwt(token, &app_state.config.jwt_secret).map_err(|e| {
            if let jsonwebtoken::errors::ErrorKind::ExpiredSignature = e.kind() {
                ApiError::Unauthorized("Token has expired".to_string())
            } else {
                ApiError::Unauthorized(format!("Invalid token: {e}"))
            }
        })?;

        Ok(AuthenticatedUser { claims })
    }
}

/// Extractor enforcing Controller or Admin role privilege on sensitive operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireController(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for RequireController
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        if user.claims.role != Role::Controller && user.claims.role != Role::Admin {
            return Err(ApiError::Forbidden(
                "Access forbidden: Controller role required".to_string(),
            ));
        }
        Ok(RequireController(user))
    }
}

/// Extractor enforcing Admin role privilege on administrative and disaster recovery operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireAdmin(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for RequireAdmin
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        if user.claims.role != Role::Admin {
            return Err(ApiError::Forbidden(
                "Access forbidden: Admin role required".to_string(),
            ));
        }
        Ok(RequireAdmin(user))
    }
}

/// Encodes given claims into a signed HS256 JWT string.
pub fn create_jwt(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let header = Header::default();
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&header, claims, &key)
}

/// Decodes and validates a signed HS256 JWT string against the secret.
pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let data = decode::<Claims>(token, &key, &validation)?;
    Ok(data.claims)
}

/// Generates a test JWT with specified subject, role, relative expiration delta, and signing secret.
pub fn generate_test_token(sub: &str, role: Role, exp_delta_secs: i64, secret: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    let exp = (now + exp_delta_secs) as usize;
    let claims = Claims {
        sub: sub.to_string(),
        role,
        exp,
    };
    create_jwt(&claims, secret).expect("Failed to encode test JWT token")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-with-minimum-32-bytes-length-12345";

    #[test]
    fn test_role_serialization_and_display() {
        assert_eq!(Role::Accountant.to_string(), "accountant");
        assert_eq!(Role::Controller.to_string(), "controller");
        assert_eq!(Role::Admin.to_string(), "admin");

        let acc_json = serde_json::to_string(&Role::Accountant).unwrap();
        assert_eq!(acc_json, "\"accountant\"");

        let de_acc: Role = serde_json::from_str("\"accountant\"").unwrap();
        assert_eq!(de_acc, Role::Accountant);

        let de_ctrl: Role = serde_json::from_str("\"controller\"").unwrap();
        assert_eq!(de_ctrl, Role::Controller);

        let de_admin: Role = serde_json::from_str("\"admin\"").unwrap();
        assert_eq!(de_admin, Role::Admin);

        let de_admin_cap: Role = serde_json::from_str("\"Admin\"").unwrap();
        assert_eq!(de_admin_cap, Role::Admin);

        let de_admin_upper: Role = serde_json::from_str("\"ADMIN\"").unwrap();
        assert_eq!(de_admin_upper, Role::Admin);
    }

    #[test]
    fn test_jwt_encode_decode_roundtrip() {
        let token = generate_test_token("test_user", Role::Accountant, 3600, TEST_SECRET);
        let claims = decode_jwt(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "test_user");
        assert_eq!(claims.role, Role::Accountant);
    }

    #[test]
    fn test_jwt_expired_rejection() {
        let token = generate_test_token("expired_user", Role::Controller, -100, TEST_SECRET);
        let err = decode_jwt(&token, TEST_SECRET).unwrap_err();
        assert_eq!(
            err.kind(),
            &jsonwebtoken::errors::ErrorKind::ExpiredSignature
        );
    }

    #[test]
    fn test_jwt_invalid_signature_rejection() {
        let token = generate_test_token("tampered_user", Role::Controller, 3600, TEST_SECRET);
        let err = decode_jwt(
            &token,
            "wrong-secret-key-with-minimum-32-bytes-length-99999",
        )
        .unwrap_err();
        assert_eq!(
            err.kind(),
            &jsonwebtoken::errors::ErrorKind::InvalidSignature
        );
    }
}
