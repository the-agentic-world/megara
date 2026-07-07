use super::*;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct UltragoalPlan {
    pub(super) version: u32,
    pub(super) scope: String,
    pub(super) session_id: String,
    pub(super) brief_path: String,
    pub(super) brief_sha256: String,
    #[serde(default)]
    pub(super) source: Option<UltragoalSource>,
    pub(super) goals: Vec<UltragoalGoal>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct UltragoalSource {
    pub(super) kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ralplan_plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ralplan_plan_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ralplan_plan_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_spec_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_spec_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct UltragoalGoal {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) objective: String,
    pub(super) status: String,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) started_at: Option<String>,
    pub(super) completed_at: Option<String>,
    pub(super) evidence: Option<String>,
    pub(super) completion_receipt: Option<CompletionReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct CompletionReceipt {
    pub(super) schema_version: u32,
    pub(super) receipt_id: String,
    pub(super) goal_id: String,
    pub(super) verified_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) brief_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source_plan_sha256: Option<String>,
    pub(super) quality_gate_sha256: String,
    pub(super) evidence_sha256: String,
}

#[derive(Debug, Serialize)]
pub(super) struct StatusReport<'a> {
    pub(super) session_id: &'a str,
    pub(super) path: &'a Path,
    pub(super) evidence_dir: &'a Path,
    pub(super) state: &'static str,
    pub(super) counts: GoalCounts,
    pub(super) active_goal: Option<&'a UltragoalGoal>,
    pub(super) goals: &'a [UltragoalGoal],
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub(super) struct GoalCounts {
    pub(super) pending: usize,
    pub(super) active: usize,
    pub(super) complete: usize,
    pub(super) failed: usize,
    pub(super) blocked: usize,
    pub(super) review_blocked: usize,
    pub(super) superseded: usize,
}

pub(super) struct BriefSource {
    pub(super) content: String,
    pub(super) source: UltragoalSource,
}
