use super::*;

pub(super) fn handle_subagent_event(
    timestamp: &str,
    state_dir: &Path,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let entry = subagent_event_entry(timestamp, options, payload, payload_file);
    append_jsonl(&state_dir.join("subagents.jsonl"), &entry)?;
    attach_to_workflow_state(timestamp, state_dir, options, payload, payload_file, &entry)?;
    Ok(0)
}

fn subagent_event_entry(
    timestamp: &str,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Value {
    let mut entry = Map::new();
    entry.insert("timestamp".to_string(), json!(timestamp));
    entry.insert("runtime".to_string(), json!(options.runtime));
    entry.insert("event".to_string(), json!(options.event));
    entry.insert("matcher".to_string(), json!(options.matcher));
    entry.insert("payload".to_string(), json!(payload_file));
    for key in [
        "session_id",
        "thread_id",
        "turn_id",
        "transcript_path",
        "cwd",
        "model",
        "agent_id",
        "agent_type",
        "subagent_id",
        "subagent_name",
        "name",
        "role",
    ] {
        if let Some(value) = payload.get(key) {
            entry.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(entry)
}

fn attach_to_workflow_state(
    timestamp: &str,
    state_dir: &Path,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
    event: &Value,
) -> Result<()> {
    for &skill in WORKFLOWS {
        let paths = workflow_paths(state_dir, payload, skill);
        reconcile_session_aliases(timestamp, payload_file, &paths, skill, payload)?;
        let Some(mut state) = load_json(&paths.session_file) else {
            continue;
        };
        if !same_cwd_scope(&state, payload) {
            continue;
        }
        state["last_subagent_event"] = event.clone();
        match options.event.as_str() {
            "SubagentStart" => {
                subagent_gate::record_start(timestamp, &mut state, payload, payload_file);
            }
            "SubagentStop" => {
                subagent_gate::record_stop_receipt(timestamp, &mut state, payload, payload_file);
                if skill == RALPLAN {
                    for review in review_passes_from_subagent_payload(payload) {
                        persist_ralplan_review(
                            timestamp,
                            payload_file,
                            &paths,
                            review,
                            &mut state,
                        )?;
                    }
                }
            }
            _ => {}
        }
        state["updated_at"] = json!(timestamp);
        write_json_atomic(&paths.session_file, &state)?;
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "subagent_event",
                "session_id": paths.session_id,
                "skill": skill,
                "subagent_event": event.get("event").cloned().unwrap_or(Value::Null),
                "subagent_payload": payload_file,
            }),
        )?;
    }
    Ok(())
}

fn review_passes_from_subagent_payload(payload: &Value) -> Vec<parser::ReviewPass> {
    payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .map(review_passes_from_text)
        .unwrap_or_default()
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
