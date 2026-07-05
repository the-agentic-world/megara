use super::*;

pub(super) fn handle_pre_tool_use(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let guard_mode = env::var("MEGARA_MUTATION_GUARD").unwrap_or_else(|_| "block".to_string());
    if guard_mode == "off" {
        return Ok(0);
    }
    if let Some(mutation) = protected_workflow_state_mutation(payload) {
        append_jsonl(
            &state_dir.join("events.jsonl"),
            &json!({
                "timestamp": timestamp,
                "event": "protected_workflow_state_mutation_blocked",
                "mutation_kind": mutation.kind,
                "mutation_value": mutation.value,
                "payload": payload_file,
            }),
        )?;
        eprintln!(
            "MEGARA mutation guard: workflow state is managed by Megara hooks. Do not edit .agents/state/workflows/deep-interview or .agents/state/workflows/ralplan directly."
        );
        return if guard_mode == "warn" { Ok(0) } else { Ok(42) };
    }
    let Some(mutation) = mutation_signal(payload) else {
        return Ok(0);
    };
    let Some((skill, state, events_file)) =
        active_workflow_state(timestamp, state_dir, payload, payload_file)?
    else {
        if let Some((state, events_file)) =
            pending_deep_interview_pipeline_lock(timestamp, state_dir, payload, payload_file)?
        {
            let session_id = state
                .get("session_id")
                .map(value_to_string)
                .unwrap_or_else(|| "unknown-session".to_string());
            append_jsonl(
                &events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "pipeline_lock_mutation_blocked",
                    "session_id": session_id,
                    "skill": DEEP_INTERVIEW,
                    "phase": state.get("phase").cloned().unwrap_or(Value::Null),
                    "mutation_kind": mutation.kind,
                    "mutation_value": mutation.value,
                    "payload": payload_file,
                }),
            )?;
            eprintln!(
                "MEGARA mutation guard: deep-interview is crystallized. Start ralplan from the locked spec before mutating files."
            );
            return if guard_mode == "warn" { Ok(0) } else { Ok(42) };
        }
        return Ok(0);
    };

    let session_id = state
        .get("session_id")
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    append_jsonl(
        &events_file,
        &json!({
            "timestamp": timestamp,
            "event": "mutation_blocked",
            "session_id": session_id,
            "skill": skill,
            "phase": state.get("phase").cloned().unwrap_or(Value::Null),
            "mutation_kind": mutation.kind,
            "mutation_value": mutation.value,
            "payload": payload_file,
        }),
    )?;

    let guidance = if skill == ULTRAGOAL {
        "run `MEGARA_BIN=\"${MEGARA_BIN:-.agents/bin/megara}\"; \"$MEGARA_BIN\" ultragoal complete-goals` and enter an active goal before mutating files"
    } else {
        "approve, refine, complete, or cancel the workflow before mutating files"
    };
    eprintln!("MEGARA mutation guard: {skill} is active. {guidance}.");
    if guard_mode == "warn" {
        Ok(0)
    } else {
        Ok(42)
    }
}

fn active_workflow_state(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<Option<(&'static str, Value, PathBuf)>> {
    for &skill in MUTATION_GUARD_WORKFLOWS {
        let paths = workflow_paths(state_dir, payload, skill);
        reconcile_session_aliases(timestamp, payload_file, &paths, skill, payload)?;
        if let Some(state) = load_json(&paths.session_file) {
            if mutation_guard_applies(skill, &state) {
                return Ok(Some((skill, state, paths.events_file)));
            }
        }
    }
    Ok(None)
}

fn mutation_guard_applies(skill: &'static str, state: &Value) -> bool {
    if state.get("active").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    if skill != ULTRAGOAL {
        return true;
    }
    matches!(
        state.get("phase").and_then(Value::as_str),
        Some("goal_planning" | "planning" | "initialized" | "handoff")
    )
}

fn pending_deep_interview_pipeline_lock(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<Option<(Value, PathBuf)>> {
    let paths = workflow_paths(state_dir, payload, DEEP_INTERVIEW);
    reconcile_session_aliases(timestamp, payload_file, &paths, DEEP_INTERVIEW, payload)?;
    let Some(state) = load_json(&paths.session_file) else {
        return Ok(None);
    };
    if !deep_interview_pipeline_lock_applies(&state) {
        return Ok(None);
    }
    if ralplan_has_taken_over(timestamp, state_dir, payload, payload_file)? {
        return Ok(None);
    }
    Ok(Some((state, paths.events_file)))
}

fn deep_interview_pipeline_lock_applies(state: &Value) -> bool {
    state.get("phase").and_then(Value::as_str) == Some("crystallized")
        && state
            .get("pipeline_lock")
            .and_then(|lock| lock.get("status"))
            .and_then(Value::as_str)
            == Some("pending_ralplan")
}

fn ralplan_has_taken_over(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<bool> {
    let paths = workflow_paths(state_dir, payload, RALPLAN);
    reconcile_session_aliases(timestamp, payload_file, &paths, RALPLAN, payload)?;
    let Some(state) = load_json(&paths.session_file) else {
        return Ok(false);
    };
    if state.get("approval_status").and_then(Value::as_str) == Some("approved")
        || state.get("phase").and_then(Value::as_str) == Some("approved")
    {
        return Ok(true);
    }
    Ok(state.get("active").and_then(Value::as_bool) == Some(true))
}
