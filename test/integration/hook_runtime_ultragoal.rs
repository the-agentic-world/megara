use super::*;

#[test]
fn hook_blocks_ultragoal_goal_planning_but_allows_active_goal_mutations() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_ultragoal_state(dir.path(), "goal_planning", "create goals");
    let state_path = dir
        .path()
        .join(".agents/state/workflows/ultragoal/sess-ug-hook.json");
    let state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "goal_planning");

    let output = run_mutation(dir.path());
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("complete-goals"));

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
            .join(".agents/state/workflows/ultragoal/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"workflow_state\""));
    assert!(events.contains("\"event\":\"mutation_blocked\""));
}

fn submit_ultragoal_state(project: &Path, status: &str, next: &str) {
    let payload = format!(
        "{{\"session_id\":\"sess-ug-hook\",\"last_assistant_message\":\"Megara Workflow State:\\n- skill: ultragoal\\n- status: {status}\\n- next: {next}\\n\\n\"}}"
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
