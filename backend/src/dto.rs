use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ledger::AccountingError;
use crate::models::{AccountingPeriod, BalanceSheetReport, IncomeStatementReport};

/// Request payload for creating a new double-entry journal voucher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateVoucherRequest {
    /// Unique voucher code (e.g. "PKT-2026-0001", "PC-0002").
    pub voucher_no: String,
    /// Accounting posting date (YYYY-MM-DD).
    pub voucher_date: NaiveDate,
    /// Description of the economic transaction.
    pub description: String,
    /// Optional reference to e-invoice code (Mã tra cứu HĐĐT Circular 91).
    #[serde(default)]
    pub invoice_ref: Option<String>,
    /// Optional creator / preparer identity (defaults to 'system' if omitted).
    #[serde(default)]
    pub created_by: Option<String>,
    /// Optional approver / Chief Accountant identity.
    #[serde(default)]
    pub approved_by: Option<String>,
    /// Debit and credit line entries (minimum 2 lines).
    pub lines: Vec<CreateVoucherLineRequest>,
}

/// Request payload for an individual debit or credit line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateVoucherLineRequest {
    /// Account code under Circular 99 (e.g. "1111", "1121", "1561", "331").
    pub account_code: String,
    /// Debit amount (Số tiền Nợ) - strictly non-negative.
    pub debit: Decimal,
    /// Credit amount (Số tiền Có) - strictly non-negative.
    pub credit: Decimal,
    /// Optional specific note for this line.
    #[serde(default)]
    pub line_desc: Option<String>,
}

/// Query parameters for filtering vouchers by date range.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DateRangeQuery {
    /// Inclusive start date (YYYY-MM-DD).
    pub from_date: Option<NaiveDate>,
    /// Inclusive end date (YYYY-MM-DD).
    pub to_date: Option<NaiveDate>,
}

/// Response payload for the statutory Trial Balance report (Bảng cân đối số phát sinh).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrialBalanceResponse {
    /// Reporting date as of when the balances are computed.
    pub as_of_date: NaiveDate,
    /// Per-account debit and credit totals.
    pub accounts: Vec<TrialBalanceRow>,
    /// Grand total of all debits across all accounts.
    pub total_debit: Decimal,
    /// Grand total of all credits across all accounts.
    pub total_credit: Decimal,
    /// Statutory balance check: true if total_debit == total_credit.
    pub is_balanced: bool,
}

/// Row representing aggregated debit/credit totals for a single account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrialBalanceRow {
    /// Circular 99 account code.
    pub account_code: String,
    /// Sum of all debit postings.
    pub total_debit: Decimal,
    /// Sum of all credit postings.
    pub total_credit: Decimal,
    /// Net balance: total_debit - total_credit.
    pub net_balance: Decimal,
}

// ============================================================================
// DTOs for Phase Epsilon (Period Closing & Statutory Financial Statements)
// ============================================================================

/// Request payload for closing an accounting period (Khóa sổ kế toán).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClosePeriodRequest {
    /// Calendar year (e.g. 2026).
    pub year: i32,
    /// Calendar month (1 to 12).
    pub month: u32,
}

/// Request payload for reopening a closed accounting period (Mở lại sổ kế toán).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReopenPeriodRequest {
    /// Calendar year (e.g. 2026).
    pub year: i32,
    /// Calendar month (1 to 12).
    pub month: u32,
    /// Optional business justification for unlocking (e.g. "Audit adjustment").
    #[serde(default)]
    pub reason: Option<String>,
}

/// Response payload representing the status of an accounting period.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeriodStatusResponse {
    /// Calendar year.
    pub year: i32,
    /// Calendar month.
    pub month: u32,
    /// Whether the period is locked against further voucher postings.
    pub is_closed: bool,
    /// Timestamp when the period was closed (None if open).
    pub closed_at: Option<String>,
}

impl From<AccountingPeriod> for PeriodStatusResponse {
    fn from(p: AccountingPeriod) -> Self {
        Self {
            year: p.year,
            month: p.month,
            is_closed: p.is_closed,
            closed_at: p.closed_at,
        }
    }
}

/// Request payload for creating an atomic SQLite ledger backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BackupRequest {
    /// Optional filename. If omitted, defaults to timestamped "accounting_YYYYMMDD_HHMMSS.db".
    #[serde(default)]
    pub filename: Option<String>,
}

/// Response payload for a successfully created database backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupResponse {
    pub status: String,
    pub backup_path: String,
    pub file_size_bytes: u64,
    pub created_at: String,
}

/// Request payload for voiding / cancelling an active journal voucher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoidVoucherRequest {
    /// Mandatory business or statutory audit justification for voiding.
    pub reason: String,
    /// Actor or auditor requesting the void (defaults to 'system' if omitted).
    #[serde(default)]
    pub actor: Option<String>,
}

/// Query parameters for forensic audit log queries.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuditLogQuery {
    /// Filter by actor identity.
    pub actor: Option<String>,
    /// Filter by action type (e.g. "CREATE_VOUCHER", "VOID_VOUCHER", "PERIOD_CLOSE", "PERIOD_REOPEN", "SYSTEM_BACKUP").
    pub action: Option<String>,
    /// Filter by resource type (e.g. "VOUCHER", "PERIOD", "SYSTEM").
    pub resource_type: Option<String>,
    /// Filter by resource ID.
    pub resource_id: Option<String>,
    /// Inclusive start timestamp (ISO 8601 string or date prefix).
    pub from_date: Option<String>,
    /// Inclusive end timestamp (ISO 8601 string or date prefix).
    pub to_date: Option<String>,
    /// Maximum number of records to return (defaults to 100, max 1000).
    pub limit: Option<usize>,
}

/// Query parameters for the statutory Balance Sheet report.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BalanceSheetQuery {
    /// Cut-off date for account balances (YYYY-MM-DD). If omitted, defaults to UTC today.
    pub as_of_date: Option<NaiveDate>,
}

/// Query parameters for the statutory Income Statement (P&L) report.
#[derive(Debug, Clone, Deserialize)]
pub struct IncomeStatementQuery {
    /// Inclusive start date (YYYY-MM-DD).
    pub from_date: NaiveDate,
    /// Inclusive end date (YYYY-MM-DD).
    pub to_date: NaiveDate,
}

/// Response payload for the statutory Balance Sheet report (Bảng cân đối kế toán)
/// compliant with Circular 99/2025/TT-BTC contra-asset rules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BalanceSheetResponse {
    /// Reporting cut-off date.
    pub as_of_date: NaiveDate,
    /// Current assets (Class 1 accounts).
    pub current_assets: Decimal,
    /// Non-current assets (Class 2 accounts, before deducting contra-assets).
    pub non_current_assets: Decimal,
    /// Accumulated depreciation (TK 214, deducted from assets).
    pub accumulated_depreciation: Decimal,
    /// Asset provisions (TK 229, deducted from assets).
    pub asset_provisions: Decimal,
    /// Total assets = (current_assets + non_current_assets) - (accumulated_depreciation + asset_provisions).
    pub total_assets: Decimal,
    /// Liabilities (Class 3 accounts).
    pub liabilities: Decimal,
    /// Owner's equity (Class 4 accounts).
    pub equity: Decimal,
    /// Total resources = liabilities + equity.
    pub total_resources: Decimal,
    /// Statutory balance check: true if total_assets == total_resources.
    pub is_balanced: bool,
}

impl From<BalanceSheetReport> for BalanceSheetResponse {
    fn from(r: BalanceSheetReport) -> Self {
        Self {
            as_of_date: r.as_of_date,
            current_assets: r.current_assets,
            non_current_assets: r.non_current_assets,
            accumulated_depreciation: r.accumulated_depreciation,
            asset_provisions: r.asset_provisions,
            total_assets: r.total_assets,
            liabilities: r.liabilities,
            equity: r.equity,
            total_resources: r.total_resources,
            is_balanced: r.is_balanced,
        }
    }
}

/// Response payload for the statutory Income Statement / P&L (Báo cáo kết quả hoạt động kinh doanh)
/// compliant with Circular 99/2025/TT-BTC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncomeStatementResponse {
    /// Reporting period start date (inclusive).
    pub from_date: NaiveDate,
    /// Reporting period end date (inclusive).
    pub to_date: NaiveDate,
    /// Gross revenue from sales & services (TK 511).
    pub gross_revenue: Decimal,
    /// Revenue deductions (TK 521: discounts, allowances, sales returns).
    pub revenue_deductions: Decimal,
    /// Net revenue = gross_revenue - revenue_deductions.
    pub net_revenue: Decimal,
    /// Cost of goods sold (TK 632).
    pub cost_of_goods_sold: Decimal,
    /// Gross profit = net_revenue - cost_of_goods_sold.
    pub gross_profit: Decimal,
    /// Financial income (TK 515).
    pub financial_income: Decimal,
    /// Financial expenses (TK 635).
    pub financial_expenses: Decimal,
    /// Selling expenses (TK 641).
    pub selling_expenses: Decimal,
    /// General & administrative expenses (TK 642).
    pub administrative_expenses: Decimal,
    /// Operating profit = gross_profit + financial_income - financial_expenses - selling_expenses - administrative_expenses.
    pub operating_profit: Decimal,
    /// Other income (TK 711).
    pub other_income: Decimal,
    /// Other expenses (TK 811).
    pub other_expenses: Decimal,
    /// Net profit before corporate tax = operating_profit + (other_income - other_expenses).
    pub net_profit_before_tax: Decimal,
    /// Corporate income tax expense (TK 821).
    pub corporate_tax: Decimal,
    /// Net profit after corporate tax = net_profit_before_tax - corporate_tax.
    pub net_profit_after_tax: Decimal,
}

impl From<IncomeStatementReport> for IncomeStatementResponse {
    fn from(r: IncomeStatementReport) -> Self {
        Self {
            from_date: r.from_date,
            to_date: r.to_date,
            gross_revenue: r.gross_revenue,
            revenue_deductions: r.revenue_deductions,
            net_revenue: r.net_revenue,
            cost_of_goods_sold: r.cost_of_goods_sold,
            gross_profit: r.gross_profit,
            financial_income: r.financial_income,
            financial_expenses: r.financial_expenses,
            selling_expenses: r.selling_expenses,
            administrative_expenses: r.administrative_expenses,
            operating_profit: r.operating_profit,
            other_income: r.other_income,
            other_expenses: r.other_expenses,
            net_profit_before_tax: r.net_profit_before_tax,
            corporate_tax: r.corporate_tax,
            net_profit_after_tax: r.net_profit_after_tax,
        }
    }
}

/// Unified API error domain for HTTP handlers.
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
pub fn compute_request_hash(req: &CreateVoucherRequest) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(req)?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let mut hash_hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write;
        let _ = write!(hash_hex, "{:02x}", byte);
    }
    Ok(hash_hex)
}
