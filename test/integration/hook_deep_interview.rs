use super::hook_deep_interview_support::*;
use super::hook_ralplan_support::{assert_success, read_json};
use super::*;

#[test]
fn projected_hook_runner_tracks_question_gate_and_blocks_mutation() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_question(dir.path());
    let state_path = state_path(dir.path());
    let state = read_json(&state_path);
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "question_pending");
    assert!(state["pending_question"]["id"]
        .as_str()
        .is_some_and(|id| id.starts_with("di-visible-")));
    assert_eq!(
        state["pending_question"]["question"],
        "What proves this is done?"
    );
    assert_eq!(state["pending_question"]["options"][0], "Unit tests");
    assert_eq!(state["pending_question"]["options"][2], "Manual QA");
    assert_eq!(
        state["pending_question"]["options"][3],
        "Direct input / not listed"
    );
    assert_eq!(
        state["pending_question"]["options"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    answer_question(dir.path());
    let state = read_json(&state_path);
    assert!(state["pending_question"].is_null());
    assert_eq!(state["phase"], "interviewing");
    assert_eq!(state["questions"][0]["status"], "answered");
    assert_eq!(
        state["questions"][0]["answer"]["content"],
        "Use both unit and E2E tests."
    );

    assert_guard_blocks(run_bash_mutation(dir.path()));
    assert_guard_blocks(run_apply_patch(dir.path()));
    assert_success(&run_read(dir.path()));

    submit_state_only_crystallized(dir.path());
    let state = read_json(&state_path);
    assert_eq!(state["active"], true);
    assert_eq!(state["phase"], "crystallization_missing_spec");
    assert!(state.get("spec_path").is_none());
    assert_guard_blocks(run_bash_mutation(dir.path()));

    submit_final_spec(dir.path());
    let state = read_json(&state_path);
    assert_eq!(state["active"], false);
    assert_eq!(state["phase"], "crystallized");
    assert_eq!(state["ambiguity"], "12%");
    assert_eq!(state["next_workflow_suggestion"]["workflow"], "ralplan");
    assert_eq!(state["next_workflow_suggestion"]["status"], "suggested");
    assert!(state["pending_question"].is_null());

    let spec_path = PathBuf::from(state["spec_path"].as_str().unwrap());
    assert!(spec_path.exists());
    assert_eq!(state["spec_sha256"].as_str().unwrap().len(), 64);
    let spec = fs::read_to_string(&spec_path).unwrap();
    assert!(spec.starts_with("---\n"));
    assert!(spec.contains("Goal: build the verified game."));
    assert!(spec.contains("Megara Workflow State:"));

    let spec_index = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/deep-interview/specs/index.jsonl"),
    )
    .unwrap();
    assert!(spec_index.contains("\"event\":\"spec_persisted\""));
    assert!(spec_index.contains(spec_path.to_str().unwrap()));

    assert_success(&run_bash_mutation(dir.path()));
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
    assert!(events.contains("\"event\":\"next_workflow_suggested\""));
    assert!(!events.contains("di-old-transcript"));
}

#[test]
fn visible_question_requires_deep_interview_catch_all_without_score_marker() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let payload = br#"{
  "session_id": "ordinary-question",
  "cwd": "/tmp/project",
  "last_assistant_message": "Which package manager should we use?\n\n1. npm\n2. pnpm\n\n"
}"#;
    assert_success(&run_hook(dir.path(), dir.path(), "Stop", None, payload));

    assert!(!dir
        .path()
        .join(".agents/state/workflows/deep-interview/ordinary-question.json")
        .exists());
}

#[test]
fn hook_state_uses_visible_thread_id_before_runtime_session_id() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let message = "Megara Question Gate:\n- id: di-visible\n- round: 0\n- component: topology\n- dimension: Outcome clarity\n- question: Is this the right shape?\n- options:\n  - Yes\n- free_text: true\n\n";
    let payload = serde_json::json!({
        "session_id": "internal-runtime-session",
        "thread_id": "visible-thread",
        "cwd": dir.path().display().to_string(),
        "last_assistant_message": message,
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        payload.as_bytes(),
    ));

    assert!(dir
        .path()
        .join(".agents/state/workflows/deep-interview/visible-thread.json")
        .exists());
    assert!(!dir
        .path()
        .join(".agents/state/workflows/deep-interview/internal-runtime-session.json")
        .exists());
}

#[test]
fn user_prompt_merges_runtime_session_alias_into_visible_thread_state() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let message = "Megara Question Gate:\n- id: di-alias\n- round: 0\n- component: topology\n- dimension: Outcome clarity\n- question: Is this the right shape?\n- options:\n  - Yes\n- free_text: true\n\n";
    let initial_payload = serde_json::json!({
        "session_id": "runtime-session",
        "cwd": dir.path().display().to_string(),
        "last_assistant_message": message,
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        initial_payload.as_bytes(),
    ));

    let answer_payload = serde_json::json!({
        "session_id": "runtime-session",
        "thread_id": "visible-thread",
        "cwd": dir.path().display().to_string(),
        "prompt": "Yes, keep that shape.",
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        answer_payload.as_bytes(),
    ));

    let visible = read_json(
        &dir.path()
            .join(".agents/state/workflows/deep-interview/visible-thread.json"),
    );
    assert!(visible["pending_question"].is_null());
    assert_eq!(visible["questions"][0]["id"], "di-alias");
    assert_eq!(
        visible["questions"][0]["answer"]["content"],
        "Yes, keep that shape."
    );
    assert_eq!(visible["session_aliases"][0], "runtime-session");

    let alias = read_json(
        &dir.path()
            .join(".agents/state/workflows/deep-interview/runtime-session.json"),
    );
    assert_eq!(alias["active"], false);
    assert_eq!(alias["phase"], "stale");
    assert_eq!(alias["stale_superseded_by"], "visible-thread");

    let events = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/deep-interview/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"session_alias_superseded\""));
    assert!(events.contains("\"event\":\"question_answered\""));
}

#[test]
fn terminal_deep_interview_closes_same_cwd_ghost_pending_state() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let question = "Megara Question Gate:\n- id: di-ghost\n- round: 0\n- component: topology\n- dimension: Outcome clarity\n- question: Ghost question?\n- options:\n  - Yes\n- free_text: true\n\n";
    let ghost_payload = serde_json::json!({
        "session_id": "ghost-session",
        "cwd": dir.path().display().to_string(),
        "last_assistant_message": question,
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        ghost_payload.as_bytes(),
    ));

    let final_spec = "**Pending Approval Specification**\n\nGoal: improve the game UI.\n\nAcceptance criteria:\n- Layout does not overflow.\n\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 9%\n- next: ralplan\n\n";
    let visible_payload = serde_json::json!({
        "session_id": "visible-session",
        "cwd": dir.path().display().to_string(),
        "last_assistant_message": final_spec,
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        visible_payload.as_bytes(),
    ));

    let ghost = read_json(
        &dir.path()
            .join(".agents/state/workflows/deep-interview/ghost-session.json"),
    );
    assert_eq!(ghost["active"], false);
    assert_eq!(ghost["phase"], "stale");
    assert_eq!(ghost["pending_question"]["status"], "stale");
    assert_eq!(ghost["stale_superseded_by"], "visible-session");

    let events = fs::read_to_string(
        dir.path()
            .join(".agents/state/workflows/deep-interview/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"stale_state_closed\""));
}

#[test]
fn projected_hook_runner_blocks_manual_workflow_state_repair() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_final_spec(dir.path());

    for path in [
        ".agents/state/workflows/deep-interview/sess-di.json",
        ".agents/state/workflows/ralplan/sess-di.json",
    ] {
        let patch = format!("*** Begin Patch\n*** Update File: {path}\n@@\n*** End Patch\n");
        let payload = serde_json::json!({
            "session_id": "sess-di",
            "tool_name": "apply_patch",
            "tool_input": {
                "patch": patch,
            },
        })
        .to_string();
        let output = run_hook(
            dir.path(),
            dir.path(),
            "PreToolUse",
            None,
            payload.as_bytes(),
        );

        assert_guard_blocks(output);
    }
}
