use super::*;

#[derive(Debug)]
pub(super) struct LinkedSpec {
    pub(super) path: String,
    pub(super) sha256: String,
    pub(super) persisted_at: String,
}

pub(super) fn linked_deep_interview_spec(paths: &WorkflowPaths) -> Option<LinkedSpec> {
    let state = deep_interview_state(paths)?;
    linked_spec_from_state(&state).or_else(|| superseded_linked_spec(paths, &state))
}

fn linked_spec_from_state(state: &Value) -> Option<LinkedSpec> {
    (state.get("status").and_then(Value::as_str) == Some("crystallized")).then_some(())?;
    Some(LinkedSpec {
        path: state.get("spec_path")?.as_str()?.to_string(),
        sha256: state.get("spec_sha256")?.as_str()?.to_string(),
        persisted_at: state
            .get("spec_persisted_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn superseded_linked_spec(paths: &WorkflowPaths, state: &Value) -> Option<LinkedSpec> {
    let superseded_by = state.get("stale_superseded_by")?.as_str()?;
    let state = deep_interview_state_for_session(paths, superseded_by)?;
    linked_spec_from_state(&state)
}

pub(super) fn ralplan_input_lock_blocker(
    state: &Value,
    linked_spec: Option<&LinkedSpec>,
    text: &str,
) -> Option<&'static str> {
    if state.get("requires_input_lock").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let Some(spec) = linked_spec else {
        return Some("missing_persisted_deep_interview_lock");
    };
    workflow_state_field(text, "input_spec_sha256")
        .is_some_and(|input_sha256| input_sha256 != spec.sha256)
        .then_some("input_spec_sha256_mismatch")
}

pub(super) fn active_deep_interview_state(paths: &WorkflowPaths) -> Option<Value> {
    let state = deep_interview_state(paths)?;
    (state.get("active").and_then(Value::as_bool) == Some(true)
        && state.get("status").and_then(Value::as_str) != Some("crystallized"))
    .then_some(state)
}

fn deep_interview_state(paths: &WorkflowPaths) -> Option<Value> {
    deep_interview_state_for_session(paths, &paths.session_id)
}

fn deep_interview_state_for_session(paths: &WorkflowPaths, session_id: &str) -> Option<Value> {
    let workflow_base = paths.workflow_dir.parent()?;
    let deep_state_path = workflow_base
        .join(DEEP_INTERVIEW)
        .join(format!("{}.json", safe_part(session_id)));
    let state = load_json(&deep_state_path)?;
    (state.get("skill").and_then(Value::as_str) == Some(DEEP_INTERVIEW)).then_some(state)
}

fn workflow_state_field(text: &str, key: &str) -> Option<String> {
    let block = parse_block(text, "Megara Workflow State:")?;
    let value = block.fields.get(key)?.trim().trim_matches('"').to_string();
    (!value.is_empty()).then_some(value)
}
