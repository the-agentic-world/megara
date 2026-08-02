use std::collections::BTreeMap;

use super::super::domain::*;
use std::collections::BTreeSet;

use super::*;

pub(crate) fn body_kind(body: &EntityBody) -> EntityKind {
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

pub(crate) fn validate_entity_body(body: &EntityBody) -> Result<(), CoreError> {
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

pub(crate) fn validate_fact_evidence_refs(
    state: &PlanningState,
    body: &EntityBody,
    source_refs: &[SourceRef],
) -> Result<(), CoreError> {
    let EntityBody::Fact { evidence_refs, .. } = body else {
        return Ok(());
    };
    let mut body_ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    for evidence_id in evidence_refs {
        if !body_ids.insert(evidence_id)
            || !evidence_id.starts_with("EVID-")
            || !state
                .repo_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.has_evidence(evidence_id))
        {
            return Err(CoreError::InvalidSourceReference);
        }
    }
    for source in source_refs {
        if let SourceRef::Evidence { id } = source {
            if !source_ids.insert(id) {
                return Err(CoreError::InvalidSourceReference);
            }
        }
    }
    if body_ids != source_ids {
        return Err(CoreError::InvalidSourceReference);
    }
    Ok(())
}

pub(crate) fn validate_entity_sources(
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

pub(crate) fn validate_question_proposal(
    state: &PlanningState,
    proposal: &QuestionProposal,
) -> Result<(), CoreError> {
    proposal
        .validate_shape()
        .map_err(CoreError::ProposalSchemaInvalid)?;
    validate_source_refs_exist(state, &proposal.source_refs)?;
    if let AnswerMode::Choice {
        recommendation: Some(recommendation),
        ..
    } = &proposal.answer
    {
        validate_source_refs_exist(state, &recommendation.source_refs)?;
    }
    Ok(())
}

pub(crate) fn validate_counterexample_review(
    state: &PlanningState,
    review: &CounterexampleReview,
    blocker_ops: &[BlockerOp],
) -> Result<(), CoreError> {
    if !review.performed {
        return Err(CoreError::ProposalSchemaInvalid(
            "counterexample review must be performed".to_string(),
        ));
    }
    let mut challenged = BTreeSet::new();
    for entity_id in &review.challenged_entity_ids {
        if !challenged.insert(entity_id) || state.entities.current(entity_id).is_none() {
            return Err(CoreError::ProposalSchemaInvalid(
                "challenged_entity_ids must be unique current entities".to_string(),
            ));
        }
    }
    for finding in &review.findings {
        if finding.statement.trim().is_empty() {
            return Err(CoreError::ProposalSchemaInvalid(
                "counterexample finding statement must not be blank".to_string(),
            ));
        }
        validate_source_refs_exist(state, &finding.source_refs)?;
        if finding.result == CounterexampleResult::Blocking
            && !blocker_ops.iter().any(|operation| {
                matches!(
                    operation,
                    BlockerOp::Create {
                        severity: BlockerSeverity::Blocking,
                        statement,
                        source_refs,
                        ..
                    } if statement == &finding.statement && source_refs == &finding.source_refs
                )
            })
        {
            return Err(CoreError::ProposalSchemaInvalid(
                "blocking counterexample finding requires a matching blocking blocker".to_string(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_source_refs_exist(
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
            SourceRef::Evidence { id }
                if !state
                    .repo_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.has_evidence(id)) =>
            {
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

pub(crate) fn resolve_endpoint(
    endpoint: &AuditEndpoint,
    temp_refs: &BTreeMap<String, EntityRef>,
) -> Result<AuditEndpoint, CoreError> {
    match endpoint {
        AuditEndpoint::TempRef { temp_ref } => temp_refs
            .get(temp_ref)
            .cloned()
            .map(|reference| AuditEndpoint::Entity {
                entity_id: reference.id,
                revision: reference.revision,
            })
            .ok_or(CoreError::InvalidSourceReference),
        other => Ok(other.clone()),
    }
}

pub(crate) fn resolve_entity_endpoint(
    endpoint: &AuditEndpoint,
    temp_refs: &BTreeMap<String, EntityRef>,
) -> Result<EntityRef, CoreError> {
    match resolve_endpoint(endpoint, temp_refs)? {
        AuditEndpoint::Entity {
            entity_id,
            revision,
        } => Ok(EntityRef {
            id: entity_id,
            revision,
        }),
        _ => Err(CoreError::ProposalSchemaInvalid(
            "edge from endpoint must be an entity".to_string(),
        )),
    }
}

pub(crate) fn next_entity_id(graph: &EntityGraph, kind: EntityKind) -> EntityId {
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

pub(crate) fn supersedes_edge(
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
