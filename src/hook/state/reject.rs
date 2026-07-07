use std::path::Path;

use serde_json::{json, Value};

use crate::hook::{
    state_fields::{remove_state_fields, PLAN_FIELDS},
    DEEP_INTERVIEW,
};

pub(crate) fn require_ralplan_input_lock(timestamp: &str, state: &mut Value, payload_file: &Path) {
    state["active"] = json!(true);
    state["phase"] = json!("input_lock_required");
    state["status"] = json!("input_lock_required");
    state["requires_input_lock"] = json!(true);
    state["required_input_workflow"] = json!(DEEP_INTERVIEW);
    state["approval_status"] = json!("awaiting_plan");
    state["last_input_lock_payload"] = json!(payload_file);
    state["updated_at"] = json!(timestamp);
}

pub(crate) fn mark_ralplan_input_lock_ready(timestamp: &str, state: &mut Value) {
    state["phase"] = json!("input_lock_ready");
    state["status"] = json!("input_lock_ready");
    state["input_lock_status"] = json!("ready");
    state["updated_at"] = json!(timestamp);
}

pub(crate) fn reject_crystallized_without_spec(timestamp: &str, state: &mut Value) {
    state["active"] = json!(true);
    state["phase"] = json!("crystallization_missing_spec");
    state["status"] = json!("crystallization_missing_spec");
    state["pending_question"] = Value::Null;
    state["updated_at"] = json!(timestamp);
}

pub(crate) fn reject_ralplan_without_plan(timestamp: &str, state: &mut Value, plan_id: &str) {
    block_ralplan(timestamp, state, "plan_missing", plan_id);
}

pub(crate) fn reject_ralplan_without_reviews(timestamp: &str, state: &mut Value, plan_id: &str) {
    block_ralplan(timestamp, state, "review_incomplete", plan_id);
}

pub(crate) fn reject_ralplan_input_lock(
    timestamp: &str,
    state: &mut Value,
    plan_id: &str,
    reason: &'static str,
) {
    remove_state_fields(state, PLAN_FIELDS);
    block_ralplan(timestamp, state, "input_lock_blocked", plan_id);
    state["requires_input_lock"] = json!(true);
    state["required_input_workflow"] = json!(DEEP_INTERVIEW);
    state["input_lock_status"] = json!(reason);
}

pub(crate) fn reject_ralplan_handoff_not_ready(
    timestamp: &str,
    state: &mut Value,
    plan_id: &str,
    blocker: &Value,
) {
    block_ralplan(timestamp, state, "handoff_not_ready", plan_id);
    state["blocked_by"] = json!(DEEP_INTERVIEW);
    state["blocked_phase"] = blocker.get("phase").cloned().unwrap_or(Value::Null);
    state["blocked_status"] = blocker.get("status").cloned().unwrap_or(Value::Null);
}

fn block_ralplan(timestamp: &str, state: &mut Value, phase: &str, plan_id: &str) {
    state["active"] = json!(true);
    state["phase"] = json!(phase);
    state["status"] = json!(phase);
    state["plan_id"] = json!(plan_id);
    state["approval_status"] = json!("blocked");
    state["updated_at"] = json!(timestamp);
}
