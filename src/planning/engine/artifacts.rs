use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde_json::json;

use super::*;

impl InMemoryPlanningCore {
    pub fn generate_spec(
        &mut self,
        command: SpecCandidateCommand,
    ) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.spec.generate",
            command_value(&command),
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
            command_value(&command),
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
            command_value(&command),
            |state, effects| {
                if !matches!(
                    state.phase,
                    LifecyclePhase::Specification | LifecyclePhase::Complete
                ) {
                    return Err(CoreError::InvalidPhase(
                        "spec revision requires Specification or Complete".to_string(),
                    ));
                }
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
            command_value(&command),
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
            command_value(&command),
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
            command_value(&command),
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
pub(crate) fn invalidate_artifacts(state: &mut PlanningState, effects: &mut Vec<EventEffect>) {
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

pub(crate) fn invalidate_evidence_entities(
    state: &mut PlanningState,
    effects: &mut Vec<EventEffect>,
    cause: SourceRef,
) {
    let stale_since = state.domain_revision;
    let mut impacted = BTreeSet::<(EntityId, u64)>::new();
    for records in state.entities.revisions.values() {
        if let Some(entity) = records.iter().find(|entity| {
            entity.is_current()
                && entity.kind == EntityKind::Fact
                && entity
                    .source_refs
                    .iter()
                    .any(|source| matches!(source, SourceRef::Evidence { .. }))
        }) {
            impacted.insert((entity.entity_id.clone(), entity.revision));
        }
    }

    let mut expanded = true;
    while expanded {
        expanded = false;
        for edge in &state.entities.edges {
            if edge.retired || edge.kind != EdgeKind::DependsOn {
                continue;
            }
            let EdgeTarget::Entity(target) = &edge.to else {
                continue;
            };
            if !impacted.contains(&(target.id.clone(), target.revision)) {
                continue;
            }
            let Some(dependent) = state
                .entities
                .at_revision(&edge.from.id, edge.from.revision)
            else {
                continue;
            };
            if dependent.is_current()
                && impacted.insert((dependent.entity_id.clone(), dependent.revision))
            {
                expanded = true;
            }
        }
    }

    for (entity_id, revision) in impacted {
        let Some(records) = state.entities.revisions.get_mut(&entity_id) else {
            continue;
        };
        let Some(entity) = records
            .iter_mut()
            .find(|entity| entity.revision == revision && entity.is_current())
        else {
            continue;
        };
        entity.validity = EntityValidity::Stale {
            since_domain_revision: stale_since,
            causes: vec![cause.clone()],
        };
        effects.push(EventEffect::EntityInvalidated { entity_id });
    }
}
