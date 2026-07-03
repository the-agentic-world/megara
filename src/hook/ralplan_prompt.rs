use super::*;

pub(super) struct RalplanPromptDecision {
    pub(super) event: &'static str,
    pub(super) handoff_target: Value,
}

pub(super) fn is_deep_interview_approval_for_ralplan(prompt: &str) -> bool {
    if parse_blocks(prompt, "Megara Approval Gate:")
        .into_iter()
        .any(|block| {
            field_eq(&block, "approved_workflow", DEEP_INTERVIEW)
                && field_eq(&block, "next_workflow", RALPLAN)
                && field_eq(&block, "approved_status", "crystallized")
        })
    {
        return true;
    }

    let normalized = prompt.to_ascii_lowercase();
    normalized.contains("ralplan")
        && (normalized.contains("proceed")
            || normalized.contains("continue")
            || normalized.contains("approve")
            || normalized.contains("진행")
            || normalized.contains("계속")
            || normalized.contains("승인"))
}

pub(super) fn apply_ralplan_prompt_decision(
    timestamp: &str,
    state: &mut Value,
    prompt: &str,
    payload_file: &Path,
) -> Option<RalplanPromptDecision> {
    if state.get("skill").and_then(Value::as_str) != Some(RALPLAN)
        || state.get("active").and_then(Value::as_bool) != Some(true)
        || state.get("phase").and_then(Value::as_str) != Some("pending_approval")
    {
        return None;
    }

    if let Some(gate) = approval_gate_from_text(prompt) {
        return Some(ralplan_approval::apply_gate(
            timestamp,
            state,
            gate,
            payload_file,
        ));
    }

    let normalized = prompt.to_ascii_lowercase();
    let plan_sha256 = state.get("plan_sha256").cloned().unwrap_or(Value::Null);
    let trimmed = prompt.trim();
    if trimmed == "2"
        || normalized.contains("approve_ultragoal")
        || (normalized.contains("approve") && normalized.contains("ultragoal"))
        || normalized.contains("ultragoal 승인")
    {
        ralplan_approval::approve_ralplan(timestamp, state, "ultragoal", plan_sha256, payload_file);
        return Some(ralplan_approval::decision(
            "plan_approved",
            json!("ultragoal"),
        ));
    }
    if trimmed == "3"
        || normalized.contains("approve_team")
        || (normalized.contains("approve") && normalized.contains("team"))
        || normalized.contains("team 승인")
    {
        ralplan_approval::approve_ralplan(timestamp, state, "team", plan_sha256, payload_file);
        return Some(ralplan_approval::decision("plan_approved", json!("team")));
    }
    if trimmed == "1"
        || normalized.contains("refine")
        || normalized.contains("iterate")
        || normalized.contains("보완")
        || normalized.contains("수정")
    {
        state["reviews"] = json!([]);
        state_fields::remove_state_fields(state, state_fields::PLAN_FIELDS);
        state["approval_status"] = json!("refine_requested");
        state["phase"] = json!("refining");
        state["updated_at"] = json!(timestamp);
        state["last_approval_payload"] = json!(payload_file);
        return Some(ralplan_approval::decision(
            "plan_refine_requested",
            Value::Null,
        ));
    }
    if trimmed == "4"
        || normalized.contains("stop_pending")
        || normalized.contains("pending")
        || normalized.contains("보류")
    {
        state["approval_status"] = json!("pending");
        state["phase"] = json!("pending_approval");
        state["updated_at"] = json!(timestamp);
        state["last_approval_payload"] = json!(payload_file);
        return Some(ralplan_approval::decision("plan_left_pending", Value::Null));
    }

    None
}

fn field_eq(block: &Block, key: &str, expected: &str) -> bool {
    block
        .fields
        .get(key)
        .map(|value| {
            value
                .trim()
                .trim_matches('"')
                .eq_ignore_ascii_case(expected)
        })
        .unwrap_or(false)
}
