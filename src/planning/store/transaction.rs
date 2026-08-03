use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::Value;

use super::super::domain::{AggregateEvent, LifecyclePhase, PlanningState};
use super::super::engine::{
    AnswerCommand, ApprovalCommand, AuditCommand, CoreError, EvidenceRefreshCommand,
    EvidenceRefreshResult, InMemoryPlanningCore, MutationResult, PlanCandidateCommand,
    RevisionRequestCommand, SpecCandidateCommand,
};
use super::command_cache::{check_idempotency, persist_command_result};
use super::persistence::{event_envelopes, insert_event, replay_core, upsert_session};
use super::replay::{EventActor, EventAdapter, EventEnvelope};
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreOutcome {
    Changed(MutationResult),
    Unchanged { state: PlanningState },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredOutcome {
    pub state: PlanningState,
    pub event: Option<AggregateEvent>,
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EventContext {
    pub actor: EventActor,
    pub adapter: EventAdapter,
    pub request_id: Option<String>,
}

impl Default for EventContext {
    fn default() -> Self {
        Self {
            actor: EventActor::System,
            adapter: EventAdapter::Core,
            request_id: None,
        }
    }
}

impl PlanningStore {
    pub(crate) fn cached_command(
        &mut self,
        command_id: &str,
        request_hash: &str,
    ) -> Result<Option<StoredOutcome>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let outcome = check_idempotency(&tx, command_id, request_hash)?;
        tx.commit()?;
        Ok(outcome)
    }

    pub fn start(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: StartCommand,
    ) -> Result<StoredOutcome, StoreError> {
        self.start_with_context(command_id, request_hash, command, EventContext::default())
    }

    pub(crate) fn start_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: StartCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        validate_command_identity(command_id, request_hash)?;
        if command.project_id != self.identity.project_id {
            return Err(StoreError::ProjectIdMismatch {
                expected: self.identity.project_id.clone(),
                actual: command.project_id,
            });
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(outcome) = check_idempotency(&tx, command_id, request_hash)? {
            tx.commit()?;
            return Ok(outcome);
        }
        if let Some(session_id) = command.session_id.as_deref() {
            if tx
                .query_row(
                    "SELECT 1 FROM sessions WHERE session_id=?1",
                    params![session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .is_some()
            {
                return Err(StoreError::Core(CoreError::SessionExists(
                    session_id.to_string(),
                )));
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
        }
        let mut core = InMemoryPlanningCore::default();
        let result = core.start(command)?;
        let state = result.state.clone();
        upsert_session(&tx, &state)?;
        let event_id = insert_event(&tx, command_id, &result.event, &state, &context)?;
        persist_command_result(
            &tx,
            command_id,
            &state,
            Some(&result.event),
            Some(&event_id),
            request_hash,
        )?;
        tx.commit()?;
        Ok(StoredOutcome {
            state,
            event: Some(result.event),
            replayed: false,
        })
    }

    pub(crate) fn execute<F>(
        &mut self,
        command_id: &str,
        request_hash: &str,
        session_id: &str,
        apply: F,
    ) -> Result<StoredOutcome, StoreError>
    where
        F: FnOnce(&mut InMemoryPlanningCore) -> Result<CoreOutcome, CoreError>,
    {
        self.execute_with_context(
            command_id,
            request_hash,
            session_id,
            apply,
            EventContext::default(),
        )
    }

    pub(crate) fn execute_with_context<F>(
        &mut self,
        command_id: &str,
        request_hash: &str,
        session_id: &str,
        apply: F,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError>
    where
        F: FnOnce(&mut InMemoryPlanningCore) -> Result<CoreOutcome, CoreError>,
    {
        validate_command_identity(command_id, request_hash)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(outcome) = check_idempotency(&tx, command_id, request_hash)? {
            tx.commit()?;
            return Ok(outcome);
        }
        let mut core = replay_core(&tx, session_id)?;
        let before_state = core
            .state(session_id)
            .cloned()
            .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))?;
        let before_event_count = core.events().len();
        let outcome = apply(&mut core)?;
        let (state, event) = match outcome {
            CoreOutcome::Changed(result) => (result.state, Some(result.event)),
            CoreOutcome::Unchanged { state } => (state, None),
        };
        let after_state = core
            .state(session_id)
            .cloned()
            .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))?;
        match event.as_ref() {
            Some(event)
                if after_state.revision == before_state.revision + 1
                    && core.events().len() == before_event_count + 1
                    && state == after_state
                    && core.events().last() == Some(event) => {}
            None if after_state == before_state
                && state == before_state
                && core.events().len() == before_event_count => {}
            Some(_) => {
                return Err(StoreError::InvalidRequest(
                    "one store mutation must append exactly one event and revision".to_string(),
                ));
            }
            None => {
                return Err(StoreError::InvalidRequest(
                    "unchanged store command must append no event or revision".to_string(),
                ));
            }
        }
        let event_id = event
            .as_ref()
            .map(|event| insert_event(&tx, command_id, event, &state, &context))
            .transpose()?;
        upsert_session(&tx, &state)?;
        persist_command_result(
            &tx,
            command_id,
            &state,
            event.as_ref(),
            event_id.as_deref(),
            request_hash,
        )?;
        tx.commit()?;
        Ok(StoredOutcome {
            state,
            event,
            replayed: false,
        })
    }

    pub fn apply_answer(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: AnswerCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.answer(command).map(CoreOutcome::Changed)
        })
    }

    pub(crate) fn apply_answer_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: AnswerCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.answer(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub fn refresh_evidence(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: EvidenceRefreshCommand,
    ) -> Result<StoredOutcome, StoreError> {
        self.refresh_evidence_with_context(
            command_id,
            request_hash,
            command,
            EventContext::default(),
        )
    }

    pub(crate) fn refresh_evidence_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: EvidenceRefreshCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| match core.refresh_evidence(command)? {
                EvidenceRefreshResult::Changed(result) => Ok(CoreOutcome::Changed(result)),
                EvidenceRefreshResult::Unchanged { state } => Ok(CoreOutcome::Unchanged { state }),
            },
            context,
        )
    }

    pub fn apply_audit(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: AuditCommand,
    ) -> Result<StoredOutcome, StoreError> {
        self.apply_audit_with_context(command_id, request_hash, command, EventContext::default())
    }

    pub(crate) fn apply_audit_with_context(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: AuditCommand,
        context: EventContext,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute_with_context(
            command_id,
            request_hash,
            &session_id,
            move |core| core.apply_audit(command).map(CoreOutcome::Changed),
            context,
        )
    }

    pub fn generate_spec(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: SpecCandidateCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.generate_spec(command).map(CoreOutcome::Changed)
        })
    }

    pub fn approve_spec(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: ApprovalCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.approve_spec(command).map(CoreOutcome::Changed)
        })
    }

    pub fn revise_spec(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: RevisionRequestCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.revise_spec(command).map(CoreOutcome::Changed)
        })
    }

    pub fn generate_plan(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: PlanCandidateCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.generate_plan(command).map(CoreOutcome::Changed)
        })
    }

    pub fn approve_plan(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: ApprovalCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.approve_plan(command).map(CoreOutcome::Changed)
        })
    }

    pub fn revise_plan(
        &mut self,
        command_id: &str,
        request_hash: &str,
        command: RevisionRequestCommand,
    ) -> Result<StoredOutcome, StoreError> {
        let session_id = command.session_id.clone();
        self.execute(command_id, request_hash, &session_id, move |core| {
            core.revise_plan(command).map(CoreOutcome::Changed)
        })
    }

    pub fn current(&self, session_id: &str) -> Result<PlanningState, StoreError> {
        let core = replay_core(&self.conn, session_id)?;
        core.state(session_id)
            .cloned()
            .ok_or_else(|| StoreError::SessionNotFound(session_id.to_string()))
    }

    pub fn replay(&self, session_id: &str) -> Result<PlanningState, StoreError> {
        self.current(session_id)
    }

    pub fn diagnostic_semantic_event_sequence(
        &self,
        session_id: &str,
    ) -> Result<Vec<String>, StoreError> {
        let events = self.event_envelopes(session_id)?;
        super::replay::normalized_semantic_event_sequence(&events)
    }

    pub fn list(&self, phase: Option<LifecyclePhase>) -> Result<Vec<PlanningState>, StoreError> {
        let mut statement = self
            .conn
            .prepare("SELECT session_id FROM sessions ORDER BY revision DESC, session_id ASC")?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.current(&id))
            .filter(|result| match (phase, result) {
                (Some(expected), Ok(state)) => state.phase == expected,
                (None, _) => true,
                (_, Err(_)) => true,
            })
            .collect()
    }

    pub fn event_count(&self, session_id: &str) -> Result<u64, StoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE session_id=?1",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn event_envelopes(&self, session_id: &str) -> Result<Vec<EventEnvelope>, StoreError> {
        let envelopes = event_envelopes(&self.conn, session_id)?;
        if envelopes.is_empty() {
            return Err(StoreError::SessionNotFound(session_id.to_string()));
        }
        Ok(envelopes)
    }

    pub fn command_result_json(&self, command_id: &str) -> Result<Option<Value>, StoreError> {
        let value = self
            .conn
            .query_row(
                "SELECT core_response_json FROM command_results WHERE command_id=?1",
                params![command_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        value
            .map(|text| serde_json::from_str(&text).map_err(StoreError::Json))
            .transpose()
    }
}

pub(crate) fn validate_command_identity(
    command_id: &str,
    request_hash: &str,
) -> Result<(), StoreError> {
    if command_id.trim().is_empty() || request_hash.trim().is_empty() {
        return Err(StoreError::InvalidRequest(
            "command_id and request_hash are required".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn replay_core_for_purge(
    tx: &Transaction<'_>,
    session_id: &str,
) -> Result<InMemoryPlanningCore, StoreError> {
    super::persistence::replay_core_for_purge(tx, session_id)
}
