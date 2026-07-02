use super::hook_ralplan_support::*;
use super::*;

#[test]
fn projected_hook_runner_prioritizes_ralplan_decision_over_stale_interview_question() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    submit_ready_reviews(dir.path(), "sess-stale-di");
    submit_plan(dir.path(), "sess-stale-di", "rp-stale-di", "safe plan.");
    let state = read_state(dir.path(), RALPLAN, "sess-stale-di");
    let plan_sha256 = state["plan_sha256"].as_str().unwrap();

    let stale_question = "Late stale question.\n\nMegara Question Gate:\n- id: di-stale\n- round: 1\n- component: stale\n- dimension: Stale state\n- question: This stale question should not consume plan approval.\n- options:\n  - stale\n- free_text: true\n\n";
    assert_success(&stop_message(dir.path(), "sess-stale-di", stale_question));

    let approve_prompt = format!(
        "Megara Approval Gate:\n- plan_id: rp-stale-di\n- plan_sha256: {plan_sha256}\n- handoff_target: ultragoal\n"
    );
    assert_success(&user_prompt(dir.path(), "sess-stale-di", &approve_prompt));

    let approved = read_state(dir.path(), RALPLAN, "sess-stale-di");
    assert_eq!(approved["phase"], "approved");
    assert_eq!(approved["approved_handoff_target"], "ultragoal");

    let deep_state = read_state(dir.path(), DEEP_INTERVIEW, "sess-stale-di");
    assert_eq!(deep_state["pending_question"]["id"], "di-stale");
    assert_eq!(deep_state["pending_question"]["status"], "pending");
}
