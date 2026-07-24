use super::*;

const MAX_STATUS_CALLS_PER_TURN: u64 = 2;

pub(super) struct StatusLoopBlock {
    pub(super) context: &'static str,
}

pub(super) fn repeated_ultragoal_status_block(
    state_dir: &Path,
    payload: &Value,
) -> Result<Option<StatusLoopBlock>> {
    let Some(command) = payload
        .pointer("/tool_input/command")
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    if !is_ultragoal_status(command) {
        return Ok(None);
    }
    let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) else {
        return Ok(None);
    };

    let path = state_dir
        .join("tool-loop")
        .join(format!("{}.json", safe_part(canonical_session_id(payload))));
    let mut state = load_json(&path).unwrap_or_else(|| json!({}));
    if state.get("turn_id").and_then(Value::as_str) != Some(turn_id) {
        state = json!({"turn_id": turn_id, "status_calls": 0});
    }
    let calls = state
        .get("status_calls")
        .and_then(Value::as_u64)
        .unwrap_or_default()
        + 1;
    state["status_calls"] = json!(calls);
    write_json_atomic(&path, &state)?;

    Ok((calls > MAX_STATUS_CALLS_PER_TURN).then_some(StatusLoopBlock {
        context: "Megara already returned the workflow status in this turn. Continue the active product task or finish the turn. Do not inspect status again this turn.",
    }))
}

fn is_ultragoal_status(command: &str) -> bool {
    let command = command.trim();
    command.contains(" ultragoal ") && command.split_whitespace().last() == Some("status")
}
