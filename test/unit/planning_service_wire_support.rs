use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use serde_json::{json, Value};

pub(crate) fn request(
    operation: &str,
    request_id: &str,
    command_id: Option<&str>,
    session_id: Option<String>,
    expected_revision: Option<u64>,
    params: Value,
) -> LogicalRequest {
    LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.to_string(),
        operation: operation.to_string(),
        command_id: command_id.map(str::to_string),
        session_id,
        expected_revision,
        params: Some(params),
    }
}

pub(crate) fn audit_proposal(
    work_item: &Value,
    mode: &str,
    entity_ops: Value,
    readiness: &str,
) -> Value {
    json!({
        "schema":"megara.audit-proposal/v1",
        "mode":mode,
        "work_item_id":work_item["work_item_id"],
        "base_revision":work_item["base_revision"],
        "base_domain_revision":work_item["base_domain_revision"],
        "input_hash":work_item["input_hash"],
        "readiness":readiness,
        "next_question":null,
        "entity_ops":entity_ops,
        "edge_ops":[],
        "blocker_ops":[],
        "counterexample_review": if mode == "full" {
            json!({"performed":true,"challenged_entity_ids":[],"findings":[]})
        } else {
            Value::Null
        }
    })
}

pub(crate) fn delta_proposal(work_item: &Value, entity_ops: Value, blocker_ops: Value) -> Value {
    let mut proposal = audit_proposal(work_item, "delta", entity_ops, "request_full_audit");
    proposal["blocker_ops"] = blocker_ops;
    proposal
}
