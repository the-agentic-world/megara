use std::{collections::BTreeMap, fmt};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::domain::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    InvalidRequest(String),
    SessionExists(SessionId),
    SessionNotFound(SessionId),
    RevisionConflict { expected: u64, actual: u64 },
    InvalidPhase(String),
    QuestionMismatch,
    ModelActionMismatch,
    ProposalSchemaInvalid(String),
    ProposalBaseMismatch,
    InvalidSourceReference,
    BlockersPresent,
    CandidateNotFound(CandidateId),
    CandidateStale,
    ApprovalBindingMismatch,
    Invariant(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            Self::SessionExists(id) => write!(f, "session already exists: {id}"),
            Self::SessionNotFound(id) => write!(f, "session not found: {id}"),
            Self::RevisionConflict { expected, actual } => {
                write!(
                    f,
                    "expected revision {expected}, current revision is {actual}"
                )
            }
            Self::InvalidPhase(message) => write!(f, "invalid phase: {message}"),
            Self::QuestionMismatch => write!(f, "question does not match pending question"),
            Self::ModelActionMismatch => write!(f, "model work item does not match current action"),
            Self::ProposalSchemaInvalid(message) => write!(f, "proposal schema invalid: {message}"),
            Self::ProposalBaseMismatch => write!(f, "proposal base does not match current state"),
            Self::InvalidSourceReference => write!(f, "invalid source reference"),
            Self::BlockersPresent => write!(f, "blocking blockers are present"),
            Self::CandidateNotFound(id) => write!(f, "candidate not found: {id}"),
            Self::CandidateStale => write!(f, "candidate is stale"),
            Self::ApprovalBindingMismatch => write!(f, "approval binding does not match candidate"),
            Self::Invariant(message) => write!(f, "state invariant failed: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartCommand {
    pub session_id: Option<SessionId>,
    pub project_id: ProjectId,
    pub request: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnswerCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub question_id: QuestionId,
    pub based_on_revision: u64,
    pub text: String,
    pub selected_choice_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditMode {
    Delta,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditReadiness {
    Continue,
    RequestFullAudit,
    Ready,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessGate {
    pub problem: bool,
    pub outcome: bool,
    pub requirement: bool,
    pub non_goal: bool,
    pub decision_boundary: bool,
    pub acceptance_criteria: bool,
    pub no_blocking_blockers: bool,
    pub no_pending_question: bool,
    pub evidence_current: bool,
    pub audit_input_current: bool,
    pub counterexample_review: bool,
}

impl ReadinessGate {
    pub fn is_ready(&self) -> bool {
        self.problem
            && self.outcome
            && self.requirement
            && self.non_goal
            && self.decision_boundary
            && self.acceptance_criteria
            && self.no_blocking_blockers
            && self.no_pending_question
            && self.evidence_current
            && self.audit_input_current
            && self.counterexample_review
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub work_item_id: WorkItemId,
    pub mode: AuditMode,
    pub base_revision: u64,
    pub base_domain_revision: u64,
    pub input_hash: String,
    pub readiness: AuditReadiness,
    pub next_question: Option<QuestionProposal>,
    pub entity_ops: Vec<EntityOp>,
    pub edge_ops: Vec<EdgeOp>,
    pub blocker_ops: Vec<BlockerOp>,
    pub counterexample_review_performed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityOp {
    Create {
        temp_ref: String,
        body: EntityBody,
        source_refs: Vec<SourceRef>,
    },
    Revise {
        entity_id: EntityId,
        base_revision: u64,
        body: EntityBody,
        source_refs: Vec<SourceRef>,
    },
    Reject {
        entity_id: EntityId,
        base_revision: u64,
        reason: String,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditEndpoint {
    TempRef(String),
    Entity(EntityRef),
    Source(SourceRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeOp {
    pub kind: EdgeKind,
    pub from: AuditEndpoint,
    pub to: AuditEndpoint,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockerOp {
    Create {
        temp_ref: String,
        kind: BlockerKind,
        severity: BlockerSeverity,
        statement: String,
        source_refs: Vec<SourceRef>,
    },
    Resolve {
        blocker_id: BlockerId,
        base_revision: u64,
        resolution: String,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecCandidateCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate: SpecCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCandidateCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate: PlanCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate_id: CandidateId,
    pub semantic_hash: String,
    pub base_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionRequestCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate_id: CandidateId,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryPlanningCore {
    sessions: BTreeMap<SessionId, PlanningState>,
    events: Vec<AggregateEvent>,
    next_session_number: u64,
}

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

    #[cfg(test)]
    pub fn seed_repo_snapshot_for_test(&mut self, session_id: &str) {
        if let Some(state) = self.sessions.get_mut(session_id) {
            state.repo_snapshot = Some(RepoEvidenceSnapshot {
                evidence_hash: "sha256:test-evidence".to_string(),
                head_oid: None,
                status_hash: "sha256:test-status".to_string(),
                cited_files_hash: "sha256:test-files".to_string(),
            });
        }
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

    pub fn apply_audit(&mut self, command: AuditCommand) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.audit.apply",
            |state, effects| {
                validate_work_item(state, &command)?;
                if command.mode == AuditMode::Delta && command.readiness == AuditReadiness::Ready {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "delta audit cannot declare ready".to_string(),
                    ));
                }
                if command.mode == AuditMode::Full && !command.counterexample_review_performed {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "full audit requires counterexample review".to_string(),
                    ));
                }
                let domain_changed = apply_audit_ops(state, &command, effects)?;
                if command.mode == AuditMode::Delta {
                    match command.readiness {
                        AuditReadiness::Continue => {
                            let question = command.next_question.clone().ok_or_else(|| {
                                CoreError::ProposalSchemaInvalid(
                                    "delta continue requires one question".to_string(),
                                )
                            })?;
                            state.required_model_action = None;
                            state.pending_question = Some(PendingQuestion {
                                question_id: format!("qst_{}", state.revision + 1),
                                based_on_revision: state.revision + 1,
                                proposal: question,
                            });
                            effects.push(EventEffect::QuestionSet {
                                question_id: format!("qst_{}", state.revision + 1),
                            });
                        }
                        AuditReadiness::RequestFullAudit => {
                            if command.next_question.is_some() {
                                return Err(CoreError::ProposalSchemaInvalid(
                                    "full audit request cannot include a question".to_string(),
                                ));
                            }
                            state.required_model_action = Some(work_item(
                                state,
                                ModelActionKind::FullAudit,
                                command.input_hash.clone(),
                            ));
                            effects.push(EventEffect::ModelActionRequested {
                                kind: ModelActionKind::FullAudit,
                            });
                        }
                        AuditReadiness::Ready => unreachable!(),
                    }
                    return Ok(json!({"readiness": format!("{:?}", command.readiness)}));
                }

                let readiness_gate = compute_readiness_gate(
                    state,
                    &command.input_hash,
                    command.counterexample_review_performed,
                );
                if command.readiness == AuditReadiness::Ready && !readiness_gate.is_ready() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "core-computed readiness gate is incomplete".to_string(),
                    ));
                }
                if domain_changed || state.has_blocking_blocker() {
                    state.phase = LifecyclePhase::Interview;
                    state.pending_question = None;
                    state.full_audit = None;
                    if !domain_changed {
                        state.domain_revision += 1;
                    }
                    invalidate_artifacts(state, effects);
                    state.required_model_action = Some(work_item(
                        state,
                        ModelActionKind::DeltaAudit,
                        command.input_hash.clone(),
                    ));
                    effects.push(EventEffect::PhaseChanged {
                        phase: LifecyclePhase::Interview,
                    });
                    effects.push(EventEffect::ModelActionRequested {
                        kind: ModelActionKind::DeltaAudit,
                    });
                } else if command.readiness == AuditReadiness::Ready {
                    state.phase = LifecyclePhase::Specification;
                    state.pending_question = None;
                    state.required_model_action = Some(work_item(
                        state,
                        ModelActionKind::GenerateSpec,
                        command.input_hash.clone(),
                    ));
                    state.full_audit = Some(FullAuditRef {
                        input_hash: command.input_hash.clone(),
                        base_domain_revision: state.domain_revision,
                        counterexample_review_performed: true,
                    });
                    effects.push(EventEffect::PhaseChanged {
                        phase: LifecyclePhase::Specification,
                    });
                    effects.push(EventEffect::ModelActionRequested {
                        kind: ModelActionKind::GenerateSpec,
                    });
                } else {
                    let question = command.next_question.clone().ok_or_else(|| {
                        CoreError::ProposalSchemaInvalid(
                            "full continue without changes requires one question".to_string(),
                        )
                    })?;
                    state.required_model_action = None;
                    state.pending_question = Some(PendingQuestion {
                        question_id: format!("qst_{}", state.revision + 1),
                        based_on_revision: state.revision + 1,
                        proposal: question,
                    });
                    effects.push(EventEffect::QuestionSet {
                        question_id: format!("qst_{}", state.revision + 1),
                    });
                }
                Ok(json!({"readiness": format!("{:?}", command.readiness)}))
            },
        )
    }

    pub fn generate_spec(
        &mut self,
        command: SpecCandidateCommand,
    ) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.spec.generate",
            |state, _effects| {
                if state.phase != LifecyclePhase::Specification {
                    return Err(CoreError::InvalidPhase(
                        "spec candidate requires Specification".to_string(),
                    ));
                }
                require_model_action(state, ModelActionKind::GenerateSpec)?;
                let candidate = &command.candidate;
                if candidate.base_domain_revision != state.domain_revision
                    || candidate.semantic_hash.trim().is_empty()
                    || candidate.audit_input_hash.trim().is_empty()
                {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_entity_refs(state, &candidate.entity_refs)?;
                state.spec.current_candidate = Some(candidate.clone());
                state.spec.approval = None;
                state.required_model_action = None;
                Ok(json!({"candidate_id": candidate.candidate_id}))
            },
        )
    }

    pub fn approve_spec(&mut self, command: ApprovalCommand) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.spec.approve",
            |state, effects| {
                if state.phase != LifecyclePhase::Specification {
                    return Err(CoreError::InvalidPhase(
                        "spec approval requires Specification".to_string(),
                    ));
                }
                if state.has_blocking_blocker() {
                    return Err(CoreError::BlockersPresent);
                }
                let candidate =
                    state.spec.current_candidate.as_ref().ok_or_else(|| {
                        CoreError::CandidateNotFound(command.candidate_id.clone())
                    })?;
                if candidate.candidate_id != command.candidate_id {
                    return Err(CoreError::CandidateNotFound(command.candidate_id.clone()));
                }
                if candidate.stale {
                    return Err(CoreError::CandidateStale);
                }
                if candidate.semantic_hash != command.semantic_hash
                    || candidate.base_domain_revision != command.base_revision
                {
                    return Err(CoreError::ApprovalBindingMismatch);
                }
                state.spec.approval = Some(ApprovalRef {
                    candidate_id: command.candidate_id.clone(),
                    semantic_hash: command.semantic_hash.clone(),
                    base_revision: command.base_revision,
                    approval_event_seq: state.revision + 1,
                });
                state.phase = LifecyclePhase::Planning;
                state.plan_revision += 1;
                state.required_model_action = Some(work_item(
                    state,
                    ModelActionKind::GeneratePlan,
                    candidate.semantic_hash.clone(),
                ));
                effects.push(EventEffect::PhaseChanged {
                    phase: LifecyclePhase::Planning,
                });
                effects.push(EventEffect::ModelActionRequested {
                    kind: ModelActionKind::GeneratePlan,
                });
                Ok(json!({"candidate_id": command.candidate_id}))
            },
        )
    }

    pub fn revise_spec(
        &mut self,
        command: RevisionRequestCommand,
    ) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.spec.revise",
            |state, effects| {
                if command.text.trim().is_empty() {
                    return Err(CoreError::InvalidRequest(
                        "revision text must not be blank".to_string(),
                    ));
                }
                let current_id = state
                    .spec
                    .current_candidate
                    .as_ref()
                    .map(|candidate| candidate.candidate_id.clone())
                    .ok_or_else(|| CoreError::CandidateNotFound(command.candidate_id.clone()))?;
                if current_id != command.candidate_id {
                    return Err(CoreError::CandidateNotFound(command.candidate_id));
                }
                state.phase = LifecyclePhase::Interview;
                state.domain_revision += 1;
                state.pending_question = None;
                state.full_audit = None;
                invalidate_artifacts(state, effects);
                state.required_model_action = Some(work_item(
                    state,
                    ModelActionKind::DeltaAudit,
                    hash_text(&command.text),
                ));
                effects.push(EventEffect::PhaseChanged {
                    phase: LifecyclePhase::Interview,
                });
                effects.push(EventEffect::ModelActionRequested {
                    kind: ModelActionKind::DeltaAudit,
                });
                Ok(json!({"feedback": command.text}))
            },
        )
    }

    pub fn generate_plan(
        &mut self,
        command: PlanCandidateCommand,
    ) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.plan.generate",
            |state, _effects| {
                if state.phase != LifecyclePhase::Planning {
                    return Err(CoreError::InvalidPhase(
                        "plan candidate requires Planning".to_string(),
                    ));
                }
                require_model_action(state, ModelActionKind::GeneratePlan)?;
                let approved_spec = state.spec.approval.as_ref().ok_or_else(|| {
                    CoreError::InvalidPhase("approved spec is required".to_string())
                })?;
                let candidate = &command.candidate;
                if candidate.spec_candidate_id != approved_spec.candidate_id
                    || candidate.spec_semantic_hash != approved_spec.semantic_hash
                    || candidate.base_plan_revision != state.plan_revision
                    || candidate.plan_input_hash.trim().is_empty()
                    || candidate.semantic_hash.trim().is_empty()
                {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                state.plan.current_candidate = Some(candidate.clone());
                state.plan.approval = None;
                state.required_model_action = None;
                Ok(json!({"candidate_id": candidate.candidate_id}))
            },
        )
    }

    pub fn approve_plan(&mut self, command: ApprovalCommand) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.plan.approve",
            |state, effects| {
                if state.phase != LifecyclePhase::Planning {
                    return Err(CoreError::InvalidPhase(
                        "plan approval requires Planning".to_string(),
                    ));
                }
                if state.has_blocking_blocker() {
                    return Err(CoreError::BlockersPresent);
                }
                let candidate =
                    state.plan.current_candidate.as_ref().ok_or_else(|| {
                        CoreError::CandidateNotFound(command.candidate_id.clone())
                    })?;
                if candidate.candidate_id != command.candidate_id {
                    return Err(CoreError::CandidateNotFound(command.candidate_id.clone()));
                }
                if candidate.stale {
                    return Err(CoreError::CandidateStale);
                }
                if candidate.semantic_hash != command.semantic_hash
                    || candidate.base_plan_revision != command.base_revision
                {
                    return Err(CoreError::ApprovalBindingMismatch);
                }
                state.plan.approval = Some(ApprovalRef {
                    candidate_id: command.candidate_id.clone(),
                    semantic_hash: command.semantic_hash.clone(),
                    base_revision: command.base_revision,
                    approval_event_seq: state.revision + 1,
                });
                state.phase = LifecyclePhase::Complete;
                state.required_model_action = None;
                effects.push(EventEffect::PhaseChanged {
                    phase: LifecyclePhase::Complete,
                });
                Ok(json!({"candidate_id": command.candidate_id}))
            },
        )
    }

    pub fn revise_plan(
        &mut self,
        command: RevisionRequestCommand,
    ) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.plan.revise",
            |state, effects| {
                if state.phase != LifecyclePhase::Planning {
                    return Err(CoreError::InvalidPhase(
                        "plan revision requires Planning".to_string(),
                    ));
                }
                if command.text.trim().is_empty() {
                    return Err(CoreError::InvalidRequest(
                        "revision text must not be blank".to_string(),
                    ));
                }
                let current_id = state
                    .plan
                    .current_candidate
                    .as_ref()
                    .map(|candidate| candidate.candidate_id.clone())
                    .ok_or_else(|| CoreError::CandidateNotFound(command.candidate_id.clone()))?;
                if current_id != command.candidate_id {
                    return Err(CoreError::CandidateNotFound(command.candidate_id));
                }
                state.plan_revision += 1;
                if let Some(candidate) = state.plan.current_candidate.as_mut() {
                    candidate.stale = true;
                }
                state.plan.approval = None;
                state.required_model_action = Some(work_item(
                    state,
                    ModelActionKind::GeneratePlan,
                    hash_text(&command.text),
                ));
                effects.push(EventEffect::ArtifactInvalidated {
                    artifact: "plan".to_string(),
                });
                effects.push(EventEffect::ModelActionRequested {
                    kind: ModelActionKind::GeneratePlan,
                });
                Ok(json!({"feedback": command.text}))
            },
        )
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

    fn mutate<F>(
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
pub struct MutationResult {
    pub state: PlanningState,
    pub event: AggregateEvent,
}

fn validate_work_item(state: &PlanningState, command: &AuditCommand) -> Result<(), CoreError> {
    let Some(work_item) = state.required_model_action.as_ref() else {
        return Err(CoreError::ModelActionMismatch);
    };
    let expected_kind = match command.mode {
        AuditMode::Delta => ModelActionKind::DeltaAudit,
        AuditMode::Full => ModelActionKind::FullAudit,
    };
    if work_item.work_item_id != command.work_item_id
        || work_item.kind != expected_kind
        || work_item.base_revision != command.base_revision
        || work_item.base_domain_revision != command.base_domain_revision
        || work_item.input_hash != command.input_hash
    {
        return Err(CoreError::ProposalBaseMismatch);
    }
    Ok(())
}

fn require_model_action(state: &PlanningState, expected: ModelActionKind) -> Result<(), CoreError> {
    if state
        .required_model_action
        .as_ref()
        .is_none_or(|work_item| work_item.kind != expected)
    {
        return Err(CoreError::ModelActionMismatch);
    }
    Ok(())
}

fn validate_entity_refs(
    state: &PlanningState,
    refs: &[EntityRevisionRef],
) -> Result<(), CoreError> {
    let mut seen = BTreeMap::new();
    for reference in refs {
        if seen
            .insert((&reference.id, reference.revision), true)
            .is_some()
            || state
                .entities
                .at_revision(&reference.id, reference.revision)
                .is_none()
            || state
                .entities
                .at_revision(&reference.id, reference.revision)
                .is_none_or(|entity| !entity.is_current())
        {
            return Err(CoreError::ProposalSchemaInvalid(
                "candidate references must be unique current entity revisions".to_string(),
            ));
        }
    }
    Ok(())
}

fn apply_audit_ops(
    state: &mut PlanningState,
    command: &AuditCommand,
    effects: &mut Vec<EventEffect>,
) -> Result<bool, CoreError> {
    let has_ops = !command.entity_ops.is_empty()
        || !command.edge_ops.is_empty()
        || !command.blocker_ops.is_empty();
    if !has_ops {
        return Ok(false);
    }

    let event_seq = state.revision + 1;
    let mut temp_refs = BTreeMap::<String, EntityRef>::new();
    let mut entity_ordinal = 0_u32;
    for operation in &command.entity_ops {
        match operation {
            EntityOp::Create {
                temp_ref,
                body,
                source_refs,
            } => {
                if temp_ref.trim().is_empty() || temp_refs.contains_key(temp_ref) {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "entity temp_ref must be unique and non-empty".to_string(),
                    ));
                }
                let kind = body_kind(body);
                validate_entity_body(body)?;
                validate_entity_sources(state, kind, source_refs)?;
                let entity_id = next_entity_id(&state.entities, kind);
                let entity_ref = EntityRef {
                    id: entity_id.clone(),
                    revision: 1,
                };
                state
                    .entities
                    .insert(EntityRecord {
                        entity_id,
                        revision: 1,
                        kind,
                        body: body.clone(),
                        disposition: EntityDisposition::Current,
                        validity: EntityValidity::Valid,
                        source_refs: source_refs.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: entity_ordinal,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                temp_refs.insert(temp_ref.clone(), entity_ref);
                entity_ordinal += 1;
            }
            EntityOp::Revise {
                entity_id,
                base_revision,
                body,
                source_refs,
            } => {
                let current = state.entities.current(entity_id).cloned().ok_or_else(|| {
                    CoreError::ProposalSchemaInvalid("revised entity must be current".to_string())
                })?;
                if current.revision != *base_revision || body_kind(body) != current.kind {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_entity_body(body)?;
                validate_entity_sources(state, current.kind, source_refs)?;
                if let Some(records) = state.entities.revisions.get_mut(entity_id) {
                    if let Some(previous) = records
                        .iter_mut()
                        .find(|record| record.revision == *base_revision)
                    {
                        previous.disposition = EntityDisposition::Superseded;
                    }
                }
                let next_revision = base_revision + 1;
                state
                    .entities
                    .insert(EntityRecord {
                        entity_id: entity_id.clone(),
                        revision: next_revision,
                        kind: current.kind,
                        body: body.clone(),
                        disposition: EntityDisposition::Current,
                        validity: EntityValidity::Valid,
                        source_refs: source_refs.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: entity_ordinal,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                state
                    .entities
                    .add_edge(supersedes_edge(
                        entity_id,
                        next_revision,
                        *base_revision,
                        source_refs.clone(),
                        event_seq,
                    ))
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                effects.push(EventEffect::EntityInvalidated {
                    entity_id: entity_id.clone(),
                });
                entity_ordinal += 1;
            }
            EntityOp::Reject {
                entity_id,
                base_revision,
                reason,
                source_refs,
            } => {
                if reason.trim().is_empty() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "rejection reason must not be blank".to_string(),
                    ));
                }
                let current = state.entities.current(entity_id).cloned().ok_or_else(|| {
                    CoreError::ProposalSchemaInvalid("rejected entity must be current".to_string())
                })?;
                if current.revision != *base_revision {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_entity_sources(state, current.kind, source_refs)?;
                if let Some(records) = state.entities.revisions.get_mut(entity_id) {
                    if let Some(previous) = records
                        .iter_mut()
                        .find(|record| record.revision == *base_revision)
                    {
                        previous.disposition = EntityDisposition::Superseded;
                    }
                }
                let next_revision = base_revision + 1;
                state
                    .entities
                    .insert(EntityRecord {
                        entity_id: entity_id.clone(),
                        revision: next_revision,
                        kind: current.kind,
                        body: current.body,
                        disposition: EntityDisposition::Rejected,
                        validity: EntityValidity::Valid,
                        source_refs: source_refs.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: entity_ordinal,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                effects.push(EventEffect::EntityInvalidated {
                    entity_id: entity_id.clone(),
                });
                entity_ordinal += 1;
            }
        }
    }

    for (edge_ordinal, operation) in command.edge_ops.iter().enumerate() {
        validate_source_refs_exist(state, &operation.source_refs)?;
        let from = resolve_entity_endpoint(&operation.from, &temp_refs)?;
        let to = match resolve_endpoint(&operation.to, &temp_refs)? {
            AuditEndpoint::Entity(reference) => EdgeTarget::Entity(reference),
            AuditEndpoint::Source(source) => EdgeTarget::Source(source),
            AuditEndpoint::TempRef(_) => unreachable!(),
        };
        state
            .entities
            .add_edge(Edge {
                edge_id: format!("edge_{event_seq}_{edge_ordinal}"),
                revision: 1,
                kind: operation.kind,
                from,
                to,
                source_refs: operation.source_refs.clone(),
                retired: false,
            })
            .map_err(CoreError::ProposalSchemaInvalid)?;
    }

    for (blocker_ordinal, operation) in command.blocker_ops.iter().enumerate() {
        match operation {
            BlockerOp::Create {
                temp_ref,
                kind,
                severity,
                statement,
                source_refs,
            } => {
                if temp_ref.trim().is_empty()
                    || statement.trim().is_empty()
                    || source_refs.is_empty()
                {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "blocker fields must not be blank".to_string(),
                    ));
                }
                validate_source_refs_exist(state, source_refs)?;
                let blocker_id = format!("blk_{event_seq}_{blocker_ordinal}");
                state.blockers.insert(
                    blocker_id.clone(),
                    Blocker {
                        blocker_id,
                        revision: 1,
                        kind: *kind,
                        severity: *severity,
                        statement: statement.clone(),
                        source_refs: source_refs.clone(),
                        resolved_at_revision: None,
                    },
                );
            }
            BlockerOp::Resolve {
                blocker_id,
                base_revision,
                resolution,
                source_refs,
            } => {
                if resolution.trim().is_empty() || source_refs.is_empty() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "blocker resolution fields must not be blank".to_string(),
                    ));
                }
                validate_source_refs_exist(state, source_refs)?;
                let blocker = state.blockers.get_mut(blocker_id).ok_or_else(|| {
                    CoreError::ProposalSchemaInvalid("blocker not found".to_string())
                })?;
                if blocker.revision != *base_revision || blocker.resolved_at_revision.is_some() {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                blocker.revision += 1;
                blocker.resolved_at_revision = Some(event_seq);
                blocker.statement = resolution.clone();
                blocker.source_refs = source_refs.clone();
            }
        }
    }
    state.domain_revision += 1;
    Ok(true)
}

fn compute_readiness_gate(
    state: &PlanningState,
    input_hash: &str,
    counterexample_review_performed: bool,
) -> ReadinessGate {
    let requirements = state.entities.current_requirements();
    let acceptance_criteria = !requirements.is_empty()
        && requirements.iter().all(|requirement| {
            state.entities.edges.iter().any(|edge| {
                !edge.retired
                    && edge.kind == EdgeKind::HasAcceptanceCriterion
                    && edge.from.id == requirement.entity_id
                    && edge.from.revision == requirement.revision
                    && matches!(
                        &edge.to,
                        EdgeTarget::Entity(reference)
                            if state
                                .entities
                                .at_revision(&reference.id, reference.revision)
                                .is_some_and(EntityRecord::is_current)
                    )
            })
        });
    ReadinessGate {
        problem: state.entities.current_count(EntityKind::Problem) > 0,
        outcome: state.entities.current_count(EntityKind::Outcome) > 0,
        requirement: !requirements.is_empty(),
        non_goal: state.entities.current_count(EntityKind::NonGoal) > 0,
        decision_boundary: state.entities.current_count(EntityKind::DecisionBoundary) > 0,
        acceptance_criteria,
        no_blocking_blockers: !state.has_blocking_blocker(),
        no_pending_question: state.pending_question.is_none(),
        evidence_current: state.repo_snapshot.is_some(),
        audit_input_current: state
            .required_model_action
            .as_ref()
            .is_some_and(|work_item| work_item.input_hash == input_hash),
        counterexample_review: counterexample_review_performed,
    }
}

fn body_kind(body: &EntityBody) -> EntityKind {
    match body {
        EntityBody::Problem { .. } => EntityKind::Problem,
        EntityBody::Outcome { .. } => EntityKind::Outcome,
        EntityBody::Fact { .. } => EntityKind::Fact,
        EntityBody::Decision { .. } => EntityKind::Decision,
        EntityBody::DecisionBoundary { .. } => EntityKind::DecisionBoundary,
        EntityBody::Requirement { .. } => EntityKind::Requirement,
        EntityBody::AcceptanceCriterion { .. } => EntityKind::AcceptanceCriterion,
        EntityBody::Constraint { .. } => EntityKind::Constraint,
        EntityBody::NonGoal { .. } => EntityKind::NonGoal,
        EntityBody::Assumption { .. } => EntityKind::Assumption,
        EntityBody::Risk { .. } => EntityKind::Risk,
        EntityBody::PlanStep { .. } => EntityKind::PlanStep,
        EntityBody::Verification { .. } => EntityKind::Verification,
    }
}

fn validate_entity_body(body: &EntityBody) -> Result<(), CoreError> {
    let non_empty = |value: &str| !value.trim().is_empty();
    let valid = match body {
        EntityBody::Problem { statement }
        | EntityBody::Constraint { statement }
        | EntityBody::NonGoal { statement }
        | EntityBody::AcceptanceCriterion { statement } => non_empty(statement),
        EntityBody::Outcome {
            statement,
            observable_result,
        } => non_empty(statement) && non_empty(observable_result),
        EntityBody::Fact {
            statement,
            evidence_refs,
        } => non_empty(statement) && !evidence_refs.is_empty(),
        EntityBody::Decision {
            statement,
            selected_option,
        } => non_empty(statement) && non_empty(selected_option),
        EntityBody::DecisionBoundary {
            autonomous_scope,
            requires_user_approval,
        } => {
            !autonomous_scope.is_empty()
                && autonomous_scope.iter().all(|item| non_empty(item))
                && requires_user_approval.iter().all(|item| non_empty(item))
        }
        EntityBody::Requirement { statement, .. } => non_empty(statement),
        EntityBody::Assumption { statement, .. } => non_empty(statement),
        EntityBody::Risk {
            statement,
            mitigation,
            ..
        } => non_empty(statement) && non_empty(mitigation),
        EntityBody::PlanStep {
            objective,
            change_surface,
            rollback_or_recovery,
        } => {
            non_empty(objective)
                && !change_surface.is_empty()
                && change_surface.iter().all(|item| non_empty(item))
                && non_empty(rollback_or_recovery)
        }
        EntityBody::Verification {
            procedure,
            expected_result,
            ..
        } => non_empty(procedure) && non_empty(expected_result),
    };
    valid.then_some(()).ok_or_else(|| {
        CoreError::ProposalSchemaInvalid("entity body has a required blank field".to_string())
    })
}

fn validate_entity_sources(
    state: &PlanningState,
    kind: EntityKind,
    source_refs: &[SourceRef],
) -> Result<(), CoreError> {
    validate_source_refs_exist(state, source_refs)?;
    let has_initial_or_answer = source_refs.iter().any(|source| {
        matches!(
            source,
            SourceRef::InitialRequest { .. } | SourceRef::Answer { .. }
        )
    });
    match kind {
        EntityKind::Fact => {
            if !source_refs
                .iter()
                .any(|source| matches!(source, SourceRef::Evidence { .. }))
            {
                return Err(CoreError::InvalidRequest(
                    "Fact requires an evidence source".to_string(),
                ));
            }
        }
        EntityKind::Decision | EntityKind::DecisionBoundary if !has_initial_or_answer => {
            return Err(CoreError::InvalidRequest(
                "decision entities require an initial request or answer source".to_string(),
            ));
        }
        EntityKind::Requirement | EntityKind::NonGoal => {
            let has_decision = source_refs.iter().any(|source| {
                matches!(source, SourceRef::Entity { id, revision }
                if state.entities.at_revision(id, *revision).is_some_and(|entity| {
                    entity.kind == EntityKind::Decision && entity.is_current()
                }))
            });
            if !has_initial_or_answer && !has_decision {
                return Err(CoreError::InvalidRequest(
                    "requirement and non-goal need a user or decision source".to_string(),
                ));
            }
        }
        EntityKind::AcceptanceCriterion if !has_initial_or_answer => {
            return Err(CoreError::InvalidRequest(
                "acceptance criterion requires a user source".to_string(),
            ));
        }
        EntityKind::PlanStep | EntityKind::Verification => {
            return Err(CoreError::InvalidRequest(
                "plan entities are not created by audit".to_string(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_source_refs_exist(
    state: &PlanningState,
    source_refs: &[SourceRef],
) -> Result<(), CoreError> {
    if source_refs.is_empty() {
        return Err(CoreError::InvalidSourceReference);
    }
    for source in source_refs {
        match source {
            SourceRef::InitialRequest { id } if id != "request" => {
                return Err(CoreError::InvalidSourceReference)
            }
            SourceRef::Answer { id } => {
                if !state
                    .transcript
                    .answers
                    .iter()
                    .any(|answer| answer.answer_id == *id)
                {
                    return Err(CoreError::InvalidSourceReference);
                }
            }
            SourceRef::Evidence { .. } if state.repo_snapshot.is_none() => {
                return Err(CoreError::InvalidSourceReference)
            }
            SourceRef::Entity { id, revision } => {
                if state.entities.at_revision(id, *revision).is_none() {
                    return Err(CoreError::InvalidSourceReference);
                }
            }
            SourceRef::ApprovedSpec { .. } => return Err(CoreError::InvalidSourceReference),
            _ => {}
        }
    }
    Ok(())
}

fn resolve_endpoint(
    endpoint: &AuditEndpoint,
    temp_refs: &BTreeMap<String, EntityRef>,
) -> Result<AuditEndpoint, CoreError> {
    match endpoint {
        AuditEndpoint::TempRef(temp_ref) => temp_refs
            .get(temp_ref)
            .cloned()
            .map(AuditEndpoint::Entity)
            .ok_or(CoreError::InvalidSourceReference),
        other => Ok(other.clone()),
    }
}

fn resolve_entity_endpoint(
    endpoint: &AuditEndpoint,
    temp_refs: &BTreeMap<String, EntityRef>,
) -> Result<EntityRef, CoreError> {
    match resolve_endpoint(endpoint, temp_refs)? {
        AuditEndpoint::Entity(reference) => Ok(reference),
        _ => Err(CoreError::ProposalSchemaInvalid(
            "edge from endpoint must be an entity".to_string(),
        )),
    }
}

fn next_entity_id(graph: &EntityGraph, kind: EntityKind) -> EntityId {
    let prefix = match kind {
        EntityKind::Problem => "PROB",
        EntityKind::Outcome => "OUT",
        EntityKind::Fact => "FACT",
        EntityKind::Decision => "DEC",
        EntityKind::DecisionBoundary => "DBND",
        EntityKind::Requirement => "REQ",
        EntityKind::AcceptanceCriterion => "AC",
        EntityKind::Constraint => "CON",
        EntityKind::NonGoal => "NG",
        EntityKind::Assumption => "ASM",
        EntityKind::Risk => "RISK",
        EntityKind::PlanStep => "STEP",
        EntityKind::Verification => "VER",
    };
    let next = graph
        .revisions
        .keys()
        .filter(|id| id.starts_with(&format!("{prefix}-")))
        .count()
        + 1;
    format!("{prefix}-{next:03}")
}

fn supersedes_edge(
    entity_id: &str,
    new_revision: u64,
    old_revision: u64,
    source_refs: Vec<SourceRef>,
    event_seq: u64,
) -> Edge {
    Edge {
        edge_id: format!("edge_{event_seq}_supersedes_{entity_id}_{new_revision}"),
        revision: 1,
        kind: EdgeKind::Supersedes,
        from: EntityRef {
            id: entity_id.to_string(),
            revision: new_revision,
        },
        to: EdgeTarget::Entity(EntityRef {
            id: entity_id.to_string(),
            revision: old_revision,
        }),
        source_refs,
        retired: false,
    }
}

fn invalidate_artifacts(state: &mut PlanningState, effects: &mut Vec<EventEffect>) {
    state.full_audit = None;
    if let Some(candidate) = state.spec.current_candidate.as_mut() {
        candidate.stale = true;
        effects.push(EventEffect::ArtifactInvalidated {
            artifact: "spec".to_string(),
        });
    }
    if let Some(candidate) = state.plan.current_candidate.as_mut() {
        candidate.stale = true;
        effects.push(EventEffect::ArtifactInvalidated {
            artifact: "plan".to_string(),
        });
    }
    if state.spec.approval.is_some() {
        state.spec.approval = None;
        effects.push(EventEffect::ApprovalsRevoked {
            artifact: "spec".to_string(),
        });
    }
    if state.plan.approval.is_some() {
        state.plan.approval = None;
        effects.push(EventEffect::ApprovalsRevoked {
            artifact: "plan".to_string(),
        });
    }
}

fn work_item(state: &PlanningState, kind: ModelActionKind, input_hash: String) -> ModelWorkItem {
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

fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}
