use serde::Deserialize;
use serde_json::{json, Value};

use crate::planning::domain::LifecyclePhase;
use crate::planning::engine::{AnswerCommand, AuditMode, EvidenceRefreshCommand, StartCommand};
use crate::planning::evidence::{
    capture_snapshot_with_previous, snapshot_is_current, EvidenceCitation,
};
use crate::planning::protocol::LogicalRequest;

use super::audit_wire::AuditApplyParams;
use super::error::ServiceError;
use super::response::mutation_response;
use super::{decode_params, required_session, PlanningService, ServiceAuthority};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartParams {
    request: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnswerParams {
    question_id: String,
    text: String,
    #[serde(default)]
    selected_choice_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRefreshParams {
    citations: Vec<EvidenceCitation>,
}

impl PlanningService {
    pub(crate) fn start(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let params = decode_params::<StartParams>(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        let outcome = self.store.start_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            StartCommand {
                session_id: None,
                project_id: self.store.project_id().to_string(),
                request: params.request,
                title: params.title,
            },
            authority.event_context(&request.request_id),
        )?;
        Ok(mutation_response(&request, outcome, json!({})))
    }

    pub(crate) fn answer(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let session_id = required_session(&request)?;
        let params = decode_params::<AnswerParams>(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            return Ok(mutation_response(&request, outcome, json!({})));
        }
        let current = self.store.current(session_id)?;
        let based_on_revision = current
            .pending_question
            .as_ref()
            .filter(|question| question.question_id == params.question_id)
            .map(|question| question.based_on_revision)
            .ok_or_else(|| {
                ServiceError::with_code(
                    "QUESTION_MISMATCH",
                    "question does not match the pending question",
                )
            })?;
        let outcome = self.store.apply_answer_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            AnswerCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                question_id: params.question_id,
                based_on_revision,
                text: params.text,
                selected_choice_ids: params.selected_choice_ids,
            },
            authority.event_context(&request.request_id),
        )?;
        Ok(mutation_response(&request, outcome, json!({})))
    }

    pub(crate) fn refresh_evidence(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let session_id = required_session(&request)?;
        let params = decode_params::<EvidenceRefreshParams>(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            return Ok(mutation_response(&request, outcome, json!({})));
        }
        let current = self.store.current(session_id)?;
        if current.revision != request.expected_revision.unwrap_or_default() {
            return Err(ServiceError::revision_conflict(
                request.expected_revision.unwrap_or_default(),
                current.revision,
            ));
        }
        let snapshot = capture_snapshot_with_previous(
            self.store.project_root(),
            &params.citations,
            current.repo_snapshot.as_ref(),
        )
        .map_err(ServiceError::evidence)?;
        let outcome = self.store.refresh_evidence_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            EvidenceRefreshCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                snapshot,
            },
            authority.event_context(&request.request_id),
        )?;
        Ok(mutation_response(&request, outcome, json!({})))
    }

    pub(crate) fn apply_audit(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        let session_id = required_session(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        if let Some(outcome) = self.store.cached_command(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
        )? {
            return Ok(mutation_response(&request, outcome, json!({})));
        }
        let current = self.store.current(session_id)?;
        if current.revision != request.expected_revision.unwrap_or_default() {
            return Err(ServiceError::revision_conflict(
                request.expected_revision.unwrap_or_default(),
                current.revision,
            ));
        }
        if current.phase != LifecyclePhase::Interview {
            return Err(ServiceError::with_code(
                "INVALID_PHASE",
                "audit apply requires Interview",
            ));
        }
        let mode = request
            .params
            .as_ref()
            .and_then(|params| params.get("mode"))
            .cloned()
            .map(|mode| {
                serde_json::from_value::<AuditMode>(mode)
                    .map_err(|error| ServiceError::proposal_schema(error.to_string()))
            })
            .transpose()?
            .ok_or_else(|| ServiceError::proposal_schema("audit mode is required"))?;
        if mode == AuditMode::Full {
            let evidence_current = current
                .repo_snapshot
                .as_ref()
                .map(|snapshot| snapshot_is_current(self.store.project_root(), snapshot))
                .transpose()
                .map_err(ServiceError::evidence)?;
            if evidence_current != Some(true) {
                return Err(ServiceError::with_code(
                    "EVIDENCE_STALE",
                    "full audit requires current repository evidence",
                ));
            }
        }
        let params = decode_audit_params(&request)?;
        params.proposal.validate_binding(&current, params.mode)?;
        let command = params.proposal.into_command(
            session_id,
            request.expected_revision.unwrap_or_default(),
            params.mode,
            &current,
        )?;
        let outcome = self.store.apply_audit_with_context(
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            command,
            authority.event_context(&request.request_id),
        )?;
        Ok(mutation_response(&request, outcome, json!({})))
    }
}

fn decode_audit_params(request: &LogicalRequest) -> Result<AuditApplyParams, ServiceError> {
    serde_json::from_value(
        request
            .params
            .clone()
            .ok_or_else(|| ServiceError::proposal_schema("audit params are required"))?,
    )
    .map_err(|error| ServiceError::proposal_schema(error.to_string()))
}
