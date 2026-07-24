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

#[test]
fn hook_blocks_repeated_status_polling_without_rewriting_the_command() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let session_id = "tool-loop-session";
    let turn_id = "tool-loop-turn";
    let status = r#"megara ultragoal --scope project --session-id tool-loop-session status"#;
    let inspect = "cat docs/parity-traceability.json";
    for command in [status, inspect, status, inspect] {
        let output = run_read_only_tool(dir.path(), session_id, turn_id, command);
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let blocked = run_read_only_tool(dir.path(), session_id, turn_id, status);
    assert!(
        blocked.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&blocked.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&blocked.stdout).unwrap();
    assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(output["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("Do not inspect status again"));
    assert!(output["hookSpecificOutput"].get("updatedInput").is_none());

    let prompt = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        br#"{"session_id":"tool-loop-session","turn_id":"next-user-turn","prompt":"Continue with the next approved task."}"#,
    );
    assert!(prompt.status.success());

    let resumed = run_read_only_tool(dir.path(), session_id, "next-user-turn", status);
    assert!(
        resumed.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&resumed.stderr)
    );
}

#[test]
fn hook_continues_once_when_an_ultragoal_has_an_active_next_goal() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    submit_ultragoal_state(dir.path(), "active", "execute G002");

    let inactive_checkpoint = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        br#"{
          "session_id": "sess-ug-hook",
          "turn_id": "checkpoint-turn",
          "stop_hook_active": false,
          "last_assistant_message": "The first product goal is complete."
        }"#,
    );
    assert!(inactive_checkpoint.status.success());
    assert!(inactive_checkpoint.stdout.is_empty());

    let recorded = run_hook(
        dir.path(),
        dir.path(),
        "PostToolUse",
        Some("Bash"),
        br#"{
          "session_id": "sess-ug-hook",
          "turn_id": "checkpoint-turn",
          "tool_use_id": "checkpoint-1",
          "tool_name": "Bash",
          "tool_input": {
            "command": "megara ultragoal --scope project --session-id sess-ug-hook checkpoint --goal-id G001 --status complete --json"
          },
          "tool_response": {
            "output": "{\"next_goal_started\":{\"id\":\"G002\"}}"
          }
        }"#,
    );
    assert!(
        recorded.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&recorded.stderr)
    );

    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        br#"{
          "session_id": "sess-ug-hook",
          "turn_id": "checkpoint-turn",
          "stop_hook_active": false,
          "last_assistant_message": "The first product goal is complete."
        }"#,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["decision"], "block");
    assert!(output["reason"]
        .as_str()
        .unwrap()
        .contains("completed checkpoint activated this next goal"));

    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        br#"{
          "session_id": "sess-ug-hook",
          "turn_id": "checkpoint-turn",
          "stop_hook_active": true,
          "last_assistant_message": "The next product step needs more investigation."
        }"#,
    );
    assert!(output.status.success());
    assert!(output.stdout.is_empty());

    let recorded = run_hook(
        dir.path(),
        dir.path(),
        "PostToolUse",
        Some("Bash"),
        br#"{
          "session_id": "sess-ug-hook",
          "turn_id": "checkpoint-turn",
          "tool_use_id": "checkpoint-2",
          "tool_name": "Bash",
          "tool_input": {
            "command": "megara ultragoal --scope project --session-id sess-ug-hook checkpoint --goal-id G002 --status=complete --json"
          },
          "tool_response": {
            "output": "ultragoal checkpoint recorded for G002 (complete); next active goal: G003 - Verify release"
          }
        }"#,
    );
    assert!(recorded.status.success());

    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        br#"{
          "session_id": "sess-ug-hook",
          "turn_id": "checkpoint-turn",
          "stop_hook_active": true,
          "last_assistant_message": "The second product goal is complete."
        }"#,
    );
    assert!(output.status.success());
    let output: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["decision"], "block");
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

fn run_read_only_tool(project: &Path, session_id: &str, turn_id: &str, command: &str) -> Output {
    let payload = serde_json::json!({
        "session_id": session_id,
        "turn_id": turn_id,
        "tool_name": "Bash",
        "tool_input": {"command": command},
    })
    .to_string();
    run_hook(
        project,
        project,
        "PreToolUse",
        Some("Bash"),
        payload.as_bytes(),
    )
}
