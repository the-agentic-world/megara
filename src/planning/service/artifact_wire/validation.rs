use std::collections::BTreeSet;

use super::super::super::domain::{
    EdgeKind, EdgeTarget, EntityKind, EntityRef, EntityRevisionRef, PlanningState,
};
use super::super::error::ServiceError;

pub(super) fn generated_candidate_id(kind: &str) -> String {
    format!("cand_{kind}_{}", uuid::Uuid::now_v7())
}

pub(super) fn require_text(value: &str, field: &str) -> Result<(), ServiceError> {
    if value.trim().is_empty() {
        return Err(ServiceError::proposal_schema(format!(
            "{field} must not be blank"
        )));
    }
    Ok(())
}

pub(super) fn validate_unique_refs(refs: &[EntityRevisionRef]) -> Result<(), ServiceError> {
    let mut seen = BTreeSet::new();
    if refs
        .iter()
        .any(|reference| !seen.insert((reference.id.as_str(), reference.revision)))
    {
        return Err(ServiceError::proposal_schema(
            "entity references must be unique",
        ));
    }
    Ok(())
}

pub(super) fn validate_refs_of_kind(
    state: &PlanningState,
    refs: &[EntityRevisionRef],
    kind: EntityKind,
) -> Result<(), ServiceError> {
    for reference in refs {
        validate_kind(state, reference, kind)?;
    }
    Ok(())
}

pub(super) fn validate_kind(
    state: &PlanningState,
    reference: &EntityRevisionRef,
    kind: EntityKind,
) -> Result<(), ServiceError> {
    let record = state
        .entities
        .at_revision(&reference.id, reference.revision)
        .ok_or_else(|| ServiceError::proposal_schema("entity reference does not exist"))?;
    if record.kind != kind || !record.is_current() {
        return Err(ServiceError::proposal_schema(
            "entity reference must target a current entity of the expected kind",
        ));
    }
    Ok(())
}

pub(super) fn validate_requirement_edges(
    state: &PlanningState,
    requirements: &[EntityRevisionRef],
    criteria: &[EntityRevisionRef],
) -> Result<(), ServiceError> {
    if requirements.is_empty() || criteria.is_empty() {
        return Err(ServiceError::proposal_schema(
            "spec requires requirements and acceptance criteria",
        ));
    }
    for requirement in requirements {
        let connected = state.entities.edges.iter().any(|edge| {
            !edge.retired
                && edge.kind == EdgeKind::HasAcceptanceCriterion
                && edge.from
                    == EntityRef {
                        id: requirement.id.clone(),
                        revision: requirement.revision,
                    }
                && criteria.iter().any(|criterion| {
                    edge.to
                        == EdgeTarget::Entity(EntityRef {
                            id: criterion.id.clone(),
                            revision: criterion.revision,
                        })
                })
        });
        if !connected {
            return Err(ServiceError::proposal_schema(
                "every requirement must connect to an acceptance criterion",
            ));
        }
    }
    for criterion in criteria {
        let connected = state.entities.edges.iter().any(|edge| {
            !edge.retired
                && edge.kind == EdgeKind::HasAcceptanceCriterion
                && edge.to
                    == EdgeTarget::Entity(EntityRef {
                        id: criterion.id.clone(),
                        revision: criterion.revision,
                    })
                && requirements.iter().any(|requirement| {
                    edge.from
                        == EntityRef {
                            id: requirement.id.clone(),
                            revision: requirement.revision,
                        }
                })
        });
        if !connected {
            return Err(ServiceError::proposal_schema(
                "every acceptance criterion must be covered by a requirement",
            ));
        }
    }
    Ok(())
}
