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
#[path = "engine/evidence_ops.rs"]
mod evidence_ops;
#[path = "engine/readiness.rs"]
mod readiness;
#[path = "engine/readiness_validation.rs"]
mod readiness_validation;
#[path = "engine/work_items.rs"]
mod work_items;

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
pub(crate) use core::command_value;
pub(crate) use evidence_ops::{evidence_changes, validate_snapshot};
pub(crate) use readiness::{
    compute_readiness_gate, require_model_action, validate_audit_shape, validate_work_item,
};
pub(crate) use readiness_validation::{validate_counterexample_review, validate_question_proposal};
pub(crate) use work_items::work_item;
