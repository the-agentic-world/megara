use super::hook_deep_interview_support::*;
use super::hook_ralplan_support::{
    assert_success, read_json, submit_plan, submit_ready_reviews, user_prompt,
};
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
    assert_eq!(
        state["pending_question"]["options"][0],
        "Unit tests (Recommended)"
    );
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
    assert_eq!(state["pipeline_lock"]["workflow"], "ralplan");
    assert_eq!(state["pipeline_lock"]["status"], "pending_ralplan");
    assert!(state["pending_question"].is_null());

    let spec_path = PathBuf::from(state["spec_path"].as_str().unwrap());
    assert!(spec_path.exists());
    assert_eq!(state["spec_sha256"].as_str().unwrap().len(), 64);
    let spec = fs::read_to_string(&spec_path).unwrap();
    assert!(spec.starts_with("---\n"));
    assert!(spec.contains("Goal: build the verified game."));
    assert!(!spec.contains("Megara Workflow State:"));
    assert!(!spec.contains("<!--"));
    assert!(!spec.contains("Transcript summary:"));

    let spec_index = fs::read_to_string(
        dir.path()
            .join(".megara/artifacts/deep-interview/specs/index.jsonl"),
    )
    .unwrap();
    assert!(spec_index.contains("\"event\":\"spec_persisted\""));
    assert!(spec_index.contains(spec_path.to_str().unwrap()));

    assert_guard_blocks(run_bash_mutation(dir.path()));
    let events = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/deep-interview/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"question_pending\""));
    assert!(events.contains("\"event\":\"question_answered\""));
    assert!(events.contains("\"event\":\"mutation_blocked\""));
    assert!(events.contains("\"event\":\"spec_missing\""));
    assert!(events.contains("\"event\":\"spec_persisted\""));
    assert!(events.contains("\"event\":\"next_workflow_suggested\""));
    assert!(events.contains("\"event\":\"pipeline_lock_mutation_blocked\""));
    assert!(!events.contains("di-old-transcript"));
}

#[test]
fn deep_interview_question_ledger_preserves_repeated_gate_ids() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let first = serde_json::json!({
        "session_id": "sess-di",
        "last_assistant_message": "Ambiguity: 42%\n\n<!--\nMegara Question Gate:\n- id: repeated\n- question: What should be tested first?\n- options:\n  - Unit tests\n  - E2E tests\n  - Manual QA\n  - Direct input / not listed\n- free_text: true\n-->\n"
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        first.as_bytes(),
    ));
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        br#"{"session_id":"sess-di","prompt":"Unit tests."}"#,
    ));

    let second = serde_json::json!({
        "session_id": "sess-di",
        "last_assistant_message": "Ambiguity: 38%\n\n<!--\nMegara Question Gate:\n- id: repeated\n- question: What should be checked next?\n- options:\n  - Keyboard flow\n  - Visual layout\n  - Score rules\n  - Direct input / not listed\n- free_text: true\n-->\n"
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        second.as_bytes(),
    ));
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        br#"{"session_id":"sess-di","prompt":"Keyboard flow."}"#,
    ));

    let state = read_json(&state_path(dir.path()));
    let questions = state["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 2);
    assert_ne!(questions[0]["id"], questions[1]["id"]);
    assert_eq!(questions[0]["answer"]["content"], "Unit tests.");
    assert_eq!(questions[1]["answer"]["content"], "Keyboard flow.");
}

#[test]
fn subagent_user_prompt_does_not_answer_deep_interview_question() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_question(dir.path());
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        Some("architect"),
        br#"{
  "session_id": "sess-di",
  "agent_id": "agent-architect-running",
  "agent_type": "Architect",
  "prompt": "Read-only architect verdict. Unit tests are enough."
}"#,
    ));

    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "question_pending");
    assert_eq!(state["pending_question"]["status"], "pending");
    assert_eq!(state["questions"][0]["status"], "pending");
    assert!(state["questions"][0].get("answer").is_none());
}

#[test]
fn delegated_user_answer_is_recorded_without_codex_wrapper() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_question(dir.path());
    let payload = br#"{
  "session_id": "sess-di",
  "prompt": "<codex_delegation><input>Use option 2 and add smoke tests.</input></codex_delegation>"
}"#;
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        payload,
    ));

    let state = read_json(&state_path(dir.path()));
    assert_eq!(
        state["questions"][0]["answer"]["content"],
        "Use option 2 and add smoke tests."
    );
}

#[test]
fn subagent_events_are_logged_and_attached_to_workflow_state() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_question(dir.path());
    let payload = br#"{
  "session_id": "sess-di",
  "cwd": "/tmp/project",
  "subagent_id": "sg-1",
  "subagent_name": "researcher"
}"#;
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "SubagentStart",
        None,
        payload,
    ));
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "SubagentStop",
        None,
        payload,
    ));

    let log = fs::read_to_string(dir.path().join(".megara/state/hooks/subagents.jsonl")).unwrap();
    assert!(log.contains("\"event\":\"SubagentStart\""));
    assert!(log.contains("\"event\":\"SubagentStop\""));
    assert!(log.contains("\"subagent_name\":\"researcher\""));

    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["last_subagent_event"]["event"], "SubagentStop");
    let events = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/deep-interview/events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("\"event\":\"subagent_event\""));
}

#[test]
fn deep_interview_start_injects_subagent_context_and_requires_receipt() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let start_payload = br#"{
  "session_id": "sess-di",
  "permission_mode": "default",
  "cwd": "/tmp/project",
  "prompt": "$deep-interview improve the game UI"
}"#;
    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        start_payload,
    );
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("additionalContext"));
    assert!(stdout.contains("Round 0 topology question before broad repository inspection"));
    assert!(stdout.contains("before broad repository inspection or source-file reads"));
    assert!(stdout.contains("do not block the immediate next question on repository inspection"));
    assert!(stdout.contains("ask one compact follow-up from the confirmed topology first"));
    assert!(stdout.contains("minimal brownfield fact pass"));
    assert!(stdout.contains("only when the next decision depends on repository facts"));
    assert!(stdout.contains("at most five focused source/test files"));
    assert!(stdout.contains("architect subagent"));
    assert!(stdout.contains("forbid tool calls"));
    assert!(stdout.contains("decide only from the prompt context"));
    assert!(stdout.contains("forbid Megara workflow/skill invocation"));
    assert!(stdout.contains("final-answer-only short direct verdict"));

    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["subagent_orchestration"]["status"], "required");
    assert_eq!(state["subagent_orchestration"]["roles"][0], "architect");

    let blocked = run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        br#"{
  "session_id": "sess-di",
  "last_assistant_message": "**Requirements Summary**\n\nGoal: improve the game UI.\n\nAcceptance criteria:\n- Layout does not overflow.\n\nNext: continue with `ralplan` from this summary.\n\n<!--\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 9%\n- next: ralplan\n-->\n"
}"#,
    );
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "subagent_review_required");
    assert!(state.get("spec_path").is_none());

    let subagent_payload = br#"{
  "session_id": "sess-di",
  "cwd": "/tmp/project",
  "agent_id": "agent-architect-1",
  "agent_type": "architect"
}"#;
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "SubagentStop",
        Some("architect"),
        subagent_payload,
    ));

    submit_final_spec(dir.path());
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "crystallized");
    assert_eq!(state["subagent_orchestration"]["status"], "satisfied");
    assert_eq!(state["subagent_receipts"][0]["role"], "architect");
}

#[test]
fn deep_interview_answer_reinforces_pending_subagent_context() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let start_payload = br#"{
  "session_id": "sess-di",
  "permission_mode": "default",
  "cwd": "/tmp/project",
  "prompt": "$deep-interview improve the game UI"
}"#;
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        start_payload,
    ));

    let question = br#"{
  "session_id": "sess-di",
  "cwd": "/tmp/project",
  "last_assistant_message": "Ambiguity: 35%\n\nWhich scope should be checked first?\n\n1. Layout\n2. Keyboard\n3. Tests\n4. Direct input / not listed\n\n"
}"#;
    assert_success(&run_hook(dir.path(), dir.path(), "Stop", None, question));

    let answer = br#"{"session_id":"sess-di","cwd":"/tmp/project","prompt":"2"}"#;
    let output = run_hook(dir.path(), dir.path(), "UserPromptSubmit", None, answer);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""additionalContext""#));
    assert!(stdout.contains("Before final crystallization"));
    assert!(stdout.contains("Missing receipt roles: architect"));
    assert!(stdout.contains("Do not delay an ordinary interview question turn"));
    assert!(!stdout.contains("Spawn only these missing roles now"));
    assert!(
        stdout.contains("Ask exactly one compact next question if more user input is still needed")
    );

    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "interviewing");
    assert_eq!(state["subagent_orchestration"]["status"], "required");
}

#[test]
fn subagent_transcript_does_not_supersede_main_workflow_state() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_question(dir.path());
    let subagent_transcript = dir
        .path()
        .join("rollout-2026-01-01T00-00-00-bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb.jsonl");
    fs::write(&subagent_transcript, "").unwrap();
    let payload = serde_json::json!({
        "session_id": "sess-di",
        "cwd": "/tmp/project",
        "agent_id": "agent-architect-1",
        "agent_type": "architect",
        "transcript_path": subagent_transcript,
    })
    .to_string();

    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "SubagentStop",
        Some("architect"),
        payload.as_bytes(),
    ));

    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["session_id"], "sess-di");
    assert_eq!(state["last_subagent_event"]["event"], "SubagentStop");
    assert_eq!(state["subagent_receipts"][0]["role"], "architect");
    assert!(!dir
        .path()
        .join(".megara/state/workflows/deep-interview/bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb.json")
        .exists());
    assert!(state.get("stale_superseded_by").is_none());
}

#[test]
fn deep_interview_milestone_blocks_ordinary_question_and_lowers_target_after_choice() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let ordinary = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 14%\n\nWhich visual issue should be fixed first?\n\n1. Layout overflow\n2. Button spacing\n3. Color contrast\n4. Direct input / not listed\n\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, ordinary);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("Megara deep-interview reached"));
    assert!(!stdout.contains("runtime instruction"));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "milestone_decision_required");
    assert!(state["pending_question"].is_null());

    let milestone = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 14%\n\nCrystallize this for ralplan now, or continue deep-interview to 5%?\n\n1. Proceed to ralplan with the current crystallized spec\n2. Continue deep-interview to 5%\n3. Continue deep-interview only on a named component or risk\n4. Direct input / not listed\n\n"
}"#;
    assert_success(&run_hook(dir.path(), dir.path(), "Stop", None, milestone));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["pending_question"]["kind"], "milestone_decision");
    assert_eq!(state["pending_question"]["milestone_target"], 15);
    assert_eq!(state["pending_question"]["next_ambiguity_target"], 5);

    let answer = br#"{"session_id":"sess-di","prompt":"2"}"#;
    let output = run_hook(dir.path(), dir.path(), "UserPromptSubmit", None, answer);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""additionalContext""#));
    assert!(stdout.contains("stricter 5% ambiguity target"));
    assert!(stdout.contains("Do not repeat the previous milestone decision"));
    assert!(!stdout.contains("Megara deep-interview reached"));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["active_ambiguity_target"], 5);
    assert_eq!(
        state["milestone_decision"]["status"],
        "continue_deep_interview"
    );

    let stale_milestone = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 14%\n\nCrystallize this for ralplan now, or continue deep-interview to 5%?\n\n1. Proceed to ralplan with the current crystallized spec\n2. Continue deep-interview to 5%\n3. Continue deep-interview only on a named component or risk\n4. Direct input / not listed\n\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, stale_milestone);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("Megara deep-interview reached"));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "interviewing");
    assert!(state["pending_question"].is_null());

    assert_success(&run_hook(dir.path(), dir.path(), "Stop", None, ordinary));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "question_pending");
    assert_eq!(
        state["pending_question"]["question"],
        "Which visual issue should be fixed first?"
    );
}

#[test]
fn deep_interview_answer_injects_milestone_preflight_context() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let question = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 17%\n\nWhat status detail matters most?\n\n1. Goal and top tile\n2. Empty cells\n3. Overlay copy\n4. Direct input / not listed\n\n"
}"#;
    assert_success(&run_hook(dir.path(), dir.path(), "Stop", None, question));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["ambiguity"], "17%");

    let answer = br#"{"session_id":"sess-di","prompt":"1"}"#;
    let output = run_hook(dir.path(), dir.path(), "UserPromptSubmit", None, answer);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""additionalContext""#));
    assert!(stdout.contains("active ambiguity target is 15%"));
    assert!(stdout.contains("if the next visible ambiguity score is <= 15%"));
    assert!(stdout.contains("do not ask an ordinary interview question"));
    assert!(stdout.contains("one recommendation line before the options"));
    assert!(stdout.contains("because the active ambiguity target has been reached"));
    assert!(stdout.contains("(Recommended)"));
}

#[test]
fn deep_interview_accepts_korean_ambiguity_synonym() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let ordinary = r#"{
  "session_id": "sess-di",
  "last_assistant_message": "모호도: 14%\n\n어떤 UI 상태를 먼저 정할까?\n\n1. 진행 중 상태\n2. 승리 상태\n3. 패배 상태\n4. 직접 입력 / 보기 외 답변\n\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, ordinary.as_bytes());
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("Megara deep-interview reached"));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "milestone_decision_required");
    assert_eq!(state["ambiguity"], "14%");
}

#[test]
fn deep_interview_milestone_uses_nearest_crossed_target() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let ordinary = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 3%\n\nWhich remaining risk should be clarified?\n\n1. Input flow\n2. Scoring\n3. Accessibility\n4. Direct input / not listed\n\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, ordinary);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("Megara deep-interview reached"));

    let stale_milestone = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 3%\n\nCrystallize this for ralplan now, or continue deep-interview to 5%?\n\n1. Proceed to ralplan with the current crystallized spec\n2. Continue deep-interview to 5%\n3. Continue deep-interview only on a named component or risk\n4. Direct input / not listed\n\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, stale_milestone);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("Megara deep-interview reached"));
}

#[test]
fn visible_hook_prompt_feedback_is_blocked_before_user_output() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_question(dir.path());

    let leaked = r#"{
  "session_id": "sess-di",
  "last_assistant_message": "<hook_prompt hook_run_id=\"stop:5:/tmp/hooks.json\">Megara deep-interview reached 14% ambiguity at the active 15% target. Do not ask another ordinary interview question. Keep this runtime instruction internal.</hook_prompt>\n\n모호성: 14%\n\n어떤 결정을 할까요?\n\n1. ralplan 진행\n2. deep-interview 계속\n3. 특정 리스크만 계속\n4. 직접 입력\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, leaked.as_bytes());
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("Megara deep-interview reached 14% ambiguity"));
    assert!(!stdout.contains("<hook_prompt"));
    let conversation =
        fs::read_to_string(dir.path().join(".megara/state/hooks/conversation.jsonl"))
            .unwrap_or_default();
    assert!(!conversation.contains("Megara deep-interview reached 14% ambiguity"));
    assert!(!conversation.contains("<hook_prompt"));
}

#[test]
fn deep_interview_milestone_proceed_blocks_followup_questions_until_spec() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let milestone = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 15%\n\nCrystallize this for ralplan now, or continue deep-interview to 5%?\n\n1. Proceed to ralplan with the current crystallized spec\n2. Continue deep-interview to 5%\n3. Continue deep-interview only on a named component or risk\n4. Direct input / not listed\n\n"
}"#;
    assert_success(&run_hook(dir.path(), dir.path(), "Stop", None, milestone));

    let answer = br#"{"session_id":"sess-di","prompt":"1"}"#;
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        answer,
    ));
    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "crystallizing");
    assert_eq!(state["milestone_decision"]["status"], "proceed_to_ralplan");
    assert!(state["pending_question"].is_null());

    let followup_question = br#"{
  "session_id": "sess-di",
  "last_assistant_message": "Ambiguity: 15%\n\nFinal sentence check: if localStorage throws, should the game keep rendering?\n\n1. Yes, lock this sentence.\n2. Mention best score fallback.\n3. Mention uncaught page errors.\n4. Direct input / not listed.\n\n"
}"#;
    let blocked = run_hook(dir.path(), dir.path(), "Stop", None, followup_question);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.trim().is_empty());

    let state = read_json(&state_path(dir.path()));
    assert_eq!(state["phase"], "crystallizing");
    assert_eq!(state["status"], "crystallizing");
    assert!(state["pending_question"].is_null());
}

#[test]
fn crystallized_pipeline_lock_blocks_until_ralplan_approval() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_final_spec(dir.path());
    assert_guard_blocks(run_bash_mutation(dir.path()));
    assert_success(&run_read(dir.path()));

    submit_ready_reviews(dir.path(), "sess-di");
    submit_plan(
        dir.path(),
        "sess-di",
        "rp-unlock-after-approval",
        "continue from the crystallized spec.",
    );
    let blocked_by_ralplan = run_bash_mutation(dir.path());
    assert_guard_blocks(blocked_by_ralplan);

    assert_success(&user_prompt(dir.path(), "sess-di", "2"));
    let blocked_by_handoff = run_bash_mutation(dir.path());
    assert_guard_blocks(blocked_by_handoff);
    submit_ultragoal_state(dir.path(), "goal_planning", "create goals");
    let blocked_by_goal_planning = run_bash_mutation(dir.path());
    assert_guard_blocks(blocked_by_goal_planning);

    submit_ultragoal_state(dir.path(), "active", "execute G001");
    assert_success(&run_bash_mutation(dir.path()));
}

#[test]
fn projected_hook_runner_persists_visible_only_crystallized_spec() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let final_spec = "**Requirements Summary**\n\nAmbiguity: 8%\n\nGoal: improve the game UI without changing game rules.\n\nScope:\n- Keep the existing 2048 mechanics.\n- Improve layout, spacing, and controls.\n\nDecisions:\n- Prioritize mobile readability first.\n\nAcceptance criteria:\n- Layout does not overflow.\n- Existing game flow still works.\n\nConstraints and risks:\n- Avoid changing save data or scoring.\n\nNext step: continue with `ralplan` from this summary. Implementation is still not allowed.\n";
    let payload = serde_json::json!({
        "session_id": "visible-final",
        "cwd": dir.path().display().to_string(),
        "last_assistant_message": final_spec,
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        payload.as_bytes(),
    ));

    let state_path = dir
        .path()
        .join(".megara/state/workflows/deep-interview/visible-final.json");
    let state = read_json(&state_path);
    assert_eq!(state["active"], false);
    assert_eq!(state["phase"], "crystallized");
    assert_eq!(state["ambiguity"], "8%");
    assert_eq!(state["next"], "ralplan");
    assert_eq!(state["next_workflow_suggestion"]["workflow"], "ralplan");

    let spec_path = PathBuf::from(state["spec_path"].as_str().unwrap());
    let spec = fs::read_to_string(spec_path).unwrap();
    assert!(spec.contains("Goal: improve the game UI"));
    assert!(!spec.contains("Megara Workflow State"));
    assert!(!spec.contains("Megara Plan Gate"));
    assert!(!spec.contains("<!--"));
}

#[test]
fn plan_mode_stop_persists_crystallized_spec_from_transcript() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let final_spec = "**Requirements Summary**\n\nAmbiguity: 0%\n\nGoal: improve the 2048 UI without changing the game rules.\n\nScope:\n- Keep scoring and board mechanics unchanged.\n- Improve layout, spacing, and touch affordances.\n\nDecisions:\n- Prioritize mobile readability.\n\nAcceptance criteria:\n- Existing tests pass.\n- The board does not overflow on mobile.\n\nConstraints and risks:\n- Avoid changing saved game state.\n\nNext step: continue with `ralplan` from this summary. Implementation is still not allowed.\n";
    let transcript = dir.path().join("plan-mode-stop.jsonl");
    fs::write(
        &transcript,
        format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "turn_context",
                "payload": {
                    "turn_id": "turn-plan-di",
                    "collaboration_mode": {"mode": "plan"}
                }
            }),
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "assistant",
                    "phase": "final",
                    "content": [{"type": "output_text", "text": final_spec}]
                }
            })
        ),
    )
    .unwrap();
    let payload = serde_json::json!({
        "session_id": "sess-plan-di",
        "turn_id": "turn-plan-di",
        "permission_mode": "plan",
        "transcript_path": transcript,
        "cwd": dir.path().display().to_string(),
    })
    .to_string();

    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        payload.as_bytes(),
    ));

    let state = read_json(
        &dir.path()
            .join(".megara/state/workflows/deep-interview/sess-plan-di.json"),
    );
    assert_eq!(state["phase"], "crystallized");
    assert_eq!(state["ambiguity"], "0%");
    let spec_path = PathBuf::from(state["spec_path"].as_str().unwrap());
    let spec = fs::read_to_string(spec_path).unwrap();
    assert!(spec.contains("Goal: improve the 2048 UI"));
    assert!(!spec.contains("Megara Workflow State"));

    let conversation =
        fs::read_to_string(dir.path().join(".megara/state/hooks/conversation.jsonl")).unwrap();
    assert!(conversation.contains("improve the 2048 UI"));
}

#[test]
fn projected_hook_runner_persists_zero_ambiguity_visible_spec() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let final_spec = "**요약**\n\n모호성: 0%\n\n목표: 게임 프로젝트에서 재현 가능한 작은 버그 하나를 테스트로 고치기.\n\n범위:\n- Galaga 탄환 충돌 로직만 수정한다.\n- 게임 규칙과 UI는 바꾸지 않는다.\n\n결정사항:\n- 단위 테스트로 먼저 재현한다.\n\n인수 기준:\n- 재현 테스트와 경계 테스트가 통과한다.\n- 전체 테스트가 통과한다.\n\n제약과 위험:\n- 점수 체계는 변경하지 않는다.\n\n다음 단계: 이 잠긴 요약에서 바로 `ralplan`을 시작한다. 구현은 아직 허용되지 않는다.\n";
    let payload = serde_json::json!({
        "session_id": "visible-zero-final",
        "cwd": dir.path().display().to_string(),
        "last_assistant_message": final_spec,
    })
    .to_string();
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "Stop",
        None,
        payload.as_bytes(),
    ));

    let state_path = dir
        .path()
        .join(".megara/state/workflows/deep-interview/visible-zero-final.json");
    let state = read_json(&state_path);
    assert_eq!(state["active"], false);
    assert_eq!(state["phase"], "crystallized");
    assert_eq!(state["ambiguity"], "0%");
    assert_eq!(state["next"], "ralplan");
    assert_eq!(state["next_workflow_suggestion"]["workflow"], "ralplan");

    let spec = fs::read_to_string(PathBuf::from(state["spec_path"].as_str().unwrap())).unwrap();
    assert!(spec.contains("모호성: 0%"));
    assert!(!spec.contains("Megara Workflow State"));
    assert!(!spec.contains("<!--"));
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
        .join(".megara/state/workflows/deep-interview/ordinary-question.json")
        .exists());
}

#[test]
fn deep_interview_pipeline_question_is_not_misclassified_as_ralplan_pending() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let message = "모호성: 85%\n\n요청을 4개 구성요소로 읽고 있습니다:\n1. `deep-interview`: 2048 UI/UX 개선 목표, 범위, 검증 기준을 질문으로 확정한다.\n2. `ralplan`: 확정된 요구사항에서 내부 초안 계획을 만들고 다중 관점 검토를 거쳐 승인 가능한 계획을 만든다.\n3. `ultragoal`: 승인된 계획을 기준으로 실제 구현과 검증을 끝까지 수행한다.\n4. 하네스 회귀 테스트: 위 워크플로우 순서, 출력 제한, subagent 제약, 구현 승인 경계를 검증한다.\n\n이 구성요소를 그대로 진행하면 됩니까, 아니면 추가/삭제/병합/분리/보류할 항목이 있습니까?\n\n1. 그대로 진행\n2. 구성요소 조정\n3. 일부 구성요소 보류 또는 우선순위 지정\n4. 직접 입력 / 선택지에 없음";
    let payload = serde_json::json!({
        "session_id": "pipeline-question",
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

    let state = read_json(
        &dir.path()
            .join(".megara/state/workflows/deep-interview/pipeline-question.json"),
    );
    assert_eq!(state["phase"], "question_pending");
    assert_eq!(
        state["pending_question"]["question"],
        "이 구성요소를 그대로 진행하면 됩니까, 아니면 추가/삭제/병합/분리/보류할 항목이 있습니까?"
    );
    assert_eq!(
        state["pending_question"]["options"][3],
        "직접 입력 / 선택지에 없음"
    );
    assert!(!dir
        .path()
        .join(".megara/state/workflows/ralplan/pipeline-question.json")
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
        .join(".megara/state/workflows/deep-interview/visible-thread.json")
        .exists());
    assert!(!dir
        .path()
        .join(".megara/state/workflows/deep-interview/internal-runtime-session.json")
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
            .join(".megara/state/workflows/deep-interview/visible-thread.json"),
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
            .join(".megara/state/workflows/deep-interview/runtime-session.json"),
    );
    assert_eq!(alias["active"], false);
    assert_eq!(alias["phase"], "stale");
    assert_eq!(alias["stale_superseded_by"], "visible-thread");

    let events = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/deep-interview/events.jsonl"),
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

    let final_spec = "**Requirements Summary**\n\nGoal: improve the game UI.\n\nAcceptance criteria:\n- Layout does not overflow.\n\n<!--\nMegara Workflow State:\n- skill: deep-interview\n- status: crystallized\n- ambiguity: 9%\n- next: ralplan\n-->\n\n";
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
            .join(".megara/state/workflows/deep-interview/ghost-session.json"),
    );
    assert_eq!(ghost["active"], false);
    assert_eq!(ghost["phase"], "stale");
    assert_eq!(ghost["pending_question"]["status"], "stale");
    assert_eq!(ghost["stale_superseded_by"], "visible-session");

    let events = fs::read_to_string(
        dir.path()
            .join(".megara/state/workflows/deep-interview/events.jsonl"),
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
        ".megara/state/workflows/deep-interview/sess-di.json",
        ".megara/state/workflows/ralplan/sess-di.json",
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
