use super::hook_ralplan_support::*;
use super::*;

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
