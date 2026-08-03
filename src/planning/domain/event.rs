use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::proposal::*;

pub const EVENT_SCHEMA_VERSION: &str = "megara.event/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EventEffect {
    QuestionSet {
        question_id: QuestionId,
    },
    AnswerSubmitted {
        answer_id: String,
    },
    ModelActionRequested {
        kind: ModelActionKind,
    },
    PhaseChanged {
        phase: LifecyclePhase,
    },
    EntityCreated {
        entity_id: EntityId,
        revision: u64,
        internal_uuid: String,
    },
    EntityInvalidated {
        entity_id: EntityId,
    },
    BlockerCreated {
        blocker_id: BlockerId,
    },
    ArtifactInvalidated {
        artifact: String,
    },
    ApprovalsRevoked {
        artifact: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateEvent {
    pub schema: String,
    pub session_id: SessionId,
    pub seq: u64,
    pub revision_after: u64,
    pub domain_revision_after: u64,
    pub plan_revision_after: u64,
    pub operation: String,
    pub primary: Value,
    pub effects: Vec<EventEffect>,
}
