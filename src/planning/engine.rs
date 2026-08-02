use std::collections::BTreeMap;

use super::domain::*;

#[path = "engine/artifacts.rs"]
mod artifacts;
#[path = "engine/audit.rs"]
mod audit;
#[path = "engine/audit_ops.rs"]
mod audit_ops;
#[path = "engine/commands.rs"]
mod commands;
#[path = "engine/core.rs"]
mod core;
#[path = "engine/error.rs"]
mod error;
#[path = "engine/readiness.rs"]
mod readiness;
#[path = "engine/readiness_validation.rs"]
mod readiness_validation;

pub use commands::{
    AnswerCommand, ApprovalCommand, AuditCommand, AuditEndpoint, AuditMode, AuditReadiness,
    BlockerOp, EdgeOp, EntityOp, EvidenceRefreshCommand, PlanCandidateCommand, ReadinessGate,
    RevisionRequestCommand, SpecCandidateCommand, StartCommand,
};
pub use core::{EvidenceRefreshResult, MutationResult};
pub use error::CoreError;

#[derive(Clone, Debug, Default)]
pub struct InMemoryPlanningCore {
    pub(crate) sessions: BTreeMap<SessionId, PlanningState>,
    pub(crate) events: Vec<AggregateEvent>,
    pub(crate) next_session_number: u64,
}

pub(crate) use artifacts::{invalidate_artifacts, invalidate_evidence_entities};
pub(crate) use audit_ops::apply_audit_ops;
pub(crate) use core::{command_value, hash_text, work_item};
pub(crate) use readiness::{compute_readiness_gate, require_model_action, validate_work_item};
