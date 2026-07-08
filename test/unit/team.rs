use super::*;

#[test]
fn selects_two_teammates_for_simple_work() {
    let roles = team::select_teammates("Fix the button label");
    assert_eq!(
        roles,
        vec![team::TeamRole::Executor, team::TeamRole::Critic]
    );
}

#[test]
fn selects_architecture_coverage_for_runtime_work() {
    let roles = team::select_teammates("Implement Codex hook runtime adapter integration");
    assert_eq!(
        roles,
        vec![
            team::TeamRole::Planner,
            team::TeamRole::Architect,
            team::TeamRole::Executor,
            team::TeamRole::Critic,
        ]
    );
}

#[test]
fn warp_layout_keeps_leader_left_and_teammates_right() {
    let layout = team::warp_layout(3).unwrap();
    assert_eq!(layout.left_column, "leader");
    assert_eq!(layout.right_rows, 3);
    assert!(team::warp_layout(1).is_none());
    assert!(team::warp_layout(5).is_none());
}

#[test]
fn team_message_requires_correlation_and_teammate_identity() {
    let message = team::TeamMessage {
        kind: team::TeamMessageKind::TeammateResult,
        correlation_id: "corr-1".to_string(),
        teammate_id: "executor-1".to_string(),
        role: "executor".to_string(),
        content: "Implemented and verified.".to_string(),
    };

    assert!(message.validates_contract());
    assert!(message.is_completion_receipt());
}

#[test]
fn malformed_team_message_cannot_complete() {
    let message = team::TeamMessage {
        kind: team::TeamMessageKind::TeammateResult,
        correlation_id: String::new(),
        teammate_id: "executor-1".to_string(),
        role: "executor".to_string(),
        content: "Implemented.".to_string(),
    };

    assert!(!message.validates_contract());
    assert!(!message.is_completion_receipt());
}

#[test]
fn warp_is_fallback_by_default() {
    assert!(!team::warp_is_supported_by_default());
    assert_eq!(
        team::FALLBACK_NOTICE,
        "Warp pane 생성 실패로 subagent fallback 사용"
    );
}
