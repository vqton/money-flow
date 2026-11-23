//! Dynamic Chart of Accounts management (CRUD and cycle detection).

use crate::database::LedgerRepository;
use crate::ledger::{AccountValidator, AccountingError};
use crate::models::{Account, AccountClass, NormalBalance};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl LedgerRepository {
    /// Lists accounts from the chart of accounts, optionally filtered by class or active status.
    pub fn list_accounts(
        &self,
        class: Option<AccountClass>,
        is_active: Option<bool>,
    ) -> Result<Vec<Account>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = "SELECT code, name, parent_code, class, is_active, normal_balance FROM accounts WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(c) = class {
            sql.push_str(" AND class = ?");
            params_vec.push(Box::new(c.to_string()));
        }
        if let Some(a) = is_active {
            sql.push_str(" AND is_active = ?");
            params_vec.push(Box::new(if a { 1 } else { 0 }));
        }
        sql.push_str(" ORDER BY code ASC");

        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| &**b).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_slice.as_slice(), |r| {
            let class_str: String = r.get(3)?;
            let normal_str: String = r.get(5)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                class_str,
                r.get::<_, i32>(4)? != 0,
                normal_str,
            ))
        })?;

        let mut accounts = Vec::new();
        for r in rows {
            let (code, name, parent_code, class_str, is_active, normal_str) = r?;
            let class = class_str.parse().unwrap_or(AccountClass::Asset);
            let normal_balance = normal_str.parse().unwrap_or(NormalBalance::Debit);
            accounts.push(Account {
                code,
                name,
                parent_code,
                class,
                is_active,
                normal_balance,
            });
        }
        Ok(accounts)
    }

    /// Retrieves an account by its code.
    pub fn get_account(&self, code: &str) -> Result<Option<Account>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let row: Option<(String, String, Option<String>, String, i32, String)> = conn
            .query_row(
                "SELECT code, name, parent_code, class, is_active, normal_balance FROM accounts WHERE code = ?1",
                params![code],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()?;

        match row {
            Some((code, name, parent_code, class_str, is_active, normal_str)) => {
                let class = class_str.parse().unwrap_or(AccountClass::Asset);
                let normal_balance = normal_str.parse().unwrap_or(NormalBalance::Debit);
                Ok(Some(Account {
                    code,
                    name,
                    parent_code,
                    class,
                    is_active: is_active != 0,
                    normal_balance,
                }))
            }
            None => Ok(None),
        }
    }

    /// Creates a new account in the chart of accounts with duplicate, parent, and cycle validation.
    /// Also records an immutable CREATE_ACCOUNT audit trail event atomically.
    pub fn create_account(
        &self,
        account: &Account,
        actor: &str,
        idempotency: Option<(&str, &str, &str, &str)>,
    ) -> Result<Account, AccountingError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. Check duplicate code (409 Conflict)
        let exists: i64 = tx.query_row(
            "SELECT count(*) FROM accounts WHERE code = ?1",
            params![account.code],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Err(AccountingError::DuplicateAccount(format!(
                "Account code {} already exists",
                account.code
            )));
        }

        // 2. Validate parent if present
        if let Some(ref parent) = account.parent_code {
            if parent == &account.code {
                return Err(AccountingError::CircularAccountReference(format!(
                    "Self-referential account parent: {}",
                    account.code
                )));
            }
            let parent_row: Option<(i32, Option<String>)> = tx
                .query_row(
                    "SELECT is_active, parent_code FROM accounts WHERE code = ?1",
                    params![parent],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;

            match parent_row {
                None => {
                    return Err(AccountingError::ParentAccountNotFound(format!(
                        "Parent account {} does not exist",
                        parent
                    )));
                }
                Some((0, _)) => {
                    return Err(AccountingError::ParentAccountInactive(format!(
                        "Parent account {} is inactive",
                        parent
                    )));
                }
                Some((_, next_parent)) => {
                    // Walk ancestor chain to detect cycle
                    let mut curr_parent = next_parent;
                    let mut visited = std::collections::HashSet::new();
                    visited.insert(parent.clone());
                    while let Some(ancestor) = curr_parent {
                        if ancestor == account.code {
                            return Err(AccountingError::CircularAccountReference(format!(
                                "Circular reference detected between {} and {}",
                                account.code, ancestor
                            )));
                        }
                        if !visited.insert(ancestor.clone()) {
                            break;
                        }
                        let anc_parent: Option<Option<String>> = tx
                            .query_row(
                                "SELECT parent_code FROM accounts WHERE code = ?1",
                                params![ancestor],
                                |r| r.get(0),
                            )
                            .optional()?;
                        curr_parent = anc_parent.flatten();
                    }
                }
            }
        }

        // 3. Insert into accounts
        tx.execute(
            "INSERT INTO accounts (code, name, parent_code, class, is_active, normal_balance) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                account.code,
                account.name,
                account.parent_code,
                account.class.to_string(),
                if account.is_active { 1 } else { 0 },
                account.normal_balance.to_string(),
            ],
        )?;

        // 4. Audit log entry
        let audit_payload = serde_json::to_string(account).unwrap_or_default();
        tx.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details) \
             VALUES (?1, ?2, ?3, 'CREATE_ACCOUNT', 'ACCOUNT', ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                chrono::Utc::now().to_rfc3339(),
                actor,
                account.code,
                audit_payload,
            ],
        )?;

        // 5. Idempotency record if key provided
        if let Some((key, actor_key, endpoint, request_hash)) = idempotency {
            let response_body = serde_json::to_string(account).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to serialize account response: {e}"))
            })?;
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO idempotency_records (key, actor, endpoint, request_hash, status_code, response_body, resource_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    key,
                    actor_key,
                    endpoint,
                    request_hash,
                    201i64,
                    response_body,
                    account.code,
                    now,
                ],
            )?;
        }

        tx.commit()?;
        Ok(account.clone())
    }
}

impl AccountValidator for LedgerRepository {
    fn validate_account(&self, code: &str) -> Result<(), AccountingError> {
        let conn = self.conn.lock().unwrap();
        let row: Option<i32> = conn
            .query_row(
                "SELECT is_active FROM accounts WHERE code = ?1",
                params![code],
                |r| r.get(0),
            )
            .optional()?;
        match row {
            None => Err(AccountingError::UnknownAccount {
                account_code: code.to_string(),
            }),
            Some(0) => Err(AccountingError::InactiveAccount {
                account_code: code.to_string(),
            }),
            Some(_) => Ok(()),
        }
    }
}
