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

#[test]
fn projected_hook_runner_invalidates_subagent_receipts_after_refine() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    assert_success(&user_prompt(
        dir.path(),
        "sess-refine-agents",
        "$ralplan improve UI",
    ));
    submit_role_subagent_review(dir.path(), "sess-refine-agents", "planner", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-refine-agents", "architect", "CLEAR");
    submit_role_subagent_review(dir.path(), "sess-refine-agents", "critic", "OKAY");
    submit_plan(
        dir.path(),
        "sess-refine-agents",
        "rp-before-agent-refine",
        "first plan.",
    );

    let state_path = workflow_state_path(dir.path(), RALPLAN, "sess-refine-agents");
    let state = read_json(&state_path);
    assert_eq!(state["phase"], "pending_approval");
    assert_eq!(state["subagent_orchestration"]["status"], "satisfied");
    assert_eq!(state["subagent_receipts"].as_array().unwrap().len(), 3);

    assert_success(&user_prompt(dir.path(), "sess-refine-agents", "refine"));
    let refined = read_json(&state_path);
    assert_eq!(refined["phase"], "refining");
    assert_eq!(refined["subagent_orchestration"]["status"], "required");
    assert!(refined["subagent_receipts"].as_array().unwrap().is_empty());

    submit_ready_reviews(dir.path(), "sess-refine-agents");
    submit_plan(
        dir.path(),
        "sess-refine-agents",
        "rp-after-agent-refine",
        "second plan.",
    );
    let blocked = read_json(&state_path);
    assert_eq!(blocked["phase"], "subagent_review_required");
    assert_eq!(
        blocked["subagent_orchestration"]["missing_roles"][0],
        "planner"
    );
    assert!(blocked.get("plan_sha256").is_none());
}

#[test]
fn projected_hook_runner_ignores_stale_subagent_receipts() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    assert_success(&user_prompt(
        dir.path(),
        "sess-stale-agents",
        "$ralplan improve UI",
    ));
    let state_path = workflow_state_path(dir.path(), RALPLAN, "sess-stale-agents");
    let mut state = read_json(&state_path);
    state["subagent_receipts"] = serde_json::json!([
        {
            "role": "planner",
            "orchestration_request_id": "ralplan:old"
        },
        {
            "role": "architect",
            "orchestration_request_id": "ralplan:old"
        },
        {
            "role": "critic",
            "orchestration_request_id": "ralplan:old"
        }
    ]);
    fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    submit_ready_reviews(dir.path(), "sess-stale-agents");
    submit_plan(
        dir.path(),
        "sess-stale-agents",
        "rp-stale-agent-receipts",
        "plan with stale receipts.",
    );

    let blocked = read_json(&state_path);
    assert_eq!(blocked["phase"], "subagent_review_required");
    assert_eq!(blocked["approval_status"], "blocked");
    assert_eq!(
        blocked["subagent_orchestration"]["missing_roles"],
        serde_json::json!(["planner", "architect", "critic"])
    );
    assert!(blocked.get("plan_sha256").is_none());
}
