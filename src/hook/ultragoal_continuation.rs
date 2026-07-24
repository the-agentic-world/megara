use super::*;

const CONTINUATION_DIR: &str = "ultragoal-continuation";

pub(super) fn record_completed_checkpoint(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<()> {
    if !completed_checkpoint_started_next_goal(payload) {
        return Ok(());
    }
    let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) else {
        return Ok(());
    };

    let paths = workflow_paths(state_dir, payload, ULTRAGOAL);
    reconcile_session_aliases(timestamp, payload_file, &paths, ULTRAGOAL, payload)?;
    let Some(state) = load_json(&paths.session_file) else {
        return Ok(());
    };
    if state.get("active").and_then(Value::as_bool) != Some(true)
        || state.get("phase").and_then(Value::as_str) != Some("active")
    {
        return Ok(());
    }

    write_json_atomic(
        &continuation_path(state_dir, payload),
        &json!({
            "turn_id": turn_id,
            "checkpoint_tool_use_id": payload.get("tool_use_id").cloned().unwrap_or(Value::Null),
            "status": "pending",
            "recorded_at": timestamp,
        }),
    )
}

pub(super) fn take_stop_continuation(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<Option<String>> {
    let path = continuation_path(state_dir, payload);
    let Some(mut continuation) = load_json(&path) else {
        return Ok(None);
    };
    if continuation.get("status").and_then(Value::as_str) != Some("pending")
        || continuation.get("turn_id").and_then(Value::as_str)
            != payload.get("turn_id").and_then(Value::as_str)
    {
        return Ok(None);
    }

    let paths = workflow_paths(state_dir, payload, ULTRAGOAL);
    reconcile_session_aliases(timestamp, payload_file, &paths, ULTRAGOAL, payload)?;
    let Some(state) = load_json(&paths.session_file) else {
        return Ok(None);
    };
    if state.get("active").and_then(Value::as_bool) != Some(true)
        || state.get("phase").and_then(Value::as_str) != Some("active")
    {
        return Ok(None);
    }

    continuation["status"] = json!("continued");
    continuation["continued_at"] = json!(timestamp);
    write_json_atomic(&path, &continuation)?;

    let title = state
        .get("active_goal_title")
        .and_then(Value::as_str)
        .unwrap_or("the active product goal");
    Ok(Some(format!(
        "Continue the active product goal now: {title}. A completed checkpoint activated this next goal in the current turn. Make the next product change or verification step; do not poll workflow status."
    )))
}

fn completed_checkpoint_started_next_goal(payload: &Value) -> bool {
    let Some(command) = payload
        .pointer("/tool_input/command")
        .and_then(Value::as_str)
    else {
        return false;
    };
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
    if !normalized.contains(" ultragoal ")
        || !normalized.contains(" checkpoint")
        || !(normalized.contains("--status complete") || normalized.contains("--status=complete"))
    {
        return false;
    }
    payload
        .get("tool_response")
        .is_some_and(response_reports_next_goal)
}

fn response_reports_next_goal(value: &Value) -> bool {
    match value {
        Value::Object(values) => {
            values
                .get("next_goal_started")
                .is_some_and(Value::is_object)
                || values.values().any(response_reports_next_goal)
        }
        Value::Array(values) => values.iter().any(response_reports_next_goal),
        Value::String(value) => {
            let normalized = value.split_whitespace().collect::<String>();
            normalized.contains("\"next_goal_started\":{")
                || normalized.contains("nextactivegoal:")
                || serde_json::from_str::<Value>(value)
                    .ok()
                    .is_some_and(|parsed| response_reports_next_goal(&parsed))
        }
        _ => false,
    }
}

fn continuation_path(state_dir: &Path, payload: &Value) -> PathBuf {
    state_dir
        .join(CONTINUATION_DIR)
        .join(format!("{}.json", safe_part(canonical_session_id(payload))))
}
