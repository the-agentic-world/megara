use super::*;

#[test]
fn projected_hook_runner_records_runtime_event() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let install = megara_with_codex_home(codex_home.path())
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let nested = dir.path().join("src").join("game");
    fs::create_dir_all(&nested).unwrap();

    let output = run_hook(
        dir.path(),
        &nested,
        "UserPromptSubmit",
        None,
        br#"{"prompt":"hello"}"#,
    );
    assert!(output.status.success());

    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        br#"{"prompt":"second"}"#,
    );
    assert!(output.status.success());

    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        br#"{"last_assistant_message":"question?"}"#,
    );
    assert!(output.status.success());

    let log = fs::read_to_string(dir.path().join(".megara/state/hooks/events.jsonl")).unwrap();
    assert!(!dir.path().join(".agents/state/hooks/events.jsonl").exists());
    assert!(log.contains("\"runtime\":\"codex\""));
    assert!(log.contains("\"event\":\"UserPromptSubmit\""));
    assert!(log.contains("/payloads/codex/UserPromptSubmit/"));
    let payload = fs::read_to_string(
        dir.path()
            .join(".megara/state/hooks/last-codex-UserPromptSubmit.json"),
    )
    .unwrap();
    assert_eq!(payload, r#"{"prompt":"second"}"#);

    let payload_paths = log
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["event"] == "UserPromptSubmit")
        .map(|entry| PathBuf::from(entry["payload"].as_str().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(payload_paths.len(), 2);
    assert_eq!(
        fs::read_to_string(&payload_paths[0]).unwrap(),
        r#"{"prompt":"hello"}"#
    );
    assert_eq!(
        fs::read_to_string(&payload_paths[1]).unwrap(),
        r#"{"prompt":"second"}"#
    );

    let conversation_events = fs::read_to_string(
        dir.path()
            .join(".megara/state/hooks/conversation-events.jsonl"),
    )
    .unwrap();
    assert!(conversation_events.contains("\"role\":\"user\""));
    assert!(conversation_events.contains("\"role\":\"assistant\""));

    let conversation =
        fs::read_to_string(dir.path().join(".megara/state/hooks/conversation.jsonl")).unwrap();
    assert!(conversation.contains("\"content\":\"hello\""));
    assert!(conversation.contains("\"content\":\"second\""));
    assert!(conversation.contains("\"content\":\"question?\""));
}

#[test]
fn projected_hook_runner_records_effective_prompt_and_surface() {
    let dir = tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"vscode","thread_source":"subagent","originator":"Codex Desktop"}}"#,
    )
    .unwrap();
    let payload = serde_json::json!({
        "session_id": "runtime-session",
        "transcript_path": transcript,
        "prompt": "<codex_delegation><input>Use option 2.</input></codex_delegation>",
    })
    .to_string();

    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        payload.as_bytes(),
    );

    assert!(output.status.success());
    let conversation =
        fs::read_to_string(dir.path().join(".megara/state/hooks/conversation.jsonl")).unwrap();
    assert!(conversation.contains("\"content\":\"Use option 2.\""));
    assert!(conversation.contains("\"surface\":\"app\""));
    assert!(conversation.contains("\"transcript_source\":\"vscode\""));
    assert!(conversation.contains("\"raw_content\":\"<codex_delegation>"));
}
