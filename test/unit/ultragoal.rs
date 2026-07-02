use super::*;

#[test]
fn parses_single_goal_without_markers() {
    let goals = parse_goals("Ship a stable game\n\n- add tests").unwrap();

    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0].title, "Ship a stable game");
    assert!(goals[0].objective.contains("add tests"));
}

#[test]
fn parses_single_goal_after_frontmatter() {
    let goals = parse_goals(
        r#"---
skill: "ralplan"
plan_id: "rp-dashboard-menu"
---

**Ralplan 실행 계획: 카드형 게임 대시보드 추가**

목표: 게임 선택 화면을 추가하고 2048 진입 흐름을 유지한다.
"#,
    )
    .unwrap();

    assert_eq!(goals.len(), 1);
    assert_eq!(
        goals[0].title,
        "Ralplan 실행 계획: 카드형 게임 대시보드 추가"
    );
    assert!(goals[0].objective.contains("목표: 게임 선택 화면"));
    assert!(!goals[0].objective.starts_with("---"));
}

#[test]
fn parses_column_zero_goal_markers_only() {
    let goals = parse_goals(
        "preamble\n@goal: Board shell\nBuild the board.\n  @goal: ignored\n@goal Score model\nTrack score.",
    )
    .unwrap();

    assert_eq!(goals.len(), 2);
    assert_eq!(goals[0].title, "Board shell");
    assert!(goals[0].objective.contains("@goal: ignored"));
    assert_eq!(goals[1].title, "Score model");
}

#[test]
fn quality_gate_requires_clear_architect_review() {
    let gate = json!({
        "architectReview": {
            "recommendation": "ITERATE",
            "architectureStatus": "CLEAR",
            "productStatus": "CLEAR",
            "codeStatus": "CLEAR",
            "evidence": "reviewed",
            "blockers": []
        },
        "executorQa": {
            "status": "passed",
            "e2eStatus": "passed",
            "redTeamStatus": "passed",
            "evidence": "tested",
            "blockers": []
        },
        "iteration": {
            "status": "passed",
            "fullRerun": true,
            "evidence": "rerun",
            "blockers": []
        }
    });

    assert!(validate_quality_gate(&gate, Path::new(".")).is_err());
}
