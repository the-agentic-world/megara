use super::*;

pub(super) fn state_path(project: &Path) -> PathBuf {
    project.join(".megara/state/workflows/deep-interview/sess-di.json")
}

pub(super) fn submit_question(project: &Path) {
    let payload = br#"{
  "session_id": "sess-di",
  "cwd": "/tmp/project",
  "last_assistant_message": "Ambiguity: 42%\n\nWhat proves this is done?\n\n1. Unit tests\n2. E2E tests\n3. Manual QA\n4. Direct input / not listed\n\n"
}"#;
    assert_success(run_hook(project, project, "Stop", None, payload));
}

pub(super) fn answer_question(project: &Path) {
    let payload = br#"{"session_id":"sess-di","prompt":"Use both unit and E2E tests."}"#;
    assert_success(run_hook(
        project,
        project,
        "UserPromptSubmit",
        None,
        payload,
    ));
}

pub(super) fn run_bash_mutation(project: &Path) -> Output {
    run_hook(
        project,
        project,
        "PreToolUse",
        Some("Bash"),
        br#"{"session_id":"sess-di","tool_input":{"command":"echo changed > app.js"}}"#,
    )
}

pub(super) fn run_apply_patch(project: &Path) -> Output {
    let payload = br#"{"session_id":"sess-di","tool_name":"apply_patch","tool_input":{"patch":"*** Begin Patch\n*** End Patch\n"}}"#;
    run_hook(project, project, "PreToolUse", None, payload)
}

pub(super) fn run_read(project: &Path) -> Output {
    let payload =
        br#"{"session_id":"sess-di","tool_name":"Read","tool_input":{"file_path":"app.js"}}"#;
    run_hook(project, project, "PreToolUse", None, payload)
}

pub(super) fn submit_state_only_crystallized(project: &Path) {
    let payload = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "<!--\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 12%\n- next: ralplan\n-->\n\n"
}"#;
    assert_success(run_hook(project, project, "Stop", None, payload));
}

pub(super) fn submit_final_spec(project: &Path) {
    let payload = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "**Requirements Summary**\n\nGoal: build the verified game.\n\nAcceptance criteria:\n- Unit tests pass\n- E2E tests pass\n\nNext: continue with `ralplan` from this summary. Implementation is still not allowed.\n\n<!--\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 12%\n- next: ralplan\n-->\n\n"
}"#;
    assert_success(run_hook(project, project, "Stop", None, payload));
}

pub(super) fn assert_guard_blocks(output: Output) {
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MEGARA mutation guard"));
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
