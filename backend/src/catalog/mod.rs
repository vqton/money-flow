//! Statutory Chart of Accounts catalog and seed data per Circular 99/2025/TT-BTC.
//!
//! Decomposed into modular levels:
//! - [`level1_2`]: Root classes (Loại 1–9) and 2-digit account groups.
//! - [`level3`]: Statutory 3-digit accounts.
//! - [`level4_5`]: Statutory 4-digit and 5-digit sub-accounts.

pub mod level1_2;
pub mod level3;
pub mod level4_5;

#[cfg(test)]
mod tests;

use crate::models::{AccountClass, NormalBalance};
pub use level1_2::LEVEL1_2_SEEDS;
pub use level3::LEVEL3_SEEDS;
pub use level4_5::LEVEL4_5_SEEDS;

/// Static seed entry for an account in the Vietnamese Chart of Accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountSeed {
    /// Account code under Circular 99 (e.g., "1", "11", "111", "1111").
    pub code: &'static str,
    /// Vietnamese account name (Tên tài khoản).
    pub name: &'static str,
    /// Parent account code (None for root classes 1..9).
    pub parent_code: Option<&'static str>,
    /// Economic classification.
    pub class: AccountClass,
    /// Active status.
    pub is_active: bool,
    /// Statutory normal balance direction.
    pub normal_balance: NormalBalance,
}

/// Helper macro / static constructor aggregating all statutory account levels.
/// Total accounts: 103 (topologically ordered: roots first, then children).
pub static STATUTORY_ACCOUNTS: &[AccountSeed] = {
    // Note: Concat at compile-time using const or macro is flattened below:
    const TOTAL_LEN: usize = LEVEL1_2_SEEDS.len() + LEVEL3_SEEDS.len() + LEVEL4_5_SEEDS.len();
    const COMBINED: &[AccountSeed; TOTAL_LEN] = &{
        let mut arr = [AccountSeed {
            code: "",
            name: "",
            parent_code: None,
            class: AccountClass::Asset,
            is_active: false,
            normal_balance: NormalBalance::Debit,
        }; TOTAL_LEN];

        let mut i = 0;
        let mut j = 0;
        while j < LEVEL1_2_SEEDS.len() {
            arr[i] = LEVEL1_2_SEEDS[j];
            i += 1;
            j += 1;
        }

        j = 0;
        while j < LEVEL3_SEEDS.len() {
            arr[i] = LEVEL3_SEEDS[j];
            i += 1;
            j += 1;
        }

        j = 0;
        while j < LEVEL4_5_SEEDS.len() {
            arr[i] = LEVEL4_5_SEEDS[j];
            i += 1;
            j += 1;
        }

        arr
    };
    COMBINED
};
