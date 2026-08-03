use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::super::domain::{AggregateEvent, LifecyclePhase, PlanningState};
use super::super::engine::InMemoryPlanningCore;
use super::hash::normalized_state_hash;
use super::replay::{
    replay_events, semantic_payload_hash, EventEnvelope, EventMetadata, EventType,
    EVENT_ENVELOPE_SCHEMA_VERSION,
};
use super::transaction::EventContext;
use super::*;

pub(crate) fn insert_event(
    tx: &Transaction<'_>,
    command_id: &str,
    event: &AggregateEvent,
    state: &PlanningState,
    context: &EventContext,
) -> Result<String, StoreError> {
    let event_type = EventType::from_operation(&event.operation)
        .ok_or_else(|| StoreError::DbCorrupt("unknown event operation".to_string()))?;
    let envelope = EventEnvelope {
        schema_version: EVENT_ENVELOPE_SCHEMA_VERSION,
        event_id: Uuid::now_v7().to_string(),
        session_id: event.session_id.clone(),
        seq: event.seq,
        revision_after: event.revision_after,
        domain_revision_after: event.domain_revision_after,
        plan_revision_after: event.plan_revision_after,
        event_type,
        metadata: EventMetadata {
            occurred_at: timestamp_now(),
            actor: context.actor.clone(),
            adapter: context.adapter.clone(),
            request_id: context.request_id.clone(),
            command_id: command_id.to_string(),
        },
        semantic_payload: event.clone(),
        semantic_payload_hash: semantic_payload_hash(event)?,
        state_hash_after: normalized_state_hash(state),
    };
    tx.execute(
        "INSERT INTO events(event_id, session_id, seq, schema_version, revision_after, domain_revision_after, plan_revision_after, event_type, semantic_payload_json, metadata_json, semantic_payload_hash, state_hash_after) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            envelope.event_id,
            envelope.session_id,
            envelope.seq,
            envelope.schema_version,
            envelope.revision_after,
            envelope.domain_revision_after,
            envelope.plan_revision_after,
            envelope.event_type.operation(),
            serde_json::to_string(&envelope.semantic_payload)?,
            serde_json::to_string(&envelope.metadata)?,
            envelope.semantic_payload_hash,
            envelope.state_hash_after,
        ],
    )?;
    Ok(envelope.event_id)
}

pub(crate) fn upsert_session(
    tx: &Transaction<'_>,
    state: &PlanningState,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO sessions(session_id, project_id, phase, revision, domain_revision, plan_revision, state_json, normalized_state_hash) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(session_id) DO UPDATE SET project_id=excluded.project_id, phase=excluded.phase, revision=excluded.revision, domain_revision=excluded.domain_revision, plan_revision=excluded.plan_revision, state_json=excluded.state_json, normalized_state_hash=excluded.normalized_state_hash",
        params![
            state.session_id,
            state.project_id,
            phase_name(state.phase),
            state.revision,
            state.domain_revision,
            state.plan_revision,
            serde_json::to_string(state)?,
            normalized_state_hash(state),
        ],
    )?;
    Ok(())
}

pub(crate) fn replay_core(
    conn: &Connection,
    session_id: &str,
) -> Result<InMemoryPlanningCore, StoreError> {
    let envelopes = load_envelopes(conn, session_id, None)?;
    if envelopes.is_empty() {
        if conn
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
        return Err(StoreError::SessionNotFound(session_id.to_string()));
    }
    let core = replay_events(&envelopes)?;
    let state = core
        .state(session_id)
        .ok_or_else(|| StoreError::DbCorrupt("reducer state missing".to_string()))?;
    let cache: (String, String, u64, u64, u64, String, String) = conn
        .query_row(
            "SELECT project_id, phase, revision, domain_revision, plan_revision, state_json, normalized_state_hash FROM sessions WHERE session_id=?1",
            params![session_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))?;
    let cached_state: PlanningState = serde_json::from_str(&cache.5)
        .map_err(|error| StoreError::ProjectionDiverged(format!("cache json: {error}")))?;
    let cached_phase = parse_phase(&cache.1).ok_or_else(|| {
        StoreError::ProjectionDiverged("session phase scalar is invalid".to_string())
    })?;
    if &cached_state != state
        || cache.0 != state.project_id
        || cached_phase != state.phase
        || cache.2 != state.revision
        || cache.3 != state.domain_revision
        || cache.4 != state.plan_revision
        || cache.6 != normalized_state_hash(state)
    {
        return Err(StoreError::ProjectionDiverged(
            "session cache differs from replay".to_string(),
        ));
    }
    Ok(core)
}

pub(crate) fn replay_state_from_events(
    conn: &Connection,
    session_id: &str,
) -> Result<PlanningState, StoreError> {
    let envelopes = load_envelopes(conn, session_id, None)?;
    if envelopes.is_empty() {
        return Err(
            if conn
                .query_row(
                    "SELECT 1 FROM purged_sessions WHERE session_id=?1",
                    params![session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .is_some()
            {
                StoreError::SessionPurged(session_id.to_string())
            } else {
                StoreError::SessionNotFound(session_id.to_string())
            },
        );
    }
    let core = replay_events(&envelopes)?;
    core.state(session_id)
        .cloned()
        .ok_or_else(|| StoreError::DbCorrupt("reducer state missing".to_string()))
}

pub(crate) fn cache_matches_state(
    conn: &Connection,
    state: &PlanningState,
) -> Result<bool, StoreError> {
    let cache: (String, String, u64, u64, u64, String, String) = conn
        .query_row(
            "SELECT project_id, phase, revision, domain_revision, plan_revision, state_json, normalized_state_hash FROM sessions WHERE session_id=?1",
            params![state.session_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::ProjectionDiverged("session cache is missing".to_string()))?;
    let cached_state: PlanningState = serde_json::from_str(&cache.5)
        .map_err(|error| StoreError::ProjectionDiverged(format!("cache json: {error}")))?;
    let cached_phase = parse_phase(&cache.1).ok_or_else(|| {
        StoreError::ProjectionDiverged("session phase scalar is invalid".to_string())
    })?;
    Ok(cached_state == *state
        && cache.0 == state.project_id
        && cached_phase == state.phase
        && cache.2 == state.revision
        && cache.3 == state.domain_revision
        && cache.4 == state.plan_revision
        && cache.6 == normalized_state_hash(state))
}

pub(crate) fn replay_core_at(
    conn: &Connection,
    session_id: &str,
    revision: u64,
) -> Result<InMemoryPlanningCore, StoreError> {
    let envelopes = load_envelopes(conn, session_id, Some(revision))?;
    if envelopes.is_empty() {
        return Err(StoreError::DbCorrupt(
            "command result revision has no events".to_string(),
        ));
    }
    replay_events(&envelopes)
}

pub(crate) fn replay_core_for_purge(
    tx: &Transaction<'_>,
    session_id: &str,
) -> Result<InMemoryPlanningCore, StoreError> {
    replay_core(tx, session_id)
}

pub(crate) fn load_envelopes(
    conn: &Connection,
    session_id: &str,
    max_seq: Option<u64>,
) -> Result<Vec<EventEnvelope>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT event_id, session_id, seq, schema_version, revision_after, domain_revision_after, plan_revision_after, event_type, semantic_payload_json, metadata_json, semantic_payload_hash, state_hash_after FROM events WHERE session_id=?1 AND (?2 IS NULL OR seq <= ?2) ORDER BY seq ASC",
    )?;
    let rows = statement
        .query_map(params![session_id, max_seq], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, u64>(4)?,
                row.get::<_, u64>(5)?,
                row.get::<_, u64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(
                event_id,
                row_session_id,
                seq,
                schema_version,
                revision_after,
                domain_revision_after,
                plan_revision_after,
                event_type_value,
                payload,
                metadata_json,
                semantic_payload_hash,
                state_hash_after,
            )| {
                let event: AggregateEvent = serde_json::from_str(&payload)
                    .map_err(|error| StoreError::DbCorrupt(format!("event payload: {error}")))?;
                let metadata = serde_json::from_str(&metadata_json)
                    .map_err(|error| StoreError::DbCorrupt(format!("event metadata: {error}")))?;
                let event_type = EventType::from_operation(&event_type_value)
                    .ok_or_else(|| StoreError::DbCorrupt("event type is unknown".to_string()))?;
                Ok(EventEnvelope {
                    schema_version,
                    event_id,
                    session_id: row_session_id,
                    seq,
                    revision_after,
                    domain_revision_after,
                    plan_revision_after,
                    event_type,
                    metadata,
                    semantic_payload: event,
                    semantic_payload_hash,
                    state_hash_after,
                })
            },
        )
        .collect()
}

pub(crate) fn event_envelopes(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<EventEnvelope>, StoreError> {
    load_envelopes(conn, session_id, None)
}

pub(crate) fn phase_name(phase: LifecyclePhase) -> &'static str {
    match phase {
        LifecyclePhase::Interview => "interview",
        LifecyclePhase::Specification => "specification",
        LifecyclePhase::Planning => "planning",
        LifecyclePhase::Complete => "complete",
    }
}

pub(crate) fn parse_phase(value: &str) -> Option<LifecyclePhase> {
    match value {
        "interview" => Some(LifecyclePhase::Interview),
        "specification" => Some(LifecyclePhase::Specification),
        "planning" => Some(LifecyclePhase::Planning),
        "complete" => Some(LifecyclePhase::Complete),
        _ => None,
    }
}

pub(crate) fn timestamp_now() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch");
    format!("{}.{:09}Z", duration.as_secs(), duration.subsec_nanos())
}
