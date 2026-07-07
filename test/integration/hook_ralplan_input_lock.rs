use super::hook_ralplan_support::*;
use super::*;

#[test]
fn deep_interview_start_that_mentions_ralplan_does_not_trigger_handoff_lock() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let output = user_prompt(
        dir.path(),
        "sess-di-mentions-rp",
        "$deep-interview improve 2048 UI. Test deep-interview -> ralplan -> ultragoal workflow consumption, then proceed only after approval.",
    );
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("additionalContext"));
    assert!(stdout.contains("architect subagent"));
    assert!(!stdout.contains("planner, architect, and critic"));

    let deep_state = read_state(dir.path(), DEEP_INTERVIEW, "sess-di-mentions-rp");
    assert_eq!(deep_state["skill"], DEEP_INTERVIEW);
    assert_eq!(
        deep_state["subagent_orchestration"]["workflow"],
        DEEP_INTERVIEW
    );
    assert_eq!(
        deep_state["subagent_orchestration"]["roles"][0],
        "architect"
    );
    assert!(!workflow_state_path(dir.path(), RALPLAN, "sess-di-mentions-rp").exists());
}

#[test]
fn projected_hook_runner_blocks_deep_interview_handoff_without_current_lock() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_crystallized_interview(dir.path(), "sess-old-di", "add Yacht to dice poker.");
    assert_success(&user_prompt(
        dir.path(),
        "sess-dashboard",
        &deep_interview_approval_prompt(),
    ));
    submit_ready_reviews(dir.path(), "sess-dashboard");
    submit_plan_with_lock(
        dir.path(),
        "sess-dashboard",
        "rp-dashboard",
        "add a dashboard from conversation-only deep-interview output.",
        "none",
    );

    let state = read_state(dir.path(), RALPLAN, "sess-dashboard");
    assert_eq!(state["phase"], "input_lock_blocked");
    assert_eq!(state["approval_status"], "blocked");
    assert_eq!(
        state["input_lock_status"],
        "missing_persisted_deep_interview_lock"
    );
    assert!(state.get("plan_sha256").is_none());

    let events = events(dir.path(), RALPLAN);
    assert!(events.contains("\"event\":\"input_lock_required\""));
    assert!(events.contains("missing_persisted_deep_interview_lock"));
}

#[test]
fn projected_hook_runner_accepts_deep_interview_handoff_with_matching_lock() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let deep_state =
        submit_crystallized_interview(dir.path(), "sess-locked-rp", "add a dashboard launcher.");
    let spec_sha256 = deep_state["spec_sha256"].as_str().unwrap().to_string();
    assert_success(&user_prompt(
        dir.path(),
        "sess-locked-rp",
        &deep_interview_approval_prompt(),
    ));
    submit_ready_reviews(dir.path(), "sess-locked-rp");
    submit_role_subagent_review(dir.path(), "sess-locked-rp", "planner", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-locked-rp", "architect", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-locked-rp", "critic", "OKAY");
    submit_plan_with_lock(
        dir.path(),
        "sess-locked-rp",
        "rp-dashboard-locked",
        "add a dashboard using the persisted deep-interview lock.",
        &spec_sha256,
    );

    let state = read_state(dir.path(), RALPLAN, "sess-locked-rp");
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["approval_status"], "pending");
    assert_eq!(state["requires_input_lock"], true);
    assert_eq!(state["input_spec_sha256"], spec_sha256);
    assert!(state["plan_sha256"].as_str().is_some());
}

#[test]
fn ralplan_start_after_crystallized_interview_links_input_lock_without_visible_hash() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let deep_state =
        submit_crystallized_interview(dir.path(), "sess-auto-lock-rp", "improve the 2048 UI.");
    let spec_sha256 = deep_state["spec_sha256"].as_str().unwrap().to_string();

    let start = user_prompt(
        dir.path(),
        "sess-auto-lock-rp",
        "$ralplan plan from the locked spec",
    );
    assert_success(&start);
    let stdout = String::from_utf8_lossy(&start.stdout);
    assert!(stdout.contains("additionalContext"));
    assert!(stdout.contains("planner, architect, critic"));
    assert!(stdout.contains("First write a short internal draft plan"));
    assert!(stdout.contains("baseline-failure policy"));
    assert!(stdout.contains("classify them as pre-existing"));
    assert!(stdout.contains("Do not put workflow or handoff names"));
    assert!(stdout.contains("reserve approval targets for the final numbered choices only"));
    assert!(!stdout.contains("First prepare a concrete internal draft plan"));
    assert!(!stdout.contains("Do not spawn duplicate or replacement subagents"));

    let state = read_state(dir.path(), RALPLAN, "sess-auto-lock-rp");
    assert_eq!(state["phase"], "input_lock_ready");
    assert_eq!(state["input_lock_status"], "ready");
    assert_eq!(state["input_spec_sha256"], spec_sha256);

    submit_role_subagent_review(dir.path(), "sess-auto-lock-rp", "planner", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-auto-lock-rp", "architect", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-auto-lock-rp", "critic", "OKAY");
    submit_plan(
        dir.path(),
        "sess-auto-lock-rp",
        "rp-auto-lock",
        "plan from the runtime-linked crystallized spec.",
    );

    let state = read_state(dir.path(), RALPLAN, "sess-auto-lock-rp");
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["approval_status"], "pending");
    assert_eq!(state["input_spec_sha256"], spec_sha256);
    assert!(state["plan_sha256"].as_str().is_some());
}
