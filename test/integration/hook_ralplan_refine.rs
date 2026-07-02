use super::hook_ralplan_support::*;
use super::*;

#[test]
fn projected_hook_runner_invalidates_reviews_after_refine() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_ready_reviews(dir.path(), "sess-refine");
    submit_plan(dir.path(), "sess-refine", "rp-before-refine", "first plan.");
    let state_path = workflow_state_path(dir.path(), RALPLAN, "sess-refine");
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["reviews"].as_array().unwrap().len(), 3);
    assert!(state["plan_sha256"].as_str().is_some());

    assert_success(&user_prompt(dir.path(), "sess-refine", "refine"));
    let refined = read_json(&state_path);
    assert_eq!(refined["phase"], "refining");
    assert_eq!(refined["approval_status"], "refine_requested");
    assert!(refined["reviews"].as_array().unwrap().is_empty());
    assert!(refined.get("plan_sha256").is_none());

    submit_plan(dir.path(), "sess-refine", "rp-after-refine", "second plan.");
    let blocked = read_json(&state_path);
    assert_eq!(blocked["phase"], "review_incomplete");
    assert_eq!(blocked["approval_status"], "blocked");
    assert!(blocked.get("plan_sha256").is_none());
}
