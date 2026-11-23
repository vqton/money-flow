use axum::{
    Json,
    extract::{Query, State},
};
use rust_decimal::Decimal;
use std::sync::Arc;

use crate::auth::{AuthenticatedUser, RequireController};
use crate::closing::PeriodClosingEngine;
use crate::dto::{
    ApiError, BalanceSheetQuery, BalanceSheetResponse, ClosePeriodRequest, IncomeStatementQuery,
    IncomeStatementResponse, PeriodStatusResponse, ReopenPeriodRequest, TrialBalanceResponse,
};
use crate::ledger::AccountingError;
use crate::models::AccountingPeriod;
use crate::state::AppState;

/// Route: GET /api/v1/reports/trial-balance
pub async fn get_trial_balance(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<TrialBalanceResponse>, ApiError> {
    let accounts = state.repo.compute_trial_balance()?;
    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;

    for row in &accounts {
        total_debit += row.total_debit;
        total_credit += row.total_credit;
    }

    let is_balanced = total_debit == total_credit;
    let as_of_date = chrono::Utc::now().date_naive();

    Ok(Json(TrialBalanceResponse {
        as_of_date,
        accounts,
        total_debit,
        total_credit,
        is_balanced,
    }))
}
/// Route: POST /api/v1/periods/close
pub async fn close_period(
    RequireController(user): RequireController,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClosePeriodRequest>,
) -> Result<Json<PeriodStatusResponse>, ApiError> {
    if req.month < 1 || req.month > 12 {
        return Err(ApiError::BadRequest(format!(
            "Invalid month {}: month must be between 1 and 12",
            req.month
        )));
    }

    // 1. Check if period is already closed
    if state.repo.is_period_closed(req.year, req.month)? {
        return Err(AccountingError::PeriodAlreadyClosed {
            year: req.year,
            month: req.month,
        }
        .into());
    }

    // 2. Build closing voucher via 4-step closing engine
    let mut closing_voucher =
        PeriodClosingEngine::build_closing_voucher(&state.repo, req.year, req.month)?;
    closing_voucher.created_by = user.claims.sub;

    // 3. Atomically persist closing voucher and mark period closed
    state
        .repo
        .close_period(req.year, req.month, &closing_voucher)?;

    // 4. Return updated period status
    let period = AccountingPeriod {
        year: req.year,
        month: req.month,
        is_closed: true,
        closed_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    Ok(Json(period.into()))
}

/// Handler for reopening a closed accounting period (Mở lại sổ kế toán).
/// Atomically deletes/voids the closing voucher (KC-YYYY-MM) and marks the period unlocked.
///
/// Route: POST /api/v1/periods/reopen
pub async fn reopen_period(
    RequireController(user): RequireController,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReopenPeriodRequest>,
) -> Result<Json<PeriodStatusResponse>, ApiError> {
    if req.month < 1 || req.month > 12 {
        return Err(ApiError::BadRequest(format!(
            "Invalid month {}: month must be between 1 and 12",
            req.month
        )));
    }

    // 1. Check if period is closed
    if !state.repo.is_period_closed(req.year, req.month)? {
        return Err(AccountingError::PeriodNotClosed {
            year: req.year,
            month: req.month,
        }
        .into());
    }

    // 2. Atomically void closing voucher and unlock period
    state
        .repo
        .reopen_period(req.year, req.month, &user.claims.sub)?;

    let period = AccountingPeriod {
        year: req.year,
        month: req.month,
        is_closed: false,
        closed_at: None,
    };

    Ok(Json(period.into()))
}

/// Handler for listing all tracked accounting periods and their lock statuses.
///
/// Route: GET /api/v1/periods
pub async fn list_periods(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PeriodStatusResponse>>, ApiError> {
    let periods = state.repo.list_periods()?;
    let responses: Vec<PeriodStatusResponse> = periods.into_iter().map(Into::into).collect();
    Ok(Json(responses))
}

/// Handler for retrieving the statutory Balance Sheet report (Bảng cân đối kế toán).
/// Applies contra-asset rules deducting Accumulated Depreciation (TK 214) and Provisions (TK 229).
///
/// Route: GET /api/v1/reports/balance-sheet?as_of_date=YYYY-MM-DD
pub async fn get_balance_sheet(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<BalanceSheetQuery>,
) -> Result<Json<BalanceSheetResponse>, ApiError> {
    let as_of_date = query
        .as_of_date
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    let report = state.repo.compute_balance_sheet(as_of_date)?;
    Ok(Json(report.into()))
}

/// Handler for retrieving the statutory Income Statement / P&L (Báo cáo KQHĐKD).
/// Computes gross revenue, deductions, net revenue, COGS, operating profit, and net profit after tax.
///
/// Route: GET /api/v1/reports/income-statement?from_date=YYYY-MM-DD&to_date=YYYY-MM-DD
pub async fn get_income_statement(
    _user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<IncomeStatementQuery>,
) -> Result<Json<IncomeStatementResponse>, ApiError> {
    if query.from_date > query.to_date {
        return Err(ApiError::BadRequest(format!(
            "Invalid date range: from_date ({}) must be <= to_date ({})",
            query.from_date, query.to_date
        )));
    }

    let report = state
        .repo
        .compute_income_statement(query.from_date, query.to_date)?;
    Ok(Json(report.into()))
}
