use serde_json::{json, Value};

use super::super::super::domain::{LifecyclePhase, ModelActionKind};
use super::super::super::engine::{ApprovalCommand, PlanCandidateCommand, RevisionRequestCommand};
use super::super::artifact_projection::{project_generated_candidate, ArtifactKind};
use super::super::artifact_wire::{decode_plan, expected_plan_input_hash};
use super::super::error::ServiceError;
use super::super::response::mutation_response;
use super::super::{
    decode_params, decode_params_or_default, required_session, PlanningService, ServiceAuthority,
};
use super::{
    artifact_query_response, check_revision, require_current_evidence, require_user,
    required_work_item, CandidateGenerateParams, CandidateReviseParams, CandidateShowParams,
};
use crate::planning::protocol::LogicalRequest;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanApproveParams {
    candidate_id: String,
    semantic_hash: String,
    base_plan_revision: u64,
}

impl PlanningService {
    pub(crate) fn plan_generate(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let params = decode_params::<CandidateGenerateParams>(&request)?;
        let session_id = required_session(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        let force = params
            .projection_policy
            .as_ref()
            .is_some_and(|policy| policy.force);
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            return Ok(project_generated_candidate(
                &request,
                outcome,
                force,
                self.store.project_root(),
                ArtifactKind::Plan,
            ));
        }
        let state = self.store.current(session_id)?;
        check_revision(&request, &state)?;
        if state.phase != LifecyclePhase::Planning {
            return Err(ServiceError::with_code(
                "INVALID_PHASE",
                "plan candidate requires Planning",
            ));
        }
        require_current_evidence(self, &state)?;
        let work_item = required_work_item(&state, ModelActionKind::GeneratePlan)?;
        let candidate = decode_plan(
            &Value::Object(params.proposal),
            &state,
            &work_item.work_item_id,
            work_item.base_revision,
            work_item.base_plan_revision,
            &expected_plan_input_hash(&state),
        )?;
        let outcome = self.store.generate_plan_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            PlanCandidateCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                candidate,
            },
            authority.event_context(&request.request_id),
        )?;
        Ok(project_generated_candidate(
            &request,
            outcome,
            force,
            self.store.project_root(),
            ArtifactKind::Plan,
        ))
    }

    pub(crate) fn plan_show(&self, request: LogicalRequest) -> Result<Value, ServiceError> {
        let params = decode_params_or_default::<CandidateShowParams>(&request)?;
        let state = self.read_session(request.session_id.as_deref())?;
        let candidate = state
            .plan
            .current_candidate
            .as_ref()
            .filter(|candidate| {
                params
                    .candidate_id
                    .as_deref()
                    .is_none_or(|id| id == candidate.candidate_id)
            })
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| ServiceError::invalid(error.to_string()))?;
        if candidate.is_none() {
            return Err(ServiceError::with_code(
                "CANDIDATE_NOT_FOUND",
                "current plan candidate was not found",
            ));
        }
        let _format = params.format;
        Ok(artifact_query_response(&request, &state, candidate))
    }

    pub(crate) fn plan_approve(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let params = decode_params::<PlanApproveParams>(&request)?;
        let session_id = required_session(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        require_user(authority, "plan approval")?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            let approval =
                serde_json::to_value(&outcome.state.plan.approval).unwrap_or(Value::Null);
            return Ok(mutation_response(
                &request,
                outcome,
                json!({"approval":approval}),
            ));
        }
        let state = self.store.current(session_id)?;
        check_revision(&request, &state)?;
        if state.phase != LifecyclePhase::Planning {
            return Err(ServiceError::with_code(
                "INVALID_PHASE",
                "plan approval requires Planning",
            ));
        }
        require_current_evidence(self, &state)?;
        let outcome = self.store.approve_plan_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            ApprovalCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                candidate_id: params.candidate_id,
                semantic_hash: params.semantic_hash,
                base_revision: params.base_plan_revision,
            },
            authority.event_context(&request.request_id),
        )?;
        let approval = serde_json::to_value(&outcome.state.plan.approval).unwrap_or(Value::Null);
        Ok(mutation_response(
            &request,
            outcome,
            json!({"approval":approval}),
        ))
    }

    pub(crate) fn plan_revise(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let params = decode_params::<CandidateReviseParams>(&request)?;
        let session_id = required_session(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        require_user(authority, "plan revision")?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            return Ok(mutation_response(&request, outcome, Value::Null));
        }
        let outcome = self.store.revise_plan_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            RevisionRequestCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                candidate_id: params.candidate_id,
                text: params.text,
            },
            authority.event_context(&request.request_id),
        )?;
        Ok(mutation_response(&request, outcome, Value::Null))
    }
}
