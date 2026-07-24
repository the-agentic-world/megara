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
    if is_subagent_prompt(payload) {
        return Ok(0);
    }
    let system_message = codex_version::outdated_notice_once(
        state_dir,
        payload,
        runtime_input::runtime_context(payload).surface,
    );

    let workflow_start = subagent_gate::workflow_start_from_prompt(&prompt);
    let explicit_ultragoal_start = subagent_gate::ultragoal_start_from_prompt(&prompt);

    let ralplan_paths = workflow_paths(state_dir, payload, RALPLAN);
    reconcile_session_aliases(timestamp, payload_file, &ralplan_paths, RALPLAN, payload)?;
    if let Some(mut state) = load_json(&ralplan_paths.session_file) {
        let ultragoal_paths = workflow_paths(state_dir, payload, ULTRAGOAL);
        reconcile_session_aliases(
            timestamp,
            payload_file,
            &ultragoal_paths,
            ULTRAGOAL,
            payload,
        )?;
        let ultragoal_is_active = load_json(&ultragoal_paths.session_file)
            .as_ref()
            .is_some_and(|state| state.get("active").and_then(Value::as_bool) == Some(true));
        if transition::ultragoal_start_pending(&state)
            || (explicit_ultragoal_start
                && !ultragoal_is_active
                && transition::ultragoal_start_recoverable(&state))
        {
            transition::mark_ultragoal_continuation_delivered(timestamp, &mut state);
            write_json_atomic(&ralplan_paths.session_file, &state)?;
            let session_id = state
                .get("session_id")
                .map(value_to_string)
                .unwrap_or_else(|| ralplan_paths.session_id.clone());
            let context = transition::ultragoal_start_context(options.scope, &session_id);
            print_user_prompt_output(Some(&context), system_message.as_deref())?;
            return Ok(0);
        }
        if let Some(decision) = ralplan_prompt::apply_ralplan_prompt_decision(
            timestamp,
            &mut state,
            &prompt,
            payload_file,
        ) {
            let handoff_target = decision.handoff_target.clone();
            if handoff_target.as_str() == Some(ULTRAGOAL) {
                transition::prepare_ultragoal(timestamp, &mut state);
                transition::mark_ultragoal_continuation_delivered(timestamp, &mut state);
            }
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
            if handoff_target.as_str() == Some(TEAM) {
                let context = start_team_from_ralplan_handoff(
                    timestamp,
                    state_dir,
                    payload,
                    payload_file,
                    &state,
                    &prompt,
                )?;
                print_user_prompt_output(Some(&context), system_message.as_deref())?;
            } else if handoff_target.as_str() == Some(ULTRAGOAL) {
                print_user_prompt_output(
                    Some(&transition::ultragoal_start_context(
                        options.scope,
                        &session_id,
                    )),
                    system_message.as_deref(),
                )?;
            } else {
                print_user_prompt_output(None, system_message.as_deref())?;
            }
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
        print_user_prompt_output(
            subagent_gate::additional_context(RALPLAN),
            system_message.as_deref(),
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
        if transition::pending_ralplan_continuation(&state) {
            transition::mark_ralplan_continuation_delivered(timestamp, &mut state);
            write_json_atomic(&deep_paths.session_file, &state)?;
            let context = format!(
                "{}\n\n{}",
                transition::ralplan_start_reason(),
                subagent_gate::additional_context(RALPLAN).unwrap_or_default()
            );
            print_user_prompt_output(Some(&context), system_message.as_deref())?;
            return Ok(0);
        }
        if is_current_active_state(&state, payload) {
            let pending_before = state.get("pending_question").cloned();
            if let Some(question_id) =
                answer_pending_question(timestamp, &mut state, &prompt, payload_file)
            {
                let milestone_answer = pending_before.as_ref().is_some_and(|question| {
                    question.get("kind").and_then(Value::as_str) == Some("milestone_decision")
                });
                if !milestone_answer {
                    deep_interview_reassessment::begin(
                        timestamp,
                        &mut state,
                        pending_before.as_ref(),
                        &prompt,
                        payload_file,
                    );
                    subagent_gate::prepare_deep_interview_reassessment_review(
                        timestamp,
                        &mut state,
                        payload_file,
                    );
                }
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
                if let Some(context) = deep_interview_reassessment::continuation_context(&state) {
                    contexts.push(context);
                }
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
                let context = (!contexts.is_empty()).then(|| contexts.join("\n\n"));
                print_user_prompt_output(context.as_deref(), system_message.as_deref())?;
                return Ok(0);
            }
            if let Some(context) =
                subagent_gate::answer_continuation_context(DEEP_INTERVIEW, &state)
            {
                print_user_prompt_output(Some(&context), system_message.as_deref())?;
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
            team::register_requirement(
                timestamp,
                state_dir,
                &mut state,
                payload,
                &prompt,
                payload_file,
            );
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
            let context = team::additional_context(payload, &prompt, &state);
            print_user_prompt_output(Some(&context), system_message.as_deref())?;
        } else {
            print_user_prompt_output(
                subagent_gate::additional_context(workflow),
                system_message.as_deref(),
            )?;
        }
        return Ok(0);
    }
    print_user_prompt_output(None, system_message.as_deref())?;
    Ok(0)
}

fn print_user_prompt_output(
    additional_context: Option<&str>,
    system_message: Option<&str>,
) -> Result<()> {
    if additional_context.is_none() && system_message.is_none() {
        return Ok(());
    }
    let mut output = Map::new();
    if let Some(system_message) = system_message {
        output.insert("systemMessage".to_string(), json!(system_message));
    }
    if let Some(additional_context) = additional_context {
        output.insert(
            "hookSpecificOutput".to_string(),
            json!({
                "hookEventName": "UserPromptSubmit",
                "additionalContext": additional_context,
            }),
        );
    }
    println!("{}", serde_json::to_string(&Value::Object(output))?);
    Ok(())
}

fn start_team_from_ralplan_handoff(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
    ralplan_state: &Value,
    approval_prompt: &str,
) -> Result<String> {
    let paths = workflow_paths(state_dir, payload, TEAM);
    reconcile_session_aliases(timestamp, payload_file, &paths, TEAM, payload)?;
    let mut state = load_json(&paths.session_file)
        .unwrap_or_else(|| new_state(TEAM, timestamp, &paths.session_id, payload));
    let team_prompt =
        approved_plan_text(ralplan_state).unwrap_or_else(|| approval_prompt.trim().to_string());

    team::register_requirement(
        timestamp,
        state_dir,
        &mut state,
        payload,
        &team_prompt,
        payload_file,
    );
    state["source_workflow"] = json!(RALPLAN);
    if let Some(plan_id) = ralplan_state.get("approved_plan_id").cloned() {
        state["source_plan_id"] = plan_id;
    } else if let Some(plan_id) = ralplan_state.get("plan_id").cloned() {
        state["source_plan_id"] = plan_id;
    }
    if let Some(plan_sha256) = ralplan_state.get("approved_plan_sha256").cloned() {
        state["source_plan_sha256"] = plan_sha256;
    } else if let Some(plan_sha256) = ralplan_state.get("plan_sha256").cloned() {
        state["source_plan_sha256"] = plan_sha256;
    }
    state["source_handoff_target"] = json!(TEAM);

    let session_id = state
        .get("session_id")
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    write_json_atomic(&paths.session_file, &state)?;
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "ralplan_team_handoff_started",
            "session_id": session_id,
            "workflow": TEAM,
            "source_workflow": RALPLAN,
            "source_plan_id": state.get("source_plan_id").cloned().unwrap_or(Value::Null),
            "source_plan_sha256": state.get("source_plan_sha256").cloned().unwrap_or(Value::Null),
            "payload": payload_file,
        }),
    )?;
    Ok(team::additional_context(payload, &team_prompt, &state))
}

fn approved_plan_text(ralplan_state: &Value) -> Option<String> {
    let plan_path = ralplan_state.get("plan_path").and_then(Value::as_str)?;
    fs::read_to_string(plan_path)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
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
