use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use tempfile::tempdir;

fn megara() -> Command {
    Command::new(env!("CARGO_BIN_EXE_megara"))
}

fn run_hook(
    script: &Path,
    cwd: &Path,
    event: &str,
    matcher: Option<&str>,
    payload: &[u8],
) -> Output {
    let mut command = Command::new("sh");
    command
        .arg(script)
        .arg("--runtime")
        .arg("codex")
        .arg("--event")
        .arg(event)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(matcher) = matcher {
        command.arg("--matcher").arg(matcher);
    }
    let mut child = command.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(payload).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn installs_project_scope_codex_harness() {
    let dir = tempdir().unwrap();

    let output = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join(".agents/megara.toml").exists());
    assert!(dir.path().join(".codex/AGENTS.md").exists());
    assert!(dir
        .path()
        .join(".codex/skills/deep-interview/SKILL.md")
        .exists());
    assert!(dir
        .path()
        .join(".agents/skill-fragments/deep-interview/auto-research-greenfield.md")
        .exists());
    assert!(dir
        .path()
        .join(".codex/skill-fragments/deep-interview/auto-research-greenfield.md")
        .exists());
    assert!(dir.path().join(".agents/hooks/megara-hook.sh").exists());
    assert!(dir.path().join(".agents/agents/executor.toml").exists());
    assert!(dir.path().join(".codex/hooks/megara-hook.sh").exists());
    assert!(dir.path().join(".codex/hooks.json").exists());
    assert!(dir.path().join(".codex/agents/executor.toml").exists());
    let hook_script = fs::read_to_string(dir.path().join(".codex/hooks/megara-hook.sh")).unwrap();
    assert!(hook_script.starts_with("#!/usr/bin/env sh\n# MEGARA:MANAGED"));
    let skill =
        fs::read_to_string(dir.path().join(".codex/skills/deep-interview/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\n"));
    assert!(skill.contains("MEGARA:MANAGED"));
    assert!(skill.contains("Ambiguity: NN% remaining"));
    assert!(skill.contains("Round 0 | Topology confirmation"));
    assert!(skill.contains("Calculate ambiguity as `100 - weighted_clarity`"));
    assert!(skill.contains("Ambiguity is bidirectional and non-monotonic"));
    assert!(skill.contains("Interview ledger update:"));
    assert!(skill.contains("Megara Question Gate:"));
    assert!(skill.contains("Megara Workflow State:"));
    assert!(skill.contains("locked markdown artifact"));
    assert!(skill.contains("spec_path"));
    assert!(skill.contains("Restate gate"));
    assert!(skill.contains("Write every user-facing sentence in the configured locale"));
    assert!(skill.contains("option labels"));
    assert!(skill.contains("free-text values"));
    let ssot_agent = fs::read_to_string(dir.path().join(".agents/agents/executor.toml")).unwrap();
    let ssot_agent: toml::Value = toml::from_str(&ssot_agent).unwrap();
    assert!(ssot_agent.get("instructions").is_some());
    assert!(ssot_agent.get("developer_instructions").is_none());

    let codex_agent = fs::read_to_string(dir.path().join(".codex/agents/executor.toml")).unwrap();
    let codex_agent: toml::Value = toml::from_str(&codex_agent).unwrap();
    assert!(codex_agent
        .get("developer_instructions")
        .and_then(toml::Value::as_str)
        .is_some_and(|instructions| instructions.contains("# Executor")));
    assert!(codex_agent.get("instructions").is_none());
    toml::from_str::<toml::Value>(
        &fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap(),
    )
    .unwrap();
    let hooks_json = fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&hooks_json).unwrap();
    assert!(hooks_json.contains("megara-hook-UserPromptSubmit"));
    let megara_config = fs::read_to_string(dir.path().join(".agents/megara.toml")).unwrap();
    assert!(megara_config.contains("locale = \"ko-KR\""));
    let agents_md = fs::read_to_string(dir.path().join(".codex/AGENTS.md")).unwrap();
    assert!(agents_md.contains("## Locale"));
    assert!(agents_md.contains("Locale: `ko-KR`"));
    assert!(agents_md.contains("Do not mix languages in explanatory prose"));
    assert!(agents_md.contains("progress updates, clarification questions, option labels"));
    assert!(agents_md.contains("stock English workflow phrases"));
    assert!(agents_md.contains("free-text values such as `question`, `options`"));
}

#[test]
fn installs_global_scope_codex_harness() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    let output = megara()
        .arg("install")
        .arg("--scope")
        .arg("global")
        .arg("--target")
        .arg("codex")
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.path().join(".megara/megara.toml").exists());
    assert!(home.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn sync_refreshes_managed_projection() {
    let dir = tempdir().unwrap();
    let agents = dir.path().join(".codex/AGENTS.md");
    let ssot_skill = dir.path().join(".agents/skills/deep-interview/SKILL.md");
    let projected_skill = dir.path().join(".codex/skills/deep-interview/SKILL.md");
    let ssot_agent = dir.path().join(".agents/agents/executor.toml");
    let projected_agent = dir.path().join(".codex/agents/executor.toml");
    let ssot_config = dir.path().join(".agents/megara.toml");

    let install = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    fs::write(&agents, "# MEGARA:MANAGED\nstale").unwrap();
    let mut ssot_content = fs::read_to_string(&ssot_skill).unwrap();
    ssot_content.push_str("\nSSOT EDIT TOKEN\n");
    fs::write(&ssot_skill, ssot_content).unwrap();
    let ssot_agent_content = fs::read_to_string(&ssot_agent).unwrap();
    fs::write(
        &ssot_agent,
        ssot_agent_content.replace(
            "Report changed files, decisions, verification performed, and remaining blockers.",
            "Report changed files, decisions, verification performed, and remaining blockers.\nSSOT AGENT TOKEN",
        ),
    )
    .unwrap();
    let config_content = fs::read_to_string(&ssot_config).unwrap();
    fs::write(&ssot_config, config_content.replace("ko-KR", "en-US")).unwrap();

    let sync = megara()
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        sync.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&sync.stderr)
    );
    let content = fs::read_to_string(agents).unwrap();
    assert!(content.contains("Megara Codex Harness"));
    assert!(content.contains("Locale: `en-US`"));
    let skill_content = fs::read_to_string(projected_skill).unwrap();
    assert!(skill_content.contains("SSOT EDIT TOKEN"));
    let agent_content = fs::read_to_string(projected_agent).unwrap();
    assert!(agent_content.contains("developer_instructions"));
    assert!(agent_content.contains("SSOT AGENT TOKEN"));
}

#[test]
fn projected_hook_runner_tracks_question_gate_and_blocks_mutation() {
    let dir = tempdir().unwrap();

    let install = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let script = dir.path().join(".codex/hooks/megara-hook.sh");
    let question_payload = br#"{
  "session_id": "sess-di",
  "cwd": "/tmp/project",
  "last_assistant_message": "Round 1 | Component: game | Targeting: Verification | Ambiguity: 42%\n\nWhat proves this is done?\n\nMegara Question Gate:\n- id: di-r1-verification\n- round: 1\n- component: game\n- dimension: Verification\n- question: What proves this is done?\n- options:\n  - Unit tests\n  - E2E tests\n- free_text: true\n\n"
}"#;
    let output = run_hook(&script, dir.path(), "Stop", None, question_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/deep-interview/sess-di.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "question_pending");
    assert_eq!(state["pending_question"]["id"], "di-r1-verification");
    assert_eq!(state["pending_question"]["options"][0], "Unit tests");
    assert_eq!(state["pending_question"]["free_text"], true);

    let answer_payload = br#"{"session_id":"sess-di","prompt":"Use both unit and E2E tests."}"#;
    let output = run_hook(
        &script,
        dir.path(),
        "UserPromptSubmit",
        None,
        answer_payload,
    );
    assert!(output.status.success());

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert!(state["pending_question"].is_null());
    assert_eq!(state["phase"], "interviewing");
    assert_eq!(state["questions"][0]["status"], "answered");
    assert_eq!(
        state["questions"][0]["answer"]["content"],
        "Use both unit and E2E tests."
    );

    let mutation_payload =
        br#"{"session_id":"sess-di","tool_input":{"command":"echo changed > app.js"}}"#;
    let output = run_hook(
        &script,
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MEGARA mutation guard"));

    let state_only_terminal_payload = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Megara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 12%\n- next: ralplan\n\n"
}"#;
    let output = run_hook(
        &script,
        dir.path(),
        "Stop",
        None,
        state_only_terminal_payload,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "crystallization_missing_spec");
    assert!(state.get("spec_path").is_none());

    let output = run_hook(
        &script,
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MEGARA mutation guard"));

    let terminal_payload = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "**Pending Approval Specification**\n\nGoal: build the verified game.\n\nAcceptance criteria:\n- Unit tests pass\n- E2E tests pass\n\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 12%\n- next: ralplan\n\n"
}"#;
    let output = run_hook(&script, dir.path(), "Stop", None, terminal_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["active"], false);
    assert_eq!(state["phase"], "crystallized");
    assert_eq!(state["ambiguity"], "12%");
    let spec_path = PathBuf::from(state["spec_path"].as_str().unwrap());
    assert!(spec_path.exists());
    assert_eq!(state["spec_sha256"].as_str().unwrap().len(), 64);
    assert!(state["spec_persisted_at"].as_str().is_some());
    let spec = fs::read_to_string(&spec_path).unwrap();
    assert!(spec.starts_with("---\n"));
    assert!(spec.contains("skill: \"deep-interview\""));
    assert!(spec.contains("session_id: \"sess-di\""));
    assert!(spec.contains("**Pending Approval Specification**"));
    assert!(spec.contains("Goal: build the verified game."));
    assert!(spec.contains("Megara Workflow State:"));

    let spec_index = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/deep-interview/specs/index.jsonl"),
    )
    .unwrap();
    assert!(spec_index.contains("\"event\":\"spec_persisted\""));
    assert!(spec_index.contains(spec_path.to_str().unwrap()));

    let output = run_hook(
        &script,
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let events = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/deep-interview/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"question_pending\""));
    assert!(events.contains("\"event\":\"question_answered\""));
    assert!(events.contains("\"event\":\"mutation_blocked\""));
    assert!(events.contains("\"event\":\"spec_missing\""));
    assert!(events.contains("\"event\":\"spec_persisted\""));
    assert!(events.contains("\"event\":\"workflow_state\""));
    assert!(events.contains(spec_path.to_str().unwrap()));
}

#[test]
fn projected_hook_runner_records_runtime_event() {
    let dir = tempdir().unwrap();

    let install = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let script = dir.path().join(".codex/hooks/megara-hook.sh");
    let mut child = Command::new("sh")
        .arg(&script)
        .arg("--runtime")
        .arg("codex")
        .arg("--event")
        .arg("UserPromptSubmit")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(br#"{"prompt":"hello"}"#)
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());

    let mut second_child = Command::new("sh")
        .arg(&script)
        .arg("--runtime")
        .arg("codex")
        .arg("--event")
        .arg("UserPromptSubmit")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    second_child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(br#"{"prompt":"second"}"#)
        .unwrap();
    let status = second_child.wait().unwrap();
    assert!(status.success());

    let mut stop_child = Command::new("sh")
        .arg(&script)
        .arg("--runtime")
        .arg("codex")
        .arg("--event")
        .arg("Stop")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    stop_child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(br#"{"last_assistant_message":"question?"}"#)
        .unwrap();
    let status = stop_child.wait().unwrap();
    assert!(status.success());

    let log = fs::read_to_string(dir.path().join(".agents/state/hooks/events.jsonl")).unwrap();
    assert!(log.contains("\"runtime\":\"codex\""));
    assert!(log.contains("\"event\":\"UserPromptSubmit\""));
    assert!(log.contains("/payloads/codex/UserPromptSubmit/"));
    let payload = fs::read_to_string(
        dir.path()
            .join(".agents/state/hooks/last-codex-UserPromptSubmit.json"),
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
            .join(".agents/state/hooks/conversation-events.jsonl"),
    )
    .unwrap();
    assert!(conversation_events.contains("\"role\":\"user\""));
    assert!(conversation_events.contains("\"role\":\"assistant\""));

    let conversation =
        fs::read_to_string(dir.path().join(".agents/state/hooks/conversation.jsonl"))
            .unwrap_or_default();
    if !conversation.is_empty() {
        assert!(conversation.contains("\"content\":\"hello\""));
        assert!(conversation.contains("\"content\":\"second\""));
        assert!(conversation.contains("\"content\":\"question?\""));
    }
}

#[test]
fn doctor_reports_missing_then_ok() {
    let dir = tempdir().unwrap();

    let missing = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(missing.status.success());
    let missing_stdout = String::from_utf8_lossy(&missing.stdout);
    assert!(missing_stdout.contains("\"ok\": false"));

    let install = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let ssot_skill = dir.path().join(".agents/skills/deep-interview/SKILL.md");
    let mut ssot_content = fs::read_to_string(&ssot_skill).unwrap();
    ssot_content.push_str("\nDOCTOR DRIFT TOKEN\n");
    fs::write(&ssot_skill, ssot_content).unwrap();

    let stale = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(stale.status.success());
    let stale_stdout = String::from_utf8_lossy(&stale.stdout);
    assert!(stale_stdout.contains("\"ok\": false"));
    assert!(stale_stdout.contains(".codex/skills/deep-interview/SKILL.md"));

    let sync = megara()
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(sync.status.success());

    let ok = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(ok.status.success());
    let ok_stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(ok_stdout.contains("\"ok\": true"));
}

#[test]
fn lists_targets_and_templates() {
    let targets = megara().arg("targets").arg("list").output().unwrap();
    assert!(targets.status.success());
    assert!(String::from_utf8_lossy(&targets.stdout).contains("codex"));

    let templates = megara().arg("templates").arg("list").output().unwrap();
    assert!(templates.status.success());
    let stdout = String::from_utf8_lossy(&templates.stdout);
    assert!(stdout.contains("deep-interview"));
    assert!(stdout.contains("deep-interview/auto-research-greenfield"));
    assert!(stdout.contains("megara-hook"));
}
