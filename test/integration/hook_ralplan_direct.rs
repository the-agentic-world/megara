use super::hook_ralplan_support::*;
use super::*;

#[test]
fn projected_hook_runner_allows_direct_ralplan_without_interview() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_ready_reviews(dir.path(), "sess-direct-rp");
    submit_plan(
        dir.path(),
        "sess-direct-rp",
        "rp-direct",
        "plan directly without a deep-interview lock.",
    );

    let state = read_state(dir.path(), RALPLAN, "sess-direct-rp");
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["approval_status"], "pending");
    assert_eq!(state["plan_id"], "rp-direct");
    assert!(state["plan_sha256"].as_str().is_some());
    assert!(state.get("input_spec_sha256").is_none());
}
