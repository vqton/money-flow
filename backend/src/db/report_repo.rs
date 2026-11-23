//! Read-only statutory reporting queries (Trial Balance, Balance Sheet, Income Statement).

use crate::database::LedgerRepository;
use crate::dto::TrialBalanceRow;
use crate::ledger::AccountingError;
use crate::models::{BalanceSheetReport, IncomeStatementReport};
use chrono::NaiveDate;
use rusqlite::params;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;

impl LedgerRepository {
    /// Retrieves account balances across a specified date range, excluding closing vouchers and voided entries.
    pub fn get_account_balances_for_range(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<BTreeMap<String, (Decimal, Decimal)>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.voucher_date >= ?1
               AND v.voucher_date <= ?2
               AND v.voucher_no NOT LIKE 'KC-%'
               AND v.status = 'POSTED'
             ORDER BY l.account_code ASC",
        )?;

        let from_str = from_date.to_string();
        let to_str = to_date.to_string();

        let rows = stmt.query_map(params![from_str, to_str], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        let mut map = BTreeMap::new();
        for r in rows {
            let (account_code, debit, credit) = r?;
            let entry = map
                .entry(account_code)
                .or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        Ok(map)
    }

    /// Computes the statutory Balance Sheet as of a specified date, strictly enforcing
    /// the contra-asset deduction formula under Circular 99/2025/TT-BTC and isolating active POSTED vouchers.
    pub fn compute_balance_sheet(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<BalanceSheetReport, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.voucher_date <= ?1
               AND v.status = 'POSTED'",
        )?;

        let as_of_str = as_of_date.to_string();
        let rows = stmt.query_map(params![as_of_str], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        let mut balances: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
        for r in rows {
            let (account_code, debit, credit) = r?;
            let entry = balances
                .entry(account_code)
                .or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        let mut current_assets = Decimal::ZERO;
        let mut non_current_assets = Decimal::ZERO;
        let mut accumulated_depreciation = Decimal::ZERO;
        let mut asset_provisions = Decimal::ZERO;
        let mut liabilities = Decimal::ZERO;
        let mut equity = Decimal::ZERO;

        for (code, (debit, credit)) in &balances {
            if code.starts_with("214") {
                accumulated_depreciation += credit - debit;
            } else if code.starts_with("229") {
                asset_provisions += credit - debit;
            } else if code.starts_with('1') {
                current_assets += debit - credit;
            } else if code.starts_with('2') {
                non_current_assets += debit - credit;
            } else if code.starts_with('3') {
                liabilities += credit - debit;
            } else if code.starts_with('4') {
                equity += credit - debit;
            }
        }

        let total_assets =
            current_assets + non_current_assets - (accumulated_depreciation + asset_provisions);
        let total_resources = liabilities + equity;
        let is_balanced = total_assets == total_resources;

        Ok(BalanceSheetReport {
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
        })
    }

    /// Computes the statutory Income Statement across a specified date range, isolating active POSTED vouchers.
    pub fn compute_income_statement(
        &self,
        from_date: NaiveDate,
        to_date: NaiveDate,
    ) -> Result<IncomeStatementReport, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.voucher_date >= ?1
               AND v.voucher_date <= ?2
               AND v.voucher_no NOT LIKE 'KC-%'
               AND v.status = 'POSTED'",
        )?;

        let from_str = from_date.to_string();
        let to_str = to_date.to_string();

        let rows = stmt.query_map(params![from_str, to_str], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        let mut gross_revenue = Decimal::ZERO;
        let mut revenue_deductions = Decimal::ZERO;
        let mut cost_of_goods_sold = Decimal::ZERO;
        let mut financial_income = Decimal::ZERO;
        let mut financial_expenses = Decimal::ZERO;
        let mut selling_expenses = Decimal::ZERO;
        let mut administrative_expenses = Decimal::ZERO;
        let mut other_income = Decimal::ZERO;
        let mut other_expenses = Decimal::ZERO;
        let mut corporate_tax = Decimal::ZERO;

        for r in rows {
            let (code, debit, credit) = r?;
            if code.starts_with("511") {
                gross_revenue += credit - debit;
            } else if code.starts_with("521") {
                revenue_deductions += debit - credit;
            } else if code.starts_with("632") {
                cost_of_goods_sold += debit - credit;
            } else if code.starts_with("515") {
                financial_income += credit - debit;
            } else if code.starts_with("635") {
                financial_expenses += debit - credit;
            } else if code.starts_with("641") {
                selling_expenses += debit - credit;
            } else if code.starts_with("642") {
                administrative_expenses += debit - credit;
            } else if code.starts_with("711") {
                other_income += credit - debit;
            } else if code.starts_with("811") {
                other_expenses += debit - credit;
            } else if code.starts_with("821") {
                corporate_tax += debit - credit;
            }
        }

        let net_revenue = gross_revenue - revenue_deductions;
        let gross_profit = net_revenue - cost_of_goods_sold;
        let operating_profit = gross_profit + financial_income
            - financial_expenses
            - selling_expenses
            - administrative_expenses;
        let net_profit_before_tax = operating_profit + (other_income - other_expenses);
        let net_profit_after_tax = net_profit_before_tax - corporate_tax;

        Ok(IncomeStatementReport {
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
        })
    }

    /// Computes the statutory Trial Balance across all active POSTED vouchers.
    pub fn compute_trial_balance(&self) -> Result<Vec<TrialBalanceRow>, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT l.account_code, l.debit, l.credit
             FROM voucher_lines l
             JOIN vouchers v ON l.voucher_id = v.id
             WHERE v.status = 'POSTED'
             ORDER BY l.account_code ASC",
        )?;

        let mut map: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();

        let rows = stmt.query_map([], |row| {
            let account_code: String = row.get(0)?;
            let debit_str: String = row.get(1)?;
            let credit_str: String = row.get(2)?;

            let debit = Decimal::from_str(&debit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let credit = Decimal::from_str(&credit_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok((account_code, debit, credit))
        })?;

        for r in rows {
            let (account_code, debit, credit) = r?;
            let entry = map
                .entry(account_code)
                .or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += debit;
            entry.1 += credit;
        }

        let trial_balance_rows = map
            .into_iter()
            .map(|(account_code, (total_debit, total_credit))| {
                let net_balance = total_debit - total_credit;
                TrialBalanceRow {
                    account_code,
                    total_debit,
                    total_credit,
                    net_balance,
                }
            })
            .collect();

        Ok(trial_balance_rows)
    }
}
