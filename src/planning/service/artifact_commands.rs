use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::super::domain::{ModelActionKind, PlanningState};
use super::super::evidence::snapshot_is_current;
use super::super::protocol::{LogicalRequest, PROTOCOL_VERSION, RESULT_SCHEMA};
use super::error::ServiceError;
use super::{PlanningService, ServiceAuthority};

#[path = "artifact_commands/plan.rs"]
mod plan;
#[path = "artifact_commands/spec.rs"]
mod spec;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CandidateGenerateParams {
    pub(super) proposal: Map<String, Value>,
    pub(super) projection_policy: Option<ProjectionPolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectionPolicy {
    #[serde(default)]
    pub(super) force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CandidateShowParams {
    pub(super) candidate_id: Option<String>,
    pub(super) format: Option<ShowFormat>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ShowFormat {
    Markdown,
    Json,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CandidateReviseParams {
    pub(super) candidate_id: String,
    pub(super) text: String,
}

pub(super) fn required_work_item(
    state: &PlanningState,
    kind: ModelActionKind,
) -> Result<&super::super::domain::ModelWorkItem, ServiceError> {
    state
        .required_model_action
        .as_ref()
        .filter(|work| work.kind == kind)
        .ok_or_else(|| {
            ServiceError::with_code("MODEL_ACTION_MISMATCH", "required model action differs")
        })
}

pub(super) fn artifact_query_response(
    request: &LogicalRequest,
    state: &PlanningState,
    candidate: Option<Value>,
) -> Value {
    json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request.request_id,
        "ok": true,
        "session_id": state.session_id,
        "revision": state.revision,
        "replayed": false,
        "result": {"schema": RESULT_SCHEMA, "operation": request.operation, "candidate": candidate},
        "observed": {"projection_status":"unchanged","evidence_current":state.repo_snapshot.is_some(),"warnings":[]}
    })
}

pub(super) fn check_revision(
    request: &LogicalRequest,
    state: &PlanningState,
) -> Result<(), ServiceError> {
    let expected = request.expected_revision.unwrap_or_default();
    if state.revision != expected {
        return Err(ServiceError::revision_conflict(expected, state.revision));
    }
    Ok(())
}

pub(super) fn require_current_evidence(
    service: &PlanningService,
    state: &PlanningState,
) -> Result<(), ServiceError> {
    let current = state
        .repo_snapshot
        .as_ref()
        .map(|snapshot| snapshot_is_current(service.store.project_root(), snapshot))
        .transpose()
        .map_err(ServiceError::evidence)?;
    if current != Some(true) {
        return Err(ServiceError::with_code(
            "EVIDENCE_STALE",
            "artifact operation requires current repository evidence",
        ));
    }
    Ok(())
}

pub(super) fn require_user(
    authority: ServiceAuthority,
    operation: &str,
) -> Result<(), ServiceError> {
    if !authority.is_user() {
        return Err(ServiceError::with_code(
            "USER_ENTRYPOINT_REQUIRED",
            format!("{operation} requires an explicit user entrypoint"),
        ));
    }
    Ok(())
}

pub(super) fn require_revision_or_export(
    authority: ServiceAuthority,
    operation: &str,
) -> Result<(), ServiceError> {
    if !authority.allows_model_revision_or_export() {
        return Err(ServiceError::with_code(
            "USER_ENTRYPOINT_REQUIRED",
            format!("{operation} requires a user entrypoint on this adapter"),
        ));
    }
    Ok(())
}
