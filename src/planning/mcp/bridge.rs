use std::sync::Mutex;

use rmcp::{
    model::{CallToolRequestParams, CallToolResponse, CallToolResult},
    ErrorData,
};
use serde_json::Value;
use uuid::Uuid;

use super::super::protocol::{
    supported_operations, LogicalRequest, OperationKind, ProtocolError, PROTOCOL_VERSION,
};
use super::super::service::{store_error_response, PlanningService};
use super::catalog::ToolSpec;

pub(crate) fn call_tool(
    service: &Mutex<PlanningService>,
    spec: ToolSpec,
    request: CallToolRequestParams,
) -> Result<CallToolResponse, ErrorData> {
    let logical = logical_request(spec, Value::Object(request.arguments.unwrap_or_default()))
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
    let request_id = logical.request_id.clone();
    let response = match service.lock() {
        Ok(mut service) if spec.prompt_required => service.handle_codex_user_request(logical),
        Ok(mut service) => service.handle_codex_request(logical),
        Err(_) => store_error_response(
            Some(&request_id),
            Some(spec.operation),
            crate::planning::store::StoreError::InvalidRequest(
                "MCP service lock is poisoned".to_string(),
            ),
        ),
    };
    let is_error = response.get("ok").and_then(Value::as_bool) == Some(false);
    Ok(if is_error {
        CallToolResult::structured_error(response).into()
    } else {
        CallToolResult::structured(response).into()
    })
}

fn logical_request(spec: ToolSpec, arguments: Value) -> Result<LogicalRequest, ProtocolError> {
    let Value::Object(mut arguments) = arguments else {
        return Err(ProtocolError::InvalidRequest(
            "tool arguments must be an object".to_string(),
        ));
    };
    for forbidden in ["actor", "adapter", "request_id", "project"] {
        if arguments.contains_key(forbidden) {
            return Err(ProtocolError::InvalidRequest(format!(
                "forbidden tool argument: {forbidden}"
            )));
        }
    }
    let command_id = arguments.remove("command_id");
    let session_id = arguments.remove("session_id");
    let expected_revision = arguments.remove("expected_revision");
    if !supported_operations().contains(&spec.operation) {
        return Err(ProtocolError::InvalidRequest(
            "unsupported planning tool".to_string(),
        ));
    }
    let kind = if matches!(
        spec.operation,
        "planning.status"
            | "planning.current"
            | "planning.list"
            | "planning.spec.show"
            | "planning.plan.show"
    ) {
        OperationKind::Query
    } else {
        OperationKind::Mutation
    };
    if kind == OperationKind::Query && command_id.is_some() {
        return Err(ProtocolError::InvalidRequest(
            "query tool forbids command_id".to_string(),
        ));
    }
    let command_id = match (kind, command_id) {
        (OperationKind::Mutation, Some(value)) => Some(string_value(value, "command_id")?),
        (OperationKind::Mutation, None) => {
            return Err(ProtocolError::InvalidRequest(
                "mutation requires command_id; reuse it when retrying the same call".to_string(),
            ))
        }
        (OperationKind::Query, None) => None,
        (OperationKind::Query, Some(_)) => unreachable!(),
    };
    let session_id = session_id
        .map(|value| string_value(value, "session_id"))
        .transpose()?;
    let expected_revision = expected_revision
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                ProtocolError::InvalidRequest("expected_revision must be an integer".to_string())
            })
        })
        .transpose()?;
    let params = if arguments.is_empty() {
        None
    } else {
        Some(Value::Object(arguments))
    };
    let request = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: new_id("req"),
        operation: spec.operation.to_string(),
        command_id,
        session_id,
        expected_revision,
        params,
    };
    request.validate()?;
    Ok(request)
}

fn string_value(value: Value, field: &str) -> Result<String, ProtocolError> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ProtocolError::InvalidRequest(format!("{field} must be a string")))
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::now_v7())
}
