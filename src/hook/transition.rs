use super::*;

const CONTRACT_VERSION: u64 = 1;
const RALPLAN_START_REASON: &str = "Start ralplan now from the crystallized specification. Do not ask for another workflow approval and do not implement yet. Produce the reviewed, verification-ready plan and its execution choices.";
const RALPLAN_CONFLICT_REASON: &str = "Ralplan could not start because this session already has a different planning state. Resume or finish that plan before retrying this transition.";

pub(super) fn already_started(state: &Value, target: &str) -> bool {
    state.get("transition").is_some_and(|transition| {
        transition.get("target").and_then(Value::as_str) == Some(target)
            && transition.get("status").and_then(Value::as_str) == Some("started")
    })
}

pub(super) fn pending_ralplan_continuation(state: &Value) -> bool {
    already_started(state, RALPLAN)
        && state
            .get("transition")
            .and_then(|transition| transition.get("continuation_status"))
            .and_then(Value::as_str)
            == Some("pending")
}

pub(super) fn mark_ralplan_continuation_delivered(timestamp: &str, state: &mut Value) {
    state["transition"]["continuation_status"] = json!("delivered");
    state["transition"]["continuation_delivered_at"] = json!(timestamp);
    state["updated_at"] = json!(timestamp);
}

pub(super) fn ralplan_start_reason() -> &'static str {
    RALPLAN_START_REASON
}

pub(super) fn start_ralplan_from_crystallized(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
    source_paths: &WorkflowPaths,
    source_state: &mut Value,
) -> Result<Option<&'static str>> {
    if source_state
        .get("milestone_decision")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        != Some("proceed_to_ralplan")
        || source_state.get("status").and_then(Value::as_str) != Some("crystallized")
    {
        return Ok(None);
    }

    let spec_path = source_state
        .get("spec_path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let spec_sha256 = source_state
        .get("spec_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if spec_path.is_empty() || spec_sha256.is_empty() {
        return Ok(None);
    }

    let transition_id = transition_id(
        &source_paths.session_id,
        DEEP_INTERVIEW,
        RALPLAN,
        &spec_sha256,
    );
    if source_state
        .get("transition")
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        == Some(transition_id.as_str())
        && source_state
            .get("transition")
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            == Some("started")
    {
        return Ok(None);
    }

    let target_paths = workflow_paths(state_dir, payload, RALPLAN);
    reconcile_session_aliases(timestamp, payload_file, &target_paths, RALPLAN, payload)?;
    let existing = load_json(&target_paths.session_file);
    let mut target_state = match existing {
        Some(state)
            if state.get("source_transition_id").and_then(Value::as_str)
                == Some(transition_id.as_str()) =>
        {
            state
        }
        Some(state) if rebindable_ralplan_state(&state, &spec_sha256) => state,
        Some(_) => {
            source_state["transition"] = json!({
                "version": CONTRACT_VERSION,
                "id": transition_id,
                "source": DEEP_INTERVIEW,
                "target": RALPLAN,
                "artifact_revision": spec_sha256,
                "status": "blocked",
                "reason": "target_state_conflict",
                "blocked_at": timestamp,
            });
            return Ok(Some(RALPLAN_CONFLICT_REASON));
        }
        None => new_state(RALPLAN, timestamp, &target_paths.session_id, payload),
    };
    require_ralplan_input_lock(timestamp, &mut target_state, payload_file);
    mark_ralplan_input_lock_ready(timestamp, &mut target_state);
    target_state["phase"] = json!("planning");
    target_state["status"] = json!("planning");
    target_state["input_spec_path"] = json!(spec_path);
    target_state["input_spec_sha256"] = json!(spec_sha256);
    target_state["input_spec_persisted_at"] = source_state
        .get("spec_persisted_at")
        .cloned()
        .unwrap_or(Value::Null);
    target_state["source_workflow"] = json!(DEEP_INTERVIEW);
    target_state["source_transition_id"] = json!(transition_id);
    subagent_gate::register_requirement(timestamp, &mut target_state, RALPLAN, payload_file);
    write_json_atomic(&target_paths.session_file, &target_state)?;

    source_state["transition"] = json!({
        "version": CONTRACT_VERSION,
        "id": transition_id,
        "source": DEEP_INTERVIEW,
        "target": RALPLAN,
        "artifact_revision": spec_sha256,
        "status": "started",
        "started_at": timestamp,
        "continuation_status": "pending",
    });
    source_state["next_workflow_suggestion"]["status"] = json!("started");
    source_state["pipeline_lock"]["status"] = json!("ralplan_started");
    append_jsonl(
        &source_paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "workflow_transition_started",
            "session_id": source_paths.session_id,
            "transition_id": transition_id,
            "source": DEEP_INTERVIEW,
            "target": RALPLAN,
            "artifact_revision": spec_sha256,
            "payload": payload_file,
        }),
    )?;
    Ok(Some(RALPLAN_START_REASON))
}

pub(super) fn prepare_ultragoal(timestamp: &str, state: &mut Value) {
    let session_id = state
        .get("session_id")
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    let plan_sha256 = state
        .get("approved_plan_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = transition_id(&session_id, RALPLAN, ULTRAGOAL, plan_sha256);
    state["transition"] = json!({
        "version": CONTRACT_VERSION,
        "id": id,
        "source": RALPLAN,
        "target": ULTRAGOAL,
        "artifact_revision": plan_sha256,
        "status": "starting",
        "started_at": timestamp,
    });
}

pub(super) fn ultragoal_start_pending(state: &Value) -> bool {
    state.get("transition").is_some_and(|transition| {
        transition.get("target").and_then(Value::as_str) == Some(ULTRAGOAL)
            && transition.get("status").and_then(Value::as_str) == Some("starting")
    })
}

pub(super) fn ultragoal_start_context(scope: ScopeArg, session_id: &str) -> String {
    let (scope, binary) = match scope {
        ScopeArg::Project => ("project", ".agents/bin/megara"),
        ScopeArg::Global => ("global", "$HOME/.megara/bin/megara"),
    };
    let session_id = shell_quote(session_id);
    format!(
        "Internal Megara workflow instruction: Start ultragoal now from the approved plan; the user's selection is the transition authorization and no separate skill invocation or approval is required. Resolve `MEGARA_BIN=\"${{MEGARA_BIN:-{binary}}}\"`, run `\"$MEGARA_BIN\" ultragoal --scope {scope} --session-id {session_id} create-goals`, then run `\"$MEGARA_BIN\" ultragoal --scope {scope} --session-id {session_id} start-goal` and begin the selected product work. Keep runtime commands and state details out of user-facing prose."
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn transition_id(session_id: &str, source: &str, target: &str, revision: &str) -> String {
    fsutil::sha256_hex(format!("{session_id}\0{source}\0{target}\0{revision}").as_bytes())
}

fn rebindable_ralplan_state(state: &Value, spec_sha256: &str) -> bool {
    let phase = state.get("phase").and_then(Value::as_str);
    let phase_is_initial = matches!(
        phase,
        Some("initialized" | "input_lock_required" | "input_lock_ready" | "planning")
    );
    let input_matches = state
        .get("input_spec_sha256")
        .and_then(Value::as_str)
        .is_none_or(|revision| revision == spec_sha256);
    phase_is_initial
        && input_matches
        && state.get("plan_id").is_none()
        && state.get("plan_path").is_none()
        && state
            .get("reviews")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
}
