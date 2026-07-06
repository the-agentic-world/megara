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
    assert_success(&output);
    let events = events(dir.path(), RALPLAN);
    assert!(events.contains("\"event\":\"plan_approved\""));
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

fn submit_early_plan_without_reviews(project: &Path) {
    let message = "**Pending Execution Plan**\n\nSummary: this should wait for review coverage.\n\nApprove this plan?\n\n1. Refine\n2. Approve via ultragoal\n3. Approve via team\n4. Stop with the plan pending\n\n<!--\nMegara Plan Gate:\n- id: rp-too-early\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-too-early\n- next: approval\n-->\n";
    assert_success(&stop_message(project, "sess-rp", message));
}

fn submit_review_coverage(project: &Path) {
    let message = "Planner, architect, and critic passes complete.\n\n<!--\nMegara Review Pass:\n- role: planner\n- round: 1\n- verdict: CLEAR\n- summary: Initial sequence is ready for approval.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: architect\n- round: 1\n- verdict: CLEAR\n- summary: Runtime boundaries are acceptable for this plan.\n- required_fixes:\n  - none\n\nMegara Review Pass:\n- role: critic\n- round: 1\n- verdict: OKAY\n- summary: The plan is specific and verifiable enough to ask for approval.\n- required_fixes:\n  - none\n-->\n";
    assert_success(&stop_message(project, "sess-rp", message));
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
