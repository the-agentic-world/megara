use super::hook_ralplan_support::*;
use super::*;

#[test]
fn projected_hook_runner_tracks_ralplan_gate_and_approval() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let deep_state =
        submit_crystallized_interview(dir.path(), "sess-rp", "add Tetris as a second game mode.");
    let spec_path = deep_state["spec_path"].as_str().unwrap().to_string();
    let spec_sha256 = deep_state["spec_sha256"].as_str().unwrap().to_string();
    submit_early_plan_without_reviews(dir.path());

    let state_path = workflow_state_path(dir.path(), RALPLAN, "sess-rp");
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "review_incomplete");
    assert!(state.get("plan_path").is_none());

    submit_review_coverage(dir.path());
    let state = read_json(&state_path);
    let review_path = PathBuf::from(state["reviews"][0]["path"].as_str().unwrap());
    assert_eq!(state["phase"], "reviewing");
    assert_eq!(state["reviews"][1]["role"], "architect");
    assert!(fs::read_to_string(&review_path)
        .unwrap()
        .contains("role: \"planner\""));

    let output = run_mutation(dir.path(), "sess-rp");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ralplan is active"));

    submit_plan_with_marker_mention(dir.path());
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["plan_id"], "rp-add-tetris");
    assert_eq!(state["input_spec_path"].as_str().unwrap(), spec_path);
    assert_eq!(state["input_spec_sha256"].as_str().unwrap(), spec_sha256);

    let plan_path = PathBuf::from(state["plan_path"].as_str().unwrap());
    let plan = fs::read_to_string(&plan_path).unwrap();
    assert!(plan.contains("This sentence is plan content, not the control block."));
    assert!(!plan.contains("- id: rp-add-tetris"));
    assert!(!plan.contains("<!--"));

    let reject_prompt = "<!--\nMegara Approval Gate:\n- plan_id: rp-add-tetris\n- plan_sha256: 0000000000000000000000000000000000000000000000000000000000000000\n- handoff_target: ultragoal\n-->\n";
    assert_success(&user_prompt(dir.path(), "sess-rp", reject_prompt));
    assert_eq!(
        read_json(&state_path)["approval_status"],
        "approval_gate_mismatch"
    );

    let approve_prompt = format!(
        "<!--\nMegara Approval Gate:\n- plan_id: rp-add-tetris\n- plan_sha256: {}\n- handoff_target: ultragoal\n-->\n",
        state["plan_sha256"].as_str().unwrap()
    );
    assert_success(&user_prompt(dir.path(), "sess-rp", &approve_prompt));
    let approved = read_json(&state_path);
    assert_eq!(approved["phase"], "approved");
    assert_eq!(approved["approved_handoff_target"], "ultragoal");

    let output = run_mutation(dir.path(), "sess-rp");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ralplan is approved for ultragoal"));
    assert!(stderr.contains("create-goals"));

    submit_ultragoal_runtime_state(dir.path(), "sess-rp", "goal_planning", "create goals");
    let output = run_mutation(dir.path(), "sess-rp");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("start-goal"));

    submit_ultragoal_runtime_state(dir.path(), "sess-rp", "active", "execute G001");
    let output = run_mutation(dir.path(), "sess-rp");
    assert_success(&output);
    let events = events(dir.path(), RALPLAN);
    assert!(events.contains("\"event\":\"plan_approved\""));
    assert!(events.contains("\"event\":\"approved_handoff_mutation_blocked\""));
    assert!(events.contains(&spec_sha256));
    assert!(events.contains(plan_path.to_str().unwrap()));
}

#[test]
fn projected_hook_runner_tracks_visible_only_plan_and_numeric_approval() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_crystallized_interview(dir.path(), "sess-visible-rp", "improve 2048 UI clarity.");
    let message = "**Pending Execution Plan**\n\nSummary: improve the 2048 UI without changing rules.\n\nScope:\n- Update layout, spacing, and controls.\n- Keep scoring and board mechanics unchanged.\n\nSteps:\n- Inspect current UI structure.\n- Adjust responsive layout.\n- Verify keyboard and touch interaction.\n\nAcceptance criteria:\n- Existing tests pass.\n- The board does not overflow on mobile.\n\nRisks:\n- Avoid changing saved game state.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n";
    assert_success(&stop_message(dir.path(), "sess-visible-rp", message));

    let state_path = workflow_state_path(dir.path(), RALPLAN, "sess-visible-rp");
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["plan_id"], "rp-plan");
    assert_eq!(state["review_source"], "runtime_visible_plan_inference");
    assert_eq!(state["reviews"][0]["role"], "planner");
    assert!(state["plan_sha256"].as_str().unwrap().len() == 64);

    let plan = fs::read_to_string(state["plan_path"].as_str().unwrap()).unwrap();
    assert!(plan.contains("Summary: improve the 2048 UI"));
    assert!(!plan.contains("Megara Plan Gate"));
    assert!(!plan.contains("Megara Workflow State"));
    assert!(!plan.contains("<!--"));

    assert_success(&user_prompt(dir.path(), "sess-visible-rp", "2"));
    let approved = read_json(&state_path);
    assert_eq!(approved["phase"], "approved");
    assert_eq!(approved["approved_handoff_target"], "ultragoal");

    let output = run_mutation(dir.path(), "sess-visible-rp");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ralplan is approved for ultragoal"));
    assert!(stderr.contains("create-goals"));
}

#[test]
fn plan_mode_stop_persists_pending_plan_from_transcript() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_crystallized_interview(dir.path(), "sess-plan-rp", "improve 2048 UI clarity.");
    let message = "**Pending Execution Plan**\n\nSummary: improve the 2048 UI without changing rules.\n\nScope:\n- Update layout, spacing, and controls.\n- Keep scoring and board mechanics unchanged.\n\nSteps:\n- Inspect current UI structure.\n- Adjust responsive layout.\n- Verify keyboard and touch interaction.\n\nAcceptance criteria:\n- Existing tests pass.\n- The board does not overflow on mobile.\n\nRisks:\n- Avoid changing saved game state.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n";
    let transcript = dir.path().join("plan-mode-ralplan-stop.jsonl");
    fs::write(
        &transcript,
        format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "turn_context",
                "payload": {
                    "turn_id": "turn-plan-rp",
                    "collaboration_mode": {"mode": "plan"}
                }
            }),
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "assistant",
                    "phase": "final",
                    "content": [{"type": "output_text", "text": message}]
                }
            })
        ),
    )
    .unwrap();
    let payload = serde_json::json!({
        "session_id": "sess-plan-rp",
        "turn_id": "turn-plan-rp",
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

    let state = read_state(dir.path(), RALPLAN, "sess-plan-rp");
    assert_eq!(state["phase"], "pending_approval");
    let plan_path = PathBuf::from(state["plan_path"].as_str().unwrap());
    let plan = fs::read_to_string(plan_path).unwrap();
    assert!(plan.contains("Summary: improve the 2048 UI"));
    assert!(!plan.contains("Megara Workflow State"));
}

#[test]
fn ralplan_korean_visible_plan_with_receipts_becomes_pending_approval() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let session_id = "sess-rp-korean-visible";
    let start = user_prompt(dir.path(), session_id, "$ralplan 2048 UI 개선점 재검토");
    assert_success(&start);
    submit_role_subagent_receipt(dir.path(), session_id, "planner");
    submit_role_subagent_receipt(dir.path(), session_id, "architect");
    submit_role_subagent_receipt(dir.path(), session_id, "critic");

    let message = "**계획**\n현재 2048 구현을 재검토해 새로 필요한 개선이 명확한지 판단한다. 명확한 사용자-visible 결함, 접근성 누락, 또는 실패 테스트가 확인될 때만 최소 범위 변경으로 진행한다.\n\n**범위**\n포함: 2048 코드 검토, 2048 관련 테스트 검토, 기존 단위 테스트 실행, 기존 E2E 테스트 실행, 사용자에게 보이는 응답의 내부 정보 노출 검사.\n\n**실행 순서**\n1. 2048 관련 코드와 테스트를 검토해 사용자-visible 결함, 접근성 누락, 실패 가능성이 명확한 후보가 있는지 확인한다.\n2. `npm test`로 기존 단위 테스트를 실행한다.\n3. `npm run test:e2e`로 기존 E2E 테스트를 실행한다.\n4. 명확한 결함이 없으면 제품 코드와 테스트를 변경하지 않고 검토 결과와 테스트 결과만 보고한다.\n\n**수용 기준**\n기존 단위 테스트 결과가 확인된다.\n\n기존 E2E 테스트 결과가 확인된다.\n\n명확한 결함이나 접근성 누락이 없고 테스트가 통과하면 코드 변경 없음이 성공이다.\n\n**위험과 대응**\nE2E 환경 문제를 제품 결함으로 오판할 수 있다. 실패 시 재현 조건과 관련 범위를 먼저 분류한다.\n\n진행할까?\n\n1. 계획을 더 다듬기\n2. `ultragoal`로 실행 승인\n3. `team`으로 실행 승인\n4. 승인 보류";
    assert_success(&stop_message(dir.path(), session_id, message));

    let state = read_state(dir.path(), RALPLAN, session_id);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["subagent_orchestration"]["status"], "satisfied");
    assert_eq!(state["review_source"], "runtime_visible_plan_inference");
    assert!(state["plan_sha256"].as_str().unwrap().len() == 64);
}

#[test]
fn ralplan_start_injects_subagent_context_and_gates_pending_approval() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let start = user_prompt(dir.path(), "sess-rp-subagents", "$ralplan improve 2048 UI");
    assert_success(&start);
    let stdout = String::from_utf8_lossy(&start.stdout);
    assert!(stdout.contains("additionalContext"));
    assert!(stdout.contains("planner"));
    assert!(stdout.contains("architect"));
    assert!(stdout.contains("critic"));
    assert!(stdout.contains("short internal draft plan"));
    assert!(stdout.contains("baseline-failure policy"));
    assert!(stdout.contains("classify them as pre-existing"));
    assert!(stdout.contains("Do not block on verification details"));
    assert!(stdout.contains("pick the stricter product-facing criterion"));
    assert!(stdout.contains("convert verification-detail feedback into concrete plan criteria"));
    assert!(stdout.contains("Do not put workflow or handoff names"));
    assert!(stdout.contains("Include the same draft plan"));
    assert!(stdout.contains("forbid tools"));
    assert!(stdout.contains("Megara workflows"));
    assert!(stdout.contains("short final verdict"));
    assert!(!stdout.contains("reviewers must never receive only the raw task or spec"));

    let message = "**Pending Execution Plan**\n\nSummary: improve the 2048 UI without changing game rules.\n\nScope:\n- Adjust layout and controls only.\n\nSteps:\n- Inspect current UI.\n- Improve responsive layout.\n- Verify keyboard interaction.\n\nAcceptance criteria:\n- Existing tests pass.\n- The board does not overflow.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: rp-subagents\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-subagents\n- next: approval\n-->\n";
    let blocked = stop_message(dir.path(), "sess-rp-subagents", message);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.contains(r#""decision":"block""#));
    assert!(stdout.contains("planner, architect, critic"));

    let state_path = workflow_state_path(dir.path(), RALPLAN, "sess-rp-subagents");
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "subagent_review_required");
    assert_eq!(state["approval_status"], "blocked");
    assert!(state.get("plan_path").is_none());

    submit_role_subagent_review(dir.path(), "sess-rp-subagents", "planner", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-rp-subagents", "architect", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-rp-subagents", "critic", "OKAY");

    assert_success(&stop_message(dir.path(), "sess-rp-subagents", message));
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["subagent_orchestration"]["status"], "satisfied");
    assert_eq!(state["reviews"].as_array().unwrap().len(), 3);
    assert!(state["plan_sha256"].as_str().unwrap().len() == 64);
}

#[test]
fn ralplan_missing_receipts_do_not_respawn_in_flight_roles() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let start = user_prompt(dir.path(), "sess-rp-inflight", "$ralplan improve 2048 UI");
    assert_success(&start);
    let planner_start = br#"{
  "session_id": "sess-rp-inflight",
  "agent_id": "agent-planner-running",
  "agent_type": "Planner",
  "turn_id": "planner-turn-1"
}"#;
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "SubagentStart",
        Some("planner"),
        planner_start,
    ));

    let message = "**Pending Execution Plan**\n\nSummary: improve the 2048 UI without changing game rules.\n\nScope:\n- Adjust layout and controls only.\n\nSteps:\n- Inspect current UI.\n- Improve responsive layout.\n- Verify keyboard interaction.\n\nAcceptance criteria:\n- Existing tests pass.\n- The board does not overflow.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: rp-inflight\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-inflight\n- next: approval\n-->\n";
    let blocked = stop_message(dir.path(), "sess-rp-inflight", message);
    assert_success(&blocked);
    let stdout = String::from_utf8_lossy(&blocked.stdout);
    assert!(stdout.contains("architect, critic"));
    assert!(stdout.contains("in-flight roles: planner"));
    assert!(stdout.contains("Do not spawn duplicate/replacement subagents"));
    assert!(stdout.contains("pending plan as the reviewed draft"));

    let state = read_state(dir.path(), RALPLAN, "sess-rp-inflight");
    assert_eq!(
        state["subagent_orchestration"]["missing_roles"][0],
        "planner"
    );
    assert_eq!(
        state["subagent_orchestration"]["in_flight_roles"][0],
        "planner"
    );
}

#[test]
fn ralplan_subagent_user_prompt_does_not_reset_in_flight_roles() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    assert_success(&user_prompt(
        dir.path(),
        "sess-rp-subagent-prompt",
        "$ralplan improve 2048 storage guards",
    ));
    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "SubagentStart",
        Some("planner"),
        br#"{
  "session_id": "sess-rp-subagent-prompt",
  "agent_id": "agent-planner-running",
  "agent_type": "Planner",
  "turn_id": "planner-turn-1"
}"#,
    ));

    let before = read_state(dir.path(), RALPLAN, "sess-rp-subagent-prompt");
    let request_id = before["subagent_orchestration"]["request_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(before["subagent_in_flight"][0]["role"], "planner");

    assert_success(&run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        Some("planner"),
        br#"{
  "session_id": "sess-rp-subagent-prompt",
  "agent_id": "agent-planner-running",
  "agent_type": "Planner",
  "prompt": "Read-only planner verdict for ralplan. Return approval-capable verdict only."
}"#,
    ));

    let after = read_state(dir.path(), RALPLAN, "sess-rp-subagent-prompt");
    assert_eq!(after["subagent_orchestration"]["request_id"], request_id);
    assert_eq!(after["subagent_in_flight"][0]["role"], "planner");
    assert_eq!(
        after["subagent_in_flight"][0]["agent_id"],
        "agent-planner-running"
    );
}

fn submit_early_plan_without_reviews(project: &Path) {
    let message = "**Pending Execution Plan**\n\nSummary: this should wait for review coverage.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: rp-too-early\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-too-early\n- next: approval\n-->\n";
    assert_success(&stop_message(project, "sess-rp", message));
}

fn submit_review_coverage(project: &Path) {
    let message = "Planner, architect, and critic passes complete.\n\n<!--\nMegara Review Pass:\n- role: planner\n- round: 1\n- verdict: CLEAR\n- summary: Initial sequence is ready for approval.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: architect\n- round: 1\n- verdict: CLEAR\n- summary: Runtime boundaries are acceptable for this plan.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: critic\n- round: 1\n- verdict: OKAY\n- summary: The plan is specific and verifiable enough to ask for approval.\n- required_fixes:\n  - none\n-->\n";
    assert_success(&stop_message(project, "sess-rp", message));
}

fn submit_ultragoal_runtime_state(project: &Path, session_id: &str, status: &str, next: &str) {
    let message = format!(
        "Status update.\n\n<!--\nMegara Workflow State:\n- skill: ultragoal\n- status: {status}\n- next: {next}\n-->\n"
    );
    assert_success(&stop_message(project, session_id, &message));
}

#[test]
fn projected_hook_runner_blocks_pending_plan_when_planner_is_still_draft() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let message = "Draft planner pass should not approve.\n\n<!--\nMegara Review Pass:\n- role: planner\n- round: 1\n- verdict: DRAFT\n- summary: Initial sequence still needs revision.\n- required_fixes:\n  - Revise the plan before approval.\n\nMegara Review Pass:\n- role: architect\n- round: 1\n- verdict: CLEAR\n- summary: Runtime boundaries are acceptable for this plan.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: critic\n- round: 1\n- verdict: OKAY\n- summary: The plan would be verifiable after planner revision.\n- required_fixes:\n  - none\n-->\n";
    assert_success(&stop_message(dir.path(), "sess-draft-planner", message));

    submit_plan(
        dir.path(),
        "sess-draft-planner",
        "rp-draft-planner",
        "should not pass while planner verdict is DRAFT.",
    );

    let state = read_state(dir.path(), RALPLAN, "sess-draft-planner");
    assert_eq!(state["phase"], "review_incomplete");
    assert_eq!(state["approval_status"], "blocked");
    assert!(state.get("plan_path").is_none());
    assert!(events(dir.path(), RALPLAN).contains("\"event\":\"review_incomplete\""));
}

fn submit_plan_with_marker_mention(project: &Path) {
    let message = "**Pending Execution Plan**\n\nSummary: add a Tetris mode without changing the current menu contract.\n\nNotes:\nThe plan body may mention this literal marker before the actual trailer.\n\nMegara Plan Gate:\nThis sentence is plan content, not the control block.\n\nSteps:\n- Add content routing.\n- Add Tetris state and rendering.\n\nAcceptance criteria:\n- Existing 2048 flow still works.\n- Tetris can start and restart.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: rp-add-tetris\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-add-tetris\n- next: approval\n-->\n";
    assert_success(&stop_message(project, "sess-rp", message));
}
