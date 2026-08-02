use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::super::error::ServiceError;
use super::validation::generated_candidate_id;
use crate::planning::canonical::canonical_hash;
use crate::planning::domain::{
    EntityRevisionRef, PlanCandidate, PlanningState, VerificationMethod,
};
use crate::planning::engine::{plan_input_hash, validate_plan_content, CoreError};

pub(crate) const PLAN_PROPOSAL_SCHEMA: &str = "megara.plan-proposal/v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanProposal {
    pub(crate) schema: String,
    pub(crate) work_item_id: String,
    pub(crate) base_revision: u64,
    pub(crate) base_plan_revision: u64,
    pub(crate) plan_input_hash: String,
    pub(crate) spec: PlanSpecBinding,
    pub(crate) baseline: PlanBaseline,
    pub(crate) steps: Vec<PlanStep>,
    pub(crate) verifications: Vec<Verification>,
    pub(crate) plan_risks: Vec<PlanRisk>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanSpecBinding {
    pub(crate) candidate_id: String,
    pub(crate) semantic_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanBaseline {
    pub(crate) commands: Vec<String>,
    pub(crate) known_failure_policy: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanStep {
    pub(crate) temp_ref: String,
    pub(crate) objective: String,
    pub(crate) requirement_refs: Vec<EntityRevisionRef>,
    pub(crate) depends_on: Vec<String>,
    pub(crate) change_surface: Vec<String>,
    pub(crate) risks: Vec<String>,
    pub(crate) rollback_or_recovery: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Verification {
    pub(crate) temp_ref: String,
    pub(crate) acceptance_criterion_ref: EntityRevisionRef,
    pub(crate) plan_step_refs: Vec<String>,
    pub(crate) method: VerificationMethod,
    pub(crate) procedure: String,
    pub(crate) expected_result: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanRisk {
    pub(crate) statement: String,
    pub(crate) mitigation: String,
}

pub(crate) fn decode_plan(
    raw: &Value,
    state: &PlanningState,
    expected_work_item_id: &str,
    expected_base_revision: u64,
    expected_base_plan_revision: u64,
    expected_plan_input_hash: &str,
) -> Result<PlanCandidate, ServiceError> {
    let proposal: PlanProposal = serde_json::from_value(raw.clone())
        .map_err(|error| ServiceError::proposal_schema(format!("proposal schema: {error}")))?;
    if proposal.schema != PLAN_PROPOSAL_SCHEMA {
        return Err(ServiceError::proposal_schema(
            "unsupported plan proposal schema",
        ));
    }
    if proposal.work_item_id != expected_work_item_id
        || proposal.base_revision != expected_base_revision
        || proposal.base_plan_revision != expected_base_plan_revision
        || proposal.plan_input_hash != expected_plan_input_hash
    {
        return Err(ServiceError::with_code(
            "PROPOSAL_BASE_MISMATCH",
            "plan proposal does not match the current GeneratePlan work item",
        ));
    }
    let Some(spec_approval) = state.spec.approval.as_ref() else {
        return Err(ServiceError::with_code(
            "PROPOSAL_BASE_MISMATCH",
            "plan requires an approved spec",
        ));
    };
    if spec_approval.candidate_id != proposal.spec.candidate_id
        || spec_approval.semantic_hash != proposal.spec.semantic_hash
    {
        return Err(ServiceError::with_code(
            "PROPOSAL_BASE_MISMATCH",
            "plan spec binding does not match the approved spec",
        ));
    }
    let content = json!({
        "baseline": proposal.baseline,
        "steps": proposal.steps,
        "verifications": proposal.verifications,
        "plan_risks": proposal.plan_risks
    });
    validate_plan_content(state, &content).map_err(map_plan_error)?;
    let created_event_seq = expected_base_revision
        .checked_add(1)
        .ok_or_else(|| ServiceError::invalid("plan candidate event sequence overflow"))?;
    Ok(PlanCandidate {
        candidate_id: generated_candidate_id("plan"),
        created_event_seq,
        created_ordinal: 0,
        base_plan_revision: proposal.base_plan_revision,
        plan_input_hash: proposal.plan_input_hash,
        semantic_hash: canonical_hash(&content),
        spec_candidate_id: proposal.spec.candidate_id,
        spec_semantic_hash: proposal.spec.semantic_hash,
        content,
        stale: false,
    })
}

pub(crate) fn expected_plan_input_hash(state: &PlanningState) -> String {
    plan_input_hash(state)
}

fn map_plan_error(error: CoreError) -> ServiceError {
    match error {
        CoreError::InvalidSourceReference => ServiceError::with_code(
            "INVALID_SOURCE_REFERENCE",
            "plan source reference is invalid",
        ),
        CoreError::ProposalBaseMismatch => {
            ServiceError::with_code("PROPOSAL_BASE_MISMATCH", "plan binding is invalid")
        }
        CoreError::ProposalSchemaInvalid(message) => ServiceError::proposal_schema(message),
        other => ServiceError::proposal_schema(other.to_string()),
    }
}
