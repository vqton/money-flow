use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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

/// Request payload for restoring the accounting database from a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestoreRequest {
    /// Filesystem path to the validated SQLite snapshot file.
    pub snapshot_path: String,
    /// Explicit confirmation flag required to prevent accidental data overwrite.
    pub confirm_overwrite: bool,
    /// Expected SHA-256 checksum (hex-encoded, 64 characters) of the snapshot file.
    pub expected_sha256: String,
}

/// Response payload for a successfully executed disaster restore operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestoreResponse {
    /// Status confirmation (e.g., "RESTORED").
    pub status: String,
    /// RFC-3339 UTC timestamp when the restore committed.
    pub timestamp: String,
    /// Path of the snapshot file that was restored.
    pub restored_from_snapshot: String,
    /// Verified SHA-256 checksum of the restored snapshot.
    pub sha256_checksum: String,
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
