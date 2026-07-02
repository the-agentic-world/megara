use super::hook_ralplan_support::*;
use super::*;

#[test]
fn projected_hook_runner_blocks_ralplan_when_interview_is_active() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let interview = "Clarify before planning.\n\nMegara Question Gate:\n- id: di-active\n- round: 1\n- component: scope\n- dimension: Goal clarity\n- question: What should be clarified first?\n- options:\n  - Scope\n- free_text: true\n\n";
    assert_success(&stop_message(dir.path(), "sess-active-di", interview));
    submit_ready_reviews(dir.path(), "sess-active-di");
    submit_plan(
        dir.path(),
        "sess-active-di",
        "rp-blocked",
        "should not pass while deep-interview is active.",
    );

    let state = read_state(dir.path(), RALPLAN, "sess-active-di");
    assert_eq!(state["phase"], "handoff_not_ready");
    assert_eq!(state["approval_status"], "blocked");
    assert_eq!(state["blocked_by"], "deep-interview");
    assert_eq!(state["blocked_phase"], "question_pending");
    assert!(state.get("plan_path").is_none());
    assert!(events(dir.path(), RALPLAN).contains("\"event\":\"handoff_blocked\""));
}
