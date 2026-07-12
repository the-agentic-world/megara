use super::hook_ralplan_support::{
    assert_success, read_json, read_state, stop_message, submit_ready_reviews, user_prompt, RALPLAN,
};
use super::ultragoal_support::*;
use super::*;

const ULTRAGOAL_TEST_SESSION: &str = "sess-ug";

#[test]
fn ultragoal_cli_creates_goals_and_records_completion_receipt() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let brief = "@goal: Board shell\nBuild the playable board shell.\n\n@goal Score model\nTrack scores and losses.";
    let direct = create_goals(dir.path(), true);
    assert!(!direct.status.success());
    assert!(String::from_utf8_lossy(&direct.stderr).contains("--allow-direct"));

    approve_ralplan_for_ultragoal(dir.path(), brief);
    let create = create_goals(dir.path(), false);
    assert!(
        create.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );

    let state_dir = dir.path().join(".megara/artifacts/ultragoal/sess-ug");
    assert!(state_dir.join("brief.md").exists());
    assert!(state_dir.join("evidence").is_dir());
    let goals_path = state_dir.join("goals.json");
    let goals = read_json(&goals_path);
    assert_eq!(goals["source"]["kind"], "ralplan");
    assert_eq!(goals["source"]["ralplan_plan_id"], PLAN_ID);
    assert_eq!(goals["goals"][0]["id"], "G001");
    assert_eq!(goals["goals"][0]["title"], "Board shell");
    assert_eq!(goals["goals"][1]["status"], "pending");

    let runtime_state_path = dir
        .path()
        .join(".megara/state/workflows/ultragoal/sess-ug.json");
    let runtime_state = read_json(&runtime_state_path);
    assert_eq!(runtime_state["phase"], "goal_planning");
    let ralplan_state = read_state(dir.path(), RALPLAN, ULTRAGOAL_TEST_SESSION);
    assert_eq!(ralplan_state["transition"]["status"], "started");
    assert_eq!(
        runtime_state["source_transition_id"],
        ralplan_state["transition"]["id"]
    );

    let next = start_goal(dir.path());
    assert!(
        next.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&next.stderr)
    );
    let next: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_eq!(next["state"], "started");
    assert_eq!(next["next_goal"]["id"], "G001");
    let runtime_state = read_json(&runtime_state_path);
    assert_eq!(runtime_state["phase"], "active");
    assert_eq!(runtime_state["active_goal_id"], "G001");
    let evidence_dir = fs::canonicalize(state_dir.join("evidence")).unwrap();
    assert_eq!(
        runtime_state["evidence_dir"],
        evidence_dir.display().to_string()
    );

    write_reviewed_product_file(dir.path());
    let checkpoint = complete_checkpoint(dir.path());
    assert!(
        checkpoint.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&checkpoint.stderr)
    );
    let checkpoint: serde_json::Value = serde_json::from_slice(&checkpoint.stdout).unwrap();
    assert_eq!(checkpoint["goal"]["status"], "complete");
    assert_eq!(checkpoint["goal"]["completion_receipt"]["goal_id"], "G001");
    assert_eq!(checkpoint["next_goal_started"]["id"], "G002");

    let goals = read_json(&goals_path);
    assert_eq!(goals["goals"][0]["status"], "complete");
    assert_eq!(
        goals["goals"][0]["completion_receipt"]["receipt_id"]
            .as_str()
            .unwrap()
            .len(),
        19
    );
    assert_eq!(goals["goals"][1]["status"], "active");

    let status = ultragoal_status(dir.path());
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["evidence_dir"], evidence_dir.display().to_string());
    assert_eq!(status["counts"]["complete"], 1);
    assert_eq!(status["counts"]["active"], 1);
    assert_eq!(status["active_goal"]["id"], "G002");

    let ledger = fs::read_to_string(state_dir.join("ledger.jsonl")).unwrap();
    assert!(ledger.contains("\"event\":\"goals_created\""));
    assert!(ledger.contains("\"event\":\"goal_started\""));
    assert!(ledger.contains("\"event\":\"goal_checkpointed\""));
}

#[test]
fn ultragoal_complete_goals_alias_starts_goal_for_compatibility() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    approve_ralplan_for_ultragoal(dir.path(), "Implement one verified slice.");
    let create = create_goals(dir.path(), false);
    assert!(
        create.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );

    let next = complete_goals_alias(dir.path());
    assert!(
        next.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&next.stderr)
    );
    let next: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_eq!(next["state"], "started");
    assert_eq!(next["next_goal"]["id"], "G001");
}

#[test]
fn ultragoal_cli_strips_visible_approval_tail_and_uses_substantive_title() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let plan = "**실행 계획**\n\n\
**요약**\n\
2048 화면에 진행 상태 row를 추가합니다. 표시 값은 `2048 목표 진행률`, `최고 타일`, `빈 칸 수`입니다.\n\n\
**범위**\n\
`game.js`, `app.js`, `index.html`, `styles.css`, `tests/game.test.js`, `tests/app.spec.js`만 수정합니다.\n\n\
**작업 순서**\n\
1. `getProgressStats(board)` 단위 테스트를 추가합니다.\n\
2. 2048 렌더링에 진행 상태 row를 연결합니다.\n\
3. 모바일 겹침 회귀 테스트를 확장합니다.\n\n\
**승인 질문**\n\
이 계획을 어떻게 처리할까요?\n\n\
1. 계획을 더 다듬기\n\
2. `ultragoal`로 실행 승인\n\
3. `team`으로 실행 승인\n\
4. 승인 보류";

    approve_visible_plan_for_ultragoal(dir.path(), plan);
    let create = create_goals(dir.path(), false);
    assert!(
        create.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );

    let goals = read_json(
        &dir.path()
            .join(".megara/artifacts/ultragoal/sess-ug/goals.json"),
    );
    assert_eq!(goals["goals"].as_array().unwrap().len(), 3);
    let title = goals["goals"][0]["title"].as_str().unwrap();
    let objective = goals["goals"][0]["objective"].as_str().unwrap();
    assert_ne!(title, "실행 계획");
    assert!(title.contains("`getProgressStats(board)` 단위 테스트를 추가합니다"));
    assert!(objective.contains("단위 테스트"));
    assert!(!objective.contains("승인 질문"));
    assert!(!objective.contains("계획을 더 다듬기"));
    assert!(!objective.contains("`ultragoal`로 실행 승인"));
    assert!(!objective.contains("승인 보류"));
    assert_eq!(
        goals["goals"][1]["title"],
        "2048 렌더링에 진행 상태 row를 연결합니다."
    );
    assert_eq!(
        goals["goals"][2]["title"],
        "모바일 겹침 회귀 테스트를 확장합니다."
    );
}

fn approve_visible_plan_for_ultragoal(project: &Path, plan: &str) {
    submit_ready_reviews(project, ULTRAGOAL_TEST_SESSION);
    let message = format!(
        "{plan}\n\n<!--\nMegara Plan Gate:\n- id: {PLAN_ID}\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n  - approve_team\n  - stop_pending\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: {PLAN_ID}\n- next: approval\n-->\n"
    );
    assert_success(&stop_message(project, ULTRAGOAL_TEST_SESSION, &message));
    let ralplan_state = read_state(project, RALPLAN, ULTRAGOAL_TEST_SESSION);
    let plan_sha256 = ralplan_state["plan_sha256"].as_str().unwrap();
    let approval_prompt = format!(
        "<!--\nMegara Approval Gate:\n- plan_id: {PLAN_ID}\n- plan_sha256: {plan_sha256}\n- handoff_target: ultragoal\n-->\n"
    );
    assert_success(&user_prompt(
        project,
        ULTRAGOAL_TEST_SESSION,
        &approval_prompt,
    ));
}
