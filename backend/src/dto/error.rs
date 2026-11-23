use crate::ledger::AccountingError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Unbalanced voucher: {0}")]
    UnbalancedVoucher(String),

    #[error("Accounting period is closed: {0}")]
    PeriodClosed(String),

    #[error("Voucher already voided: {0}")]
    VoucherAlreadyVoided(String),

    #[error("Cannot void voucher in closed period: {0}")]
    CannotVoidInClosedPeriod(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal database error: {0}")]
    InternalError(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Unknown account: {0}")]
    UnknownAccount(String),

    #[error("Account inactive: {0}")]
    InactiveAccount(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::Unauthorized(ref msg) => {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.as_str())
            }
            ApiError::Forbidden(ref msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.as_str()),
            ApiError::UnbalancedVoucher(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNBALANCED_VOUCHER",
                msg.as_str(),
            ),
            ApiError::PeriodClosed(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "PERIOD_CLOSED",
                msg.as_str(),
            ),
            ApiError::VoucherAlreadyVoided(ref msg) => (
                StatusCode::BAD_REQUEST,
                "VOUCHER_ALREADY_VOIDED",
                msg.as_str(),
            ),
            ApiError::CannotVoidInClosedPeriod(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "CANNOT_VOID_IN_CLOSED_PERIOD",
                msg.as_str(),
            ),
            ApiError::UnknownAccount(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNKNOWN_ACCOUNT",
                msg.as_str(),
            ),
            ApiError::InactiveAccount(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "ACCOUNT_INACTIVE",
                msg.as_str(),
            ),
            ApiError::UnprocessableEntity(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNPROCESSABLE_ENTITY",
                msg.as_str(),
            ),
            ApiError::BadRequest(ref msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.as_str()),
            ApiError::NotFound(ref msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.as_str()),
            ApiError::Conflict(ref msg) => (StatusCode::CONFLICT, "CONFLICT", msg.as_str()),
            ApiError::InternalError(ref msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg.as_str(),
            ),
        };

        let body = Json(json!({
            "error": code,
            "message": message,
        }));

        if status.is_server_error() {
            tracing::error!(
                status = %status.as_u16(),
                error_code = code,
                error_message = %message,
                "API internal server error occurred"
            );
        } else {
            tracing::warn!(
                status = %status.as_u16(),
                error_code = code,
                error_message = %message,
                "API request rejected with client error"
            );
        }

        (status, body).into_response()
    }
}

impl From<AccountingError> for ApiError {
    fn from(err: AccountingError) -> Self {
        match err {
            AccountingError::UnbalancedVoucher {
                voucher_no,
                total_debit,
                total_credit,
            } => ApiError::UnbalancedVoucher(format!(
                "Voucher {} is unbalanced: total debit ({}) != total credit ({})",
                voucher_no, total_debit, total_credit
            )),
            AccountingError::NegativeAmount { account_code } => ApiError::BadRequest(format!(
                "Negative amounts are forbidden in accounting entries. Account: {}",
                account_code
            )),
            AccountingError::EmptyOrSingleLineVoucher => ApiError::BadRequest(
                "Voucher must have at least 2 lines (at least one debit and one credit)"
                    .to_string(),
            ),
            AccountingError::InvalidLine {
                account_code,
                reason,
            } => ApiError::BadRequest(format!(
                "Invalid line for account {}: {}",
                account_code, reason
            )),
            AccountingError::ZeroValueVoucher => ApiError::BadRequest(
                "Zero total value: transaction must have positive monetary volume".to_string(),
            ),
            AccountingError::PeriodClosed { year, month } => ApiError::PeriodClosed(format!(
                "Accounting period {:04}-{:02} is closed for postings",
                year, month
            )),
            AccountingError::PeriodAlreadyClosed { year, month } => ApiError::BadRequest(format!(
                "Accounting period {:04}-{:02} is already closed",
                year, month
            )),
            AccountingError::PeriodNotClosed { year, month } => ApiError::BadRequest(format!(
                "Accounting period {:04}-{:02} is not closed",
                year, month
            )),
            AccountingError::InvalidPeriod { year, month } => ApiError::BadRequest(format!(
                "Invalid accounting period: year={}, month={}. Month must be between 1 and 12",
                year, month
            )),
            AccountingError::VoucherAlreadyVoided { voucher_no } => {
                ApiError::VoucherAlreadyVoided(format!("Voucher {} is already voided", voucher_no))
            }
            AccountingError::CannotVoidInClosedPeriod {
                voucher_no,
                year,
                month,
            } => ApiError::CannotVoidInClosedPeriod(format!(
                "Cannot void voucher {} in closed period {:04}-{:02}",
                voucher_no, year, month
            )),
            AccountingError::VoucherNotFound(id) => {
                ApiError::NotFound(format!("Voucher with id '{}' not found", id))
            }
            AccountingError::IdempotencyConflict { key } => ApiError::Conflict(format!(
                "Idempotency conflict for key '{}': request payload does not match original request",
                key
            )),
            AccountingError::UnknownAccount { account_code } => ApiError::UnknownAccount(format!(
                "Account {} does not exist in chart of accounts",
                account_code
            )),
            AccountingError::InactiveAccount { account_code } => {
                ApiError::InactiveAccount(format!("Account {} is inactive", account_code))
            }
            AccountingError::DuplicateAccount(msg) => ApiError::Conflict(msg),
            AccountingError::ParentAccountNotFound(msg) => ApiError::BadRequest(msg),
            AccountingError::ParentAccountInactive(msg) => ApiError::UnprocessableEntity(msg),
            AccountingError::CircularAccountReference(msg) => ApiError::BadRequest(msg),
            AccountingError::DatabaseError(msg) => {
                if msg.contains("UNIQUE constraint failed: vouchers.voucher_no") {
                    ApiError::BadRequest("Voucher number already exists".to_string())
                } else {
                    ApiError::InternalError(msg)
                }
            }
        }
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(err: rusqlite::Error) -> Self {
        match &err {
            rusqlite::Error::SqliteFailure(_err_code, Some(msg)) => {
                if msg.contains("UNIQUE constraint failed: vouchers.voucher_no") {
                    ApiError::BadRequest("Voucher number already exists".to_string())
                } else {
                    ApiError::InternalError(format!("Database error: {}", msg))
                }
            }
            _ => ApiError::InternalError(err.to_string()),
        }
    }
}
