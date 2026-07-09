use serde::{Deserialize, Serialize};

pub(crate) const FALLBACK_NOTICE: &str = "CLI split pane 생성 실패로 subagent fallback 사용";

#[path = "team/split.rs"]
pub(crate) mod split;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TeamMessageKind {
    Assignment,
    TeammateStatus,
    TeammateResult,
    TeammateFailure,
    LeaderSynthesis,
    FallbackNotice,
}

impl TeamMessageKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TeamMessageKind::Assignment => "assignment",
            TeamMessageKind::TeammateStatus => "teammate-status",
            TeamMessageKind::TeammateResult => "teammate-result",
            TeamMessageKind::TeammateFailure => "teammate-failure",
            TeamMessageKind::LeaderSynthesis => "leader-synthesis",
            TeamMessageKind::FallbackNotice => "fallback-notice",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TeamMessage {
    pub kind: TeamMessageKind,
    pub correlation_id: String,
    pub teammate_id: String,
    pub role: String,
    pub content: String,
}

impl TeamMessage {
    pub(crate) fn validates_contract(&self) -> bool {
        !self.correlation_id.trim().is_empty()
            && !self.teammate_id.trim().is_empty()
            && !self.role.trim().is_empty()
            && !self.content.trim().is_empty()
    }

    pub(crate) fn is_completion_receipt(&self) -> bool {
        self.validates_contract()
            && matches!(
                self.kind,
                TeamMessageKind::TeammateResult | TeamMessageKind::TeammateFailure
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamRole {
    Planner,
    Architect,
    Executor,
    Critic,
}

impl TeamRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TeamRole::Planner => "planner",
            TeamRole::Architect => "architect",
            TeamRole::Executor => "executor",
            TeamRole::Critic => "critic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitPaneLayout {
    pub left_column: &'static str,
    pub right_rows: usize,
}

pub(crate) fn select_teammates(task: &str) -> Vec<TeamRole> {
    let lower = task.to_ascii_lowercase();
    let has_architecture_risk = contains_any(
        &lower,
        &[
            "architecture",
            "architect",
            "adapter",
            "runtime",
            "hook",
            "integration",
            "migration",
            "boundary",
            "cross-file",
            "cross module",
            "cmux",
            "tmux",
            "orca",
            "codex",
        ],
    ) || task.contains("아키텍처")
        || task.contains("런타임")
        || task.contains("훅")
        || task.contains("통합")
        || task.contains("마이그레이션");
    let has_planning_risk = contains_any(
        &lower,
        &["plan", "sequence", "workflow", "coordination", "handoff"],
    ) || task.contains("계획")
        || task.contains("워크플로우")
        || task.contains("조율");

    if has_architecture_risk {
        return vec![
            TeamRole::Planner,
            TeamRole::Architect,
            TeamRole::Executor,
            TeamRole::Critic,
        ];
    }

    if has_planning_risk {
        return vec![TeamRole::Planner, TeamRole::Executor, TeamRole::Critic];
    }

    vec![TeamRole::Executor, TeamRole::Critic]
}

pub(crate) fn role_names(roles: &[TeamRole]) -> Vec<&'static str> {
    roles.iter().map(|role| role.as_str()).collect()
}

pub(crate) fn parse_role(value: &str) -> Option<TeamRole> {
    match value.trim().to_ascii_lowercase().as_str() {
        "planner" => Some(TeamRole::Planner),
        "architect" => Some(TeamRole::Architect),
        "executor" => Some(TeamRole::Executor),
        "critic" => Some(TeamRole::Critic),
        _ => None,
    }
}

pub(crate) fn team_correlation_id(timestamp: &str) -> String {
    format!("team-{}", safe_correlation_part(timestamp))
}

pub(crate) fn cli_split_transports() -> [&'static str; 3] {
    ["cmux", "tmux", "orca"]
}

pub(crate) fn message_contract_kinds() -> [&'static str; 6] {
    [
        TeamMessageKind::Assignment.as_str(),
        TeamMessageKind::TeammateStatus.as_str(),
        TeamMessageKind::TeammateResult.as_str(),
        TeamMessageKind::TeammateFailure.as_str(),
        TeamMessageKind::LeaderSynthesis.as_str(),
        TeamMessageKind::FallbackNotice.as_str(),
    ]
}

pub(crate) fn message_contract_example(correlation_id: &str, teammate_id: &str) -> TeamMessage {
    let message = TeamMessage {
        kind: TeamMessageKind::Assignment,
        correlation_id: correlation_id.to_string(),
        teammate_id: teammate_id.to_string(),
        role: "executor".to_string(),
        content: "Bounded teammate task with acceptance criteria.".to_string(),
    };
    debug_assert!(message.validates_contract());
    debug_assert!(!message.is_completion_receipt());
    message
}

pub(crate) fn split_layout(teammate_count: usize) -> Option<SplitPaneLayout> {
    if !(2..=4).contains(&teammate_count) {
        return None;
    }
    Some(SplitPaneLayout {
        left_column: "leader",
        right_rows: teammate_count,
    })
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn safe_correlation_part(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    safe.trim_matches('-').to_string()
}
