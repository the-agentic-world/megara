use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use super::super::domain::{AggregateEvent, PlanningState};
use super::persistence::{replay_core, replay_core_at};
use super::transaction::StoredOutcome;
use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct PersistedOutcome {
    pub state: PlanningState,
    pub event: Option<AggregateEvent>,
}

pub(crate) fn check_idempotency(
    tx: &Transaction<'_>,
    command_id: &str,
    request_hash: &str,
) -> Result<Option<StoredOutcome>, StoreError> {
    if let Some((stored_hash, session_id, response, resulting_event_id, resulting_revision)) = tx
        .query_row(
            "SELECT request_hash, session_id, core_response_json, resulting_event_id, resulting_revision FROM command_results WHERE command_id=?1",
            params![command_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, u64>(4)?,
                ))
            },
        )
        .optional()?
    {
        if stored_hash != request_hash {
            return Err(StoreError::CommandIdReuse);
        }
        let persisted: PersistedOutcome = serde_json::from_str(&response)
            .map_err(|error| StoreError::DbCorrupt(format!("command result: {error}")))?;
        let core = replay_core(tx, &session_id)?;
        let at_revision = replay_core_at(tx, &session_id, resulting_revision)?;
        let current = at_revision
            .state(&session_id)
            .ok_or_else(|| StoreError::DbCorrupt("command result session missing".to_string()))?;
        let expected_event = if let Some(event_id) = resulting_event_id.as_deref() {
            let (event_session, event_seq): (String, u64) = tx
                .query_row(
                    "SELECT session_id, seq FROM events WHERE event_id=?1",
                    params![event_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
                .ok_or_else(|| StoreError::DbCorrupt("command result event missing".to_string()))?;
            if event_session != session_id || event_seq != resulting_revision {
                return Err(StoreError::DbCorrupt(
                    "command result event binding mismatch".to_string(),
                ));
            }
            at_revision.events().last()
        } else {
            None
        };
        if persisted.state.session_id != session_id
            || persisted.state.revision != resulting_revision
            || &persisted.state != current
            || persisted.event.as_ref() != expected_event
            || (resulting_event_id.is_some() != persisted.event.is_some())
            || core.state(&session_id).is_none()
        {
            return Err(StoreError::DbCorrupt(
                "command result differs from authoritative replay".to_string(),
            ));
        }
        return Ok(Some(StoredOutcome {
            state: current.clone(),
            event: persisted.event,
            replayed: true,
        }));
    }
    if tx
        .query_row(
            "SELECT 1 FROM purged_command_ids WHERE command_id=?1 UNION SELECT 1 FROM purged_sessions WHERE purge_command_id=?1",
            params![command_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some()
    {
        return Err(StoreError::CommandIdRetired);
    }
    Ok(None)
}

pub(crate) fn persist_command_result(
    tx: &Transaction<'_>,
    command_id: &str,
    state: &PlanningState,
    event: Option<&AggregateEvent>,
    event_id: Option<&str>,
    request_hash: &str,
) -> Result<(), StoreError> {
    let response = PersistedOutcome {
        state: state.clone(),
        event: event.cloned(),
    };
    tx.execute(
        "INSERT INTO command_results(command_id, session_id, request_hash, core_response_json, resulting_event_id, resulting_revision) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            command_id,
            state.session_id,
            request_hash,
            serde_json::to_string(&response)?,
            event_id,
            state.revision,
        ],
    )?;
    Ok(())
}
