use super::*;

pub(super) fn reconcile_session_aliases(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    skill: &str,
    payload: &Value,
) -> Result<()> {
    for alias_id in state_paths::session_alias_ids(payload) {
        let alias_file = paths
            .workflow_dir
            .join(format!("{}.json", safe_part(&alias_id)));
        if alias_file == paths.session_file || !alias_file.exists() {
            continue;
        }

        let Some(mut alias_state) = load_json(&alias_file) else {
            continue;
        };
        if alias_state.get("skill").and_then(Value::as_str) != Some(skill) {
            continue;
        }
        if !same_cwd_scope(&alias_state, payload) {
            continue;
        }

        let mut state = load_json(&paths.session_file)
            .unwrap_or_else(|| new_state(skill, timestamp, &paths.session_id, payload));
        if !same_state_cwd(&state, &alias_state) {
            continue;
        }

        merge_alias_state(
            timestamp,
            &mut state,
            &alias_state,
            &alias_id,
            &paths.session_id,
        );
        mark_alias_superseded(timestamp, &mut alias_state, &paths.session_id);
        write_json_atomic(&paths.session_file, &state)?;
        write_json_atomic(&alias_file, &alias_state)?;
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "session_alias_superseded",
                "session_id": alias_id,
                "canonical_session_id": paths.session_id,
                "path": alias_file,
                "payload": payload_file,
            }),
        )?;
    }
    Ok(())
}

fn same_cwd_scope(state: &Value, payload: &Value) -> bool {
    let Some(payload_cwd) = payload.get("cwd") else {
        return true;
    };
    match state.get("cwd") {
        Some(state_cwd) if !state_cwd.is_null() => state_cwd == payload_cwd,
        _ => true,
    }
}

fn same_state_cwd(state: &Value, alias_state: &Value) -> bool {
    let state_cwd = state.get("cwd").filter(|value| !value.is_null());
    let alias_cwd = alias_state.get("cwd").filter(|value| !value.is_null());
    match (state_cwd, alias_cwd) {
        (Some(state_cwd), Some(alias_cwd)) => state_cwd == alias_cwd,
        _ => true,
    }
}

fn merge_alias_state(
    timestamp: &str,
    state: &mut Value,
    alias_state: &Value,
    alias_id: &str,
    canonical_id: &str,
) {
    state["session_id"] = json!(canonical_id);
    merge_alias_ids(state, alias_state, alias_id);
    merge_questions(state, alias_state);
    if state.get("pending_question").is_none_or(Value::is_null) {
        if let Some(pending) = alias_state.get("pending_question") {
            if pending.get("status").and_then(Value::as_str) == Some("pending") {
                state["pending_question"] = pending.clone();
            }
        }
    }
    state["updated_at"] = json!(timestamp);
}

fn merge_alias_ids(state: &mut Value, alias_state: &Value, alias_id: &str) {
    let mut aliases = state
        .get("session_aliases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    push_json_string(&mut aliases, alias_id);
    if let Some(existing) = alias_state.get("session_aliases").and_then(Value::as_array) {
        for alias in existing {
            if let Some(alias) = alias.as_str() {
                push_json_string(&mut aliases, alias);
            }
        }
    }
    state["session_aliases"] = json!(aliases);
}

fn push_json_string(values: &mut Vec<Value>, candidate: &str) {
    if values.iter().any(|value| value.as_str() == Some(candidate)) {
        return;
    }
    values.push(json!(candidate));
}

fn merge_questions(state: &mut Value, alias_state: &Value) {
    let mut questions = state
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut existing_ids = questions
        .iter()
        .filter_map(|question| question.get("id").map(value_to_string))
        .collect::<Vec<_>>();

    let Some(alias_questions) = alias_state.get("questions").and_then(Value::as_array) else {
        return;
    };
    for question in alias_questions {
        let Some(id) = question.get("id").map(value_to_string) else {
            continue;
        };
        if existing_ids.contains(&id) {
            continue;
        }
        questions.push(question.clone());
        existing_ids.push(id);
    }
    state["questions"] = json!(questions);
}

fn mark_alias_superseded(timestamp: &str, state: &mut Value, canonical_id: &str) {
    if let Some(pending) = state.get_mut("pending_question") {
        if pending.get("status").and_then(Value::as_str) == Some("pending") {
            pending["status"] = json!("stale");
            pending["stale_at"] = json!(timestamp);
            pending["stale_superseded_by"] = json!(canonical_id);
        }
    }
    state["active"] = json!(false);
    state["phase"] = json!("stale");
    state["status"] = json!("stale");
    state["stale_at"] = json!(timestamp);
    state["stale_reason"] = json!("merged into canonical session state");
    state["stale_superseded_by"] = json!(canonical_id);
    state["canonical_session_id"] = json!(canonical_id);
    state["updated_at"] = json!(timestamp);
}
