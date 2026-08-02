use serde::Deserialize;
use serde_json::{json, Value};

use super::domain::{LifecyclePhase, PlanningState};
use super::engine::{AnswerCommand, AuditMode, StartCommand};
use super::evidence::{capture_snapshot_with_previous, snapshot_is_current, EvidenceCitation};
use super::protocol::{state::project_state, LogicalRequest, PROTOCOL_VERSION, RESULT_SCHEMA};
use super::store::{EventActor, EventAdapter, EventContext, PlanningStore, StoreError};

#[path = "service/audit_wire.rs"]
mod audit_wire;
#[path = "service/error.rs"]
mod error;
#[path = "service/response.rs"]
mod response;

use audit_wire::AuditApplyParams;
use error::ServiceError;
pub(crate) use response::{error_response, protocol_error_response, store_error_response};
use response::{mutation_response, observed_list};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceAuthority {
    ModelPi,
    UserCli,
}

impl ServiceAuthority {
    fn event_context(self, request_id: &str) -> EventContext {
        match self {
            Self::ModelPi => EventContext {
                actor: EventActor::Model,
                adapter: EventAdapter::Pi,
                request_id: Some(request_id.to_string()),
            },
            Self::UserCli => EventContext {
                actor: EventActor::User,
                adapter: EventAdapter::Cli,
                request_id: Some(request_id.to_string()),
            },
        }
    }
}

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
struct ListParams {
    phase: Option<LifecyclePhase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PurgeParams {
    confirm: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRefreshParams {
    citations: Vec<EvidenceCitation>,
}

pub struct PlanningService {
    store: PlanningStore,
}

impl PlanningService {
    pub fn open_project(root: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        Ok(Self {
            store: PlanningStore::open_project(root)?,
        })
    }

    pub fn project_id(&self) -> &str {
        self.store.project_id()
    }

    pub fn handle_request(&mut self, request: LogicalRequest) -> Value {
        self.handle(request, ServiceAuthority::ModelPi)
    }

    pub fn handle_user_request(&mut self, request: LogicalRequest) -> Value {
        self.handle(request, ServiceAuthority::UserCli)
    }

    fn handle(&mut self, request: LogicalRequest, authority: ServiceAuthority) -> Value {
        let request_id = request.request_id.clone();
        let operation = request.operation.clone();
        match self.dispatch(request, authority) {
            Ok(mut response) => {
                self.refresh_observed(&mut response);
                response
            }
            Err(error) => error_response(Some(&request_id), Some(&operation), error),
        }
    }

    fn refresh_observed(&self, response: &mut Value) {
        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            return;
        }
        let Some(session_id) = response.get("session_id").and_then(Value::as_str) else {
            return;
        };
        let current = match self.store.current(session_id) {
            Ok(state) => state,
            Err(_) => return,
        };
        let health = current
            .repo_snapshot
            .as_ref()
            .map(|snapshot| snapshot_is_current(self.store.project_root(), snapshot));
        let (evidence_current, warnings) = match health {
            None => (false, json!([])),
            Some(Ok(true)) => (true, json!([])),
            Some(Ok(false)) => (false, json!(["EVIDENCE_STALE"])),
            Some(Err(_)) => (false, json!(["EVIDENCE_HEALTH_IO"])),
        };
        response["observed"]["evidence_current"] = json!(evidence_current);
        response["observed"]["warnings"] = warnings;
    }

    fn dispatch(
        &mut self,
        request: LogicalRequest,
        authority: ServiceAuthority,
    ) -> Result<Value, ServiceError> {
        request.validate().map_err(ServiceError::protocol)?;
        match request.operation.as_str() {
            "planning.start" => self.start(request, authority),
            "planning.answer" => self.answer(request, authority),
            "planning.status" | "planning.current" => self.status(request),
            "planning.list" => self.list(request),
            "planning.evidence.refresh" => self.refresh_evidence(request, authority),
            "planning.audit.apply" => self.apply_audit(request, authority),
            "planning.purge" if authority == ServiceAuthority::UserCli => self.purge(request),
            "planning.purge" => Err(ServiceError::with_code(
                "USER_ENTRYPOINT_REQUIRED",
                "purge requires an explicit user entrypoint",
            )),
            operation => Err(ServiceError::invalid(format!(
                "{operation} is not available in this service boundary"
            ))),
        }
    }

    fn start(
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

    fn answer(
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

    fn status(&self, request: LogicalRequest) -> Result<Value, ServiceError> {
        let state = self.read_session(request.session_id.as_deref())?;
        Ok(response::query_response_with_health(&request, state, None))
    }

    fn refresh_evidence(
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
            super::engine::EvidenceRefreshCommand {
                session_id: session_id.to_string(),
                expected_revision: request.expected_revision.unwrap_or_default(),
                snapshot,
            },
            authority.event_context(&request.request_id),
        )?;
        Ok(mutation_response(&request, outcome, json!({})))
    }

    fn apply_audit(
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

    fn list(&self, request: LogicalRequest) -> Result<Value, ServiceError> {
        let params = decode_params_or_default::<ListParams>(&request)?;
        let sessions = self
            .store
            .list(params.phase)?
            .iter()
            .map(project_state)
            .collect::<Vec<_>>();
        Ok(json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": request.request_id,
            "ok": true,
            "replayed": false,
            "result": {
                "schema": RESULT_SCHEMA,
                "operation": request.operation,
                "sessions": sessions,
            },
            "observed": observed_list()
        }))
    }

    fn purge(&mut self, request: LogicalRequest) -> Result<Value, ServiceError> {
        let session_id = required_session(&request)?;
        let params = decode_params::<PurgeParams>(&request)?;
        let request_hash = request
            .canonical_request_hash(self.store.project_id())
            .map_err(ServiceError::protocol)?;
        let receipt = self.store.purge(
            session_id,
            request.command_id.as_deref().unwrap_or_default(),
            &request_hash,
            request.expected_revision.unwrap_or_default(),
            &params.confirm,
        )?;
        Ok(json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": request.request_id,
            "ok": true,
            "command_id": request.command_id,
            "session_id": receipt.session_id,
            "replayed": receipt.replayed,
            "result": {
                "schema": RESULT_SCHEMA,
                "operation": request.operation,
                "purged": receipt.purged,
                "cleanup_state": receipt.cleanup_state,
            },
            "observed": observed_list()
        }))
    }

    fn read_session(&self, session_id: Option<&str>) -> Result<PlanningState, ServiceError> {
        if let Some(session_id) = session_id {
            return self.store.current(session_id).map_err(Into::into);
        }
        let states = self.store.list(None)?;
        let active = states
            .iter()
            .filter(|state| state.phase != LifecyclePhase::Complete)
            .collect::<Vec<_>>();
        match active.as_slice() {
            [state] => Ok((*state).clone()),
            [] if states.len() == 1 => Ok(states[0].clone()),
            [] => Err(ServiceError::with_code(
                "SESSION_NOT_FOUND",
                "no planning session exists",
            )),
            _ => Err(ServiceError::session_ambiguous()),
        }
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

fn decode_params<T: for<'de> Deserialize<'de>>(
    request: &LogicalRequest,
) -> Result<T, ServiceError> {
    serde_json::from_value(
        request
            .params
            .clone()
            .ok_or_else(|| ServiceError::invalid("params are required"))?,
    )
    .map_err(|error| ServiceError::invalid(error.to_string()))
}

fn decode_params_or_default<T: for<'de> Deserialize<'de>>(
    request: &LogicalRequest,
) -> Result<T, ServiceError> {
    serde_json::from_value(request.params.clone().unwrap_or_else(|| json!({})))
        .map_err(|error| ServiceError::invalid(error.to_string()))
}

fn required_session(request: &LogicalRequest) -> Result<&str, ServiceError> {
    request
        .session_id
        .as_deref()
        .filter(|session_id| !session_id.trim().is_empty())
        .ok_or_else(|| ServiceError::with_code("SESSION_REQUIRED", "session_id is required"))
}
