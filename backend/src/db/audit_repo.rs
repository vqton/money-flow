//! Immutable audit logging and idempotency record queries.

use crate::database::LedgerRepository;
use crate::dto::AuditLogQuery;
use crate::ledger::AccountingError;
use crate::models::{AuditLogEntry, IdempotencyRecord};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl LedgerRepository {
    /// Internal helper to record an immutable audit log entry within an active transaction.
    pub(crate) fn record_audit_log_tx(
        tx: &rusqlite::Transaction,
        entry: &AuditLogEntry,
    ) -> Result<(), rusqlite::Error> {
        tx.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.id.to_string(),
                entry.timestamp,
                entry.actor,
                entry.action,
                entry.resource_type,
                entry.resource_id,
                entry.details,
                entry.client_ip,
            ],
        )?;
        Ok(())
    }

    /// Retrieves an idempotency record by key, actor, and endpoint.
    pub fn get_idempotency_record(
        &self,
        key: &str,
        actor: &str,
        endpoint: &str,
    ) -> Result<Option<IdempotencyRecord>, AccountingError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, actor, endpoint, request_hash, status_code, response_body, resource_id, created_at
             FROM idempotency_records
             WHERE key = ?1 AND actor = ?2 AND endpoint = ?3",
        )?;
        let record = stmt
            .query_row(params![key, actor, endpoint], |row| {
                let status_raw: i64 = row.get(4)?;
                Ok(IdempotencyRecord {
                    key: row.get(0)?,
                    actor: row.get(1)?,
                    endpoint: row.get(2)?,
                    request_hash: row.get(3)?,
                    status_code: status_raw as u16,
                    response_body: row.get(5)?,
                    resource_id: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .optional()?;
        Ok(record)
    }

    /// Queries the immutable audit trail with optional filtering and pagination.
    pub fn list_audit_logs(
        &self,
        query: &AuditLogQuery,
    ) -> Result<Vec<AuditLogEntry>, AccountingError> {
        let conn = self.conn.lock().unwrap();

        let mut sql =
            "SELECT id, timestamp, actor, action, resource_type, resource_id, details, client_ip
                       FROM audit_logs WHERE 1=1"
                .to_string();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(ref actor) = query.actor {
            sql.push_str(" AND actor = ?");
            params_vec.push(rusqlite::types::Value::Text(actor.clone()));
        }
        if let Some(ref action) = query.action {
            sql.push_str(" AND action = ?");
            params_vec.push(rusqlite::types::Value::Text(action.clone()));
        }
        if let Some(ref r_type) = query.resource_type {
            sql.push_str(" AND resource_type = ?");
            params_vec.push(rusqlite::types::Value::Text(r_type.clone()));
        }
        if let Some(ref r_id) = query.resource_id {
            sql.push_str(" AND resource_id = ?");
            params_vec.push(rusqlite::types::Value::Text(r_id.clone()));
        }
        if let Some(ref from_d) = query.from_date {
            sql.push_str(" AND timestamp >= ?");
            params_vec.push(rusqlite::types::Value::Text(from_d.clone()));
        }
        if let Some(ref to_d) = query.to_date {
            sql.push_str(" AND timestamp <= ?");
            params_vec.push(rusqlite::types::Value::Text(to_d.clone()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        let limit = query.limit.unwrap_or(100).min(1000);
        sql.push_str(&format!(" LIMIT {}", limit));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            let id_str: String = row.get(0)?;
            let id = Uuid::parse_str(&id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let timestamp: String = row.get(1)?;
            let actor: String = row.get(2)?;
            let action: String = row.get(3)?;
            let resource_type: String = row.get(4)?;
            let resource_id: Option<String> = row.get(5)?;
            let details: Option<String> = row.get(6)?;
            let client_ip: Option<String> = row.get(7)?;

            Ok(AuditLogEntry {
                id,
                timestamp,
                actor,
                action,
                resource_type,
                resource_id,
                details,
                client_ip,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }
}
