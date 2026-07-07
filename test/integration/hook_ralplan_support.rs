use super::*;

pub(super) const RALPLAN: &str = "ralplan";
pub(super) const DEEP_INTERVIEW: &str = "deep-interview";

pub(super) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn stop_message(project: &Path, session_id: &str, message: &str) -> Output {
    let payload = serde_json::json!({
        "session_id": session_id,
        "last_assistant_message": message,
    })
    .to_string();
    run_hook(project, project, "Stop", None, payload.as_bytes())
}

pub(super) fn user_prompt(project: &Path, session_id: &str, prompt: &str) -> Output {
    let payload = serde_json::json!({
        "session_id": session_id,
        "prompt": prompt,
    })
    .to_string();
    run_hook(
        project,
        project,
        "UserPromptSubmit",
        None,
        payload.as_bytes(),
    )
}

pub(super) fn workflow_state_path(project: &Path, workflow: &str, session_id: &str) -> PathBuf {
    project
        .join(".megara/state/workflows")
        .join(workflow)
        .join(format!("{session_id}.json"))
}

pub(super) fn read_state(project: &Path, workflow: &str, session_id: &str) -> serde_json::Value {
    read_json(&workflow_state_path(project, workflow, session_id))
}

pub(super) fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

pub(super) fn submit_ready_reviews(project: &Path, session_id: &str) {
    let payload = ready_ralplan_reviews_payload(session_id);
    let output = run_hook(project, project, "Stop", None, payload.as_bytes());
    assert_success(&output);
}

pub(super) fn deep_interview_approval_prompt() -> String {
    "<!--\nMegara Approval Gate:\n- approved_workflow: deep-interview\n- approved_status: crystallized\n- approved_ambiguity: 9%\n- next_workflow: ralplan\n- implementation_allowed_now: false\n-->\n"
        .to_string()
}

fn ready_ralplan_reviews_payload(session_id: &str) -> String {
    let message = "Review coverage complete.\n\n<!--\nMegara Review Pass:\n- role: planner\n- round: 1\n- verdict: CLEAR\n- summary: Planner pass is ready.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: architect\n- round: 1\n- verdict: CLEAR\n- summary: Architecture pass is clear.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: critic\n- round: 1\n- verdict: OKAY\n- summary: Critic pass approves planning quality.\n- required_fixes:\n  - none\n-->\n";
    serde_json::json!({
        "session_id": session_id,
        "last_assistant_message": message,
    })
    .to_string()
}

fn pending_ralplan_plan_payload(session_id: &str, plan_id: &str, summary: &str) -> String {
    let message = format!(
        "**Pending Execution Plan**\n\nSummary: {summary}\n\nSteps:\n- Keep the change small.\n- Verify the expected behavior.\n\nAcceptance criteria:\n- Existing tests pass.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: {plan_id}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {plan_id}\n- next: approval\n-->\n"
    );
    serde_json::json!({
        "session_id": session_id,
        "last_assistant_message": message,
    })
    .to_string()
}

fn pending_ralplan_plan_payload_with_input_spec(
    session_id: &str,
    plan_id: &str,
    summary: &str,
    input_spec_sha256: &str,
) -> String {
    let message = format!(
        "**Pending Execution Plan**\n\nSummary: {summary}\n\nSteps:\n- Keep the change small.\n- Verify the expected behavior.\n\nAcceptance criteria:\n- Existing tests pass.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: {plan_id}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {plan_id}\n- input_spec_sha256: {input_spec_sha256}\n- next: approval\n-->\n"
    );
    serde_json::json!({
        "session_id": session_id,
        "last_assistant_message": message,
    })
    .to_string()
}

pub(super) fn submit_plan(project: &Path, session_id: &str, plan_id: &str, summary: &str) {
    let payload = pending_ralplan_plan_payload(session_id, plan_id, summary);
    let output = run_hook(project, project, "Stop", None, payload.as_bytes());
    assert_success(&output);
}

pub(super) fn submit_plan_with_lock(
    project: &Path,
    session_id: &str,
    plan_id: &str,
    summary: &str,
    spec_sha256: &str,
) {
    let payload =
        pending_ralplan_plan_payload_with_input_spec(session_id, plan_id, summary, spec_sha256);
    let output = run_hook(project, project, "Stop", None, payload.as_bytes());
    assert_success(&output);
}

pub(super) fn submit_role_subagent_review(
    project: &Path,
    session_id: &str,
    role: &str,
    verdict: &str,
) {
    let message = format!(
        "Review complete.\n\n<!--\nMegara Review Pass:\n- role: {role}\n- round: 1\n- verdict: {verdict}\n- summary: {role} review is ready.\n- required_fixes:\n  - none\n-->\n"
    );
    let payload = serde_json::json!({
        "session_id": session_id,
        "agent_id": format!("agent-{role}-1"),
        "agent_type": role,
        "last_assistant_message": message,
    })
    .to_string();
    assert_success(&run_hook(
        project,
        project,
        "SubagentStop",
        Some(role),
        payload.as_bytes(),
    ));
}

pub(super) fn submit_role_subagent_receipt(project: &Path, session_id: &str, role: &str) {
    let payload = serde_json::json!({
        "session_id": session_id,
        "agent_id": format!("agent-{role}-receipt"),
        "agent_type": role,
    })
    .to_string();
    assert_success(&run_hook(
        project,
        project,
        "SubagentStop",
        Some(role),
        payload.as_bytes(),
    ));
}

pub(super) fn submit_crystallized_interview(
    project: &Path,
    session_id: &str,
    goal: &str,
) -> serde_json::Value {
    let message = format!(
        "**Requirements Summary**\n\nGoal: {goal}\n\nAcceptance criteria:\n- The requested behavior works.\n\nNext: continue with `ralplan` from this summary.\n\n<!--\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 8%\n- next: ralplan\n-->\n"
    );
    let output = stop_message(project, session_id, &message);
    assert_success(&output);
    read_state(project, DEEP_INTERVIEW, session_id)
}

pub(super) fn run_mutation(project: &Path, session_id: &str) -> Output {
    let payload = format!(
        r#"{{"session_id":"{session_id}","tool_input":{{"command":"echo changed > app.js"}}}}"#
    );
    run_hook(
        project,
        project,
        "PreToolUse",
        Some("Bash"),
        payload.as_bytes(),
    )
}

pub(super) fn events(project: &Path, workflow: &str) -> String {
    fs::read_to_string(project.join(format!(".megara/state/workflows/{workflow}/events.jsonl")))
        .unwrap()
}
