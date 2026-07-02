use super::hook_ralplan_support::{
    assert_success, read_state, stop_message, submit_ready_reviews, user_prompt, RALPLAN,
};
use super::*;

pub(super) const PLAN_ID: &str = "rp-ultragoal";
const SESSION: &str = "sess-ug";

pub(super) fn approve_ralplan_for_ultragoal(project: &Path, brief: &str) {
    submit_ready_reviews(project, SESSION);
    let plan_message = format!(
        "**Pending Execution Plan**\n\n{brief}\n\nAcceptance criteria:\n- Both goals are verified before completion.\n\nMegara Plan Gate:\n- id: {PLAN_ID}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {PLAN_ID}\n- next: approval\n\n"
    );
    assert_success(&stop_message(project, SESSION, &plan_message));
    let ralplan_state = read_state(project, RALPLAN, SESSION);
    let plan_sha256 = ralplan_state["plan_sha256"].as_str().unwrap();
    let approval_prompt = format!(
        "Megara Approval Gate:\n- plan_id: {PLAN_ID}\n- plan_sha256: {plan_sha256}\n- handoff_target: ultragoal\n"
    );
    assert_success(&user_prompt(project, SESSION, &approval_prompt));
}

pub(super) fn create_goals(project: &Path, direct: bool) -> Output {
    let mut command = megara();
    command
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg(SESSION)
        .arg("create-goals");
    if direct {
        command
            .arg("--brief")
            .arg("@goal: Board shell\nBuild the playable board shell.");
    } else {
        command.arg("--json");
    }
    command.current_dir(project).output().unwrap()
}

pub(super) fn complete_goals(project: &Path) -> Output {
    megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg(SESSION)
        .arg("complete-goals")
        .arg("--json")
        .current_dir(project)
        .output()
        .unwrap()
}

pub(super) fn write_quality_artifacts(project: &Path) -> PathBuf {
    fs::write(
        project.join("reviewed.md"),
        "Reviewed board and score boundaries.",
    )
    .unwrap();
    fs::write(
        project.join("verification.log"),
        "cargo test passed; manual board smoke check passed",
    )
    .unwrap();
    let quality_gate = project.join("quality-gate.json");
    fs::write(&quality_gate, passing_quality_gate_json()).unwrap();
    quality_gate
}

pub(super) fn complete_checkpoint(project: &Path, quality_gate: &Path) -> Output {
    megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg(SESSION)
        .arg("checkpoint")
        .arg("--goal-id")
        .arg("G001")
        .arg("--status")
        .arg("complete")
        .arg("--evidence")
        .arg("cargo test passed; manual board smoke check passed")
        .arg("--quality-gate-json")
        .arg(quality_gate)
        .arg("--json")
        .current_dir(project)
        .output()
        .unwrap()
}

pub(super) fn ultragoal_status(project: &Path) -> Output {
    megara()
        .arg("ultragoal")
        .arg("--scope")
        .arg("project")
        .arg("--session-id")
        .arg(SESSION)
        .arg("status")
        .arg("--json")
        .current_dir(project)
        .output()
        .unwrap()
}

fn passing_quality_gate_json() -> String {
    serde_json::json!({
        "architectReview": {
            "recommendation": "APPROVE",
            "architectureStatus": "CLEAR",
            "productStatus": "CLEAR",
            "codeStatus": "CLEAR",
            "evidence": "Architecture, product behavior, and code boundaries reviewed.",
            "reviewedFiles": ["reviewed.md"],
            "blockers": []
        },
        "executorQa": {
            "status": "passed",
            "e2eStatus": "passed",
            "redTeamStatus": "passed",
            "evidence": "Focused tests and manual regression checks passed.",
            "commands": ["cargo test"],
            "artifactRefs": ["verification.log"],
            "blockers": []
        },
        "iteration": {
            "status": "passed",
            "fullRerun": true,
            "evidence": "Final verification reran after cleanup.",
            "commands": ["cargo test"],
            "artifactRefs": ["verification.log"],
            "blockers": []
        }
    })
    .to_string()
}
