use super::*;

pub(super) fn handle_user_prompt(
    timestamp: &str,
    state_dir: &Path,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let Some(prompt) = runtime_input::effective_prompt_from_payload(payload) else {
        return Ok(0);
    };
    if prompt.trim().is_empty() {
        return Ok(0);
    }

    if let Some(reason) =
        codex_plan::deep_interview_plan_mode_block_reason(options, payload, &prompt)?
    {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "decision": "block",
                "reason": reason,
            }))?
        );
        return Ok(0);
    }

    let ralplan_paths = workflow_paths(state_dir, payload, RALPLAN);
    reconcile_session_aliases(timestamp, payload_file, &ralplan_paths, RALPLAN, payload)?;
    if let Some(mut state) = load_json(&ralplan_paths.session_file) {
        if let Some(decision) = ralplan_prompt::apply_ralplan_prompt_decision(
            timestamp,
            &mut state,
            &prompt,
            payload_file,
        ) {
            let session_id = state
                .get("session_id")
                .map(value_to_string)
                .unwrap_or_else(|| "unknown-session".to_string());
            write_json_atomic(&ralplan_paths.session_file, &state)?;
            append_jsonl(
                &ralplan_paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": decision.event,
                    "session_id": session_id,
                    "handoff_target": decision.handoff_target,
                    "plan_id": state.get("plan_id").cloned().unwrap_or(Value::Null),
                    "plan_sha256": state.get("plan_sha256").cloned().unwrap_or(Value::Null),
                    "payload": payload_file,
                }),
            )?;
            return Ok(0);
        }
    }

    if ralplan_prompt::is_deep_interview_approval_for_ralplan(&prompt) {
        let mut state = load_json(&ralplan_paths.session_file)
            .unwrap_or_else(|| new_state(RALPLAN, timestamp, &ralplan_paths.session_id, payload));
        require_ralplan_input_lock(timestamp, &mut state, payload_file);
        let session_id = state
            .get("session_id")
            .map(value_to_string)
            .unwrap_or_else(|| "unknown-session".to_string());
        write_json_atomic(&ralplan_paths.session_file, &state)?;
        append_jsonl(
            &ralplan_paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "input_lock_required",
                "session_id": session_id,
                "required_workflow": DEEP_INTERVIEW,
                "payload": payload_file,
            }),
        )?;
        return Ok(0);
    }

    let deep_paths = workflow_paths(state_dir, payload, DEEP_INTERVIEW);
    reconcile_session_aliases(
        timestamp,
        payload_file,
        &deep_paths,
        DEEP_INTERVIEW,
        payload,
    )?;
    if let Some(mut state) = load_json(&deep_paths.session_file) {
        if !is_current_active_state(&state, payload) {
            return Ok(0);
        }
        if let Some(question_id) =
            answer_pending_question(timestamp, &mut state, &prompt, payload_file)
        {
            let session_id = state
                .get("session_id")
                .map(value_to_string)
                .unwrap_or_else(|| "unknown-session".to_string());
            write_json_atomic(&deep_paths.session_file, &state)?;
            append_jsonl(
                &deep_paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "question_answered",
                    "session_id": session_id,
                    "question_id": question_id,
                    "payload": payload_file,
                }),
            )?;
            return Ok(0);
        }
    }
    Ok(0)
}

fn is_current_active_state(state: &Value, payload: &Value) -> bool {
    if state
        .get("active")
        .and_then(Value::as_bool)
        .is_some_and(|active| !active)
    {
        return false;
    }

    let state_cwd = state.get("cwd").and_then(Value::as_str);
    let payload_cwd = payload.get("cwd").and_then(Value::as_str);
    match (state_cwd, payload_cwd) {
        (Some(state_cwd), Some(payload_cwd)) => state_cwd == payload_cwd,
        _ => true,
    }
}
