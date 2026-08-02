use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::domain::SessionId;
use super::super::engine::CoreError;
use super::persistence::timestamp_now;
use super::transaction::replay_core_for_purge;
use super::*;

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

        let pending_backup_id = format!("cleanup_{}", Uuid::now_v7());
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
                pending_backup_id,
            ],
        )?;
        tx.commit()?;

        if cleanup_storage(&self.conn).is_ok() {
            receipt.cleanup_state = "clean".to_string();
            if update_cleanup_receipt(self, &receipt, command_id, None).is_err() {
                receipt.cleanup_state = "pending".to_string();
            }
        }
        Ok(receipt)
    }
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
