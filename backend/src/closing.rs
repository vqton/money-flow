use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::database::LedgerRepository;
use crate::ledger::{AccountingError, LedgerEngine};
use crate::models::{JournalVoucher, VoucherLine, VoucherStatus};

/// Engine implementing the statutory 4-step period closing sequence per Circular 99/2025/TT-BTC.
pub struct PeriodClosingEngine;

impl PeriodClosingEngine {
    /// Returns the last calendar day of the specified fiscal month.
    pub fn last_day_of_month(year: i32, month: u32) -> Result<NaiveDate, AccountingError> {
        if !(1..=12).contains(&month) {
            return Err(AccountingError::InvalidPeriod { year, month });
        }

        let next_month_date = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        };

        next_month_date
            .and_then(|d| d.pred_opt())
            .ok_or(AccountingError::InvalidPeriod { year, month })
    }

    /// Formats the statutory closing voucher number: `KC-{year}-{month:02}`.
    pub fn closing_voucher_no(year: i32, month: u32) -> String {
        format!("KC-{year}-{month:02}")
    }

    /// Calculates the start and end NaiveDate bounds for a given fiscal year and starting month per Circular 99 Điều 31.
    pub fn fiscal_year_bounds(
        fiscal_year: i32,
        start_month: u8,
    ) -> Result<(NaiveDate, NaiveDate), AccountingError> {
        fiscal_year_bounds(fiscal_year, start_month)
    }

    /// Determines the fiscal year for a given calendar date and starting month.
    pub fn fiscal_year_for_date(date: NaiveDate, start_month: u8) -> i32 {
        fiscal_year_for_date(date, start_month)
    }

    /// Determines the 1-based fiscal period (1..=12) for a given calendar date and starting month.
    pub fn fiscal_period_for_date(date: NaiveDate, start_month: u8) -> u32 {
        fiscal_period_for_date(date, start_month)
    }

    /// Pure closing calculation: constructs the 4-step closing voucher from pre-aggregated account balances.
    ///
    /// Balances map is structured as `account_code -> (total_debit, total_credit)`.
    ///
    /// The 4-step sequence:
    /// - Step 1: Offset TK 521 into TK 511 (Debit 511 / Credit 521)
    /// - Step 2: Transfer net revenues (511, 515, 711) to TK 911 (Debit 511, 515, 711 / Credit 911)
    /// - Step 3: Transfer expenses (632, 635, 641, 642, 811, 821) to TK 911 (Debit 911 / Credit expenses)
    /// - Step 4: Transfer Net Profit/Loss between 911 and 4212:
    ///   - Profit (Credit 911 > Debit 911): Debit 911 / Credit 4212
    ///   - Loss (Debit 911 > Credit 911): Debit 4212 / Credit 911
    pub fn create_closing_voucher_from_balances(
        year: i32,
        month: u32,
        balances: &BTreeMap<String, (Decimal, Decimal)>,
    ) -> Result<JournalVoucher, AccountingError> {
        let voucher_date = Self::last_day_of_month(year, month)?;
        let voucher_no = Self::closing_voucher_no(year, month);
        let mut lines = Vec::new();

        // -----------------------------------------------------------------
        // Step 1: Offset TK 521 into TK 511
        // -----------------------------------------------------------------
        let mut total_521 = Decimal::ZERO;
        let mut deduction_lines = Vec::new();

        for (code, (debit, credit)) in balances {
            if code.starts_with("521") {
                let net_deduction = debit - credit;
                if net_deduction > Decimal::ZERO {
                    total_521 += net_deduction;
                    deduction_lines.push(VoucherLine {
                        id: Uuid::new_v4(),
                        line_no: 0,
                        account_code: code.clone(),
                        debit: Decimal::ZERO,
                        credit: net_deduction,
                        line_desc: Some(format!("Kết chuyển giảm trừ doanh thu TK {code}")),
                    });
                }
            }
        }

        // Determine the primary 511 account code
        let primary_511 = balances
            .keys()
            .find(|code| code.as_str() == "511" || code.starts_with("511"))
            .cloned()
            .unwrap_or_else(|| "511".to_string());

        let mut account_offsets: BTreeMap<String, Decimal> = BTreeMap::new();

        if total_521 > Decimal::ZERO {
            let mut remaining_deduction = total_521;

            // Cascade total_521 across 511% accounts
            for (code, (debit, credit)) in balances {
                if code.starts_with("511") {
                    let gross_rev = credit - debit;
                    if gross_rev > Decimal::ZERO && remaining_deduction > Decimal::ZERO {
                        let offset = remaining_deduction.min(gross_rev);
                        if offset > Decimal::ZERO {
                            account_offsets.insert(code.clone(), offset);
                            lines.push(VoucherLine {
                                id: Uuid::new_v4(),
                                line_no: 0,
                                account_code: code.clone(),
                                debit: offset,
                                credit: Decimal::ZERO,
                                line_desc: Some(format!(
                                    "Kết chuyển giảm trừ doanh thu vào TK {code}"
                                )),
                            });
                            remaining_deduction -= offset;
                        }
                    }
                }
            }

            // If remaining_deduction > Decimal::ZERO after exhausting all 511 accounts,
            // debit the remaining amount to the primary 511 account.
            if remaining_deduction > Decimal::ZERO {
                *account_offsets
                    .entry(primary_511.clone())
                    .or_insert(Decimal::ZERO) += remaining_deduction;
                lines.push(VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 0,
                    account_code: primary_511.clone(),
                    debit: remaining_deduction,
                    credit: Decimal::ZERO,
                    line_desc: Some("Kết chuyển giảm trừ doanh thu còn lại vào TK 511".to_string()),
                });
            }

            // Credit 521
            lines.extend(deduction_lines);
        }

        // -----------------------------------------------------------------
        // Step 2: Transfer net revenues (511, 515, 711) to TK 911
        // -----------------------------------------------------------------
        let mut total_revenue = Decimal::ZERO;
        let mut revenue_lines = Vec::new();

        for (code, (debit, credit)) in balances {
            let is_rev_acct =
                code.starts_with("511") || code.starts_with("515") || code.starts_with("711");

            if is_rev_acct {
                let gross_rev = credit - debit;
                let net_rev = if code.starts_with("511") {
                    let offset = account_offsets.get(code).copied().unwrap_or(Decimal::ZERO);
                    gross_rev - offset
                } else {
                    gross_rev
                };

                if net_rev > Decimal::ZERO {
                    total_revenue += net_rev;
                    revenue_lines.push(VoucherLine {
                        id: Uuid::new_v4(),
                        line_no: 0,
                        account_code: code.clone(),
                        debit: net_rev,
                        credit: Decimal::ZERO,
                        line_desc: Some(format!("Kết chuyển doanh thu TK {code} sang TK 911")),
                    });
                }
            }
        }

        if total_revenue > Decimal::ZERO {
            lines.extend(revenue_lines);
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: 0,
                account_code: "911".to_string(),
                debit: Decimal::ZERO,
                credit: total_revenue,
                line_desc: Some("Kết chuyển doanh thu thuần, thu nhập sang TK 911".to_string()),
            });
        }

        // -----------------------------------------------------------------
        // Step 3: Transfer expenses (632, 635, 641, 642, 811, 821) to TK 911
        // -----------------------------------------------------------------
        let mut total_expenses = Decimal::ZERO;
        let mut expense_lines = Vec::new();

        for (code, (debit, credit)) in balances {
            let is_exp_acct = code.starts_with("632")
                || code.starts_with("635")
                || code.starts_with("641")
                || code.starts_with("642")
                || code.starts_with("811")
                || code.starts_with("821");

            if is_exp_acct {
                let net_exp = debit - credit;
                if net_exp > Decimal::ZERO {
                    total_expenses += net_exp;
                    expense_lines.push(VoucherLine {
                        id: Uuid::new_v4(),
                        line_no: 0,
                        account_code: code.clone(),
                        debit: Decimal::ZERO,
                        credit: net_exp,
                        line_desc: Some(format!("Kết chuyển chi phí TK {code} sang TK 911")),
                    });
                }
            }
        }

        if total_expenses > Decimal::ZERO {
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: 0,
                account_code: "911".to_string(),
                debit: total_expenses,
                credit: Decimal::ZERO,
                line_desc: Some("Kết chuyển chi phí sang TK 911".to_string()),
            });
            lines.extend(expense_lines);
        }

        // -----------------------------------------------------------------
        // Step 4: Transfer Net Profit/Loss between 911 and 4212
        // -----------------------------------------------------------------
        if total_revenue > total_expenses {
            // Net Profit: Debit 911, Credit 4212
            let net_profit = total_revenue - total_expenses;
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: 0,
                account_code: "911".to_string(),
                debit: net_profit,
                credit: Decimal::ZERO,
                line_desc: Some("Kết chuyển lợi nhuận sau thuế sang TK 4212".to_string()),
            });
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: 0,
                account_code: "4212".to_string(),
                debit: Decimal::ZERO,
                credit: net_profit,
                line_desc: Some("Lợi nhuận sau thuế chưa phân phối năm nay".to_string()),
            });
        } else if total_expenses > total_revenue {
            // Net Loss: Debit 4212, Credit 911
            let net_loss = total_expenses - total_revenue;
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: 0,
                account_code: "4212".to_string(),
                debit: net_loss,
                credit: Decimal::ZERO,
                line_desc: Some("Kết chuyển lỗ hoạt động kinh doanh sang TK 4212".to_string()),
            });
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: 0,
                account_code: "911".to_string(),
                debit: Decimal::ZERO,
                credit: net_loss,
                line_desc: Some("Kết chuyển lỗ sang TK 911".to_string()),
            });
        }

        if lines.is_empty() {
            return Err(AccountingError::ZeroValueVoucher);
        }

        // Assign strictly ordered 1-based line numbers
        for (idx, line) in lines.iter_mut().enumerate() {
            line.line_no = (idx + 1) as u32;
        }

        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no,
            voucher_date,
            description: format!(
                "Bút toán kết chuyển xác định kết quả kinh doanh tháng {:02}/{}",
                month, year
            ),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            lines,
        };

        // Strictly validate with LedgerEngine before returning
        LedgerEngine::new().validate_voucher(&voucher)?;

        Ok(voucher)
    }

    /// Persistence-integrated builder: queries the repository and constructs the closing voucher.
    pub fn build_closing_voucher(
        repo: &LedgerRepository,
        year: i32,
        month: u32,
    ) -> Result<JournalVoucher, AccountingError> {
        let from_date = NaiveDate::from_ymd_opt(year, month, 1)
            .ok_or(AccountingError::InvalidPeriod { year, month })?;
        let to_date = Self::last_day_of_month(year, month)?;

        let balances = repo.get_account_balances_for_range(from_date, to_date)?;

        Self::create_closing_voucher_from_balances(year, month, &balances)
    }
}

/// Returns the statutory start and end calendar dates for a fiscal year per Circular 99 Điều 31.
///
/// For calendar fiscal years (`start_month == 1`), bounds are `(YYYY-01-01, YYYY-12-31)`.
/// For non-calendar fiscal years (`start_month > 1`), bounds span from `YYYY-{start_month:02}-01`
/// to the last day of month `start_month - 1` in calendar year `YYYY + 1`.
pub fn fiscal_year_bounds(
    fiscal_year: i32,
    start_month: u8,
) -> Result<(NaiveDate, NaiveDate), AccountingError> {
    if !(1..=12).contains(&start_month) {
        return Err(AccountingError::InvalidPeriod {
            year: fiscal_year,
            month: start_month as u32,
        });
    }

    let start_date = NaiveDate::from_ymd_opt(fiscal_year, start_month as u32, 1).ok_or(
        AccountingError::InvalidPeriod {
            year: fiscal_year,
            month: start_month as u32,
        },
    )?;

    let (end_year, end_month) = if start_month == 1 {
        (fiscal_year, 12u32)
    } else {
        let next_year = fiscal_year
            .checked_add(1)
            .ok_or(AccountingError::InvalidPeriod {
                year: fiscal_year,
                month: start_month as u32,
            })?;
        (next_year, (start_month - 1) as u32)
    };

    let end_date = PeriodClosingEngine::last_day_of_month(end_year, end_month)?;
    Ok((start_date, end_date))
}

/// Determines the fiscal year for a given calendar date and starting month.
///
/// For dates on or after `start_month`, the fiscal year is `date.year()`.
/// For dates before `start_month`, the fiscal year is `date.year() - 1`.
/// If `start_month` is outside `1..=12`, falls back to calendar year (start month 1).
pub fn fiscal_year_for_date(date: NaiveDate, start_month: u8) -> i32 {
    use chrono::Datelike;
    let s = if (1..=12).contains(&start_month) {
        start_month
    } else {
        1
    };
    let m = date.month() as u8;
    if m >= s { date.year() } else { date.year() - 1 }
}

/// Determines the 1-based fiscal period (1..=12) for a given calendar date and starting month.
///
/// For example, with `start_month = 4`:
/// - April is Period 1
/// - December is Period 9
/// - January of next year is Period 10
/// - March of next year is Period 12
///
/// If `start_month` is outside `1..=12`, falls back to calendar month (start month 1).
pub fn fiscal_period_for_date(date: NaiveDate, start_month: u8) -> u32 {
    use chrono::Datelike;
    let s = if (1..=12).contains(&start_month) {
        start_month as u32
    } else {
        1
    };
    let m = date.month();
    if m >= s { m - s + 1 } else { 12 - s + m + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_closing_engine_with_profit_and_deductions() {
        let mut balances = BTreeMap::new();
        // Gross revenue: Credit 511 = 120,000,000
        balances.insert("511".to_string(), (dec!(0), dec!(120000000)));
        // Revenue deductions: Debit 521 = 20,000,000
        balances.insert("521".to_string(), (dec!(20000000), dec!(0)));
        // Expenses
        balances.insert("632".to_string(), (dec!(50000000), dec!(0)));
        balances.insert("642".to_string(), (dec!(10000000), dec!(0)));
        balances.insert("811".to_string(), (dec!(5000000), dec!(0)));

        let voucher = PeriodClosingEngine::create_closing_voucher_from_balances(2026, 1, &balances)
            .expect("Closing voucher creation should succeed");

        assert_eq!(voucher.voucher_no, "KC-2026-01");
        assert_eq!(
            voucher.voucher_date,
            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
        );

        // Validate complete balance in 911
        let mut debit_911 = Decimal::ZERO;
        let mut credit_911 = Decimal::ZERO;
        let mut credit_4212 = Decimal::ZERO;

        for line in &voucher.lines {
            if line.account_code == "911" {
                debit_911 += line.debit;
                credit_911 += line.credit;
            }
            if line.account_code == "4212" {
                credit_4212 += line.credit;
            }
        }

        assert_eq!(
            debit_911, credit_911,
            "Account 911 must be perfectly balanced"
        );
        assert_eq!(
            credit_4212,
            dec!(35000000),
            "Net profit of 35M must be credited to 4212"
        );
    }

    #[test]
    fn test_closing_engine_with_loss() {
        let mut balances = BTreeMap::new();
        balances.insert("511".to_string(), (dec!(0), dec!(40000000)));
        balances.insert("632".to_string(), (dec!(50000000), dec!(0)));
        balances.insert("642".to_string(), (dec!(15000000), dec!(0)));

        let voucher = PeriodClosingEngine::create_closing_voucher_from_balances(2026, 2, &balances)
            .expect("Closing voucher creation should succeed");

        assert_eq!(voucher.voucher_no, "KC-2026-02");

        let mut debit_4212 = Decimal::ZERO;
        let mut debit_911 = Decimal::ZERO;
        let mut credit_911 = Decimal::ZERO;

        for line in &voucher.lines {
            if line.account_code == "911" {
                debit_911 += line.debit;
                credit_911 += line.credit;
            }
            if line.account_code == "4212" {
                debit_4212 += line.debit;
            }
        }

        assert_eq!(
            debit_911, credit_911,
            "Account 911 must balance with loss transfer"
        );
        assert_eq!(
            debit_4212,
            dec!(25000000),
            "Net loss of 25M must be debited to 4212"
        );
    }

    #[test]
    fn test_closing_engine_with_multiple_511_subaccounts_and_deductions() {
        let mut balances = BTreeMap::new();
        // 5111: Credit 10,000,000
        balances.insert("5111".to_string(), (dec!(0), dec!(10000000)));
        // 5112: Credit 200,000,000
        balances.insert("5112".to_string(), (dec!(0), dec!(200000000)));
        // 5211: Debit 15,000,000
        balances.insert("5211".to_string(), (dec!(15000000), dec!(0)));
        // 632: Debit 80,000,000
        balances.insert("632".to_string(), (dec!(80000000), dec!(0)));

        let voucher = PeriodClosingEngine::create_closing_voucher_from_balances(2026, 1, &balances)
            .expect("Closing voucher creation should succeed");

        assert_eq!(voucher.voucher_no, "KC-2026-01");
        assert_eq!(
            voucher.voucher_date,
            NaiveDate::from_ymd_opt(2026, 1, 31).unwrap()
        );

        let mut total_debit_5111 = Decimal::ZERO;
        let mut total_debit_5112 = Decimal::ZERO;
        let mut debit_911 = Decimal::ZERO;
        let mut credit_911 = Decimal::ZERO;
        let mut credit_4212 = Decimal::ZERO;

        for line in &voucher.lines {
            if line.account_code == "5111" {
                total_debit_5111 += line.debit;
            }
            if line.account_code == "5112" {
                total_debit_5112 += line.debit;
            }
            if line.account_code == "911" {
                debit_911 += line.debit;
                credit_911 += line.credit;
            }
            if line.account_code == "4212" {
                credit_4212 += line.credit;
            }
        }

        // Verify: 5111 debited by 10M, 5112 debited by 195M (5M offset + 195M to 911),
        // 4212 credited with 115,000,000 net profit, 911 balances to 0.00.
        let debits_5112: Vec<Decimal> = voucher
            .lines
            .iter()
            .filter(|l| l.account_code == "5112")
            .map(|l| l.debit)
            .collect();
        assert_eq!(
            debits_5112,
            vec![dec!(5000000), dec!(195000000)],
            "5112 must have 5M offset in Step 1 and 195M transfer to 911 in Step 2"
        );
        assert_eq!(
            total_debit_5111,
            dec!(10000000),
            "5111 must be debited by 10M to fully offset"
        );
        assert_eq!(
            total_debit_5112,
            dec!(200000000),
            "5112 debited by 200M (5M offset + 195M to 911)"
        );
        assert_eq!(
            credit_4212,
            dec!(115000000),
            "Net profit credited to 4212 must be 115,000,000"
        );
        assert_eq!(debit_911, credit_911, "Account 911 must balance to 0.00");
        assert_eq!(
            debit_911,
            dec!(195000000),
            "Account 911 total throughput must be 195,000,000"
        );
    }

    #[test]
    fn test_fiscal_year_bounds_calendar() {
        let (start, end) = fiscal_year_bounds(2026, 1).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn test_fiscal_year_bounds_non_calendar() {
        let (start, end) = fiscal_year_bounds(2026, 4).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2027, 3, 31).unwrap());

        let (start, end) = fiscal_year_bounds(2026, 7).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2027, 6, 30).unwrap());

        let (start, end) = fiscal_year_bounds(2026, 10).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2027, 9, 30).unwrap());
    }

    #[test]
    fn test_fiscal_year_bounds_leap_year() {
        let (start, end) = fiscal_year_bounds(2027, 3).unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2027, 3, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2028, 2, 29).unwrap());
    }

    #[test]
    fn test_fiscal_year_bounds_invalid_start_month() {
        assert!(fiscal_year_bounds(2026, 0).is_err());
        assert!(fiscal_year_bounds(2026, 13).is_err());
    }

    #[test]
    fn test_fiscal_year_bounds_overflow_protection() {
        assert!(fiscal_year_bounds(i32::MAX, 4).is_err());
    }

    #[test]
    fn test_fiscal_year_for_date() {
        let d1 = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        assert_eq!(fiscal_year_for_date(d1, 4), 2026);

        let d2 = NaiveDate::from_ymd_opt(2027, 3, 31).unwrap();
        assert_eq!(fiscal_year_for_date(d2, 4), 2026);

        let d3 = NaiveDate::from_ymd_opt(2027, 4, 1).unwrap();
        assert_eq!(fiscal_year_for_date(d3, 4), 2027);

        let d4 = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        assert_eq!(fiscal_year_for_date(d4, 1), 2026);
    }

    #[test]
    fn test_fiscal_period_for_date() {
        let d_apr = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        assert_eq!(fiscal_period_for_date(d_apr, 4), 1);

        let d_dec = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
        assert_eq!(fiscal_period_for_date(d_dec, 4), 9);

        let d_jan = NaiveDate::from_ymd_opt(2027, 1, 15).unwrap();
        assert_eq!(fiscal_period_for_date(d_jan, 4), 10);

        let d_mar = NaiveDate::from_ymd_opt(2027, 3, 31).unwrap();
        assert_eq!(fiscal_period_for_date(d_mar, 4), 12);
    }
}
