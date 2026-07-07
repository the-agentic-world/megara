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
fn strips_ralplan_approval_choices_from_goal_objective() {
    let goals = parse_goals(
        r#"---
skill: "ralplan"
---

**실행 계획**

목표: 2048 모바일 화면을 개선한다.

검증:
- npm test
- npm run test:e2e

이 계획으로 다음 단계를 어떻게 할까?

1. 계획을 더 다듬기
2. `ultragoal`로 실행 승인
3. `team`으로 실행 승인
4. 승인 보류
"#,
    )
    .unwrap();

    assert_eq!(goals.len(), 1);
    assert!(goals[0].objective.contains("2048 모바일 화면"));
    assert!(!goals[0].objective.contains("ultragoal"));
    assert!(!goals[0].objective.contains("승인 보류"));
}

#[test]
fn strips_hidden_control_comments_from_goal_objective() {
    let goals = parse_goals(
        "Ship feature\n\n<!--\nMegara Workflow State:\n- status: pending\n-->\n\n- verify behavior",
    )
    .unwrap();

    assert_eq!(goals.len(), 1);
    assert!(goals[0].objective.contains("verify behavior"));
    assert!(!goals[0].objective.contains("Megara Workflow State"));
}

#[test]
fn parses_work_steps_as_goals_without_approval_choices() {
    let goals = parse_goals(
        r#"**실행 계획**

**요약과 목표**
2048 화면의 남은 UI 결함을 수정한다.

**작업 순서**
1. 기준 상태 확인
   `npm test`와 `npm run test:e2e`를 먼저 실행한다.
2. 결함 조사
   세 viewport에서 상태 표시 영역과 보드를 확인한다.
3. 최소 수정 구현
   2048 전용 선택자를 우선 사용한다.

**승인 기준**
- 주요 UI 요소가 겹치지 않는다.

이 계획을 어떻게 처리할까요?

1. 계획을 더 다듬기
2. `ultragoal`로 실행 승인
3. `team`으로 실행 승인
4. 승인 보류
"#,
    )
    .unwrap();

    assert_eq!(goals.len(), 3);
    assert_eq!(goals[0].title, "기준 상태 확인");
    assert!(goals[0].objective.contains("npm test"));
    assert_eq!(goals[1].title, "결함 조사");
    assert_eq!(goals[2].title, "최소 수정 구현");
    for goal in goals {
        assert!(!goal.objective.contains("ultragoal"));
        assert!(!goal.objective.contains("승인 보류"));
        assert!(!goal.objective.contains("승인 기준"));
    }
}

#[test]
fn compacts_long_step_titles_without_dangling_code_delimiters() {
    let goals = parse_goals(
        r#"**작업 순서**
1. 전체 검증을 다시 실행한다: `npm test`, `npm run test:e2e`, `npx playwright test tests/app.spec.js --grep "2048"`.
2. 최종 결과를 기록한다.
"#,
    )
    .unwrap();

    assert_eq!(goals.len(), 2);
    assert_eq!(goals[0].title, "전체 검증을 다시 실행한다");
    assert!(!goals[0].title.contains('`'));
    assert!(!goals[0].title.contains('"'));
    assert!(goals[0].objective.contains("npm run test:e2e"));
}

#[test]
fn uses_short_colon_labels_for_execution_step_titles() {
    let goals = parse_goals(
        r#"**작업 순서**
1. 기준 상태 확인: `npm test`, `npm run test:e2e`를 먼저 실행해 기존 실패 여부를 분류한다.
2. 2048 재검증: Playwright 로컬 서버에서 데스크톱/모바일 viewport를 확인하고, overflow, 보드/조작 UI, 키보드, 터치/스와이프 증거를 새 JSON/스크린샷으로 남긴다.
3. 실행 흐름 확인: 최종 실행 요약에 실제 실행 단위 전환이 있었는지 기록한다. 인정 증거는 실행 단위 제목과 전환 순서가 보이는 로그 발췌 또는 그 로그를 근거로 한 요약 문장이다.
"#,
    )
    .unwrap();

    assert_eq!(goals.len(), 3);
    assert_eq!(goals[0].title, "기준 상태 확인");
    assert_eq!(goals[1].title, "2048 재검증");
    assert_eq!(goals[2].title, "실행 흐름 확인");
    assert!(goals[1].objective.contains("Playwright 로컬 서버"));
    assert!(goals[2].objective.contains("실제 실행 단위 전환"));
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

#[test]
fn quality_gate_allows_skipped_e2e_with_evidence() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("reviewed.md"), "reviewed files").unwrap();
    let gate = json!({
        "architectReview": {
            "recommendation": "APPROVE",
            "architectureStatus": "CLEAR",
            "productStatus": "CLEAR",
            "codeStatus": "CLEAR",
            "evidence": "Architecture, product, and code boundaries were reviewed.",
            "reviewedFiles": ["reviewed.md"],
            "blockers": []
        },
        "executorQa": {
            "status": "passed",
            "e2eStatus": "skipped",
            "redTeamStatus": "passed",
            "evidence": "E2E was skipped because the completed change did not alter UI behavior.",
            "commands": ["cargo test"],
            "blockers": []
        },
        "iteration": {
            "status": "passed",
            "fullRerun": true,
            "evidence": "Final verification reran after the skipped E2E decision.",
            "commands": ["cargo test"],
            "blockers": []
        }
    });

    validate_quality_gate(&gate, dir.path()).unwrap();
}

#[test]
fn quality_gate_validates_optional_artifact_refs_when_present() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("reviewed.md"), "reviewed files").unwrap();
    let gate = json!({
        "architectReview": {
            "recommendation": "APPROVE",
            "architectureStatus": "CLEAR",
            "productStatus": "CLEAR",
            "codeStatus": "CLEAR",
            "evidence": "Architecture, product, and code boundaries were reviewed.",
            "reviewedFiles": ["reviewed.md"],
            "blockers": []
        },
        "executorQa": {
            "status": "passed",
            "e2eStatus": "passed",
            "redTeamStatus": "passed",
            "evidence": "Focused tests and manual regression checks passed.",
            "commands": ["cargo test"],
            "artifactRefs": ["missing.log"],
            "blockers": []
        },
        "iteration": {
            "status": "passed",
            "fullRerun": true,
            "evidence": "Final verification reran after cleanup.",
            "commands": ["cargo test"],
            "blockers": []
        }
    });

    assert!(validate_quality_gate(&gate, dir.path()).is_err());
}
