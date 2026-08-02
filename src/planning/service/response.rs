use serde_json::{json, Value};

use super::super::domain::PlanningState;
use super::super::protocol::{LogicalRequest, ProtocolError, PROTOCOL_VERSION, RESULT_SCHEMA};
use super::super::store::{StoreError, StoredOutcome};
use super::error::ServiceError;

pub(crate) fn mutation_response(
    request: &LogicalRequest,
    outcome: StoredOutcome,
    extra: Value,
) -> Value {
    let state = outcome.state;
    let mut result = serde_json::Map::new();
    result.insert("schema".to_string(), json!(RESULT_SCHEMA));
    result.insert("operation".to_string(), json!(request.operation));
    result.insert("state".to_string(), json!(state));
    result.insert("next_action".to_string(), next_action(&state));
    if let Value::Object(fields) = extra {
        result.extend(fields);
    }
    json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request.request_id,
        "ok": true,
        "command_id": request.command_id,
        "session_id": state.session_id,
        "revision": state.revision,
        "replayed": outcome.replayed,
        "result": Value::Object(result),
        "observed": observed(&state),
    })
}

pub(crate) fn query_response(request: &LogicalRequest, state: PlanningState) -> Value {
    json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request.request_id,
        "ok": true,
        "session_id": state.session_id,
        "revision": state.revision,
        "replayed": false,
        "result": {
            "schema": RESULT_SCHEMA,
            "operation": request.operation,
            "state": state,
        },
        "observed": observed(&state),
    })
}

fn next_action(state: &PlanningState) -> Value {
    if let Some(question) = state.pending_question.as_ref() {
        json!({"kind":"question", "question":question})
    } else if let Some(work_item) = state.required_model_action.as_ref() {
        json!({"kind":"model", "work_item":work_item})
    } else {
        Value::Null
    }
}

fn observed(state: &PlanningState) -> Value {
    json!({
        "projection_status":"unchanged",
        "evidence_current": state.repo_snapshot.is_some(),
        "warnings": [],
    })
}

pub(crate) fn observed_list() -> Value {
    json!({
        "projection_status":"unchanged",
        "evidence_current": null,
        "warnings": [],
    })
}

pub(crate) fn error_response(
    request_id: Option<&str>,
    operation: Option<&str>,
    error: ServiceError,
) -> Value {
    let mut body = json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id,
        "ok": false,
        "error": {
            "code": error.code,
            "message": error.message,
            "retryable": error.retryable,
        }
    });
    if let Some(operation) = operation {
        body["operation"] = json!(operation);
    }
    if !error.details.is_null() {
        body["error"]["details"] = error.details;
    }
    body
}

pub(crate) fn store_error_response(
    request_id: Option<&str>,
    operation: Option<&str>,
    error: StoreError,
) -> Value {
    error_response(request_id, operation, ServiceError::from(error))
}

pub(crate) fn protocol_error_response(
    request_id: Option<&str>,
    operation: Option<&str>,
    error: ProtocolError,
) -> Value {
    error_response(request_id, operation, ServiceError::protocol(error))
}
