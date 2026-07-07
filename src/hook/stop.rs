use super::*;

pub(super) fn handle_stop(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let text = runtime_input::assistant_message_from_payload(payload).unwrap_or_default();
    let text = text.as_str();

    if let Some(reason) =
        block_visible_runtime_reference(timestamp, state_dir, payload, payload_file, text)?
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

    if let Some(reason) =
        git_guard::block_completion_if_needed(timestamp, state_dir, payload, payload_file, text)?
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

    for review in review_passes_from_text(text) {
        let paths = workflow_paths(state_dir, payload, RALPLAN);
        reconcile_session_aliases(timestamp, payload_file, &paths, RALPLAN, payload)?;
        let mut state = load_json(&paths.session_file)
            .unwrap_or_else(|| new_state(RALPLAN, timestamp, &paths.session_id, payload));
        persist_ralplan_review(timestamp, payload_file, &paths, review, &mut state)?;
        write_json_atomic(&paths.session_file, &state)?;
    }

    if let Some(terminal) = workflow_state_from_text(text) {
        let paths = workflow_paths(state_dir, payload, &terminal.skill);
        reconcile_session_aliases(timestamp, payload_file, &paths, &terminal.skill, payload)?;
        let mut state = load_json(&paths.session_file)
            .unwrap_or_else(|| new_state(&terminal.skill, timestamp, &paths.session_id, payload));
        if terminal.skill == DEEP_INTERVIEW {
            if let Some(reason) = subagent_gate::block_terminal_if_missing_receipts(
                timestamp,
                payload_file,
                &paths,
                &terminal,
                &mut state,
            )? {
                write_json_atomic(&paths.session_file, &state)?;
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "decision": "block",
                        "reason": reason,
                    }))?
                );
                return Ok(0);
            }
        }
        match terminal.skill.as_str() {
            DEEP_INTERVIEW => terminal::handle_deep_interview_terminal(
                timestamp,
                text,
                payload_file,
                &paths,
                &terminal,
                &mut state,
            )?,
            RALPLAN => terminal::handle_ralplan_terminal(
                timestamp,
                text,
                payload_file,
                &paths,
                &terminal,
                &mut state,
            )?,
            ULTRAGOAL => terminal::handle_generic_terminal(
                timestamp,
                payload_file,
                &paths,
                &terminal,
                &mut state,
            )?,
            _ => return Ok(0),
        }
        write_json_atomic(&paths.session_file, &state)?;
        if terminal.skill == DEEP_INTERVIEW
            && !state.get("active").and_then(Value::as_bool).unwrap_or(true)
        {
            mark_stale_deep_interview_peers(timestamp, payload_file, &paths, &state)?;
        }
        return Ok(0);
    }

    let Some(mut question) = question_from_text(timestamp, text, payload_file) else {
        return Ok(0);
    };
    let paths = workflow_paths(state_dir, payload, DEEP_INTERVIEW);
    reconcile_session_aliases(timestamp, payload_file, &paths, DEEP_INTERVIEW, payload)?;
    let mut state = load_json(&paths.session_file)
        .unwrap_or_else(|| new_state(DEEP_INTERVIEW, timestamp, &paths.session_id, payload));
    let question_decision = deep_interview_milestone::prepare_question(&state, text, &mut question);
    if let Some(ambiguity) = question.get("ambiguity").cloned() {
        state["ambiguity"] = ambiguity;
    }
    match question_decision {
        deep_interview_milestone::QuestionDecision::Allow => {}
        deep_interview_milestone::QuestionDecision::Block { reason, kind } => {
            state["active"] = json!(true);
            let event = match kind {
                deep_interview_milestone::QuestionBlockKind::MilestoneDecision => {
                    state["phase"] = json!("milestone_decision_required");
                    state["status"] = json!("milestone_decision_required");
                    state["milestone_blocked_at"] = json!(timestamp);
                    "milestone_decision_required"
                }
                deep_interview_milestone::QuestionBlockKind::OrdinaryQuestion => {
                    state["phase"] = json!("interviewing");
                    state["status"] = json!("interviewing");
                    state["ordinary_question_blocked_at"] = json!(timestamp);
                    "ordinary_question_required"
                }
                deep_interview_milestone::QuestionBlockKind::CrystallizedSpec => {
                    state["phase"] = json!("crystallizing");
                    state["status"] = json!("crystallizing");
                    state["crystallization_blocked_at"] = json!(timestamp);
                    "crystallized_spec_required"
                }
            };
            state["updated_at"] = json!(timestamp);
            write_json_atomic(&paths.session_file, &state)?;
            append_jsonl(
                &paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": event,
                    "session_id": paths.session_id,
                    "payload": payload_file,
                }),
            )?;
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "decision": "block",
                    "reason": reason,
                }))?
            );
            return Ok(0);
        }
    }

    let question_id = question
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let round = question.get("round").cloned().unwrap_or(Value::Null);
    let component = question.get("component").cloned().unwrap_or(Value::Null);
    let dimension = question.get("dimension").cloned().unwrap_or(Value::Null);
    upsert_question(timestamp, &mut state, question);
    let pending_question = state
        .get("pending_question")
        .cloned()
        .unwrap_or(Value::Null);
    deep_interview_milestone::mark_pending_state(timestamp, &mut state, &pending_question);
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "question_pending",
            "session_id": paths.session_id,
            "question_id": question_id,
            "round": round,
            "component": component,
            "dimension": dimension,
            "payload": payload_file,
        }),
    )?;

    write_json_atomic(&paths.session_file, &state)?;
    Ok(0)
}

fn mark_stale_deep_interview_peers(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal_state: &Value,
) -> Result<()> {
    let cwd = terminal_state.get("cwd").cloned().unwrap_or(Value::Null);
    if !paths.workflow_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&paths.workflow_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path == paths.session_file || !path.is_file() {
            continue;
        }
        if path
            .extension()
            .is_some_and(|extension| extension != "json")
        {
            continue;
        }

        let Some(mut state) = load_json(&path) else {
            continue;
        };
        let same_cwd = state.get("cwd").cloned().unwrap_or(Value::Null) == cwd;
        let active_pending = state
            .get("active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && state
                .get("pending_question")
                .and_then(|pending| pending.get("status"))
                .and_then(Value::as_str)
                == Some("pending");
        if !same_cwd || !active_pending {
            continue;
        }

        if let Some(pending) = state.get_mut("pending_question") {
            pending["status"] = json!("stale");
            pending["stale_at"] = json!(timestamp);
            pending["stale_superseded_by"] = json!(paths.session_id);
        }
        state["active"] = json!(false);
        state["phase"] = json!("stale");
        state["status"] = json!("stale");
        state["stale_at"] = json!(timestamp);
        state["stale_reason"] = json!("superseded by terminal deep-interview state in same cwd");
        state["stale_superseded_by"] = json!(paths.session_id);
        state["updated_at"] = json!(timestamp);
        write_json_atomic(&path, &state)?;
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "stale_state_closed",
                "session_id": state.get("session_id").cloned().unwrap_or(Value::Null),
                "superseded_by": paths.session_id,
                "path": path,
                "payload": payload_file,
            }),
        )?;
    }
    Ok(())
}

fn block_visible_runtime_reference(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
    text: &str,
) -> Result<Option<String>> {
    let visible = parser::text_before_first_workflow_block(text);
    if !contains_runtime_reference(&visible) || !has_workflow_state(state_dir, payload) {
        return Ok(None);
    }

    let paths = workflow_paths(
        state_dir,
        payload,
        workflow_with_state(state_dir, payload).unwrap_or(ULTRAGOAL),
    );
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "visible_runtime_reference_blocked",
            "session_id": paths.session_id,
            "payload": payload_file,
        }),
    )?;
    Ok(Some(
        "Megara runtime artifact or state paths are internal. Rewrite the response without .megara/.agents runtime paths, session ids, receipts, checkpoints, or quality-gate file links. Summarize only product-facing changes, verification commands, pass/fail results, and user-actionable blockers."
            .to_string(),
    ))
}

fn contains_runtime_reference(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    [
        ".megara/artifacts",
        ".megara/state",
        "~/.megara/artifacts",
        "~/.megara/state",
        "/.megara/artifacts",
        "/.megara/state",
        "<hook_prompt",
        "megara deep-interview reached",
        "internal megara workflow instruction",
        "keep this runtime instruction internal",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn has_workflow_state(state_dir: &Path, payload: &Value) -> bool {
    workflow_with_state(state_dir, payload).is_some()
}

fn workflow_with_state(state_dir: &Path, payload: &Value) -> Option<&'static str> {
    WORKFLOWS.iter().copied().find(|workflow| {
        workflow_paths(state_dir, payload, workflow)
            .session_file
            .exists()
    })
}
