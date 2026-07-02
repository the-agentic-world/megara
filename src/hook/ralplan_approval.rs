use super::*;

pub(super) fn apply_gate(
    timestamp: &str,
    state: &mut Value,
    gate: parser::ApprovalGate,
    payload_file: &Path,
) -> ralplan_prompt::RalplanPromptDecision {
    let current_plan_id = state
        .get("plan_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let current_plan_sha256 = state
        .get("plan_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if gate.plan_id == current_plan_id
        && gate.plan_sha256 == current_plan_sha256
        && matches!(gate.handoff_target.as_str(), "ultragoal" | "team")
    {
        let handoff_target = gate.handoff_target;
        approve_ralplan(
            timestamp,
            state,
            &handoff_target,
            json!(gate.plan_sha256),
            payload_file,
        );
        return decision("plan_approved", json!(handoff_target));
    }

    state["approval_status"] = json!("approval_gate_mismatch");
    state["phase"] = json!("pending_approval");
    state["updated_at"] = json!(timestamp);
    state["last_approval_payload"] = json!(payload_file);
    decision("plan_approval_rejected", Value::Null)
}

pub(super) fn approve_ralplan(
    timestamp: &str,
    state: &mut Value,
    handoff_target: &str,
    plan_sha256: Value,
    payload_file: &Path,
) {
    state["active"] = json!(false);
    state["phase"] = json!("approved");
    state["status"] = json!("approved");
    state["approval_status"] = json!("approved");
    state["approved_handoff_target"] = json!(handoff_target);
    if let Some(plan_id) = state.get("plan_id").cloned() {
        state["approved_plan_id"] = plan_id;
    }
    state["approved_plan_sha256"] = plan_sha256;
    state["approved_at"] = json!(timestamp);
    state["closed_at"] = json!(timestamp);
    state["last_approval_payload"] = json!(payload_file);
    state["updated_at"] = json!(timestamp);
}

pub(super) fn decision(
    event: &'static str,
    handoff_target: Value,
) -> ralplan_prompt::RalplanPromptDecision {
    ralplan_prompt::RalplanPromptDecision {
        event,
        handoff_target,
    }
}
