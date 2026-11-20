use backend::{
    database::LedgerRepository,
    ledger::{AccountingError, LedgerEngine},
    models::{BalanceSheetReport, JournalVoucher, VoucherLine, VoucherStatus},
};
use chrono::NaiveDate;
use proptest::prelude::*;
use rusqlite::Connection;
use rust_decimal::Decimal;
use uuid::Uuid;

// Property 1: Double-Entry Golden Rule Invariant
// - Valid compound vouchers (sum(debits) == sum(credits) > 0, 2+ lines) pass validation.
// - Perturbing the balance (adding non-zero delta) strictly triggers UnbalancedVoucher.
// - Perturbing an amount to negative strictly triggers NegativeAmount.
// - Perturbing a line to both zero debit and zero credit triggers InvalidLine.
// - A single-line voucher triggers EmptyOrSingleLineVoucher.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn prop_double_entry_balance_invariant(
        amounts in prop::collection::vec(1..100_000_000i64, 1..6),
        perturbation in 1..10_000i64,
    ) {
        let engine = LedgerEngine::new();
        let total: i64 = amounts.iter().sum();
        let total_dec = Decimal::from(total);

        // 1. Construct valid balanced compound voucher (N debit lines, 1 credit line)
        let mut lines = Vec::new();
        for (i, amt) in amounts.iter().enumerate() {
            lines.push(VoucherLine {
                id: Uuid::new_v4(),
                line_no: (i + 1) as u32,
                account_code: format!("152{}", i),
                debit: Decimal::from(*amt),
                credit: Decimal::ZERO,
                line_desc: None,
            });
        }
        lines.push(VoucherLine {
            id: Uuid::new_v4(),
            line_no: (amounts.len() + 1) as u32,
            account_code: "331".to_string(),
            debit: Decimal::ZERO,
            credit: total_dec,
            line_desc: None,
        });

        let voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: format!("BAL-{}", Uuid::new_v4()),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "Balanced compound transaction".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T00:00:00Z".to_string(),
            lines,
        };

        prop_assert!(engine.validate_voucher(&voucher).is_ok());

        // 2. Perturb debit leg by adding non-zero delta -> UnbalancedVoucher
        let mut unbalanced_voucher = voucher.clone();
        unbalanced_voucher.lines[0].debit += Decimal::from(perturbation);
        let res_unbalanced = engine.validate_voucher(&unbalanced_voucher);
        let is_unbalanced = matches!(res_unbalanced, Err(AccountingError::UnbalancedVoucher { .. }));
        prop_assert!(is_unbalanced);

        // 3. Negative amount perturbation -> NegativeAmount
        let mut neg_voucher = voucher.clone();
        neg_voucher.lines[0].debit = Decimal::from(-100);
        let res_neg = engine.validate_voucher(&neg_voucher);
        let is_neg = matches!(res_neg, Err(AccountingError::NegativeAmount { .. }));
        prop_assert!(is_neg);

        // 4. Zero line perturbation -> InvalidLine
        let mut zero_line_voucher = voucher.clone();
        zero_line_voucher.lines[0].debit = Decimal::ZERO;
        zero_line_voucher.lines[0].credit = Decimal::ZERO;
        let res_zero = engine.validate_voucher(&zero_line_voucher);
        let is_invalid_line = matches!(res_zero, Err(AccountingError::InvalidLine { .. }));
        prop_assert!(is_invalid_line);

        // 5. Single line voucher -> EmptyOrSingleLineVoucher
        let single_line_voucher = JournalVoucher {
            id: Uuid::new_v4(),
            voucher_no: format!("SINGLE-{}", Uuid::new_v4()),
            voucher_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            description: "Single line voucher".to_string(),
            invoice_ref: None,
            status: VoucherStatus::Posted,
            created_by: "system".to_string(),
            approved_by: None,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            created_at: "2026-01-15T00:00:00Z".to_string(),
            lines: vec![voucher.lines[0].clone()],
        };
        let res_single = engine.validate_voucher(&single_line_voucher);
        let is_single = matches!(res_single, Err(AccountingError::EmptyOrSingleLineVoucher));
        prop_assert!(is_single);
    }
}

// Property 2: Multi-Voucher Accumulation Equilibrium in Trial Balance
// - Post randomized sequences of valid vouchers across diverse VAS accounts.
// - Verify that compute_trial_balance() produces:
//     sum(total_debit) == sum(total_credit)
//     sum(net_balance) == 0
//     and for every row: net_balance == total_debit - total_credit.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_trial_balance_accumulation_equilibrium(
        vouchers_data in prop::collection::vec(
            (
                prop::sample::select(vec!["1111", "1121", "152", "156", "2111", "641", "642"]),
                prop::sample::select(vec!["1111", "1121", "331", "5111", "711", "4111"]),
                1..50_000_000i64,
            ),
            1..15
        )
    ) {
        let conn = Connection::open_in_memory().unwrap();
        let repo = LedgerRepository::new(conn);
        repo.init_schema().unwrap();

        for (idx, (dr_acct, cr_acct, amt_raw)) in vouchers_data.into_iter().enumerate() {
            let amt = Decimal::from(amt_raw);
            let v = JournalVoucher {
                id: Uuid::new_v4(),
                voucher_no: format!("PROP-V-{:04}", idx),
                voucher_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
                description: "Proptest transaction".to_string(),
                invoice_ref: None,
                status: VoucherStatus::Posted,
                created_by: "system".to_string(),
                approved_by: None,
                voided_at: None,
                voided_by: None,
                void_reason: None,
                created_at: "2026-01-10T00:00:00Z".to_string(),
                lines: vec![
                    VoucherLine {
                        id: Uuid::new_v4(),
                        line_no: 1,
                        account_code: dr_acct.to_string(),
                        debit: amt,
                        credit: Decimal::ZERO,
                        line_desc: None,
                    },
                    VoucherLine {
                        id: Uuid::new_v4(),
                        line_no: 2,
                        account_code: cr_acct.to_string(),
                        debit: Decimal::ZERO,
                        credit: amt,
                        line_desc: None,
                    },
                ],
            };
            repo.save_voucher(&v).unwrap();
        }

        let tb = repo.compute_trial_balance().unwrap();
        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;
        let mut net_sum = Decimal::ZERO;

        for row in tb {
            total_debit += row.total_debit;
            total_credit += row.total_credit;
            net_sum += row.net_balance;
            prop_assert_eq!(row.net_balance, row.total_debit - row.total_credit);
        }

        prop_assert_eq!(total_debit, total_credit);
        prop_assert_eq!(net_sum, Decimal::ZERO);
    }
}

// Property 3: Contra-Asset Balance Sheet Identity
// - Under Circular 99/2025/TT-BTC:
//     Total Assets = (Current Assets + Non-Current Assets) - (Accumulated Depreciation + Asset Provisions)
//     Total Resources = Liabilities + Equity
//     Total Assets == Total Resources (is_balanced == true)
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn prop_contra_asset_balance_sheet_identity(
        current_assets in 100_000_000..500_000_000i64,
        non_current_assets in 100_000_000..500_000_000i64,
        accumulated_depreciation in 1_000_000..50_000_000i64,
        asset_provisions in 1_000_000..20_000_000i64,
        liabilities in 10_000_000..100_000_000i64,
    ) {
        let ca = Decimal::from(current_assets);
        let nca = Decimal::from(non_current_assets);
        let ad = Decimal::from(accumulated_depreciation);
        let ap = Decimal::from(asset_provisions);
        let liab = Decimal::from(liabilities);

        // Under double-entry accounting identity: Equity = Total Assets - Liabilities
        let net_assets = (ca + nca) - (ad + ap);
        let equity = net_assets - liab;

        let date = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let bs = BalanceSheetReport::new(date, ca, nca, ad, ap, liab, equity);

        // Statutory Invariants under Circular 99
        prop_assert_eq!(
            bs.total_assets,
            (bs.current_assets + bs.non_current_assets) - (bs.accumulated_depreciation + bs.asset_provisions)
        );
        prop_assert_eq!(bs.total_resources, bs.liabilities + bs.equity);
        prop_assert_eq!(bs.total_assets, bs.total_resources);
        prop_assert!(bs.is_balanced);
    }
}
