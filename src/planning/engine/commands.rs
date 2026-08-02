use super::super::domain::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StartCommand {
    pub session_id: Option<SessionId>,
    pub project_id: ProjectId,
    pub request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub question_id: QuestionId,
    pub based_on_revision: u64,
    pub text: String,
    pub selected_choice_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRefreshCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub snapshot: RepoEvidenceSnapshot,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    Delta,
    Full,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditReadiness {
    Continue,
    RequestFullAudit,
    Ready,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessGate {
    pub problem: bool,
    pub outcome: bool,
    pub requirement: bool,
    pub non_goal: bool,
    pub decision_boundary: bool,
    pub acceptance_criteria: bool,
    pub no_blocking_blockers: bool,
    pub no_pending_question: bool,
    pub evidence_current: bool,
    pub audit_input_current: bool,
    pub counterexample_review: bool,
}

impl ReadinessGate {
    pub fn is_ready(&self) -> bool {
        self.problem
            && self.outcome
            && self.requirement
            && self.non_goal
            && self.decision_boundary
            && self.acceptance_criteria
            && self.no_blocking_blockers
            && self.no_pending_question
            && self.evidence_current
            && self.audit_input_current
            && self.counterexample_review
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub work_item_id: WorkItemId,
    pub mode: AuditMode,
    pub base_revision: u64,
    pub base_domain_revision: u64,
    pub input_hash: String,
    pub readiness: AuditReadiness,
    pub next_question: Option<QuestionProposal>,
    pub entity_ops: Vec<EntityOp>,
    pub edge_ops: Vec<EdgeOp>,
    pub blocker_ops: Vec<BlockerOp>,
    pub counterexample_review: Option<CounterexampleReview>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntityOp {
    Create {
        temp_ref: String,
        body: EntityBody,
        source_refs: Vec<SourceRef>,
    },
    Revise {
        entity_id: EntityId,
        base_entity_revision: u64,
        body: EntityBody,
        source_refs: Vec<SourceRef>,
    },
    Reject {
        entity_id: EntityId,
        base_entity_revision: u64,
        reason: String,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum AuditEndpoint {
    TempRef { temp_ref: String },
    Entity { entity_id: EntityId, revision: u64 },
    Source(SourceRef),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum EdgeOp {
    Add {
        kind: EdgeKind,
        from: AuditEndpoint,
        to: AuditEndpoint,
        source_refs: Vec<SourceRef>,
    },
    Retire {
        edge_id: EdgeId,
        base_edge_revision: u64,
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum BlockerOp {
    Create {
        temp_ref: String,
        kind: BlockerKind,
        severity: BlockerSeverity,
        statement: String,
        source_refs: Vec<SourceRef>,
    },
    Resolve {
        blocker_id: BlockerId,
        base_blocker_revision: u64,
        resolution: String,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecCandidateCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate: SpecCandidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCandidateCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate: PlanCandidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate_id: CandidateId,
    pub semantic_hash: String,
    pub base_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionRequestCommand {
    pub session_id: SessionId,
    pub expected_revision: u64,
    pub candidate_id: CandidateId,
    pub text: String,
}
