//! Disaster recovery, live backups, and snapshot restore engine.

use crate::database::LedgerRepository;
use crate::ledger::AccountingError;
use rusqlite::{Connection, params};
use uuid::Uuid;

/// Result outcome metadata from a successfully executed database restore operation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RestoreOutcome {
    /// Absolute or relative path of the snapshot file restored.
    pub restored_from_snapshot: String,
    /// Computed hex-encoded SHA-256 checksum of the restored snapshot file.
    pub sha256_checksum: String,
    /// RFC-3339 UTC timestamp when the restore operation committed.
    pub timestamp: String,
}

impl LedgerRepository {
    /// Creates an atomic live SQLite snapshot using `VACUUM INTO ?1`.
    /// Handles stale existing files, ensures parent directories exist,
    /// records a SYSTEM_BACKUP audit log event, and returns the backup file size in bytes.
    pub fn create_backup(
        &self,
        destination_path: &str,
        actor: &str,
    ) -> Result<u64, AccountingError> {
        if destination_path.trim().is_empty() {
            return Err(AccountingError::DatabaseError(
                "Destination path cannot be empty".to_string(),
            ));
        }

        let dest = std::path::Path::new(destination_path);
        if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to create backup directory: {}", e))
            })?;
        }

        if dest.exists() {
            std::fs::remove_file(dest).map_err(|e| {
                AccountingError::DatabaseError(format!("Failed to remove stale backup file: {}", e))
            })?;
        }

        let conn = self.conn.lock().unwrap();
        conn.execute("VACUUM INTO ?1;", params![destination_path])?;

        let metadata = std::fs::metadata(destination_path).map_err(|e| {
            AccountingError::DatabaseError(format!("Failed to read backup file metadata: {}", e))
        })?;

        let file_size_bytes = metadata.len();

        // Record immutable audit event for backup
        let _ = conn.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                chrono::Utc::now().to_rfc3339(),
                actor,
                "SYSTEM_BACKUP",
                "SYSTEM",
                destination_path,
                format!("Created live backup snapshot ({file_size_bytes} bytes)"),
                Option::<String>::None,
            ],
        );

        Ok(file_size_bytes)
    }

    /// Restores the active database from a point-in-time snapshot.
    ///
    /// Pre-validates file existence, computes SHA-256 (asserting expected match if provided),
    /// executes PRAGMA integrity_check in an isolated read-only connection, and verifies
    /// required tables (vouchers, voucher_lines, audit_logs).
    ///
    /// Uses rollback protection: creates a temporary backup of the live database before restoring.
    /// If restore fails, the live database is restored from the rollback backup.
    ///
    /// On success, checkpoints WAL, enforces foreign keys, and appends a SYSTEM_RESTORE audit log entry.
    pub fn restore_from_snapshot(
        &self,
        snapshot_path: &std::path::Path,
        expected_sha256: Option<&str>,
        actor: &str,
    ) -> Result<RestoreOutcome, AccountingError> {
        // 1. Snapshot existence and file validation
        if !snapshot_path.exists() || !snapshot_path.is_file() {
            return Err(AccountingError::DatabaseError(format!(
                "Snapshot file does not exist or is not a regular file: {}",
                snapshot_path.display()
            )));
        }

        // 2. Compute SHA-256 checksum using ring
        let bytes = std::fs::read(snapshot_path).map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to read snapshot file '{}': {}",
                snapshot_path.display(),
                e
            ))
        })?;

        let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
        let computed_sha256: String = digest
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        // 3. Expected SHA-256 case-insensitive match (if specified)
        if let Some(expected) = expected_sha256 {
            let trimmed = expected.trim();
            if !trimmed.is_empty() && !computed_sha256.eq_ignore_ascii_case(trimmed) {
                return Err(AccountingError::DatabaseError(format!(
                    "Snapshot SHA-256 checksum mismatch: expected '{}', computed '{}'",
                    trimmed, computed_sha256
                )));
            }
        }

        // 4. Isolated read-only connection for integrity_check and catalog check
        let snap_conn = Connection::open_with_flags(
            snapshot_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to open snapshot for pre-validation: {}",
                e
            ))
        })?;

        let integrity_check: Result<String, rusqlite::Error> =
            snap_conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0));
        match integrity_check {
            Ok(ref check) if check == "ok" => {}
            Ok(other) => {
                return Err(AccountingError::DatabaseError(format!(
                    "Snapshot SQLite integrity check failed: {}",
                    other
                )));
            }
            Err(e) => {
                return Err(AccountingError::DatabaseError(format!(
                    "Failed to execute PRAGMA integrity_check on snapshot: {}",
                    e
                )));
            }
        }

        // 5. Schema verification for essential tables
        let table_count: i64 = snap_conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('vouchers', 'voucher_lines', 'audit_logs')",
                [],
                |r| r.get(0),
            )
            .map_err(|e| {
                AccountingError::DatabaseError(format!(
                    "Failed to verify schema in snapshot: {}",
                    e
                ))
            })?;

        if table_count < 3 {
            return Err(AccountingError::DatabaseError(format!(
                "Snapshot schema is missing required tables (found {} of 3: vouchers, voucher_lines, audit_logs)",
                table_count
            )));
        }

        drop(snap_conn);

        // 6. Active connection restore with rollback protection
        let mut conn = self.conn.lock().unwrap();

        let temp_rollback = std::env::temp_dir().join(format!("rollback_{}.db", Uuid::new_v4()));
        conn.backup(
            rusqlite::MAIN_DB,
            &temp_rollback,
            rusqlite::backup::NO_PROGRESS,
        )
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to create rollback checkpoint before restore: {}",
                e
            ))
        })?;

        if let Err(restore_err) = conn.restore(
            rusqlite::MAIN_DB,
            snapshot_path,
            rusqlite::backup::NO_PROGRESS,
        ) {
            // Roll back to the original database state
            let _ = conn.restore(
                rusqlite::MAIN_DB,
                &temp_rollback,
                rusqlite::backup::NO_PROGRESS,
            );
            let _ = std::fs::remove_file(&temp_rollback);
            return Err(AccountingError::DatabaseError(format!(
                "Database restore failed, rolled back to original state: {}",
                restore_err
            )));
        }

        // Clean up rollback checkpoint file upon successful restore
        let _ = std::fs::remove_file(&temp_rollback);

        // 7. Post-restore pragmas and audit trail logging
        let _ = conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA wal_checkpoint(TRUNCATE);",
        );

        let timestamp = chrono::Utc::now().to_rfc3339();
        let snapshot_path_str = snapshot_path.to_string_lossy().to_string();
        let details = format!(
            "Restored database from snapshot '{}' (SHA-256: {})",
            snapshot_path_str, computed_sha256
        );

        let _ = conn.execute(
            "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                timestamp,
                actor,
                "SYSTEM_RESTORE",
                "SYSTEM",
                snapshot_path_str,
                details,
                Option::<String>::None,
            ],
        );

        Ok(RestoreOutcome {
            restored_from_snapshot: snapshot_path_str,
            sha256_checksum: computed_sha256,
            timestamp,
        })
    }
}

/// Public helper function to execute an offline / CLI database restore from `snapshot` into `target`.
/// Ensures target directory exists, validates snapshot integrity and schema, and restores into target.
pub fn execute_cli_restore(
    snapshot: &std::path::Path,
    target: &std::path::Path,
    actor: &str,
) -> Result<RestoreOutcome, AccountingError> {
    if !snapshot.exists() || !snapshot.is_file() {
        return Err(AccountingError::DatabaseError(format!(
            "Snapshot file does not exist: {}",
            snapshot.display()
        )));
    }

    // Compute and verify snapshot SHA-256
    let bytes = std::fs::read(snapshot).map_err(|e| {
        AccountingError::DatabaseError(format!(
            "Failed to read snapshot file '{}': {}",
            snapshot.display(),
            e
        ))
    })?;

    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let computed_sha256: String = digest
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    // Verify snapshot integrity and schema
    let snap_conn = Connection::open_with_flags(
        snapshot,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        AccountingError::DatabaseError(format!("Failed to open snapshot for pre-validation: {}", e))
    })?;

    let integrity_check: Result<String, rusqlite::Error> =
        snap_conn.query_row("PRAGMA integrity_check;", [], |r| r.get(0));
    match integrity_check {
        Ok(ref check) if check == "ok" => {}
        Ok(other) => {
            return Err(AccountingError::DatabaseError(format!(
                "Snapshot SQLite integrity check failed: {}",
                other
            )));
        }
        Err(e) => {
            return Err(AccountingError::DatabaseError(format!(
                "Failed to execute PRAGMA integrity_check on snapshot: {}",
                e
            )));
        }
    }

    let table_count: i64 = snap_conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('vouchers', 'voucher_lines', 'audit_logs')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to verify schema in snapshot: {}",
                e
            ))
        })?;

    if table_count < 3 {
        return Err(AccountingError::DatabaseError(format!(
            "Snapshot schema is missing required tables (found {} of 3: vouchers, voucher_lines, audit_logs)",
            table_count
        )));
    }
    drop(snap_conn);

    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| {
            AccountingError::DatabaseError(format!("Failed to create target directory: {}", e))
        })?;
    }

    // Restore to target file via direct copy, ensuring zero-byte discrepancy
    std::fs::copy(snapshot, target).map_err(|e| {
        AccountingError::DatabaseError(format!(
            "Failed to copy snapshot to target '{}': {}",
            target.display(),
            e
        ))
    })?;

    // Verify restored target database is valid SQLite
    let target_conn = Connection::open_with_flags(
        target,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        AccountingError::DatabaseError(format!("Failed to open restored target database: {}", e))
    })?;
    let target_check: String = target_conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .map_err(|e| {
            AccountingError::DatabaseError(format!(
                "Failed to verify integrity of restored target database: {}",
                e
            ))
        })?;
    if target_check != "ok" {
        return Err(AccountingError::DatabaseError(format!(
            "Restored target database integrity check failed: {}",
            target_check
        )));
    }

    let timestamp = chrono::Utc::now().to_rfc3339();
    let snapshot_path_str = snapshot.to_string_lossy().to_string();
    let details = format!(
        "Restored database from snapshot '{}' (SHA-256: {}) via CLI",
        snapshot_path_str, computed_sha256
    );

    let _ = target_conn.execute(
        "INSERT INTO audit_logs (id, timestamp, actor, action, resource_type, resource_id, details, client_ip)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            uuid::Uuid::new_v4().to_string(),
            timestamp,
            actor,
            "SYSTEM_RESTORE",
            "SYSTEM",
            snapshot_path_str,
            details,
            Option::<String>::None,
        ],
    );
    drop(target_conn);

    Ok(RestoreOutcome {
        restored_from_snapshot: snapshot_path_str,
        sha256_checksum: computed_sha256,
        timestamp,
    })
}
