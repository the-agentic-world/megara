use super::*;

#[test]
fn hook_blocks_ultragoal_goal_planning_but_allows_active_goal_mutations() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_ultragoal_state(dir.path(), "goal_planning", "create goals");
    let state_path = dir
        .path()
        .join(".megara/state/workflows/ultragoal/sess-ug-hook.json");
    let state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "goal_planning");

    let output = run_mutation(dir.path());
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("start-goal"));

    submit_ultragoal_state(dir.path(), "active", "execute G001");
    let state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "active");
    assert_eq!(state["next"], "execute G001");

    let output = run_mutation(dir.path());
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/ultragoal/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"workflow_state\""));
    assert!(events.contains("\"event\":\"mutation_blocked\""));
}

#[test]
fn hook_blocks_visible_runtime_artifact_links_during_workflow() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_ultragoal_state(dir.path(), "active", "execute G001");
    let payload = br#"{
  "session_id": "sess-ug-hook",
  "last_assistant_message": "Done. See [.megara evidence](.megara/artifacts/ultragoal/sess-ug-hook/evidence/report.md) for details."
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());

    let events = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/ultragoal/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"visible_runtime_reference_blocked\""));
}

#[test]
fn hook_blocks_direct_runtime_artifact_writes() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let output = run_hook(
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        br#"{"session_id":"sess-ug-hook","tool_input":{"command":"mkdir -p .megara/artifacts/ultragoal/sess-ug-hook/evidence && printf ok > .megara/artifacts/ultragoal/sess-ug-hook/evidence/verification.log"}}"#,
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("runtime state and artifacts are managed"));

    let output = run_hook(
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Write"),
        br#"{"session_id":"sess-ug-hook","tool_name":"Write","tool_input":{"file_path":".megara/artifacts/ultragoal/sess-ug-hook/evidence/verification.log","content":"ok"}}"#,
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("runtime state and artifacts are managed"));
}

fn submit_ultragoal_state(project: &Path, status: &str, next: &str) {
    let payload = format!(
        "{{\"session_id\":\"sess-ug-hook\",\"last_assistant_message\":\"Status update.\\n\\n<!--\\nMegara Workflow State:\\n- skill: ultragoal\\n- status: {status}\\n- next: {next}\\n-->\\n\"}}"
    );
    let output = run_hook(project, project, "Stop", None, payload.as_bytes());
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_mutation(project: &Path) -> Output {
    run_hook(
        project,
        project,
        "PreToolUse",
        Some("Bash"),
        br#"{"session_id":"sess-ug-hook","tool_input":{"command":"echo changed > app.js"}}"#,
    )
}
