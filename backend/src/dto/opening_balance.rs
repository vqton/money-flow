//! DTOs for Opening Balance import

use rust_decimal::Decimal;
use chrono::NaiveDate;

/// A single line in the opening‑balance import payload.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct OpeningBalanceLine {
    /// Account code (TT‑99) – must exist in `accounts` table.
    pub account_code: String,
    /// Debit amount – exactly one of `debit` / `credit` must be `Some`.
    #[serde(default, deserialize_with = "crate::dto::deserialize_decimal_opt")]
    pub debit: Option<Decimal>,
    /// Credit amount – exactly one of `debit` / `credit` must be `Some`.
    #[serde(default, deserialize_with = "crate::dto::deserialize_decimal_opt")]
    pub credit: Option<Decimal>,
    /// Optional detail key required for certain statutory accounts (e.g. 138).
    pub detail_key: Option<String>,
}

/// Request body for `POST /api/v1/opening-balances`.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct OpeningBalanceRequest {
    /// Fiscal year the opening balances belong to.
    pub fiscal_year: i32,
    /// Effective date of the opening balances (usually first day of fiscal year).
    pub effective_date: NaiveDate,
    /// Collection of balance lines.
    pub lines: Vec<OpeningBalanceLine>,
}
