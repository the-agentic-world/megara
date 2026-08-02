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
