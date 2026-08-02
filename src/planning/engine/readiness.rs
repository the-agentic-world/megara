use std::collections::BTreeMap;

use super::super::domain::*;
use super::*;
pub(crate) fn validate_work_item(
    state: &PlanningState,
    command: &AuditCommand,
) -> Result<(), CoreError> {
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

pub(crate) fn require_model_action(
    state: &PlanningState,
    expected: ModelActionKind,
) -> Result<(), CoreError> {
    if state
        .required_model_action
        .as_ref()
        .is_none_or(|work_item| work_item.kind != expected)
    {
        return Err(CoreError::ModelActionMismatch);
    }
    Ok(())
}

pub(crate) fn apply_audit_ops(
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
                let previous = state
                    .entities
                    .revisions
                    .get(entity_id)
                    .and_then(|records| records.iter().max_by_key(|record| record.revision))
                    .cloned()
                    .ok_or_else(|| {
                        CoreError::ProposalSchemaInvalid(
                            "revised entity must have a recorded revision".to_string(),
                        )
                    })?;
                if previous.disposition != EntityDisposition::Current {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "only the latest current entity revision can be revised".to_string(),
                    ));
                }
                if previous.revision != *base_revision || body_kind(body) != previous.kind {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_entity_body(body)?;
                validate_entity_sources(state, previous.kind, source_refs)?;
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
                        kind: previous.kind,
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

pub(crate) fn compute_readiness_gate(
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
