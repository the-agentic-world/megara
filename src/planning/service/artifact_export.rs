use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::super::domain::PlanningState;
use super::super::engine::{
    plan_input_hash, spec_semantic_hash, validate_entity_refs, validate_plan_content, CoreError,
};
use super::super::protocol::state::project_state;
use super::super::protocol::{LogicalRequest, PROTOCOL_VERSION, RESULT_SCHEMA};
use super::super::store::PlanningStore;
use super::artifact_commands::require_revision_or_export;
use super::artifact_projection::{atomic_export_write, render_export_markdown};
use super::error::ServiceError;
use super::response::observed_list;
use super::{decode_params, PlanningService, ServiceAuthority};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExportParams {
    pub(super) out: String,
    pub(super) format: ExportFormat,
    #[serde(default)]
    pub(super) include_transcript: bool,
    #[serde(default)]
    pub(super) force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ExportFormat {
    Bundle,
    StateJson,
    EventsJsonl,
}

impl ExportFormat {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Bundle => "bundle",
            Self::StateJson => "state-json",
            Self::EventsJsonl => "events-jsonl",
        }
    }
}

impl PlanningService {
    pub(super) fn export(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<serde_json::Value, ServiceError> {
        require_revision_or_export(authority, "export")?;
        let params = decode_params::<ExportParams>(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        let command_id = request.command_id.as_deref().unwrap_or_default();
        if let Some(outcome) = self.store.cached_command(command_id, &request_hash)? {
            return Ok(export_response(&request, outcome, params));
        }
        let state = self.read_session(request.session_id.as_deref())?;
        if matches!(params.format, ExportFormat::Bundle) {
            require_approved_bundle(&state)?;
            let current = state
                .repo_snapshot
                .as_ref()
                .map(|snapshot| {
                    super::super::evidence::snapshot_is_current(self.store.project_root(), snapshot)
                })
                .transpose()
                .map_err(ServiceError::evidence)?;
            if current != Some(true) {
                return Err(ServiceError::with_code(
                    "EVIDENCE_STALE",
                    "bundle export requires current repository evidence",
                ));
            }
        }
        let files = write_export(
            &params.out,
            &state,
            &self.store,
            &params.format,
            params.include_transcript,
            params.force,
        )?;
        let outcome = self
            .store
            .record_noop(command_id, &request_hash, &state.session_id)?;
        Ok(export_response_with_files(&request, outcome, params, files))
    }
}

fn require_approved_bundle(state: &PlanningState) -> Result<(), ServiceError> {
    if state.phase != super::super::domain::LifecyclePhase::Complete {
        return Err(ServiceError::with_code(
            "INVALID_PHASE",
            "bundle export requires a completed plan",
        ));
    }
    let spec = state
        .spec
        .current_candidate
        .as_ref()
        .ok_or_else(|| ServiceError::with_code("INVALID_PHASE", "approved spec is required"))?;
    let spec_approval = state
        .spec
        .approval
        .as_ref()
        .ok_or_else(|| ServiceError::with_code("INVALID_PHASE", "approved spec is required"))?;
    let plan = state
        .plan
        .current_candidate
        .as_ref()
        .ok_or_else(|| ServiceError::with_code("INVALID_PHASE", "approved plan is required"))?;
    let plan_approval = state
        .plan
        .approval
        .as_ref()
        .ok_or_else(|| ServiceError::with_code("INVALID_PHASE", "approved plan is required"))?;
    if spec.stale || plan.stale {
        return Err(ServiceError::with_code(
            "CANDIDATE_STALE",
            "bundle artifacts are stale",
        ));
    }
    if spec_approval.candidate_id != spec.candidate_id
        || spec_approval.semantic_hash != spec.semantic_hash
        || spec_approval.base_revision != spec.base_domain_revision
        || plan_approval.candidate_id != plan.candidate_id
        || plan_approval.semantic_hash != plan.semantic_hash
        || plan_approval.base_revision != plan.base_plan_revision
    {
        return Err(ServiceError::with_code(
            "APPROVAL_BINDING_MISMATCH",
            "bundle approvals do not match current candidates",
        ));
    }
    if state.has_blocking_blocker() {
        return Err(ServiceError::with_code(
            "BLOCKERS_PRESENT",
            "bundle export requires no blocking blockers",
        ));
    }
    let full_audit = state.full_audit.as_ref().ok_or_else(|| {
        ServiceError::with_code("APPROVAL_BINDING_MISMATCH", "full audit is missing")
    })?;
    if spec.audit_input_hash != full_audit.input_hash
        || spec.base_domain_revision != full_audit.base_domain_revision
        || spec.base_domain_revision != state.domain_revision
        || spec_semantic_hash(state, &spec.content) != spec.semantic_hash
    {
        return Err(ServiceError::with_code(
            "APPROVAL_BINDING_MISMATCH",
            "approved spec binding is no longer current",
        ));
    }
    validate_entity_refs(state, &spec.entity_refs).map_err(map_binding_error)?;
    if plan.spec_candidate_id != spec_approval.candidate_id
        || plan.spec_semantic_hash != spec_approval.semantic_hash
        || plan.base_plan_revision != state.plan_revision
        || plan.plan_input_hash != plan_input_hash(state)
        || super::super::canonical::canonical_hash(&plan.content) != plan.semantic_hash
    {
        return Err(ServiceError::with_code(
            "APPROVAL_BINDING_MISMATCH",
            "approved plan binding is no longer current",
        ));
    }
    validate_plan_content(state, &plan.content).map_err(map_binding_error)?;
    Ok(())
}

fn map_binding_error(error: CoreError) -> ServiceError {
    match error {
        CoreError::ProposalSchemaInvalid(message) => ServiceError::proposal_schema(message),
        CoreError::InvalidSourceReference => ServiceError::with_code(
            "INVALID_SOURCE_REFERENCE",
            "artifact source reference is invalid",
        ),
        CoreError::ProposalBaseMismatch => {
            ServiceError::with_code("APPROVAL_BINDING_MISMATCH", "artifact binding is invalid")
        }
        other => ServiceError::with_code("APPROVAL_BINDING_MISMATCH", other.to_string()),
    }
}

fn export_response(
    request: &LogicalRequest,
    outcome: super::super::store::StoredOutcome,
    params: ExportParams,
) -> serde_json::Value {
    let files = included_files(&params, &outcome.state);
    export_response_with_files(request, outcome, params, files)
}

fn included_files(params: &ExportParams, state: &PlanningState) -> Vec<String> {
    match params.format {
        ExportFormat::StateJson => vec!["state.json".to_string()],
        ExportFormat::EventsJsonl => vec!["events.jsonl".to_string()],
        ExportFormat::Bundle => {
            let mut files = vec!["manifest.json".to_string()];
            if state.spec.current_candidate.is_some() {
                files.push("spec.md".to_string());
            }
            if state.plan.current_candidate.is_some() {
                files.push("plan.md".to_string());
            }
            if params.include_transcript {
                files.push("transcript.json".to_string());
                files.push("events.jsonl".to_string());
            }
            files
        }
    }
}

fn export_response_with_files(
    request: &LogicalRequest,
    outcome: super::super::store::StoredOutcome,
    params: ExportParams,
    files: Vec<String>,
) -> serde_json::Value {
    json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request.request_id,
        "ok": true,
        "command_id": request.command_id,
        "session_id": outcome.state.session_id,
        "revision": outcome.state.revision,
        "replayed": outcome.replayed,
        "result": {
            "schema": RESULT_SCHEMA,
            "operation": request.operation,
            "path": params.out,
            "format": params.format.as_str(),
            "included": files,
        },
        "observed": observed_list(),
    })
}

pub(super) fn write_export(
    output: &str,
    state: &PlanningState,
    store: &PlanningStore,
    format: &ExportFormat,
    include_transcript: bool,
    force: bool,
) -> Result<Vec<String>, ServiceError> {
    let path = Path::new(output);
    match format {
        ExportFormat::StateJson => {
            let value = project_state(state);
            let bytes = serde_json::to_vec_pretty(&value)
                .map_err(|error| ServiceError::invalid(error.to_string()))?;
            atomic_export_write(path, &bytes, force)?;
            Ok(vec!["state.json".to_string()])
        }
        ExportFormat::EventsJsonl => {
            let bytes = event_bytes(store, &state.session_id)?;
            atomic_export_write(path, &bytes, force)?;
            Ok(vec!["events.jsonl".to_string()])
        }
        ExportFormat::Bundle => write_bundle(path, state, store, include_transcript, force),
    }
}

fn write_bundle(
    path: &Path,
    state: &PlanningState,
    store: &PlanningStore,
    include_transcript: bool,
    force: bool,
) -> Result<Vec<String>, ServiceError> {
    if path.exists() && !force {
        return Err(ServiceError::with_code(
            "PROJECTION_DIVERGED",
            "export output already exists",
        ));
    }
    if path.exists() && !path.is_dir() {
        return Err(ServiceError::with_code(
            "IO_ERROR",
            "bundle output is not a directory",
        ));
    }
    let mut payloads = Vec::<(String, Vec<u8>)>::new();
    let mut included = vec!["manifest.json".to_string()];
    for (name, candidate, kind) in [
        (
            "spec.md",
            state
                .spec
                .current_candidate
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| ServiceError::invalid(error.to_string()))?,
            "spec",
        ),
        (
            "plan.md",
            state
                .plan
                .current_candidate
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| ServiceError::invalid(error.to_string()))?,
            "plan",
        ),
    ] {
        if let Some(candidate) = candidate {
            payloads.push((
                name.to_string(),
                render_export_markdown(&state.session_id, kind, &candidate).into_bytes(),
            ));
            included.push(name.to_string());
        }
    }
    if include_transcript {
        payloads.push((
            "transcript.json".to_string(),
            serde_json::to_vec_pretty(&state.transcript)
                .map_err(|error| ServiceError::invalid(error.to_string()))?,
        ));
        payloads.push((
            "events.jsonl".to_string(),
            event_bytes(store, &state.session_id)?,
        ));
        included.push("transcript.json".to_string());
        included.push("events.jsonl".to_string());
    }
    let manifest = json!({
        "schema":"megara.export-manifest/v1",
        "session_id":state.session_id,
        "revision":state.revision,
        "include_transcript":include_transcript,
        "transcript_included":include_transcript,
        "events_included":include_transcript,
        "spec": state.spec.current_candidate.as_ref().map(|candidate| json!({
            "candidate_id": candidate.candidate_id,
            "semantic_hash": candidate.semantic_hash,
        })),
        "plan": state.plan.current_candidate.as_ref().map(|candidate| json!({
            "candidate_id": candidate.candidate_id,
            "semantic_hash": candidate.semantic_hash,
        })),
        "included":included
    });
    payloads.push((
        "manifest.json".to_string(),
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| ServiceError::invalid(error.to_string()))?,
    ));
    atomic_bundle_write(path, &payloads, force)?;
    Ok(included)
}

fn event_bytes(store: &PlanningStore, session_id: &str) -> Result<Vec<u8>, ServiceError> {
    let mut bytes = Vec::new();
    for event in store.event_envelopes(session_id)? {
        serde_json::to_writer(&mut bytes, &event)
            .map_err(|error| ServiceError::invalid(error.to_string()))?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn atomic_bundle_write(
    path: &Path,
    payloads: &[(String, Vec<u8>)],
    force: bool,
) -> Result<(), ServiceError> {
    let parent = path
        .parent()
        .ok_or_else(|| ServiceError::with_code("IO_ERROR", "bundle has no parent"))?;
    fs::create_dir_all(parent)
        .map_err(|error| ServiceError::with_code("IO_ERROR", error.to_string()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bundle");
    let staging = parent.join(format!(".{name}.tmp-{}", Uuid::now_v7()));
    fs::create_dir(&staging)
        .map_err(|error| ServiceError::with_code("IO_ERROR", error.to_string()))?;
    let result = (|| {
        for (file_name, bytes) in payloads {
            atomic_export_write(&staging.join(file_name), bytes, true)?;
        }
        if path.exists() {
            if !force {
                return Err(ServiceError::with_code(
                    "PROJECTION_DIVERGED",
                    "export output already exists",
                ));
            }
            let backup = parent.join(format!(".{name}.old-{}", Uuid::now_v7()));
            fs::rename(path, &backup)
                .map_err(|error| ServiceError::with_code("IO_ERROR", error.to_string()))?;
            if let Err(error) = fs::rename(&staging, path) {
                let _ = fs::rename(&backup, path);
                return Err(ServiceError::with_code("IO_ERROR", error.to_string()));
            }
            let _ = remove_path(&backup);
        } else {
            fs::rename(&staging, path)
                .map_err(|error| ServiceError::with_code("IO_ERROR", error.to_string()))?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_path(&staging);
    }
    result
}

fn remove_path(path: &Path) -> Result<(), ServiceError> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|error| ServiceError::with_code("IO_ERROR", error.to_string()))
}
