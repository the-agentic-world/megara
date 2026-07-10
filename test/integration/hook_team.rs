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
fn team_start_in_cli_uses_limited_split_context() {
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
    assert!(stdout.contains("team split --transport auto --roles planner,executor,critic"));
    assert!(stdout.contains("--correlation-id team-"));
    assert!(stdout.contains("--open"));
    assert!(stdout.contains("two columns"));
    assert!(stdout.contains("cmux, tmux, and orca"));
    assert!(stdout.contains("CLI split pane 생성 실패로 subagent fallback 사용"));
    assert_eq!(
        stdout
            .matches("CLI split pane 생성 실패로 subagent fallback 사용")
            .count(),
        1
    );
    assert!(stdout.contains("unavailable"));
    assert!(stdout.contains("fails"));
    assert!(stdout.contains("one-line notice once"));
    assert!(stdout.contains("Do not include split failure details"));
    assert!(stdout.contains("use Codex subagents instead"));
    assert!(!stdout.contains("team warp"));
    assert!(!stdout.contains("Warp Tab Config"));
    assert!(!stdout.contains("sess-team-cli"));
    assert!(!stdout.contains("state path"));
    assert!(!stdout.contains("payload"));
    assert!(stdout.contains("planner, executor, critic"));

    let state = read_state(dir.path(), TEAM, "sess-team-cli");
    assert_eq!(state["team"]["surface"], "cli");
    assert_eq!(state["team"]["transport"], "split-pane");
    assert_eq!(state["team"]["teammate_count"], 3);
    assert!(state["team"]["correlation_id"]
        .as_str()
        .unwrap()
        .starts_with("team-"));
    assert_eq!(state["team"]["split_transports"][0], "cmux");
    assert_eq!(state["team"]["split_transports"][1], "tmux");
    assert_eq!(state["team"]["split_transports"][2], "orca");
    assert!(state["team"]["split_receipt_dir"].as_str().is_some());
}

#[test]
fn team_split_command_prepares_tmux_commands_without_opening() {
    let dir = tempdir().unwrap();
    let runtime_root = dir.path().join(".megara");

    let output = megara()
        .arg("team")
        .arg("split")
        .arg("--transport")
        .arg("tmux")
        .arg("--roles")
        .arg("executor,critic")
        .arg("--correlation-id")
        .arg("team-test")
        .arg("--task")
        .arg("Improve menu contrast.")
        .arg("--cwd")
        .arg(dir.path())
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--megara-bin")
        .arg(".agents/bin/megara")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "prepared");
    assert_eq!(report["transport"], "tmux");
    assert_eq!(report["roles"][0], "executor");
    assert_eq!(report["roles"][1], "critic");
    assert!(report["commands"][0]
        .as_str()
        .unwrap()
        .contains("tmux split-window -h"));
    assert!(report["commands"][1]
        .as_str()
        .unwrap()
        .contains("tmux split-window -v"));
    assert!(report["commands"][0]
        .as_str()
        .unwrap()
        .contains("megara team teammate"));
    assert!(runtime_root
        .join("state/team/split/team-test/task.md")
        .exists());
}

#[test]
fn team_split_command_prepares_cmux_and_orca_commands_without_opening() {
    let dir = tempdir().unwrap();
    let runtime_root = dir.path().join(".megara");

    for (transport, first_command) in [
        ("cmux", "cmux new-split right"),
        ("orca", "orca terminal split --direction horizontal"),
    ] {
        let output = megara()
            .arg("team")
            .arg("split")
            .arg("--transport")
            .arg(transport)
            .arg("--roles")
            .arg("executor,critic")
            .arg("--correlation-id")
            .arg(format!("team-{transport}"))
            .arg("--task")
            .arg("Improve menu contrast.")
            .arg("--cwd")
            .arg(dir.path())
            .arg("--runtime-root")
            .arg(&runtime_root)
            .arg("--megara-bin")
            .arg(".agents/bin/megara")
            .arg("--json")
            .current_dir(dir.path())
            .output()
            .unwrap();

        assert_success(&output);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "prepared");
        assert_eq!(report["transport"], transport);
        assert!(report["commands"][0]
            .as_str()
            .unwrap()
            .contains(first_command));
        if transport == "cmux" {
            let first = report["commands"][0].as_str().unwrap();
            let second = report["commands"][1].as_str().unwrap();
            assert!(first.contains("--focus true"));
            assert!(first.contains("env -u CMUX_SURFACE_ID cmux send"));
            assert!(first.contains("\\n"));
            assert!(second.contains("env -u CMUX_SURFACE_ID cmux new-split down --focus true"));
            assert!(second.contains("env -u CMUX_SURFACE_ID cmux send"));
            assert!(second.contains("\\n"));
        }
    }
}

#[test]
fn team_stop_accepts_split_receipts() {
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
        "session_id": "sess-team-split-receipts",
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
    let state = read_state(dir.path(), TEAM, "sess-team-split-receipts");
    let correlation_id = state["team"]["correlation_id"].as_str().unwrap();
    let receipt_dir = PathBuf::from(state["team"]["split_receipt_dir"].as_str().unwrap());
    fs::create_dir_all(&receipt_dir).unwrap();
    for role in ["planner", "executor", "critic"] {
        fs::write(
            receipt_dir.join(format!("{role}-1.json")),
            serde_json::json!({
                "kind": "teammate-result",
                "status": "succeeded",
                "transport": "tmux",
                "workflow": "team",
                "role": role,
                "teammate_id": format!("{role}-1"),
                "correlation_id": correlation_id,
                "orchestration_request_id": correlation_id,
                "content_file": receipt_dir.join(format!("{role}-1.md")),
            })
            .to_string(),
        )
        .unwrap();
    }

    let final_message = "**팀 완료**\n\n팀메이트 상태: planner, executor, critic 결과를 통합했습니다.\n\n통합 notes:\n- 변경은 하나의 의도로 묶었습니다.\n\n검증:\n- cargo test 통과.\n\n완료.";
    let complete = stop_message(dir.path(), "sess-team-split-receipts", final_message);
    assert_success(&complete);
    let state = read_state(dir.path(), TEAM, "sess-team-split-receipts");
    assert_eq!(state["phase"], "complete");
    assert_eq!(state["subagent_orchestration"]["status"], "satisfied");
    assert_eq!(state["subagent_receipts"].as_array().unwrap().len(), 3);
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

#[test]
fn ralplan_team_numeric_approval_opens_team_workflow() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let session_id = "sess-rp-team-handoff";
    assert_success(&user_prompt(
        dir.path(),
        session_id,
        "$ralplan implement Codex hook runtime adapter integration",
    ));
    submit_role_subagent_review(dir.path(), session_id, "planner", "CLEAR");
    submit_role_subagent_review(dir.path(), session_id, "architect", "CLEAR");
    submit_role_subagent_review(dir.path(), session_id, "critic", "OKAY");
    submit_plan(
        dir.path(),
        session_id,
        "rp-team-handoff",
        "implement Codex hook runtime adapter integration",
    );
    let ralplan_state = read_state(dir.path(), RALPLAN, session_id);
    assert_eq!(ralplan_state["phase"], "pending_approval");

    let transcript = dir.path().join("app-session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"vscode","originator":"Codex Desktop"}}"#,
    )
    .unwrap();
    let payload = serde_json::json!({
        "session_id": session_id,
        "cwd": dir.path(),
        "transcript_path": transcript,
        "prompt": "3",
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
    assert!(stdout.contains("planner, architect, executor, critic"));

    let approved = read_state(dir.path(), RALPLAN, session_id);
    assert_eq!(approved["phase"], "approved");
    assert_eq!(approved["approved_handoff_target"], TEAM);

    let team_state = read_state(dir.path(), TEAM, session_id);
    assert_eq!(team_state["source_workflow"], RALPLAN);
    assert_eq!(team_state["source_plan_id"], "rp-team-handoff");
    assert_eq!(team_state["team"]["surface"], "app");
    assert_eq!(team_state["team"]["leader"], "current-session");
    assert_eq!(team_state["team"]["transport"], "subagent-fallback");
    assert_eq!(team_state["subagent_orchestration"]["status"], "required");
    assert_eq!(team_state["subagent_orchestration"]["roles"][0], "planner");
    assert_eq!(
        team_state["subagent_orchestration"]["roles"][1],
        "architect"
    );
    assert_eq!(team_state["subagent_orchestration"]["roles"][2], "executor");
    assert_eq!(team_state["subagent_orchestration"]["roles"][3], "critic");
}
