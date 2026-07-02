use super::*;

pub(super) fn handle_stop(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let text = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or_default();

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

    let Some(question) = question_from_text(timestamp, text, payload_file) else {
        return Ok(0);
    };
    let paths = workflow_paths(state_dir, payload, DEEP_INTERVIEW);
    reconcile_session_aliases(timestamp, payload_file, &paths, DEEP_INTERVIEW, payload)?;
    let mut state = load_json(&paths.session_file)
        .unwrap_or_else(|| new_state(DEEP_INTERVIEW, timestamp, &paths.session_id, payload));

    let question_id = question
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let round = question.get("round").cloned().unwrap_or(Value::Null);
    let component = question.get("component").cloned().unwrap_or(Value::Null);
    let dimension = question.get("dimension").cloned().unwrap_or(Value::Null);
    upsert_question(timestamp, &mut state, question);
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
