use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::super::error::ServiceError;
use super::validation::{
    generated_candidate_id, require_text, validate_kind, validate_refs_of_kind,
    validate_requirement_edges, validate_unique_refs,
};
use crate::planning::domain::{
    entity_record_value, EntityKind, EntityRecord, EntityRevisionRef, PlanningState, SourceRef,
    SpecCandidate,
};
use crate::planning::engine::{spec_semantic_hash, validate_source_refs_exist};

pub(crate) const SPEC_PROPOSAL_SCHEMA: &str = "megara.spec-proposal/v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpecProposal {
    pub(crate) schema: String,
    pub(crate) work_item_id: String,
    pub(crate) base_revision: u64,
    pub(crate) base_domain_revision: u64,
    pub(crate) audit_input_hash: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) problem_ref: EntityRevisionRef,
    pub(crate) outcome_refs: Vec<EntityRevisionRef>,
    pub(crate) decision_refs: Vec<EntityRevisionRef>,
    pub(crate) decision_boundary_refs: Vec<EntityRevisionRef>,
    pub(crate) requirement_refs: Vec<EntityRevisionRef>,
    pub(crate) acceptance_criterion_refs: Vec<EntityRevisionRef>,
    pub(crate) constraint_refs: Vec<EntityRevisionRef>,
    pub(crate) non_goal_refs: Vec<EntityRevisionRef>,
    pub(crate) assumption_refs: Vec<EntityRevisionRef>,
    pub(crate) risk_refs: Vec<EntityRevisionRef>,
    pub(crate) advisories: Vec<Advisory>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Advisory {
    pub(crate) statement: String,
    pub(crate) source_refs: Vec<SourceRef>,
}

pub(crate) fn decode_spec(
    raw: &Value,
    state: &PlanningState,
    expected_work_item_id: &str,
    expected_base_revision: u64,
    expected_base_domain_revision: u64,
    expected_audit_input_hash: &str,
) -> Result<SpecCandidate, ServiceError> {
    let proposal: SpecProposal = serde_json::from_value(raw.clone())
        .map_err(|error| ServiceError::proposal_schema(format!("proposal schema: {error}")))?;
    if proposal.schema != SPEC_PROPOSAL_SCHEMA {
        return Err(ServiceError::proposal_schema(
            "unsupported spec proposal schema",
        ));
    }
    if proposal.work_item_id != expected_work_item_id
        || proposal.base_revision != expected_base_revision
        || proposal.base_domain_revision != expected_base_domain_revision
        || proposal.audit_input_hash != expected_audit_input_hash
    {
        return Err(ServiceError::with_code(
            "PROPOSAL_BASE_MISMATCH",
            "spec proposal does not match the current GenerateSpec work item",
        ));
    }
    require_text(&proposal.title, "title")?;
    require_text(&proposal.summary, "summary")?;
    let refs = spec_refs(&proposal);
    validate_unique_refs(&refs)?;
    validate_kind(state, &proposal.problem_ref, EntityKind::Problem)?;
    validate_refs_of_kind(state, &proposal.outcome_refs, EntityKind::Outcome)?;
    validate_refs_of_kind(state, &proposal.decision_refs, EntityKind::Decision)?;
    validate_refs_of_kind(
        state,
        &proposal.decision_boundary_refs,
        EntityKind::DecisionBoundary,
    )?;
    validate_refs_of_kind(state, &proposal.requirement_refs, EntityKind::Requirement)?;
    validate_refs_of_kind(
        state,
        &proposal.acceptance_criterion_refs,
        EntityKind::AcceptanceCriterion,
    )?;
    validate_refs_of_kind(state, &proposal.constraint_refs, EntityKind::Constraint)?;
    validate_refs_of_kind(state, &proposal.non_goal_refs, EntityKind::NonGoal)?;
    validate_refs_of_kind(state, &proposal.assumption_refs, EntityKind::Assumption)?;
    validate_refs_of_kind(state, &proposal.risk_refs, EntityKind::Risk)?;
    require_exact_current_refs(
        state,
        std::slice::from_ref(&proposal.problem_ref),
        EntityKind::Problem,
    )?;
    for (refs, kind) in [
        (&proposal.outcome_refs, EntityKind::Outcome),
        (&proposal.decision_refs, EntityKind::Decision),
        (
            &proposal.decision_boundary_refs,
            EntityKind::DecisionBoundary,
        ),
        (&proposal.requirement_refs, EntityKind::Requirement),
        (
            &proposal.acceptance_criterion_refs,
            EntityKind::AcceptanceCriterion,
        ),
        (&proposal.constraint_refs, EntityKind::Constraint),
        (&proposal.non_goal_refs, EntityKind::NonGoal),
        (&proposal.assumption_refs, EntityKind::Assumption),
        (&proposal.risk_refs, EntityKind::Risk),
    ] {
        require_exact_current_refs(state, refs, kind)?;
    }
    for advisory in &proposal.advisories {
        require_text(&advisory.statement, "advisory.statement")?;
        if advisory.source_refs.is_empty() {
            return Err(ServiceError::proposal_schema(
                "advisory source_refs must not be empty",
            ));
        }
        validate_source_refs_exist(state, &advisory.source_refs).map_err(|_| {
            ServiceError::with_code(
                "INVALID_SOURCE_REFERENCE",
                "advisory source_refs must resolve in the current state",
            )
        })?;
    }
    validate_requirement_edges(
        state,
        &proposal.requirement_refs,
        &proposal.acceptance_criterion_refs,
    )?;
    let content = spec_content(&proposal, state, &refs)?;
    let created_event_seq = expected_base_revision
        .checked_add(1)
        .ok_or_else(|| ServiceError::invalid("spec candidate event sequence overflow"))?;
    Ok(SpecCandidate {
        candidate_id: generated_candidate_id("spec"),
        created_event_seq,
        created_ordinal: 0,
        base_domain_revision: proposal.base_domain_revision,
        audit_input_hash: proposal.audit_input_hash,
        semantic_hash: spec_semantic_hash(state, &content),
        entity_refs: refs,
        content,
        stale: false,
    })
}

fn spec_refs(proposal: &SpecProposal) -> Vec<EntityRevisionRef> {
    let mut refs = vec![proposal.problem_ref.clone()];
    for group in [
        &proposal.outcome_refs,
        &proposal.decision_refs,
        &proposal.decision_boundary_refs,
        &proposal.requirement_refs,
        &proposal.acceptance_criterion_refs,
        &proposal.constraint_refs,
        &proposal.non_goal_refs,
        &proposal.assumption_refs,
        &proposal.risk_refs,
    ] {
        refs.extend(group.iter().cloned());
    }
    refs
}

fn require_exact_current_refs(
    state: &PlanningState,
    refs: &[EntityRevisionRef],
    kind: EntityKind,
) -> Result<(), ServiceError> {
    let actual = refs
        .iter()
        .map(|reference| (reference.id.clone(), reference.revision))
        .collect::<BTreeSet<_>>();
    let expected = state
        .entities
        .revisions
        .values()
        .flat_map(|records| records.iter())
        .filter(|record| record.is_current() && record.kind == kind)
        .map(|record| (record.entity_id.clone(), record.revision))
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(ServiceError::proposal_schema(format!(
            "spec refs for {kind:?} must equal current entities"
        )));
    }
    Ok(())
}

fn spec_content(
    proposal: &SpecProposal,
    state: &PlanningState,
    refs: &[EntityRevisionRef],
) -> Result<Value, ServiceError> {
    let mut entities = refs
        .iter()
        .map(|reference| {
            state
                .entities
                .at_revision(&reference.id, reference.revision)
                .map(spec_entity_value)
                .ok_or_else(|| ServiceError::proposal_schema("entity reference disappeared"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entities.sort_by(|left, right| {
        left["entity_id"]
            .as_str()
            .cmp(&right["entity_id"].as_str())
            .then_with(|| left["revision"].as_u64().cmp(&right["revision"].as_u64()))
    });
    Ok(json!({
        "schema":"megara.canonical-spec/v1",
        "title":proposal.title,
        "summary":proposal.summary,
        "problem_ref":proposal.problem_ref,
        "outcome_refs":proposal.outcome_refs,
        "decision_refs":proposal.decision_refs,
        "decision_boundary_refs":proposal.decision_boundary_refs,
        "requirement_refs":proposal.requirement_refs,
        "acceptance_criterion_refs":proposal.acceptance_criterion_refs,
        "constraint_refs":proposal.constraint_refs,
        "non_goal_refs":proposal.non_goal_refs,
        "assumption_refs":proposal.assumption_refs,
        "risk_refs":proposal.risk_refs,
        "advisories":proposal.advisories,
        "entities":entities
    }))
}

fn spec_entity_value(record: &EntityRecord) -> Value {
    let full = entity_record_value(record);
    json!({
        "entity_id": record.entity_id,
        "revision": record.revision,
        "kind": record.kind,
        "body": full["body"],
        "source_refs": record.source_refs,
    })
}
