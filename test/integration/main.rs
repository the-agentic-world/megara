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

fn megara_with_codex_home(codex_home: &Path) -> Command {
    let mut command = megara();
    command.env("CODEX_HOME", codex_home);
    command
}

fn run_hook(
    project_root: &Path,
    cwd: &Path,
    event: &str,
    matcher: Option<&str>,
    payload: &[u8],
) -> Output {
    let mut command = megara();
    command
        .arg("hook")
        .arg("--scope")
        .arg("project")
        .arg("--project-root")
        .arg(project_root)
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

fn install_project_harness(project: &Path, codex_home: &Path) {
    let install = megara_with_codex_home(codex_home)
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(project)
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );
}

fn ready_ralplan_reviews_payload(session_id: &str) -> String {
    let message = "Review coverage complete.\n\nMegara Review Pass:\n- role: planner\n- round: 1\n- verdict: DRAFT\n- summary: Planner pass is ready.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: architect\n- round: 1\n- verdict: CLEAR\n- summary: Architecture pass is clear.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: critic\n- round: 1\n- verdict: OKAY\n- summary: Critic pass approves planning quality.\n- required_fixes:\n  - none\n\n";
    serde_json::json!({
        "session_id": session_id,
        "last_assistant_message": message,
    })
    .to_string()
}

fn pending_ralplan_plan_payload(session_id: &str, plan_id: &str, summary: &str) -> String {
    let message = format!(
        "**Pending Execution Plan**\n\nSummary: {summary}\n\nSteps:\n- Keep the change small.\n- Verify the expected behavior.\n\nAcceptance criteria:\n- Existing tests pass.\n\nMegara Plan Gate:\n- id: {plan_id}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {plan_id}\n- next: approval\n\n"
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
        "**Pending Execution Plan**\n\nSummary: {summary}\n\nInput lock: {input_spec_sha256}\n\nSteps:\n- Keep the change small.\n- Verify the expected behavior.\n\nAcceptance criteria:\n- Existing tests pass.\n\nMegara Plan Gate:\n- id: {plan_id}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {plan_id}\n- input_spec_sha256: {input_spec_sha256}\n- next: approval\n\n"
    );
    serde_json::json!({
        "session_id": session_id,
        "last_assistant_message": message,
    })
    .to_string()
}

fn deep_interview_approval_prompt() -> String {
    "Megara Approval Gate:\n- approved_workflow: deep-interview\n- approved_status: crystallized\n- approved_ambiguity: 9%\n- next_workflow: ralplan\n- implementation_allowed_now: false\n"
        .to_string()
}

fn passing_quality_gate_json() -> String {
    serde_json::json!({
        "architectReview": {
            "recommendation": "APPROVE",
            "architectureStatus": "CLEAR",
            "productStatus": "CLEAR",
            "codeStatus": "CLEAR",
            "evidence": "Architecture, product behavior, and code boundaries reviewed.",
            "reviewedFiles": ["reviewed.md"],
            "blockers": []
        },
        "executorQa": {
            "status": "passed",
            "e2eStatus": "passed",
            "redTeamStatus": "passed",
            "evidence": "Focused tests and manual regression checks passed.",
            "commands": ["cargo test"],
            "artifactRefs": ["verification.log"],
            "blockers": []
        },
        "iteration": {
            "status": "passed",
            "fullRerun": true,
            "evidence": "Final verification reran after cleanup.",
            "commands": ["cargo test"],
            "artifactRefs": ["verification.log"],
            "blockers": []
        }
    })
    .to_string()
}

#[test]
fn installs_project_scope_codex_harness() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let output = megara_with_codex_home(codex_home.path())
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
    assert!(dir.path().join(".agents/agents/executor.toml").exists());
    assert!(dir.path().join(".codex/hooks.json").exists());
    assert!(dir.path().join(".codex/agents/executor.toml").exists());
    let skill =
        fs::read_to_string(dir.path().join(".codex/skills/deep-interview/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\n"));
    assert!(skill.contains("MEGARA:MANAGED"));
    assert!(skill.contains("<configured-locale ambiguity label>: NN%"));
    assert!(skill.contains("<configured-locale round 0 topology heading>"));
    assert!(skill.contains("Calculate ambiguity as `100 - weighted_clarity`"));
    assert!(skill.contains("Ambiguity is bidirectional and non-monotonic"));
    assert!(skill.contains("Interview ledger update:"));
    assert!(skill.contains("Megara Question Gate:"));
    assert!(skill.contains("Megara Workflow State:"));
    assert!(skill.contains("locked markdown artifact"));
    assert!(skill.contains("spec_path"));
    assert!(skill.contains("Write every user-facing sentence in the configured locale"));
    assert!(skill.contains("option labels"));
    assert!(skill.contains("free-text values"));
    assert!(!skill.contains("Deep Interview threshold:"));
    assert!(!skill.contains("I'm reading this as"));
    assert!(!skill.contains("Restate gate"));
    let ralplan = fs::read_to_string(dir.path().join(".codex/skills/ralplan/SKILL.md")).unwrap();
    assert!(ralplan.contains("Megara Review Pass:"));
    assert!(ralplan.contains("Megara Plan Gate:"));
    assert!(ralplan.contains("Megara Approval Gate:"));
    assert!(ralplan.contains("input_spec_sha256"));
    assert!(ralplan.contains("plan_sha256"));
    assert!(ralplan.contains("pending_approval"));
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
    let hooks: serde_json::Value = serde_json::from_str(&hooks_json).unwrap();
    assert!(hooks_json.contains("megara-hook-UserPromptSubmit"));
    assert!(hooks_json.contains("megara-hook-PreToolUse"));
    assert!(
        hooks_json.contains("hook --managed-marker MEGARA:MANAGED --scope project --project-root")
    );
    assert!(hooks_json.contains("--runtime codex --event UserPromptSubmit"));
    let command = hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(command.starts_with('"'));
    assert!(!command.starts_with("megara hook"));
    assert!(!hooks_json.contains("megara-hook.sh"));
    assert!(!hooks_json.contains("python3"));
    assert!(!hooks_json.contains(r#""matcher": "Bash""#));
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
    let codex_home = home.path().join(".codex");

    let output = megara_with_codex_home(&codex_home)
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
    let codex_home = tempdir().unwrap();
    let agents = dir.path().join(".codex/AGENTS.md");
    let ssot_skill = dir.path().join(".agents/skills/deep-interview/SKILL.md");
    let projected_skill = dir.path().join(".codex/skills/deep-interview/SKILL.md");
    let ssot_agent = dir.path().join(".agents/agents/executor.toml");
    let projected_agent = dir.path().join(".codex/agents/executor.toml");
    let ssot_config = dir.path().join(".agents/megara.toml");

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

    let sync = megara_with_codex_home(codex_home.path())
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

    let question_payload = br#"{
  "session_id": "sess-di",
  "cwd": "/tmp/project",
  "last_assistant_message": "Round 1 | Component: game | Targeting: Verification | Ambiguity: 42%\n\nWhat proves this is done?\n\nMegara Question Gate:\n- id: di-r1-verification\n- round: 1\n- component: game\n- dimension: Verification\n- question: What proves this is done?\n- options:\n  - Unit tests\n  - E2E tests\n- free_text: true\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, question_payload);
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
        dir.path(),
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
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MEGARA mutation guard"));

    let edit_payload = br#"{"session_id":"sess-di","tool_name":"apply_patch","tool_input":{"patch":"*** Begin Patch\n*** End Patch\n"}}"#;
    let output = run_hook(dir.path(), dir.path(), "PreToolUse", None, edit_payload);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MEGARA mutation guard"));

    let read_payload =
        br#"{"session_id":"sess-di","tool_name":"Read","tool_input":{"file_path":"app.js"}}"#;
    let output = run_hook(dir.path(), dir.path(), "PreToolUse", None, read_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_only_terminal_payload = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Megara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 12%\n- next: ralplan\n\n"
}"#;
    let output = run_hook(
        dir.path(),
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
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MEGARA mutation guard"));

    let terminal_payload = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "**Pending Approval Specification**\n\nGoal: build the verified game.\n\nTranscript summary:\nMegara Question Gate:\n- id: di-old-transcript\n- round: 0\n- component: topology\n- dimension: Outcome clarity\n- question: Historical question embedded in the final spec.\n- options:\n  - Historical option\n- free_text: true\n\nAcceptance criteria:\n- Unit tests pass\n- E2E tests pass\n\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 12%\n- next: ralplan\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, terminal_payload);
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
    assert!(state["pending_question"].is_null());
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
        dir.path(),
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
    assert!(!events.contains("di-old-transcript"));
}

#[test]
fn projected_hook_runner_tracks_ralplan_gate_and_approval() {
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

    let interview_message = "**Pending Approval Specification**\n\nGoal: add Tetris as a second game mode.\n\nConstraints:\n- Keep the existing 2048 flow working.\n- Add content routing before game-specific state changes.\n\nAcceptance criteria:\n- Existing 2048 flow still works.\n- Tetris can start and restart.\n\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 8%\n- next: ralplan\n\n";
    let interview_payload = serde_json::json!({
        "session_id": "sess-rp",
        "last_assistant_message": interview_message,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        interview_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let deep_state_path = dir
        .path()
        .join(".agents/state/workflows/deep-interview/sess-rp.json");
    let deep_state = fs::read_to_string(&deep_state_path).unwrap();
    let deep_state: serde_json::Value = serde_json::from_str(&deep_state).unwrap();
    let spec_path = deep_state["spec_path"].as_str().unwrap().to_string();
    let spec_sha256 = deep_state["spec_sha256"].as_str().unwrap().to_string();

    let early_plan_message = "**Pending Execution Plan**\n\nSummary: this should wait for review coverage.\n\nMegara Plan Gate:\n- id: rp-too-early\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-too-early\n- next: approval\n\n";
    let early_plan_payload = serde_json::json!({
        "session_id": "sess-rp",
        "last_assistant_message": early_plan_message,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        early_plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-rp.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "review_incomplete");
    assert_eq!(state["approval_status"], "blocked");
    assert!(state.get("plan_path").is_none());

    let review_message = "Planner, architect, and critic passes complete.\n\nMegara Review Pass:\n- role: planner\n- round: 1\n- verdict: DRAFT\n- summary: Initial sequence is ready for architecture review.\n- required_fixes:\n  - Architect must verify runtime boundaries.\n\nMegara Review Pass:\n- role: architect\n- round: 1\n- verdict: CLEAR\n- summary: Runtime boundaries are acceptable for this plan.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: critic\n- round: 1\n- verdict: OKAY\n- summary: The plan is specific and verifiable enough to ask for approval.\n- required_fixes:\n  - none\n\n";
    let review_payload = serde_json::json!({
        "session_id": "sess-rp",
        "last_assistant_message": review_message,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["skill"], "ralplan");
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "reviewing");
    assert_eq!(state["reviews"][0]["role"], "planner");
    assert_eq!(state["reviews"][0]["round"], 1);
    assert_eq!(state["reviews"][0]["verdict"], "DRAFT");
    assert_eq!(state["reviews"][0]["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(state["reviews"][1]["role"], "architect");
    assert_eq!(state["reviews"][1]["verdict"], "CLEAR");
    assert_eq!(state["reviews"][1]["required_fixes"][0], "none");
    assert_eq!(state["reviews"][2]["role"], "critic");
    assert_eq!(state["reviews"][2]["verdict"], "OKAY");
    let review_path = PathBuf::from(state["reviews"][0]["path"].as_str().unwrap());
    let architect_review_path = PathBuf::from(state["reviews"][1]["path"].as_str().unwrap());
    let critic_review_path = PathBuf::from(state["reviews"][2]["path"].as_str().unwrap());
    assert!(review_path.exists());
    assert!(architect_review_path.exists());
    assert!(critic_review_path.exists());
    let review = fs::read_to_string(&review_path).unwrap();
    assert!(review.contains("skill: \"ralplan\""));
    assert!(review.contains("role: \"planner\""));
    assert!(review.contains("Architect must verify runtime boundaries."));
    let architect_review = fs::read_to_string(&architect_review_path).unwrap();
    assert!(architect_review.contains("role: \"architect\""));
    assert!(architect_review.contains("Runtime boundaries are acceptable"));
    let critic_review = fs::read_to_string(&critic_review_path).unwrap();
    assert!(critic_review.contains("role: \"critic\""));
    assert!(critic_review.contains("specific and verifiable"));

    let mutation_payload =
        br#"{"session_id":"sess-rp","tool_input":{"command":"echo changed > app.js"}}"#;
    let output = run_hook(
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ralplan is active"));

    let plan_payload = br#"{
  "session_id": "sess-rp",
  "last_assistant_message": "**Pending Execution Plan**\n\nSummary: add a Tetris mode without changing the current menu contract.\n\nNotes:\nThe plan body may mention this literal marker before the actual trailer.\n\nMegara Plan Gate:\nThis sentence is plan content, not the control block.\n\nSteps:\n- Add content routing.\n- Add Tetris state and rendering.\n\nAcceptance criteria:\n- Existing 2048 flow still works.\n- Tetris can start and restart.\n\nMegara Plan Gate:\n- id: rp-add-tetris\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-add-tetris\n- next: approval\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, plan_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["skill"], "ralplan");
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["approval_status"], "pending");
    assert_eq!(state["plan_id"], "rp-add-tetris");
    assert_eq!(state["plan_gate"]["options"][2], "approve_team");
    assert_eq!(state["plan_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(state["input_spec_path"].as_str().unwrap(), spec_path);
    assert_eq!(state["input_spec_sha256"].as_str().unwrap(), spec_sha256);
    assert_eq!(state["reviews"][0]["role"], "planner");
    assert_eq!(state["reviews"][1]["role"], "architect");
    assert_eq!(state["reviews"][2]["role"], "critic");

    let plan_path = PathBuf::from(state["plan_path"].as_str().unwrap());
    assert!(plan_path.exists());
    let plan = fs::read_to_string(&plan_path).unwrap();
    assert!(plan.starts_with("---\n"));
    assert!(plan.contains("skill: \"ralplan\""));
    assert!(plan.contains("plan_id: \"rp-add-tetris\""));
    assert!(plan.contains(&format!("input_spec_sha256: \"{spec_sha256}\"")));
    assert!(plan.contains("This sentence is plan content, not the control block."));
    assert!(plan.contains("**Pending Execution Plan**"));
    assert!(plan.contains("Megara Plan Gate:"));
    assert!(!plan.contains("- id: rp-add-tetris"));

    let output = run_hook(
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ralplan is active"));

    let reject_prompt = "Megara Approval Gate:\n- plan_id: rp-add-tetris\n- plan_sha256: 0000000000000000000000000000000000000000000000000000000000000000\n- handoff_target: ultragoal\n";
    let reject_payload = serde_json::json!({
        "session_id": "sess-rp",
        "prompt": reject_prompt,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        reject_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rejected = fs::read_to_string(&state_path).unwrap();
    let rejected: serde_json::Value = serde_json::from_str(&rejected).unwrap();
    assert_eq!(rejected["active"], true);
    assert_eq!(rejected["approval_status"], "approval_gate_mismatch");

    let plan_sha256 = state["plan_sha256"].as_str().unwrap();
    let approve_prompt = format!(
        "Megara Approval Gate:\n- plan_id: rp-add-tetris\n- plan_sha256: {plan_sha256}\n- handoff_target: ultragoal\n"
    );
    let approve_payload = serde_json::json!({
        "session_id": "sess-rp",
        "prompt": approve_prompt,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        approve_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let approved = fs::read_to_string(&state_path).unwrap();
    let approved: serde_json::Value = serde_json::from_str(&approved).unwrap();
    assert_eq!(approved["active"], false);
    assert_eq!(approved["phase"], "approved");
    assert_eq!(approved["approval_status"], "approved");
    assert_eq!(approved["approved_handoff_target"], "ultragoal");
    assert_eq!(approved["approved_plan_id"], "rp-add-tetris");
    assert_eq!(approved["approved_plan_sha256"], state["plan_sha256"]);

    let output = run_hook(
        dir.path(),
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

    let plan_index = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/ralplan/plans/index.jsonl"),
    )
    .unwrap();
    assert!(plan_index.contains("\"event\":\"plan_persisted\""));
    assert!(plan_index.contains(plan_path.to_str().unwrap()));
    assert!(plan_index.contains(&spec_sha256));

    let review_index = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/ralplan/reviews/index.jsonl"),
    )
    .unwrap();
    assert!(review_index.contains("\"event\":\"review_persisted\""));
    assert!(review_index.contains(review_path.to_str().unwrap()));
    assert!(review_index.contains(architect_review_path.to_str().unwrap()));
    assert!(review_index.contains(critic_review_path.to_str().unwrap()));

    let events = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/ralplan/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"review_persisted\""));
    assert!(events.contains("\"event\":\"plan_persisted\""));
    assert!(events.contains("\"event\":\"mutation_blocked\""));
    assert!(events.contains("\"event\":\"plan_approval_rejected\""));
    assert!(events.contains("\"event\":\"plan_approved\""));
    assert!(events.contains("\"handoff_target\":\"ultragoal\""));
    assert!(events.contains(&spec_sha256));
}

#[test]
fn projected_hook_runner_allows_direct_ralplan_without_interview() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let review_payload = ready_ralplan_reviews_payload("sess-direct-rp");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let plan_payload = pending_ralplan_plan_payload(
        "sess-direct-rp",
        "rp-direct",
        "plan directly without a deep-interview lock.",
    );
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-direct-rp.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["approval_status"], "pending");
    assert_eq!(state["plan_id"], "rp-direct");
    assert!(state["plan_sha256"].as_str().is_some());
    assert!(state.get("input_spec_sha256").is_none());
}

#[test]
fn projected_hook_runner_blocks_deep_interview_handoff_without_current_lock() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let stale_interview_message = "**Pending Approval Specification**\n\nGoal: add Yacht to dice poker.\n\nAcceptance criteria:\n- Yacht is scored.\n\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 8%\n- next: ralplan\n\n";
    let stale_interview_payload = serde_json::json!({
        "session_id": "sess-old-di",
        "last_assistant_message": stale_interview_message,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        stale_interview_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let approval_payload = serde_json::json!({
        "session_id": "sess-dashboard",
        "prompt": deep_interview_approval_prompt(),
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        approval_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-dashboard.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["requires_input_lock"], true);
    assert_eq!(state["phase"], "input_lock_required");

    let review_payload = ready_ralplan_reviews_payload("sess-dashboard");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let plan_payload = pending_ralplan_plan_payload_with_input_spec(
        "sess-dashboard",
        "rp-dashboard",
        "add a dashboard from conversation-only deep-interview output.",
        "none",
    );
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["phase"], "input_lock_blocked");
    assert_eq!(state["approval_status"], "blocked");
    assert_eq!(
        state["input_lock_status"],
        "missing_persisted_deep_interview_lock"
    );
    assert!(state.get("plan_path").is_none());
    assert!(state.get("plan_sha256").is_none());

    let events = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/ralplan/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"input_lock_required\""));
    assert!(events.contains("\"event\":\"input_lock_blocked\""));
    assert!(events.contains("missing_persisted_deep_interview_lock"));
}

#[test]
fn projected_hook_runner_accepts_deep_interview_handoff_with_matching_lock() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let interview_message = "**Pending Approval Specification**\n\nGoal: add a dashboard launcher.\n\nAcceptance criteria:\n- Dashboard is the default screen.\n- Cards enter each game.\n\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 9%\n- next: ralplan\n\n";
    let interview_payload = serde_json::json!({
        "session_id": "sess-locked-rp",
        "last_assistant_message": interview_message,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        interview_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let deep_state_path = dir
        .path()
        .join(".agents/state/workflows/deep-interview/sess-locked-rp.json");
    let deep_state = fs::read_to_string(&deep_state_path).unwrap();
    let deep_state: serde_json::Value = serde_json::from_str(&deep_state).unwrap();
    let spec_sha256 = deep_state["spec_sha256"].as_str().unwrap().to_string();

    let approval_payload = serde_json::json!({
        "session_id": "sess-locked-rp",
        "prompt": deep_interview_approval_prompt(),
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        approval_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let review_payload = ready_ralplan_reviews_payload("sess-locked-rp");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let plan_payload = pending_ralplan_plan_payload_with_input_spec(
        "sess-locked-rp",
        "rp-dashboard-locked",
        "add a dashboard using the persisted deep-interview lock.",
        &spec_sha256,
    );
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-locked-rp.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["approval_status"], "pending");
    assert_eq!(state["requires_input_lock"], true);
    assert_eq!(state["input_spec_sha256"], spec_sha256);
    assert!(state["plan_sha256"].as_str().is_some());
}

#[test]
fn projected_hook_runner_blocks_ralplan_when_interview_is_active() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let interview_payload = br#"{
  "session_id": "sess-active-di",
  "last_assistant_message": "Clarify before planning.\n\nMegara Question Gate:\n- id: di-active\n- round: 1\n- component: scope\n- dimension: Goal clarity\n- question: What should be clarified first?\n- options:\n  - Scope\n- free_text: true\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, interview_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let review_payload = ready_ralplan_reviews_payload("sess-active-di");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let plan_payload = pending_ralplan_plan_payload(
        "sess-active-di",
        "rp-blocked",
        "should not pass while deep-interview is active.",
    );
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-active-di.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["phase"], "handoff_not_ready");
    assert_eq!(state["approval_status"], "blocked");
    assert_eq!(state["blocked_by"], "deep-interview");
    assert_eq!(state["blocked_phase"], "question_pending");
    assert!(state.get("plan_path").is_none());

    let events = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/ralplan/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"handoff_blocked\""));
}

#[test]
fn projected_hook_runner_invalidates_reviews_after_refine() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let review_payload = ready_ralplan_reviews_payload("sess-refine");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(output.status.success());

    let first_plan_payload =
        pending_ralplan_plan_payload("sess-refine", "rp-before-refine", "first plan.");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        first_plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-refine.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["reviews"].as_array().unwrap().len(), 3);
    assert!(state["plan_sha256"].as_str().is_some());

    let refine_payload = br#"{"session_id":"sess-refine","prompt":"refine"}"#;
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        refine_payload,
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let refined = fs::read_to_string(&state_path).unwrap();
    let refined: serde_json::Value = serde_json::from_str(&refined).unwrap();
    assert_eq!(refined["phase"], "refining");
    assert_eq!(refined["approval_status"], "refine_requested");
    assert!(refined["reviews"].as_array().unwrap().is_empty());
    assert!(refined.get("plan_sha256").is_none());
    assert!(refined.get("plan_path").is_none());

    let second_plan_payload =
        pending_ralplan_plan_payload("sess-refine", "rp-after-refine", "second plan.");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        second_plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let blocked = fs::read_to_string(&state_path).unwrap();
    let blocked: serde_json::Value = serde_json::from_str(&blocked).unwrap();
    assert_eq!(blocked["phase"], "review_incomplete");
    assert_eq!(blocked["approval_status"], "blocked");
    assert!(blocked.get("plan_sha256").is_none());
}

#[test]
fn projected_hook_runner_prioritizes_ralplan_decision_over_stale_interview_question() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let review_payload = ready_ralplan_reviews_payload("sess-stale-di");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(output.status.success());

    let plan_payload = pending_ralplan_plan_payload("sess-stale-di", "rp-stale-di", "safe plan.");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let ralplan_state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-stale-di.json");
    let ralplan_state = fs::read_to_string(&ralplan_state_path).unwrap();
    let ralplan_state: serde_json::Value = serde_json::from_str(&ralplan_state).unwrap();
    let plan_sha256 = ralplan_state["plan_sha256"].as_str().unwrap();

    let stale_question_payload = br#"{
  "session_id": "sess-stale-di",
  "last_assistant_message": "Late stale question.\n\nMegara Question Gate:\n- id: di-stale\n- round: 1\n- component: stale\n- dimension: Stale state\n- question: This stale question should not consume plan approval.\n- options:\n  - stale\n- free_text: true\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, stale_question_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let approve_prompt = format!(
        "Megara Approval Gate:\n- plan_id: rp-stale-di\n- plan_sha256: {plan_sha256}\n- handoff_target: ultragoal\n"
    );
    let approve_payload = serde_json::json!({
        "session_id": "sess-stale-di",
        "prompt": approve_prompt,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        approve_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let approved = fs::read_to_string(&ralplan_state_path).unwrap();
    let approved: serde_json::Value = serde_json::from_str(&approved).unwrap();
    assert_eq!(approved["phase"], "approved");
    assert_eq!(approved["approved_handoff_target"], "ultragoal");

    let deep_state_path = dir
        .path()
        .join(".agents/state/workflows/deep-interview/sess-stale-di.json");
    let deep_state = fs::read_to_string(&deep_state_path).unwrap();
    let deep_state: serde_json::Value = serde_json::from_str(&deep_state).unwrap();
    assert_eq!(deep_state["pending_question"]["id"], "di-stale");
    assert_eq!(deep_state["pending_question"]["status"], "pending");
}

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
        fs::read_to_string(dir.path().join(".agents/state/hooks/conversation.jsonl")).unwrap();
    assert!(conversation.contains("\"content\":\"hello\""));
    assert!(conversation.contains("\"content\":\"second\""));
    assert!(conversation.contains("\"content\":\"question?\""));
}

#[test]
fn project_hook_rejects_cwd_outside_project_root() {
    let project = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let install = megara_with_codex_home(codex_home.path())
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let output = run_hook(
        project.path(),
        outside.path(),
        "UserPromptSubmit",
        None,
        br#"{"prompt":"outside"}"#,
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside project root"));
    assert!(!project
        .path()
        .join(".agents/state/hooks/events.jsonl")
        .exists());
    assert!(!outside
        .path()
        .join(".agents/state/hooks/events.jsonl")
        .exists());
}

#[test]
fn ultragoal_cli_creates_goals_and_records_completion_receipt() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let brief = "@goal: Board shell\nBuild the playable board shell.\n\n@goal Score model\nTrack scores and losses.";
    let direct = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg("sess-ug")
        .arg("create-goals")
        .arg("--brief")
        .arg(brief)
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!direct.status.success());
    assert!(String::from_utf8_lossy(&direct.stderr).contains("--allow-direct"));

    let review_payload = ready_ralplan_reviews_payload("sess-ug");
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        review_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan_id = "rp-ultragoal";
    let plan_message = format!(
        "**Pending Execution Plan**\n\n{brief}\n\nAcceptance criteria:\n- Both goals are verified before completion.\n\nMegara Plan Gate:\n- id: {plan_id}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {plan_id}\n- next: approval\n\n"
    );
    let plan_payload = serde_json::json!({
        "session_id": "sess-ug",
        "last_assistant_message": plan_message,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        plan_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let ralplan_state_path = dir
        .path()
        .join(".agents/state/workflows/ralplan/sess-ug.json");
    let ralplan_state = fs::read_to_string(&ralplan_state_path).unwrap();
    let ralplan_state: serde_json::Value = serde_json::from_str(&ralplan_state).unwrap();
    let plan_sha256 = ralplan_state["plan_sha256"].as_str().unwrap();
    let approval_prompt = format!(
        "Megara Approval Gate:\n- plan_id: {plan_id}\n- plan_sha256: {plan_sha256}\n- handoff_target: ultragoal\n"
    );
    let approval_payload = serde_json::json!({
        "session_id": "sess-ug",
        "prompt": approval_prompt,
    })
    .to_string();
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        approval_payload.as_bytes(),
    );
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let create = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg("sess-ug")
        .arg("create-goals")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );

    let state_dir = dir.path().join(".agents/state/workflows/ultragoal/sess-ug");
    assert!(state_dir.join("brief.md").exists());
    let goals_path = state_dir.join("goals.json");
    let goals = fs::read_to_string(&goals_path).unwrap();
    let goals: serde_json::Value = serde_json::from_str(&goals).unwrap();
    assert_eq!(goals["source"]["kind"], "ralplan");
    assert_eq!(goals["source"]["ralplan_plan_id"], plan_id);
    assert_eq!(goals["goals"][0]["id"], "G001");
    assert_eq!(goals["goals"][0]["title"], "Board shell");
    assert_eq!(goals["goals"][1]["id"], "G002");
    assert_eq!(goals["goals"][1]["status"], "pending");
    let runtime_state_path = dir
        .path()
        .join(".agents/state/workflows/ultragoal/sess-ug.json");
    let runtime_state = fs::read_to_string(&runtime_state_path).unwrap();
    let runtime_state: serde_json::Value = serde_json::from_str(&runtime_state).unwrap();
    assert_eq!(runtime_state["phase"], "goal_planning");

    let next = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg("sess-ug")
        .arg("complete-goals")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        next.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&next.stderr)
    );
    let next: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_eq!(next["state"], "started");
    assert_eq!(next["next_goal"]["id"], "G001");
    let runtime_state = fs::read_to_string(&runtime_state_path).unwrap();
    let runtime_state: serde_json::Value = serde_json::from_str(&runtime_state).unwrap();
    assert_eq!(runtime_state["phase"], "active");
    assert_eq!(runtime_state["active_goal_id"], "G001");

    fs::write(
        dir.path().join("reviewed.md"),
        "Reviewed board and score boundaries.",
    )
    .unwrap();
    fs::write(
        dir.path().join("verification.log"),
        "cargo test passed; manual board smoke check passed",
    )
    .unwrap();
    let quality_gate = dir.path().join("quality-gate.json");
    fs::write(&quality_gate, passing_quality_gate_json()).unwrap();
    let checkpoint = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg("sess-ug")
        .arg("checkpoint")
        .arg("--goal-id")
        .arg("G001")
        .arg("--status")
        .arg("complete")
        .arg("--evidence")
        .arg("cargo test passed; manual board smoke check passed")
        .arg("--quality-gate-json")
        .arg(&quality_gate)
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        checkpoint.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&checkpoint.stderr)
    );
    let checkpoint: serde_json::Value = serde_json::from_slice(&checkpoint.stdout).unwrap();
    assert_eq!(checkpoint["goal"]["status"], "complete");
    assert_eq!(checkpoint["goal"]["completion_receipt"]["goal_id"], "G001");
    assert_eq!(checkpoint["next_goal_started"]["id"], "G002");

    let goals = fs::read_to_string(&goals_path).unwrap();
    let goals: serde_json::Value = serde_json::from_str(&goals).unwrap();
    assert_eq!(goals["goals"][0]["status"], "complete");
    assert_eq!(
        goals["goals"][0]["completion_receipt"]["receipt_id"]
            .as_str()
            .unwrap()
            .len(),
        19
    );
    assert_eq!(goals["goals"][1]["status"], "active");

    let status = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg("sess-ug")
        .arg("status")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["counts"]["complete"], 1);
    assert_eq!(status["counts"]["active"], 1);
    assert_eq!(status["active_goal"]["id"], "G002");

    let ledger = fs::read_to_string(state_dir.join("ledger.jsonl")).unwrap();
    assert!(ledger.contains("\"event\":\"goals_created\""));
    assert!(ledger.contains("\"event\":\"goal_started\""));
    assert!(ledger.contains("\"event\":\"goal_checkpointed\""));
}

#[test]
fn hook_blocks_ultragoal_goal_planning_but_allows_active_goal_mutations() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let planning_payload = br#"{
  "session_id": "sess-ug-hook",
  "last_assistant_message": "Megara Workflow State:\n- skill: ultragoal\n- status: goal_planning\n- next: create goals\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, planning_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state_path = dir
        .path()
        .join(".agents/state/workflows/ultragoal/sess-ug-hook.json");
    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "goal_planning");

    let mutation_payload =
        br#"{"session_id":"sess-ug-hook","tool_input":{"command":"echo changed > app.js"}}"#;
    let output = run_hook(
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        mutation_payload,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("complete-goals"));

    let active_payload = br#"{
  "session_id": "sess-ug-hook",
  "last_assistant_message": "Megara Workflow State:\n- skill: ultragoal\n- status: active\n- next: execute G001\n\n"
}"#;
    let output = run_hook(dir.path(), dir.path(), "Stop", None, active_payload);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "active");
    assert_eq!(state["next"], "execute G001");

    let output = run_hook(
        dir.path(),
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
            .join(".agents/state/workflows/ultragoal/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"workflow_state\""));
    assert!(events.contains("\"event\":\"mutation_blocked\""));
}

#[test]
fn doctor_reports_missing_then_ok() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

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

    let sync = megara_with_codex_home(codex_home.path())
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
    assert!(ok_stdout.contains("\"warnings\": []"));
}

#[test]
fn install_registers_codex_hook_trust_state_once() {
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
    assert!(
        install.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );
    let install_stdout = String::from_utf8_lossy(&install.stdout);
    assert!(install_stdout.contains("hook trust: registered=3, unchanged=0"));

    let config_path = codex_home.path().join("config.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let hooks_path = fs::canonicalize(dir.path().join(".codex/hooks.json")).unwrap();
    let hooks_path = hooks_path.display().to_string();
    for event in ["pre_tool_use", "stop", "user_prompt_submit"] {
        let header = format!("[hooks.state.\"{hooks_path}:{event}:0:0\"]");
        assert_eq!(occurrences(&config, &header), 1);
    }
    assert_eq!(occurrences(&config, "trusted_hash = \"sha256:"), 3);

    let sync = megara_with_codex_home(codex_home.path())
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
    let sync_stdout = String::from_utf8_lossy(&sync.stdout);
    assert!(sync_stdout.contains("hook trust: registered=0, unchanged=3"));
    assert_eq!(fs::read_to_string(config_path).unwrap(), config);
}

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
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
    assert!(!stdout.contains("megara-hook"));
}
