use super::hook_deep_interview_support::state_path_for;
use super::hook_ralplan_support::{
    assert_success, read_json, submit_plan_with_lock, submit_role_subagent_review,
    workflow_state_path, RALPLAN,
};
use super::*;

const SESSION: &str = "sess-app-pipeline";

#[test]
fn app_surface_consumes_deep_interview_ralplan_and_ultragoal_once() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let transcript = dir.path().join("app-session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"vscode","thread_source":"user","originator":"Codex Desktop"}}"#,
    )
    .unwrap();

    assert_success(&app_prompt(
        dir.path(),
        &transcript,
        "$deep-interview improve the restart experience",
    ));
    let question = serde_json::json!({
        "session_id": SESSION,
        "transcript_path": transcript,
        "last_assistant_message": "Ambiguity: 40%\n\nHow should restart recover an invalid saved board?\n\n1. Restore a deterministic fallback board (Recommended)\n2. Start a random board\n3. Show an error and wait\n4. Direct input / not listed\n\nRecommendation: Option 1 - it is deterministic and immediately playable.\n"
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        question.as_bytes(),
    ));
    assert_success(&app_prompt(dir.path(), &transcript, "1"));
    let milestone = serde_json::json!({
        "session_id": SESSION,
        "transcript_path": transcript,
        "last_assistant_message": "Ambiguity: 15%\n\n\"Restart preserves the best score and always restores a playable board.\"\nIs this the right basis for implementation planning?\n\n1. Run ralplan (Recommended)\n2. Continue deep-interview to 5%\n3. Clarify storage failure recovery\n4. Clarify restart animation scope\n5. Direct input / not listed\n\nRecommendation: Option 1 - scope and verification are ready for planning.\n"
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        milestone.as_bytes(),
    ));
    let candidate = read_json(&state_path_for(dir.path(), SESSION));
    let candidate = fs::read_to_string(
        candidate["crystallization_candidate"]["path"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(candidate.contains("How should restart recover an invalid saved board?"));
    assert!(candidate.contains("Restore a deterministic fallback board"));

    let selected = app_prompt(dir.path(), &transcript, "1");
    assert_success(&selected);
    assert_hidden_context(&selected, &["researcher", "contrarian", "simplifier"]);
    let deep_path = state_path_for(dir.path(), SESSION);
    let request_id = read_json(&deep_path)["subagent_orchestration"]["request_id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut transition_count = 0;
    for role in ["researcher", "contrarian", "simplifier"] {
        let payload = serde_json::json!({
            "session_id": SESSION,
            "agent_id": format!("agent-{role}"),
            "agent_type": role,
            "last_assistant_message": format!(
                "MEGARA_ROLE={role}\nMEGARA_REQUEST={request_id}\nReview complete."
            ),
        })
        .to_string();
        assert_success(&run_hook(
            dir.path(),
            dir.path(),
            "SubagentStart",
            Some(role),
            payload.as_bytes(),
        ));
        let stopped = run_hook(
            dir.path(),
            dir.path(),
            "SubagentStop",
            Some(role),
            payload.as_bytes(),
        );
        assert_success(&stopped);
        if !stopped.stdout.is_empty() {
            assert_hidden_context(&stopped, &["Start ralplan now", "planner", "critic"]);
            transition_count += 1;
        }
    }
    assert_eq!(transition_count, 1);

    let deep = read_json(&deep_path);
    let spec_sha256 = deep["spec_sha256"].as_str().unwrap();
    for (role, verdict) in [
        ("planner", "CLEAR"),
        ("architect", "CLEAR"),
        ("critic", "OKAY"),
    ] {
        submit_role_subagent_review(dir.path(), SESSION, role, verdict);
    }
    submit_plan_with_lock(
        dir.path(),
        SESSION,
        "rp-app-pipeline",
        "preserve best score and restore a playable board",
        spec_sha256,
    );
    let ralplan = read_json(&workflow_state_path(dir.path(), RALPLAN, SESSION));
    assert_eq!(ralplan["phase"], "pending_approval");

    let approved = app_prompt(dir.path(), &transcript, "2");
    assert_success(&approved);
    assert_hidden_context(
        &approved,
        &["Start ultragoal now", "create-goals", "start-goal"],
    );

    let create = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg(SESSION)
        .arg("create-goals")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_success(&create);
    let start = megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg(SESSION)
        .arg("start-goal")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_success(&start);
    let ultragoal = read_json(
        &dir.path()
            .join(".megara/state/workflows/ultragoal")
            .join(format!("{SESSION}.json")),
    );
    assert_eq!(ultragoal["phase"], "active");

    let transitions = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/deep-interview/events.jsonl"),
    )
    .unwrap();
    assert_eq!(
        transitions.matches("workflow_transition_started").count(),
        1
    );
}

fn app_prompt(project: &Path, transcript: &Path, prompt: &str) -> Output {
    let payload = serde_json::json!({
        "session_id": SESSION,
        "transcript_path": transcript,
        "prompt": format!("<codex_delegation><input>{prompt}</input></codex_delegation>"),
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

fn assert_hidden_context(output: &Output, expected: &[&str]) {
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(payload.get("continue").is_none());
    assert!(payload.get("stopReason").is_none());
    let context = payload["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    for value in expected {
        assert!(context.contains(value), "missing {value} in {context}");
    }
    assert!(!context.contains("<hook_prompt"));
}
