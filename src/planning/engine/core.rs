use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;

impl InMemoryPlanningCore {
    pub fn state(&self, session_id: &str) -> Option<&PlanningState> {
        self.sessions.get(session_id)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &PlanningState> {
        self.sessions.values()
    }

    pub fn events(&self) -> &[AggregateEvent] {
        &self.events
    }

    pub fn start(&mut self, command: StartCommand) -> Result<MutationResult, CoreError> {
        if command.project_id.trim().is_empty() || command.request.trim().is_empty() {
            return Err(CoreError::InvalidRequest(
                "project_id and request must not be blank".to_string(),
            ));
        }
        let session_id = command.session_id.unwrap_or_else(|| {
            self.next_session_number += 1;
            format!("pln_{:04}", self.next_session_number)
        });
        if self.sessions.contains_key(&session_id) {
            return Err(CoreError::SessionExists(session_id));
        }
        let mut state = PlanningState::new(session_id.clone(), command.project_id, command.request);
        state.domain_revision = 1;
        state.required_model_action = Some(work_item(
            &state,
            ModelActionKind::DeltaAudit,
            hash_text(&state.transcript.initial_request),
        ));
        state.revision = 1;
        let event = AggregateEvent {
            session_id: session_id.clone(),
            seq: 1,
            revision_after: 1,
            domain_revision_after: 1,
            plan_revision_after: 0,
            operation: "planning.start".to_string(),
            primary: json!({"initial_request": state.transcript.initial_request}),
            effects: vec![EventEffect::ModelActionRequested {
                kind: ModelActionKind::DeltaAudit,
            }],
        };
        self.insert_started(state.clone(), event.clone())?;
        Ok(MutationResult { state, event })
    }

    pub fn answer(&mut self, command: AnswerCommand) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.answer",
            |state, effects| {
                if state.phase != LifecyclePhase::Interview {
                    return Err(CoreError::InvalidPhase(
                        "answer requires Interview".to_string(),
                    ));
                }
                let pending = state
                    .pending_question
                    .clone()
                    .ok_or(CoreError::QuestionMismatch)?;
                if pending.question_id != command.question_id
                    || pending.based_on_revision != command.based_on_revision
                {
                    return Err(CoreError::QuestionMismatch);
                }
                if command.text.trim().is_empty() {
                    return Err(CoreError::InvalidRequest(
                        "answer must not be blank".to_string(),
                    ));
                }
                state.pending_question = None;
                state.domain_revision += 1;
                state.transcript.answers.push(AnswerRecord {
                    answer_id: format!("ans_{}", state.revision + 1),
                    question_id: command.question_id.clone(),
                    based_on_revision: command.based_on_revision,
                    text: command.text.clone(),
                    selected_choice_ids: command.selected_choice_ids.clone(),
                });
                invalidate_artifacts(state, effects);
                let next = work_item(state, ModelActionKind::DeltaAudit, hash_text(&command.text));
                state.required_model_action = Some(next);
                effects.push(EventEffect::ModelActionRequested {
                    kind: ModelActionKind::DeltaAudit,
                });
                Ok(json!({"question_id": command.question_id}))
            },
        )
    }

    pub fn refresh_evidence(
        &mut self,
        command: EvidenceRefreshCommand,
    ) -> Result<EvidenceRefreshResult, CoreError> {
        let current = self
            .sessions
            .get(&command.session_id)
            .cloned()
            .ok_or_else(|| CoreError::SessionNotFound(command.session_id.clone()))?;
        if current.revision != command.expected_revision {
            return Err(CoreError::RevisionConflict {
                expected: command.expected_revision,
                actual: current.revision,
            });
        }
        if current.repo_snapshot.as_ref() == Some(&command.snapshot) {
            return Ok(EvidenceRefreshResult::Unchanged { state: current });
        }
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.evidence.refresh",
            |state, effects| {
                state.repo_snapshot = Some(command.snapshot.clone());
                state.domain_revision += 1;
                invalidate_evidence_entities(
                    state,
                    effects,
                    SourceRef::Evidence {
                        id: command.snapshot.evidence_hash.clone(),
                    },
                );
                if state.phase != LifecyclePhase::Interview {
                    state.phase = LifecyclePhase::Interview;
                    effects.push(EventEffect::PhaseChanged {
                        phase: LifecyclePhase::Interview,
                    });
                }
                state.pending_question = None;
                state.full_audit = None;
                invalidate_artifacts(state, effects);
                state.required_model_action = Some(work_item(
                    state,
                    ModelActionKind::DeltaAudit,
                    command.snapshot.evidence_hash.clone(),
                ));
                effects.push(EventEffect::ModelActionRequested {
                    kind: ModelActionKind::DeltaAudit,
                });
                Ok(json!({"evidence_hash": command.snapshot.evidence_hash}))
            },
        )
        .map(EvidenceRefreshResult::Changed)
    }
    fn insert_started(
        &mut self,
        state: PlanningState,
        event: AggregateEvent,
    ) -> Result<(), CoreError> {
        state.assert_invariants().map_err(CoreError::Invariant)?;
        self.sessions.insert(state.session_id.clone(), state);
        self.events.push(event);
        Ok(())
    }

    pub(crate) fn mutate<F>(
        &mut self,
        session_id: &str,
        expected_revision: u64,
        operation: &str,
        apply: F,
    ) -> Result<MutationResult, CoreError>
    where
        F: FnOnce(&mut PlanningState, &mut Vec<EventEffect>) -> Result<Value, CoreError>,
    {
        let current = self
            .sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| CoreError::SessionNotFound(session_id.to_string()))?;
        if current.revision != expected_revision {
            return Err(CoreError::RevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let mut next = current.clone();
        let mut effects = Vec::new();
        let primary = apply(&mut next, &mut effects)?;
        next.revision += 1;
        if next.domain_revision < current.domain_revision
            || next.domain_revision > current.domain_revision + 1
            || next.plan_revision < current.plan_revision
            || next.plan_revision > current.plan_revision + 1
        {
            return Err(CoreError::Invariant(
                "one mutation may advance each derived revision at most once".to_string(),
            ));
        }
        next.assert_invariants().map_err(CoreError::Invariant)?;
        let event = AggregateEvent {
            session_id: session_id.to_string(),
            seq: next.revision,
            revision_after: next.revision,
            domain_revision_after: next.domain_revision,
            plan_revision_after: next.plan_revision,
            operation: operation.to_string(),
            primary,
            effects,
        };
        self.sessions.insert(session_id.to_string(), next.clone());
        self.events.push(event.clone());
        Ok(MutationResult { state: next, event })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceRefreshResult {
    Changed(MutationResult),
    Unchanged { state: PlanningState },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationResult {
    pub state: PlanningState,
    pub event: AggregateEvent,
}
pub(crate) fn work_item(
    state: &PlanningState,
    kind: ModelActionKind,
    input_hash: String,
) -> ModelWorkItem {
    let output_schema = match kind {
        ModelActionKind::DeltaAudit | ModelActionKind::FullAudit => "megara.audit-proposal/v1",
        ModelActionKind::GenerateSpec => "megara.spec-proposal/v1",
        ModelActionKind::GeneratePlan => "megara.plan-proposal/v1",
    };
    ModelWorkItem {
        kind,
        work_item_id: format!(
            "wrk_{:?}_{}_{}_{}",
            kind,
            state.revision + 1,
            state.domain_revision,
            state.plan_revision
        )
        .to_lowercase(),
        session_id: state.session_id.clone(),
        base_revision: state.revision + 1,
        base_domain_revision: state.domain_revision,
        base_plan_revision: state.plan_revision,
        input_hash,
        output_schema: output_schema.to_string(),
        context: json!({"initial_request": state.transcript.initial_request}),
        question_authoring: matches!(
            kind,
            ModelActionKind::DeltaAudit | ModelActionKind::FullAudit
        )
        .then(QuestionAuthoring::v1),
    }
}

pub(crate) fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}
