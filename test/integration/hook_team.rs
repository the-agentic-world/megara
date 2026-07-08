use super::hook_ralplan_support::*;
use super::*;

#[test]
fn team_start_in_app_requires_subagent_teammates() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let transcript = dir.path().join("app-session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"vscode","originator":"Codex Desktop"}}"#,
    )
    .unwrap();
    let payload = serde_json::json!({
        "session_id": "sess-team-app",
        "cwd": dir.path(),
        "transcript_path": transcript,
        "prompt": "$team implement Codex hook runtime adapter integration",
    })
    .to_string();

    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        payload.as_bytes(),
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("this Codex App session is the team leader"));
    assert!(stdout.contains("Use Codex subagents as teammates"));
    assert!(stdout.contains("planner, architect, executor, critic"));

    let state = read_state(dir.path(), TEAM, "sess-team-app");
    assert_eq!(state["team"]["surface"], "app");
    assert_eq!(state["team"]["leader"], "current-session");
    assert_eq!(state["team"]["transport"], "subagent-fallback");
    assert_eq!(state["subagent_orchestration"]["status"], "required");
    assert_eq!(state["subagent_orchestration"]["roles"][0], "planner");
    assert_eq!(state["subagent_orchestration"]["roles"][1], "architect");
    assert_eq!(state["subagent_orchestration"]["roles"][2], "executor");
    assert_eq!(state["subagent_orchestration"]["roles"][3], "critic");
}

#[test]
fn team_start_in_cli_uses_warp_fallback_context() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let transcript = dir.path().join("cli-session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"exec"}}"#,
    )
    .unwrap();
    let payload = serde_json::json!({
        "session_id": "sess-team-cli",
        "cwd": dir.path(),
        "transcript_path": transcript,
        "prompt": "$team improve menu workflow",
    })
    .to_string();

    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        payload.as_bytes(),
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("this Codex CLI session is the team leader"));
    assert!(stdout.contains("two columns"));
    assert!(stdout.contains("Warp pane 생성 실패로 subagent fallback 사용"));
    assert!(stdout.contains("planner, executor, critic"));

    let state = read_state(dir.path(), TEAM, "sess-team-cli");
    assert_eq!(state["team"]["surface"], "cli");
    assert_eq!(state["team"]["transport"], "subagent-fallback");
    assert_eq!(state["team"]["teammate_count"], 3);
}

#[test]
fn team_completion_is_blocked_until_teammate_receipts_exist() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    assert_success(&user_prompt(
        dir.path(),
        "sess-team-gate",
        "$team fix one simple UI issue",
    ));

    let final_message = "**팀 완료**\n\n팀메이트 상태: executor와 critic 결과를 통합했습니다.\n\n통합 notes:\n- 변경은 하나의 의도로 묶었습니다.\n\n검증:\n- cargo test 통과.\n\n완료.";
    let blocked = stop_message(dir.path(), "sess-team-gate", final_message);
    assert_success(&blocked);
    assert!(String::from_utf8_lossy(&blocked.stdout).trim().is_empty());
    let state = read_state(dir.path(), TEAM, "sess-team-gate");
    assert_eq!(state["phase"], "subagent_review_required");
    assert_eq!(
        state["subagent_orchestration"]["missing_roles"][0],
        "executor"
    );
    assert_eq!(
        state["subagent_orchestration"]["missing_roles"][1],
        "critic"
    );

    submit_role_subagent_receipt(dir.path(), "sess-team-gate", "executor");
    submit_role_subagent_receipt(dir.path(), "sess-team-gate", "critic");
    let complete = stop_message(dir.path(), "sess-team-gate", final_message);
    assert_success(&complete);
    let state = read_state(dir.path(), TEAM, "sess-team-gate");
    assert_eq!(state["phase"], "complete");
    assert_eq!(state["subagent_orchestration"]["status"], "satisfied");
}
