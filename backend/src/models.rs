use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lifecycle status for an accounting voucher (Trạng thái chứng từ).
/// Enforces non-destructive audit compliance per Law on Accounting 88/2015/QH13.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoucherStatus {
    /// Initial draft status: unapproved, excluded from general ledger and financial reports.
    Draft,
    /// Active, balanced, and posted into statutory general ledger balances.
    #[default]
    Posted,
    /// Voided / cancelled for audit remediation; excluded from statutory reports
    /// but permanently preserved in the relational ledger for forensic trail.
    Voided,
}

impl std::fmt::Display for VoucherStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoucherStatus::Draft => write!(f, "DRAFT"),
            VoucherStatus::Posted => write!(f, "POSTED"),
            VoucherStatus::Voided => write!(f, "VOIDED"),
        }
    }
}

impl std::str::FromStr for VoucherStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DRAFT" => Ok(VoucherStatus::Draft),
            "POSTED" => Ok(VoucherStatus::Posted),
            "VOIDED" => Ok(VoucherStatus::Voided),
            other => Err(format!("Unknown voucher status: {}", other)),
        }
    }
}

/// Represents a statutory accounting voucher header (Chứng từ kế toán).
/// Follows Vietnamese Accounting Standards (Circular 99/2025/TT-BTC)
/// and E-Invoice metadata linkages (Circular 91/2026/TT-BTC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JournalVoucher {
    /// Unique internal identifier for the voucher.
    pub id: Uuid,
    /// Official voucher code (e.g. "PKT-2026-0001", "PC-0002", "PT-0003").
    pub voucher_no: String,
    /// Statutory date of the transaction / accounting posting date.
    pub voucher_date: NaiveDate,
    /// Business description of the economic event.
    pub description: String,
    /// Reference to external e-invoice lookup code (Mã tra cứu hóa đơn điện tử).
    pub invoice_ref: Option<String>,
    /// Lifecycle status: POSTED (active in general ledger) or VOIDED (cancelled, excluded from reports).
    pub status: VoucherStatus,
    /// User or service identity who prepared/created the transaction.
    pub created_by: String,
    /// Optional Chief Accountant or authorized approver identity.
    pub approved_by: Option<String>,
    /// ISO 8601 UTC timestamp when the voucher was voided (None if POSTED).
    pub voided_at: Option<String>,
    /// User or authority identity who authorized the voiding (None if POSTED).
    pub voided_by: Option<String>,
    /// Mandatory business or statutory reason for voiding (None if POSTED).
    pub void_reason: Option<String>,
    /// ISO 8601 UTC timestamp when the voucher was created.
    pub created_at: String,
    /// Individual debit and credit legs of the transaction.
    pub lines: Vec<VoucherLine>,
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use uuid::Uuid;
    use chrono::Utc;

    #[test]
    fn voucher_attachment_serde_roundtrip() {
        let att = VoucherAttachment {
            id: Uuid::new_v4(),
            voucher_id: Uuid::new_v4(),
            file_name: "invoice.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            file_size: 12345,
            sha256_hash: "deadbeef".repeat(8), // 64 hex chars
            uploaded_by: "alice".to_string(),
            uploaded_at: Utc::now().to_rfc3339(),
        };
        let json = serde_json::to_string(&att).expect("serialize");
        let de: VoucherAttachment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(att, de);
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoucherLine {
    /// Unique identifier for the voucher leg.
    pub id: Uuid,
    /// 1-based sequential line ordering within the voucher.
    pub line_no: u32,
    /// Account code under Circular 99 (e.g. "1111", "1121", "1561", "331", "5111").
    pub account_code: String,
    /// Debit amount (Số tiền Nợ) - strictly non-negative.
    pub debit: Decimal,
    /// Credit amount (Số tiền Có) - strictly non-negative.
    pub credit: Decimal,
    /// Optional specific description for this line item.
    pub line_desc: Option<String>,
}

/// Dedicated immutable audit trail entry recording high-impact operational and governance events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditLogEntry {
    /// Unique audit log record identifier.
    pub id: Uuid,
    /// ISO 8601 UTC timestamp when the action occurred.
    pub timestamp: String,
    /// User, role, or system identity responsible for the event.
    pub actor: String,
    /// Operational or governance action (e.g. "CREATE_VOUCHER", "VOID_VOUCHER", "PERIOD_CLOSE", "PERIOD_REOPEN", "SYSTEM_BACKUP").
    pub action: String,
    /// Category of resource affected (e.g. "VOUCHER", "PERIOD", "SYSTEM").
    pub resource_type: String,
    /// Identifier or business reference of the target resource.
    pub resource_id: Option<String>,
    /// Additional context or operational parameters (JSON or textual details).
    pub details: Option<String>,
    /// Client IP address or network origin.
    pub client_ip: Option<String>,
}

/// Represents an accounting period lock status (Kỳ kế toán).
/// Governs whether retroactive postings are permitted for a specific year and month.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountingPeriod {
    /// Four-digit fiscal year (e.g. 2026).
    pub year: i32,
    /// Calendar month (1 to 12).
    pub month: u32,
    /// Lock status: true if the period has been closed and locked against postings.
    pub is_closed: bool,
    /// ISO 8601 UTC timestamp when the period was officially closed (None if open).
    pub closed_at: Option<String>,
}

impl AccountingPeriod {
    pub fn new_open(year: i32, month: u32) -> Self {
        Self {
            year,
            month,
            is_closed: false,
            closed_at: None,
        }
    }

    pub fn new_closed(year: i32, month: u32, closed_at: String) -> Self {
        Self {
            year,
            month,
            is_closed: true,
            closed_at: Some(closed_at),
        }
    }

    pub fn closing_voucher_no(year: i32, month: u32) -> String {
        format!("KC-{year}-{month:02}")
    }
}

/// Statutory Balance Sheet Report (Bảng cân đối kế toán) under Circular 99/2025/TT-BTC.
/// Implements contra-asset accounting deductions for accumulated depreciation (214) and provisions (229).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BalanceSheetReport {
    /// Cutoff date as of which ledger balances are aggregated.
    pub as_of_date: NaiveDate,
    /// Gross current assets (Class 1 accounts: 111, 112, 121, 131, 133, 141, 151-158).
    pub current_assets: Decimal,
    /// Gross non-current assets (Class 2 accounts: 211, 213, 217, 241, 242, 244, etc. excluding 214 and 229).
    pub non_current_assets: Decimal,
    /// Accumulated depreciation (TK 214 credit balance, contra-asset deducted from assets).
    pub accumulated_depreciation: Decimal,
    /// Asset impairment provisions (TK 229 credit balance, contra-asset deducted from assets).
    pub asset_provisions: Decimal,
    /// Net total assets: current_assets + non_current_assets - (accumulated_depreciation + asset_provisions).
    pub total_assets: Decimal,
    /// Total liabilities (Class 3 accounts: 331, 333, 334, 338, 341, etc.).
    pub liabilities: Decimal,
    /// Total equity (Class 4 accounts: 411, 418, 4211, 4212, etc.).
    pub equity: Decimal,
    /// Total resources: liabilities + equity.
    pub total_resources: Decimal,
    /// Statutory balance check: true if total_assets == total_resources.
    pub is_balanced: bool,
}

impl BalanceSheetReport {
    pub fn new(
        as_of_date: NaiveDate,
        current_assets: Decimal,
        non_current_assets: Decimal,
        accumulated_depreciation: Decimal,
        asset_provisions: Decimal,
        liabilities: Decimal,
        equity: Decimal,
    ) -> Self {
        let total_assets =
            current_assets + non_current_assets - (accumulated_depreciation + asset_provisions);
        let total_resources = liabilities + equity;
        let is_balanced = total_assets == total_resources;

        Self {
            as_of_date,
            current_assets,
            non_current_assets,
            accumulated_depreciation,
            asset_provisions,
            total_assets,
            liabilities,
            equity,
            total_resources,
            is_balanced,
        }
    }
}

/// Statutory Income Statement / P&L Report (Báo cáo kết quả hoạt động kinh doanh) under Circular 99/2025/TT-BTC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncomeStatementReport {
    /// Beginning date of reporting period.
    pub from_date: NaiveDate,
    /// Ending date of reporting period.
    pub to_date: NaiveDate,
    /// Gross revenue from sales & services (TK 511 credit).
    pub gross_revenue: Decimal,
    /// Revenue deductions: discounts, returns, rebates (TK 521 debit).
    pub revenue_deductions: Decimal,
    /// Net revenue: gross_revenue - revenue_deductions.
    pub net_revenue: Decimal,
    /// Cost of Goods Sold (TK 632 debit).
    pub cost_of_goods_sold: Decimal,
    /// Gross profit: net_revenue - cost_of_goods_sold.
    pub gross_profit: Decimal,
    /// Financial income: interest, dividends, forex gains (TK 515 credit).
    pub financial_income: Decimal,
    /// Financial expenses: loan interest, forex losses (TK 635 debit).
    pub financial_expenses: Decimal,
    /// Selling expenses (TK 641 debit).
    pub selling_expenses: Decimal,
    /// General & administrative expenses (TK 642 debit).
    pub administrative_expenses: Decimal,
    /// Operating profit: gross_profit + financial_income - financial_expenses - selling_expenses - administrative_expenses.
    pub operating_profit: Decimal,
    /// Other income: asset disposals, penalties received (TK 711 credit).
    pub other_income: Decimal,
    /// Other expenses (TK 811 debit).
    pub other_expenses: Decimal,
    /// Net accounting profit before tax: operating_profit + (other_income - other_expenses).
    pub net_profit_before_tax: Decimal,
    /// Corporate income tax expense (TK 821 debit).
    pub corporate_tax: Decimal,
    /// Net profit after tax: net_profit_before_tax - corporate_tax.
    pub net_profit_after_tax: Decimal,
}

impl IncomeStatementReport {
    #[allow(clippy::too_many_arguments)]
    pub fn calculate(
        from_date: NaiveDate,
        to_date: NaiveDate,
        gross_revenue: Decimal,
        revenue_deductions: Decimal,
        cost_of_goods_sold: Decimal,
        financial_income: Decimal,
        financial_expenses: Decimal,
        selling_expenses: Decimal,
        administrative_expenses: Decimal,
        other_income: Decimal,
        other_expenses: Decimal,
        corporate_tax: Decimal,
    ) -> Self {
        let net_revenue = gross_revenue - revenue_deductions;
        let gross_profit = net_revenue - cost_of_goods_sold;
        let operating_profit = gross_profit + financial_income
            - financial_expenses
            - selling_expenses
            - administrative_expenses;
        let net_profit_before_tax = operating_profit + (other_income - other_expenses);
        let net_profit_after_tax = net_profit_before_tax - corporate_tax;

        Self {
            from_date,
            to_date,
            gross_revenue,
            revenue_deductions,
            net_revenue,
            cost_of_goods_sold,
            gross_profit,
            financial_income,
            financial_expenses,
            selling_expenses,
            administrative_expenses,
            operating_profit,
            other_income,
            other_expenses,
            net_profit_before_tax,
            corporate_tax,
            net_profit_after_tax,
        }
    }
}

/// Immutable idempotency tracking entry for API deduplication and replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdempotencyRecord {
    pub key: String,
    pub actor: String,
    pub endpoint: String,
    pub request_hash: String,
    pub status_code: u16,
    pub response_body: String,
    pub resource_id: Option<String>,
    pub created_at: String,
}

/// Account classification according to Vietnamese Accounting Standards (Circular 99/2025/TT-BTC).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountClass {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
    Closing,
}

impl std::fmt::Display for AccountClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asset => write!(f, "ASSET"),
            Self::Liability => write!(f, "LIABILITY"),
            Self::Equity => write!(f, "EQUITY"),
            Self::Revenue => write!(f, "REVENUE"),
            Self::Expense => write!(f, "EXPENSE"),
            Self::Closing => write!(f, "CLOSING"),
        }
    }
}

impl std::str::FromStr for AccountClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("ASSET") {
            Ok(Self::Asset)
        } else if trimmed.eq_ignore_ascii_case("LIABILITY") {
            Ok(Self::Liability)
        } else if trimmed.eq_ignore_ascii_case("EQUITY") {
            Ok(Self::Equity)
        } else if trimmed.eq_ignore_ascii_case("REVENUE") {
            Ok(Self::Revenue)
        } else if trimmed.eq_ignore_ascii_case("EXPENSE") {
            Ok(Self::Expense)
        } else if trimmed.eq_ignore_ascii_case("CLOSING") {
            Ok(Self::Closing)
        } else {
            Err(format!("Unknown account class: {s}"))
        }
    }
}

/// Normal balance rule for double-entry accounts (Debit or Credit).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NormalBalance {
    Debit,
    Credit,
}

impl std::fmt::Display for NormalBalance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debit => write!(f, "DEBIT"),
            Self::Credit => write!(f, "CREDIT"),
        }
    }
}

impl std::str::FromStr for NormalBalance {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("DEBIT") {
            Ok(Self::Debit)
        } else if trimmed.eq_ignore_ascii_case("CREDIT") {
            Ok(Self::Credit)
        } else {
            Err(format!("Unknown normal balance: {s}"))
        }
    }
}

/// Account entity representing a node in the dynamic chart of accounts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub code: String,
    pub name: String,
    pub parent_code: Option<String>,
    pub class: AccountClass,
    pub is_active: bool,
    pub normal_balance: NormalBalance,
}
