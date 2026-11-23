use super::*;
use std::collections::HashSet;

#[test]
fn test_statutory_accounts_count_and_completeness() {
    assert!(STATUTORY_ACCOUNTS.len() >= 50);
    let codes: HashSet<&str> = STATUTORY_ACCOUNTS.iter().map(|a| a.code).collect();
    assert_eq!(
        codes.len(),
        STATUTORY_ACCOUNTS.len(),
        "Account codes must be unique"
    );

    // Verify raw materials 1520..1525 exist
    for code in ["1520", "1521", "1522", "1523", "1524", "1525"] {
        assert!(codes.contains(code), "Account {code} must exist in seed");
    }

    // Verify zero orphan parents
    for acct in STATUTORY_ACCOUNTS {
        if let Some(parent) = acct.parent_code {
            assert!(
                codes.contains(parent),
                "Account {} has orphan parent {}",
                acct.code,
                parent
            );
        }
    }

    // Verify specific required properties
    let a_2141 = STATUTORY_ACCOUNTS
        .iter()
        .find(|a| a.code == "2141")
        .unwrap();
    assert_eq!(a_2141.normal_balance, NormalBalance::Credit);

    let a_5211 = STATUTORY_ACCOUNTS
        .iter()
        .find(|a| a.code == "5211")
        .unwrap();
    assert_eq!(a_5211.normal_balance, NormalBalance::Debit);

    let a_911 = STATUTORY_ACCOUNTS.iter().find(|a| a.code == "911").unwrap();
    assert_eq!(a_911.class, AccountClass::Closing);
}
