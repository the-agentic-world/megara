use super::*;

pub(super) fn handle_deep_interview_terminal(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    let spec = persist_crystallized_spec(
        timestamp,
        &paths.artifact_dir,
        &paths.session_id,
        terminal,
        text,
        payload_file,
    )?;
    if terminal.status == "crystallized" && spec.is_none() {
        reject_crystallized_without_spec(timestamp, state);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "spec_missing",
                "session_id": paths.session_id,
                "status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        return Ok(());
    }

    update_terminal_state(timestamp, state, terminal, spec.as_ref());
    let suggested_next = record_next_workflow_suggestion(timestamp, terminal, state);
    let mut entry = json!({
        "timestamp": timestamp,
        "event": "workflow_state",
        "session_id": paths.session_id,
        "skill": terminal.skill,
        "status": terminal.status,
        "payload": payload_file,
    });
    if let Some(spec) = spec {
        entry["spec_path"] = json!(spec.path);
        entry["spec_sha256"] = json!(spec.sha256);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "spec_persisted",
                "session_id": paths.session_id,
                "path": spec.path,
                "sha256": spec.sha256,
                "payload": payload_file,
            }),
        )?;
    }
    if let Some(next_workflow) = suggested_next {
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "next_workflow_suggested",
                "session_id": paths.session_id,
                "workflow": next_workflow,
                "source_status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        entry["next_workflow_suggestion"] = state["next_workflow_suggestion"].clone();
    }
    append_jsonl(&paths.events_file, &entry)
}

pub(super) fn handle_ralplan_terminal(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    ralplan_context::handle_ralplan_terminal(timestamp, text, payload_file, paths, terminal, state)
}

pub(super) fn handle_generic_terminal(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    update_terminal_state(timestamp, state, terminal, None);
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "workflow_state",
            "session_id": paths.session_id,
            "skill": terminal.skill,
            "status": terminal.status,
            "payload": payload_file,
        }),
    )
}

fn record_next_workflow_suggestion(
    timestamp: &str,
    terminal: &TerminalState,
    state: &mut Value,
) -> Option<String> {
    if terminal.skill != DEEP_INTERVIEW
        || terminal.status != "crystallized"
        || terminal.next != RALPLAN
    {
        return None;
    }
    state["next_workflow_suggestion"] = json!({
        "workflow": RALPLAN,
        "status": "suggested",
        "suggested_at": timestamp,
        "implementation_allowed_now": false,
    });
    state["pipeline_lock"] = json!({
        "workflow": RALPLAN,
        "status": "pending_ralplan",
        "created_at": timestamp,
        "implementation_allowed_now": false,
        "unlock_condition": "ralplan_pending_or_approved",
    });
    Some(RALPLAN.to_string())
}
