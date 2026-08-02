use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::proposal::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventEffect {
    QuestionSet { question_id: QuestionId },
    ModelActionRequested { kind: ModelActionKind },
    PhaseChanged { phase: LifecyclePhase },
    EntityInvalidated { entity_id: EntityId },
    ArtifactInvalidated { artifact: String },
    ApprovalsRevoked { artifact: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateEvent {
    pub session_id: SessionId,
    pub seq: u64,
    pub revision_after: u64,
    pub domain_revision_after: u64,
    pub plan_revision_after: u64,
    pub operation: String,
    pub primary: Value,
    pub effects: Vec<EventEffect>,
}
