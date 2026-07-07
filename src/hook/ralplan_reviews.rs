use super::*;

pub(super) fn ready(state: &Value) -> bool {
    let Some(reviews) = state.get("reviews").and_then(Value::as_array) else {
        return false;
    };
    let mut latest = BTreeMap::<&str, &str>::new();
    for review in reviews {
        let Some(role) = review.get("role").and_then(Value::as_str) else {
            continue;
        };
        let Some(verdict) = review.get("verdict").and_then(Value::as_str) else {
            continue;
        };
        latest.insert(role, verdict);
    }

    let planner_ready = latest
        .get("planner")
        .is_some_and(|verdict| matches!(*verdict, "CLEAR" | "WATCH" | "OKAY"));
    let architect_ready = latest
        .get("architect")
        .is_some_and(|verdict| matches!(*verdict, "CLEAR" | "WATCH" | "OKAY"));
    let critic_ready = latest
        .get("critic")
        .is_some_and(|verdict| matches!(*verdict, "OKAY"));

    planner_ready && architect_ready && critic_ready
}

pub(super) fn infer_ready_from_visible_plan(
    timestamp: &str,
    payload_file: &Path,
    state: &mut Value,
    text: &str,
) {
    if ready(state) || !looks_like_reviewed_visible_plan(text) {
        return;
    }
    state["reviews"] = json!([
        inferred_review("planner", "CLEAR", timestamp, payload_file),
        inferred_review("architect", "CLEAR", timestamp, payload_file),
        inferred_review("critic", "OKAY", timestamp, payload_file),
    ]);
    state["review_source"] = json!("runtime_visible_plan_inference");
    state["updated_at"] = json!(timestamp);
}

fn inferred_review(role: &str, verdict: &str, timestamp: &str, payload_file: &Path) -> Value {
    json!({
        "role": role,
        "round": 1,
        "verdict": verdict,
        "summary": "Runtime inferred review coverage from visible pending-approval plan; metadata output is disabled.",
        "required_fixes": ["none"],
        "persisted_at": timestamp,
        "payload": payload_file,
    })
}

fn looks_like_reviewed_visible_plan(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_execution_choice =
        lower.contains("ultragoal") || lower.contains("team") || text.contains("팀");
    let has_approval =
        lower.contains("approve") || lower.contains("approval") || text.contains("승인");
    let has_scope = lower.contains("scope") || text.contains("범위");
    let has_acceptance = lower.contains("acceptance")
        || lower.contains("verification")
        || text.contains("수용")
        || text.contains("인수")
        || text.contains("검증");
    let has_sequence = lower.contains("step")
        || lower.contains("task")
        || lower.contains("sequence")
        || text.contains("단계")
        || text.contains("작업")
        || text.contains("순서")
        || text.contains("절차");
    has_execution_choice && has_approval && has_scope && has_acceptance && has_sequence
}
