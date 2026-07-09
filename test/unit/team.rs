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
fn split_layout_keeps_leader_left_and_teammates_right() {
    let layout = team::split_layout(3).unwrap();
    assert_eq!(layout.left_column, "leader");
    assert_eq!(layout.right_rows, 3);
    assert!(team::split_layout(1).is_none());
    assert!(team::split_layout(5).is_none());
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
fn cli_split_transports_are_limited() {
    assert_eq!(team::cli_split_transports(), ["cmux", "tmux", "orca"]);
    assert_eq!(
        team::FALLBACK_NOTICE,
        "CLI split pane 생성 실패로 subagent fallback 사용"
    );
}

#[test]
fn parses_known_team_roles() {
    assert_eq!(team::parse_role("planner"), Some(team::TeamRole::Planner));
    assert_eq!(
        team::parse_role("architect"),
        Some(team::TeamRole::Architect)
    );
    assert_eq!(team::parse_role("executor"), Some(team::TeamRole::Executor));
    assert_eq!(team::parse_role("critic"), Some(team::TeamRole::Critic));
    assert_eq!(team::parse_role("unknown"), None);
}

#[test]
fn team_correlation_id_is_safe() {
    assert_eq!(team::team_correlation_id("12:34/path"), "team-12-34-path");
}
