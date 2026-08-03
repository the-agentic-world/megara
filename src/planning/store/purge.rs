use std::{
    fs,
    path::{Component, Path},
};

use super::super::domain::SessionId;
use super::super::engine::CoreError;
use super::persistence::timestamp_now;
use super::transaction::replay_core_for_purge;
use super::*;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PurgeReceipt {
    pub session_id: SessionId,
    pub purged: bool,
    pub cleanup_state: String,
    pub replayed: bool,
}

impl PlanningStore {
    pub fn purge(
        &mut self,
        session_id: &str,
        command_id: &str,
        request_hash: &str,
        expected_revision: u64,
        confirmation: &str,
    ) -> Result<PurgeReceipt, StoreError> {
        let _lock = crate::planning::migration::acquire_project_lock(self.project_root())
            .map_err(|error| StoreError::InvalidRequest(error.to_string()))?;
        self.purge_with_options(
            session_id,
            command_id,
            request_hash,
            expected_revision,
            confirmation,
            true,
        )
    }

    pub(crate) fn purge_for_rollback(
        &mut self,
        session_id: &str,
        command_id: &str,
        request_hash: &str,
        expected_revision: u64,
        confirmation: &str,
    ) -> Result<PurgeReceipt, StoreError> {
        self.purge_with_options(
            session_id,
            command_id,
            request_hash,
            expected_revision,
            confirmation,
            false,
        )
    }

    fn purge_with_options(
        &mut self,
        session_id: &str,
        command_id: &str,
        request_hash: &str,
        expected_revision: u64,
        confirmation: &str,
        delete_linked_backup: bool,
    ) -> Result<PurgeReceipt, StoreError> {
        if command_id.trim().is_empty() || request_hash.trim().is_empty() {
            return Err(StoreError::InvalidRequest(
                "command_id and request_hash are required".to_string(),
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((stored_hash, response)) = tx
            .query_row(
                "SELECT request_hash, core_response_json FROM purged_sessions WHERE purge_command_id=?1",
                params![command_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if stored_hash != request_hash {
                return Err(StoreError::CommandIdReuse);
            }
            let mut receipt: PurgeReceipt = serde_json::from_str(&response)
                .map_err(|error| StoreError::DbCorrupt(format!("purge receipt: {error}")))?;
            receipt.replayed = true;
            tx.commit()?;
            return Ok(receipt);
        }
        if tx
            .query_row(
                "SELECT 1 FROM command_results WHERE command_id=?1",
                params![command_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Err(StoreError::CommandIdReuse);
        }
        if tx
            .query_row(
                "SELECT 1 FROM purged_command_ids WHERE command_id=?1",
                params![command_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Err(StoreError::CommandIdRetired);
        }
        if tx
            .query_row(
                "SELECT 1 FROM purged_sessions WHERE session_id=?1",
                params![session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            return Err(StoreError::SessionPurged(session_id.to_string()));
        }

        let core = replay_core_for_purge(&tx, session_id)?;
        let state = core
            .state(session_id)
            .cloned()
            .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))?;
        if confirmation != session_id {
            return Err(StoreError::PurgeConfirmationMismatch);
        }
        if state.revision != expected_revision {
            return Err(StoreError::Core(CoreError::RevisionConflict {
                expected: expected_revision,
                actual: state.revision,
            }));
        }
        let linked_backup_id = state
            .legacy_import
            .as_ref()
            .map(|legacy| legacy.source_backup_id.clone());
        let command_ids = tx
            .prepare("SELECT command_id FROM command_results WHERE session_id=?1")?
            .query_map(params![session_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for retired_id in command_ids
            .into_iter()
            .chain(std::iter::once(command_id.to_string()))
        {
            tx.execute(
                "INSERT OR IGNORE INTO purged_command_ids(command_id, session_id) VALUES(?1, ?2)",
                params![retired_id, session_id],
            )?;
        }
        tx.execute(
            "DELETE FROM command_results WHERE session_id=?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM events WHERE session_id=?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM sessions WHERE session_id=?1",
            params![session_id],
        )?;

        let mut receipt = PurgeReceipt {
            session_id: session_id.to_string(),
            purged: true,
            cleanup_state: "pending".to_string(),
            replayed: false,
        };
        tx.execute(
            "INSERT INTO purged_sessions(session_id, purged_at, purge_schema_version, purge_command_id, request_hash, core_response_json, cleanup_state, pending_backup_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id,
                timestamp_now(),
                STORE_SCHEMA_VERSION,
                command_id,
                request_hash,
                serde_json::to_string(&receipt)?,
                receipt.cleanup_state,
                linked_backup_id
                    .as_deref()
                    .filter(|_| delete_linked_backup),
            ],
        )?;
        tx.commit()?;

        let backup_clean = if delete_linked_backup {
            match linked_backup_id.as_deref() {
                Some(backup_id) => remove_linked_backup(self.project_root(), backup_id).is_ok(),
                None => true,
            }
        } else {
            true
        };
        let database_clean = cleanup_storage(&self.conn).is_ok();
        if backup_clean && database_clean {
            receipt.cleanup_state = "clean".to_string();
            if update_cleanup_receipt(self, &receipt, command_id, None).is_err() {
                receipt.cleanup_state = "pending".to_string();
            }
        } else if database_clean {
            let _ = update_cleanup_receipt(
                self,
                &receipt,
                command_id,
                linked_backup_id.as_deref().filter(|_| delete_linked_backup),
            );
        }
        Ok(receipt)
    }

    pub fn repair_pending_cleanup(&mut self) -> Result<u64, StoreError> {
        let _lock = crate::planning::migration::acquire_project_lock(self.project_root())
            .map_err(|error| StoreError::InvalidRequest(error.to_string()))?;
        self.repair_pending_cleanup_unlocked()
    }

    fn repair_pending_cleanup_unlocked(&mut self) -> Result<u64, StoreError> {
        let pending = {
            let mut statement = self.conn.prepare(
                "SELECT purge_command_id, pending_backup_id, core_response_json FROM purged_sessions WHERE cleanup_state='pending'",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, rusqlite::Error>>()?;
            rows
        };
        let mut repaired = 0;
        for (command_id, backup_id, response) in pending {
            let backup_clean = match backup_id.as_deref() {
                Some(id) => remove_linked_backup(self.project_root(), id).is_ok(),
                None => true,
            };
            if !backup_clean || cleanup_storage(&self.conn).is_err() {
                continue;
            }
            let mut receipt: PurgeReceipt = serde_json::from_str(&response)
                .map_err(|error| StoreError::DbCorrupt(format!("purge receipt: {error}")))?;
            receipt.cleanup_state = "clean".to_string();
            update_cleanup_receipt(self, &receipt, &command_id, None)?;
            repaired += 1;
        }
        Ok(repaired)
    }

    pub(crate) fn complete_purge_cleanup(&mut self, command_id: &str) -> Result<(), StoreError> {
        let response = self
            .conn
            .query_row(
                "SELECT core_response_json FROM purged_sessions WHERE purge_command_id=?1",
                params![command_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(StoreError::CommandIdRetired)?;
        let mut receipt: PurgeReceipt = serde_json::from_str(&response)
            .map_err(|error| StoreError::DbCorrupt(format!("purge receipt: {error}")))?;
        cleanup_storage(&self.conn)?;
        receipt.cleanup_state = "clean".to_string();
        update_cleanup_receipt(self, &receipt, command_id, None)
    }

    pub(crate) fn purge_receipt_exists(&self, command_id: &str) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM purged_sessions WHERE purge_command_id=?1",
                params![command_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some())
    }
}

fn remove_linked_backup(project: &Path, backup_id: &str) -> Result<(), StoreError> {
    validate_backup_id(backup_id)?;
    let root = project.join(".megara/migration-backups");
    let mut current = project.to_path_buf();
    for component in Path::new(".megara/migration-backups")
        .join(backup_id)
        .components()
    {
        let Component::Normal(part) = component else {
            continue;
        };
        current.push(part);
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(StoreError::InvalidRequest(
                "linked migration backup path contains a symlink".to_string(),
            ));
        }
    }
    let target = root.join(backup_id);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            crate::planning::migration::remove_tree_nofollow(&target).map_err(StoreError::Io)?;
            fs::File::open(&root)?.sync_all()?;
            Ok(())
        }
        Ok(_) => Err(StoreError::InvalidRequest(
            "linked migration backup is not a directory".to_string(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_backup_id(value: &str) -> Result<(), StoreError> {
    let Some(suffix) = value.strip_prefix("mig_") else {
        return Err(StoreError::InvalidRequest(
            "linked migration backup id is invalid".to_string(),
        ));
    };
    if suffix.is_empty()
        || value.len() > 128
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'z' | b'-'))
    {
        return Err(StoreError::InvalidRequest(
            "linked migration backup id is invalid".to_string(),
        ));
    }
    Ok(())
}

fn update_cleanup_receipt(
    store: &mut PlanningStore,
    receipt: &PurgeReceipt,
    command_id: &str,
    pending_backup_id: Option<&str>,
) -> Result<(), StoreError> {
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "UPDATE purged_sessions SET core_response_json=?1, cleanup_state=?2, pending_backup_id=?3 WHERE purge_command_id=?4",
        params![
            serde_json::to_string(receipt)?,
            receipt.cleanup_state,
            pending_backup_id,
            command_id,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn cleanup_storage(conn: &rusqlite::Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA wal_checkpoint(TRUNCATE);",
    )?;
    Ok(())
}
