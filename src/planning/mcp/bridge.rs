use std::sync::Mutex;

use rmcp::{
    model::{CallToolRequestParams, CallToolResponse, CallToolResult},
    ErrorData,
};
use serde_json::{json, Value};
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
    let mut response = match service.lock() {
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
    if !is_error {
        attach_delta_audit_host_adapter(&mut response);
    }
    Ok(if is_error {
        CallToolResult::structured_error(response).into()
    } else {
        CallToolResult::structured(response).into()
    })
}

fn attach_delta_audit_host_adapter(response: &mut Value) {
    let Some(work_item) = response
        .pointer("/result/state/required_model_action")
        .filter(|item| item.get("kind").and_then(Value::as_str) == Some("delta_audit"))
        .cloned()
    else {
        return;
    };
    let Some(session_id) = response.get("session_id").and_then(Value::as_str) else {
        return;
    };
    let Some(expected_revision) = response.get("revision").and_then(Value::as_u64) else {
        return;
    };
    let Some(work_item_id) = work_item.get("work_item_id").and_then(Value::as_str) else {
        return;
    };
    let Some(base_revision) = work_item.get("base_revision").and_then(Value::as_u64) else {
        return;
    };
    let Some(base_domain_revision) = work_item
        .get("base_domain_revision")
        .and_then(Value::as_u64)
    else {
        return;
    };
    let Some(input_hash) = work_item.get("input_hash").and_then(Value::as_str) else {
        return;
    };
    let command_id = format!(
        "mcp-audit-{}",
        work_item_id.strip_prefix("wrk_").unwrap_or(work_item_id)
    );
    let adapter = json!({
        "schema": "megara.codex-host-adapter/v1",
        "operation": "planning_audit_apply",
        "instruction": "Copy arguments_template exactly, then replace only the question prose and choice content. Keep every field, null, empty array, binding value, and source ref shape. Do not add question id or prompt fields.",
        "arguments_template": {
            "session_id": session_id,
            "expected_revision": expected_revision,
            "mode": "delta",
            "proposal": {
                "schema": "megara.audit-proposal/v1",
                "mode": "delta",
                "work_item_id": work_item_id,
                "base_revision": base_revision,
                "base_domain_revision": base_domain_revision,
                "input_hash": input_hash,
                "readiness": "continue",
                "next_question": {
                    "context": "The current request needs one more decision before a precise specification can be written. Choose the direction that best matches the intended first outcome.",
                    "question": "Which outcome should the first iteration prioritize?",
                    "why_it_matters": "The answer determines which requirement and acceptance criteria the specification will prioritize.",
                    "technical_terms": [],
                    "source_refs": [{"kind": "initial_request", "id": "request"}],
                    "answer": {
                        "mode": "choice",
                        "choices": [
                            {
                                "id": "focused-first-result",
                                "label": "Focus the first result",
                                "direction": "Deliver the smallest useful outcome before expanding the scope.",
                                "benefits": ["The first result can be reviewed sooner."],
                                "tradeoffs": ["Some secondary needs will wait for a later iteration."]
                            },
                            {
                                "id": "broader-first-result",
                                "label": "Cover the broader outcome",
                                "direction": "Include the related needs in the first specification.",
                                "benefits": ["The first specification covers more of the requested experience."],
                                "tradeoffs": ["The first result takes longer to define and review."]
                            }
                        ],
                        "recommendation": null,
                        "freeform_hint": "Choose one option or describe another preferred direction."
                    }
                },
                "entity_ops": [],
                "edge_ops": [],
                "blocker_ops": [],
                "counterexample_review": null
            },
            "command_id": command_id
        }
    });
    if let Some(result) = response.get_mut("result").and_then(Value::as_object_mut) {
        result.insert("host_adapter".to_string(), adapter);
    }
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
