use super::*;

pub(super) fn handle_user_prompt(
    timestamp: &str,
    state_dir: &Path,
    _options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let Some(prompt) = runtime_input::effective_prompt_from_payload(payload) else {
        return Ok(0);
    };
    if prompt.trim().is_empty() {
        return Ok(0);
    }
    if is_subagent_prompt(payload) {
        return Ok(0);
    }

    let workflow_start = subagent_gate::workflow_start_from_prompt(&prompt);

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
        if let Some(spec) = ralplan_input::linked_deep_interview_spec(&ralplan_paths) {
            mark_ralplan_input_lock_ready(timestamp, &mut state);
            state["input_spec_path"] = json!(spec.path);
            state["input_spec_sha256"] = json!(spec.sha256);
            state["input_spec_persisted_at"] = json!(spec.persisted_at);
        }
        subagent_gate::register_requirement(timestamp, &mut state, RALPLAN, payload_file);
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
        subagent_gate::print_user_prompt_context(RALPLAN)?;
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
        if is_current_active_state(&state, payload) {
            let pending_before = state.get("pending_question").cloned();
            if let Some(question_id) =
                answer_pending_question(timestamp, &mut state, &prompt, payload_file)
            {
                if let Some(event) = deep_interview_milestone::apply_answer(
                    timestamp,
                    &mut state,
                    pending_before.as_ref(),
                    &prompt,
                    payload_file,
                ) {
                    append_jsonl(
                        &deep_paths.events_file,
                        &json!({
                            "timestamp": timestamp,
                            "event": event,
                            "session_id": deep_paths.session_id,
                            "question_id": question_id,
                            "payload": payload_file,
                        }),
                    )?;
                }
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
                let mut contexts = Vec::new();
                if let Some(context) = deep_interview_milestone::answer_continuation_context(
                    &state,
                    pending_before.as_ref(),
                ) {
                    contexts.push(context);
                }
                if let Some(context) =
                    subagent_gate::answer_continuation_context(DEEP_INTERVIEW, &state)
                {
                    contexts.push(context);
                }
                if !contexts.is_empty() {
                    print_additional_context(&contexts.join("\n\n"))?;
                }
                return Ok(0);
            }
        }
    }
    if let Some(workflow) = workflow_start {
        let paths = workflow_paths(state_dir, payload, workflow);
        reconcile_session_aliases(timestamp, payload_file, &paths, workflow, payload)?;
        let mut state = load_json(&paths.session_file)
            .unwrap_or_else(|| new_state(workflow, timestamp, &paths.session_id, payload));
        if workflow == RALPLAN {
            if let Some(spec) = ralplan_input::linked_deep_interview_spec(&paths) {
                require_ralplan_input_lock(timestamp, &mut state, payload_file);
                mark_ralplan_input_lock_ready(timestamp, &mut state);
                state["input_spec_path"] = json!(spec.path);
                state["input_spec_sha256"] = json!(spec.sha256);
                state["input_spec_persisted_at"] = json!(spec.persisted_at);
            }
        }
        if workflow == TEAM {
            team::register_requirement(timestamp, &mut state, payload, &prompt, payload_file);
        } else {
            subagent_gate::register_requirement(timestamp, &mut state, workflow, payload_file);
        }
        let session_id = state
            .get("session_id")
            .map(value_to_string)
            .unwrap_or_else(|| "unknown-session".to_string());
        write_json_atomic(&paths.session_file, &state)?;
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "subagent_orchestration_required",
                "session_id": session_id,
                "workflow": workflow,
                "payload": payload_file,
            }),
        )?;
        if workflow == TEAM {
            team::print_user_prompt_context(payload, &prompt, &state)?;
        } else {
            subagent_gate::print_user_prompt_context(workflow)?;
        }
    }
    Ok(0)
}

fn print_additional_context(context: &str) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": context,
            }
        }))?
    );
    Ok(())
}

fn is_subagent_prompt(payload: &Value) -> bool {
    payload.get("agent_id").is_some() || payload.get("subagent_id").is_some()
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
