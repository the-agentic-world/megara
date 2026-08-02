use serde_json::{json, Value};

use super::super::super::domain::{LifecyclePhase, ModelActionKind};
use super::super::super::engine::{ApprovalCommand, RevisionRequestCommand, SpecCandidateCommand};
use super::super::artifact_projection::{project_generated_candidate, ArtifactKind};
use super::super::artifact_wire::decode_spec;
use super::super::error::ServiceError;
use super::super::response::mutation_response;
use super::super::{
    decode_params, decode_params_or_default, required_session, PlanningService, ServiceAuthority,
};
use super::{
    artifact_query_response, check_revision, require_current_evidence, require_revision_or_export,
    require_user, required_work_item, CandidateGenerateParams, CandidateReviseParams,
    CandidateShowParams,
};
use crate::planning::protocol::LogicalRequest;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecApproveParams {
    candidate_id: String,
    semantic_hash: String,
    base_domain_revision: u64,
}

impl PlanningService {
    pub(crate) fn spec_generate(
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
                ArtifactKind::Spec,
            ));
        }
        let state = self.store.current(session_id)?;
        check_revision(&request, &state)?;
        if state.phase != LifecyclePhase::Specification {
            return Err(ServiceError::with_code(
                "INVALID_PHASE",
                "spec candidate requires Specification",
            ));
        }
        require_current_evidence(self, &state)?;
        let work_item = required_work_item(&state, ModelActionKind::GenerateSpec)?;
        let candidate = decode_spec(
            &Value::Object(params.proposal),
            &state,
            &work_item.work_item_id,
            work_item.base_revision,
            work_item.base_domain_revision,
            state
                .full_audit
                .as_ref()
                .map(|audit| audit.input_hash.as_str())
                .unwrap_or_default(),
        )?;
        let outcome = self.store.generate_spec_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            SpecCandidateCommand {
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
            ArtifactKind::Spec,
        ))
    }

    pub(crate) fn spec_show(&self, request: LogicalRequest) -> Result<Value, ServiceError> {
        let params = decode_params_or_default::<CandidateShowParams>(&request)?;
        let state = self.read_session(request.session_id.as_deref())?;
        let candidate = state
            .spec
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
                "current spec candidate was not found",
            ));
        }
        let _format = params.format;
        Ok(artifact_query_response(&request, &state, candidate))
    }

    pub(crate) fn spec_approve(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let params = decode_params::<SpecApproveParams>(&request)?;
        let session_id = required_session(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        require_user(authority, "spec approval")?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            let approval =
                serde_json::to_value(&outcome.state.spec.approval).unwrap_or(Value::Null);
            return Ok(mutation_response(
                &request,
                outcome,
                json!({"approval":approval}),
            ));
        }
        let state = self.store.current(session_id)?;
        check_revision(&request, &state)?;
        if state.phase != LifecyclePhase::Specification {
            return Err(ServiceError::with_code(
                "INVALID_PHASE",
                "spec approval requires Specification",
            ));
        }
        require_current_evidence(self, &state)?;
        let outcome = self.store.approve_spec_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            ApprovalCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                candidate_id: params.candidate_id,
                semantic_hash: params.semantic_hash,
                base_revision: params.base_domain_revision,
            },
            authority.event_context(&request.request_id),
        )?;
        let approval = serde_json::to_value(&outcome.state.spec.approval).unwrap_or(Value::Null);
        Ok(mutation_response(
            &request,
            outcome,
            json!({"approval":approval}),
        ))
    }

    pub(crate) fn spec_revise(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let params = decode_params::<CandidateReviseParams>(&request)?;
        let session_id = required_session(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        require_revision_or_export(authority, "spec revision")?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            return Ok(mutation_response(&request, outcome, Value::Null));
        }
        let outcome = self.store.revise_spec_with_context(
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
