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
                let audit = state
                    .full_audit
                    .as_ref()
                    .ok_or(CoreError::ProposalBaseMismatch)?;
                let expected_created_event_seq =
                    state.revision.checked_add(1).ok_or_else(|| {
                        CoreError::Invariant("candidate event sequence overflow".to_string())
                    })?;
                if candidate.base_domain_revision != state.domain_revision
                    || candidate.created_event_seq != expected_created_event_seq
                    || candidate.created_ordinal != 0
                    || candidate.semantic_hash.trim().is_empty()
                    || candidate.audit_input_hash.trim().is_empty()
                    || audit.input_hash != candidate.audit_input_hash
                    || audit.base_domain_revision != candidate.base_domain_revision
                    || super::spec_semantic_hash(state, &candidate.content)
                        != candidate.semantic_hash
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
                    || super::spec_semantic_hash(state, &candidate.content)
                        != candidate.semantic_hash
                {
                    return Err(CoreError::ApprovalBindingMismatch);
                }
                let audit = state
                    .full_audit
                    .as_ref()
                    .ok_or(CoreError::ApprovalBindingMismatch)?;
                if audit.input_hash != candidate.audit_input_hash
                    || audit.base_domain_revision != candidate.base_domain_revision
                {
                    return Err(CoreError::ApprovalBindingMismatch);
                }
                validate_entity_refs(state, &candidate.entity_refs)?;
                state.spec.approval = Some(ApprovalRef {
                    candidate_id: command.candidate_id.clone(),
                    semantic_hash: command.semantic_hash.clone(),
                    base_revision: command.base_revision,
                    approval_event_seq: state.revision + 1,
                });
                state.phase = LifecyclePhase::Planning;
                state.plan_revision += 1;
                let next_work_item = work_item(state, ModelActionKind::GeneratePlan);
                state.required_model_action = Some(next_work_item.clone());
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
                let next_work_item = work_item(state, ModelActionKind::DeltaAudit);
                state.required_model_action = Some(next_work_item.clone());
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
                let expected_created_event_seq =
                    state.revision.checked_add(1).ok_or_else(|| {
                        CoreError::Invariant("candidate event sequence overflow".to_string())
                    })?;
                if candidate.spec_candidate_id != approved_spec.candidate_id
                    || candidate.spec_semantic_hash != approved_spec.semantic_hash
                    || candidate.created_event_seq != expected_created_event_seq
                    || candidate.created_ordinal != 0
                    || candidate.base_plan_revision != state.plan_revision
                    || candidate.plan_input_hash.trim().is_empty()
                    || candidate.semantic_hash.trim().is_empty()
                    || candidate.plan_input_hash != super::plan_input_hash(state)
                    || super::super::canonical::canonical_hash(&candidate.content)
                        != candidate.semantic_hash
                {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_plan_content(state, &candidate.content)?;
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
                    || super::super::canonical::canonical_hash(&candidate.content)
                        != candidate.semantic_hash
                    || state.spec.approval.as_ref().is_none_or(|approval| {
                        candidate.spec_candidate_id != approval.candidate_id
                            || candidate.spec_semantic_hash != approval.semantic_hash
                    })
                    || candidate.plan_input_hash != super::plan_input_hash(state)
                {
                    return Err(CoreError::ApprovalBindingMismatch);
                }
                validate_plan_content(state, &candidate.content)?;
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
                let next_work_item = work_item(state, ModelActionKind::GeneratePlan);
                state.required_model_action = Some(next_work_item.clone());
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
    changed_evidence_ids: &BTreeSet<String>,
    broad: bool,
) {
    let stale_since = state.domain_revision;
    let mut impacted = BTreeMap::<(EntityId, u64), Vec<SourceRef>>::new();
    for records in state.entities.revisions.values() {
        if let Some(entity) = records.iter().find(|entity| {
            entity.is_current()
                && entity.kind == EntityKind::Fact
                && evidence_causes(&entity.source_refs, changed_evidence_ids, broad).is_some()
        }) {
            let causes = evidence_causes(&entity.source_refs, changed_evidence_ids, broad)
                .expect("fact selected by evidence causes");
            impacted.insert((entity.entity_id.clone(), entity.revision), causes);
        }
    }
    for edge in &state.entities.edges {
        if edge.retired || edge.kind != EdgeKind::DerivedFrom {
            continue;
        }
        let EdgeTarget::Source(SourceRef::Evidence { id }) = &edge.to else {
            continue;
        };
        if !broad && !changed_evidence_ids.contains(id) {
            continue;
        }
        let Some(entity) = state
            .entities
            .at_revision(&edge.from.id, edge.from.revision)
            .filter(|entity| entity.is_current())
        else {
            continue;
        };
        let entry = impacted
            .entry((entity.entity_id.clone(), entity.revision))
            .or_default();
        append_unique_evidence_causes(entry, &[SourceRef::Evidence { id: id.clone() }]);
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
            let Some(target_causes) = impacted.get(&(target.id.clone(), target.revision)).cloned()
            else {
                continue;
            };
            let Some(dependent) = state
                .entities
                .at_revision(&edge.from.id, edge.from.revision)
            else {
                continue;
            };
            if dependent.is_current() {
                let entry = impacted
                    .entry((dependent.entity_id.clone(), dependent.revision))
                    .or_default();
                let before = entry.len();
                append_unique_evidence_causes(entry, &target_causes);
                if entry.len() != before {
                    expanded = true;
                }
            }
        }
    }

    for ((entity_id, revision), causes) in impacted {
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
            causes,
        };
        effects.push(EventEffect::EntityInvalidated { entity_id });
    }
}

fn evidence_causes(
    source_refs: &[SourceRef],
    changed_evidence_ids: &BTreeSet<String>,
    broad: bool,
) -> Option<Vec<SourceRef>> {
    let mut causes = Vec::new();
    for source in source_refs {
        if let SourceRef::Evidence { id } = source {
            if broad || changed_evidence_ids.contains(id) {
                append_unique_evidence_causes(&mut causes, std::slice::from_ref(source));
            }
        }
    }
    (!causes.is_empty()).then_some(causes)
}

fn append_unique_evidence_causes(target: &mut Vec<SourceRef>, causes: &[SourceRef]) {
    for cause in causes {
        if !target.iter().any(|existing| existing == cause) {
            target.push(cause.clone());
        }
    }
}
