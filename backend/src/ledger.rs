use crate::models::JournalVoucher;
use rust_decimal::Decimal;

/// Core business error domain for accounting invariant violations.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum AccountingError {
    #[error("Voucher {voucher_no} is unbalanced: total debit ({total_debit}) != total credit ({total_credit})")]
    UnbalancedVoucher {
        voucher_no: String,
        total_debit: Decimal,
        total_credit: Decimal,
    },

    #[error("Negative amounts are forbidden in accounting entries. Account: {account_code}")]
    NegativeAmount { account_code: String },

    #[error("Voucher must have at least 2 lines (at least one debit and one credit)")]
    EmptyOrSingleLineVoucher,

    #[error("Invalid line for account {account_code}: {reason}")]
    InvalidLine {
        account_code: String,
        reason: String,
    },

    #[error("Zero total value: transaction must have positive monetary volume")]
    ZeroValueVoucher,

    #[error("Accounting period {year}-{month:02} is closed for postings")]
    PeriodClosed { year: i32, month: u32 },

    #[error("Accounting period {year}-{month:02} is already closed")]
    PeriodAlreadyClosed { year: i32, month: u32 },

    #[error("Accounting period {year}-{month:02} is not closed")]
    PeriodNotClosed { year: i32, month: u32 },

    #[error("Invalid period: year={year}, month={month}. Month must be between 1 and 12")]
    InvalidPeriod { year: i32, month: u32 },

    #[error("Voucher {voucher_no} is already voided")]
    VoucherAlreadyVoided { voucher_no: String },

    #[error("Cannot void voucher {voucher_no} in closed period {year}-{month:02}")]
    CannotVoidInClosedPeriod {
        voucher_no: String,
        year: i32,
        month: u32,
    },

    #[error("Voucher with id '{0}' not found")]
    VoucherNotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl From<rusqlite::Error> for AccountingError {
    fn from(err: rusqlite::Error) -> Self {
        AccountingError::DatabaseError(err.to_string())
    }
}

/// The core Ledger Engine responsible for validating accounting invariants
/// per Vietnamese Accounting Standards (VAS) and Circular 99/2025/TT-BTC.
#[derive(Default)]
pub struct LedgerEngine;

impl LedgerEngine {
    pub fn new() -> Self {
        Self
    }

    /// Validates that a journal voucher satisfies all statutory double-entry accounting rules:
    /// 1. Contains at least two line items.
    /// 2. All amounts are strictly non-negative (>= 0).
    /// 3. A line cannot have both Debit > 0 and Credit > 0 simultaneously.
    /// 4. Every line must have either Debit > 0 or Credit > 0 (no empty zero lines).
    /// 5. Golden Rule: Total Debits == Total Credits > 0.
    pub fn validate_voucher(&self, voucher: &JournalVoucher) -> Result<(), AccountingError> {
        if voucher.lines.len() < 2 {
            return Err(AccountingError::EmptyOrSingleLineVoucher);
        }

        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;

        for line in &voucher.lines {
            // Rule 1: No negative amounts allowed
            if line.debit < Decimal::ZERO || line.credit < Decimal::ZERO {
                return Err(AccountingError::NegativeAmount {
                    account_code: line.account_code.clone(),
                });
            }

            // Rule 2: A single line cannot have both Debit and Credit
            if line.debit > Decimal::ZERO && line.credit > Decimal::ZERO {
                return Err(AccountingError::InvalidLine {
                    account_code: line.account_code.clone(),
                    reason: "A single line item cannot contain both Debit and Credit amounts"
                        .to_string(),
                });
            }

            // Rule 3: Line must not be completely zero
            if line.debit.is_zero() && line.credit.is_zero() {
                return Err(AccountingError::InvalidLine {
                    account_code: line.account_code.clone(),
                    reason: "Line amount cannot be zero for both Debit and Credit".to_string(),
                });
            }

            total_debit += line.debit;
            total_credit += line.credit;
        }

        // Rule 4: Total monetary value must be strictly positive
        if total_debit.is_zero() {
            return Err(AccountingError::ZeroValueVoucher);
        }

        // Rule 5: Double-Entry Balance Check
        if total_debit != total_credit {
            return Err(AccountingError::UnbalancedVoucher {
                voucher_no: voucher.voucher_no.clone(),
                total_debit,
                total_credit,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JournalVoucher, VoucherLine, VoucherStatus};
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn sample_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
    }

    #[test]
    fn test_valid_simple_cash_withdrawal_voucher() {
        let engine = LedgerEngine::new();
        // Rút tiền gửi ngân hàng về nhập quỹ tiền mặt (15,000,000 VND)
        // Nợ TK 1111: 15,000,000 / Có TK 1121: 15,000,000
        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "PT-2026-0001".to_string(),
            voucher_date: sample_date(),
            description: "Rút tiền gửi ngân hàng về quỹ tiền mặt".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T08:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(15000000),
                    credit: dec!(0),
                    line_desc: Some("Thu tiền mặt".to_string()),
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(15000000),
                    line_desc: Some("Giảm tiền gửi VCB".to_string()),
                },
            ],
        };

        assert!(engine.validate_voucher(&voucher).is_ok());
    }

    #[test]
    fn test_valid_compound_purchase_with_vat_voucher() {
        let engine = LedgerEngine::new();
        // Mua hàng hóa nhập kho có thuế GTGT (1-to-many compound transaction)
        // Nợ TK 1561: 100,000,000
        // Nợ TK 1331: 10,000,000
        // Có TK 331:  110,000,000
        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "PNK-2026-0042".to_string(),
            voucher_date: sample_date(),
            description: "Mua hàng hóa công ty ABC có hóa đơn GTGT".to_string(),
            invoice_ref: Some("HD-00987".to_string()),
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T08:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1561".to_string(),
                    debit: dec!(10000000),
                    credit: dec!(0),
                    line_desc: Some("Tiền hàng".to_string()),
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1331".to_string(),
                    debit: dec!(1000000),
                    credit: dec!(0),
                    line_desc: Some("Thuế GTGT 10%".to_string()),
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 3,
                    account_code: "331".to_string(),
                    debit: dec!(0),
                    credit: dec!(11000000),
                    line_desc: Some("Phải trả người bán".to_string()),
                },
            ],
        };

        assert!(engine.validate_voucher(&voucher).is_ok());
    }

    #[test]
    fn test_unbalanced_voucher_is_rejected() {
        let engine = LedgerEngine::new();
        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "PKT-ERR-001".to_string(),
            voucher_date: sample_date(),
            description: "Mất cân đối Nợ Có".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T08:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(1000000),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "5111".to_string(),
                    debit: dec!(0),
                    credit: dec!(999000), // Lệch 1,000 VND
                    line_desc: None,
                },
            ],
        };

        match engine.validate_voucher(&voucher) {
            Err(AccountingError::UnbalancedVoucher {
                total_debit,
                total_credit,
                ..
            }) => {
                assert_eq!(total_debit, dec!(1000000));
                assert_eq!(total_credit, dec!(999000));
            }
            other => panic!("Expected UnbalancedVoucher, got {:?}", other),
        }
    }

    #[test]
    fn test_negative_amounts_are_rejected() {
        let engine = LedgerEngine::new();
        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "PKT-ERR-002".to_string(),
            voucher_date: sample_date(),
            description: "Số tiền âm không hợp lệ".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T08:00:00Z".to_string(),
            lines: vec![
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 1,
                    account_code: "1111".to_string(),
                    debit: dec!(-500000),
                    credit: dec!(0),
                    line_desc: None,
                },
                VoucherLine {
                    id: Uuid::new_v4(),
                    line_no: 2,
                    account_code: "1121".to_string(),
                    debit: dec!(0),
                    credit: dec!(-500000),
                    line_desc: None,
                },
            ],
        };

        assert!(matches!(
            engine.validate_voucher(&voucher),
            Err(AccountingError::NegativeAmount { .. })
        ));
    }

    #[test]
    fn test_single_line_voucher_is_rejected() {
        let engine = LedgerEngine::new();
        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: "PKT-ERR-003".to_string(),
            voucher_date: sample_date(),
            description: "Chứng từ thiếu định khoản đối ứng".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T08:00:00Z".to_string(),
            lines: vec![VoucherLine {
                id: Uuid::new_v4(),
                line_no: 1,
                account_code: "1111".to_string(),
                debit: dec!(500000),
                credit: dec!(0),
                line_desc: None,
            }],
        };

        assert!(matches!(
            engine.validate_voucher(&voucher),
            Err(AccountingError::EmptyOrSingleLineVoucher)
        ));
    }
}
