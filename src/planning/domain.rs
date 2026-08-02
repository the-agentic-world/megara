use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type SessionId = String;
pub type ProjectId = String;
pub type QuestionId = String;
pub type WorkItemId = String;
pub type EntityId = String;
pub type BlockerId = String;
pub type CandidateId = String;
pub type EdgeId = String;

pub const QUESTION_AUTHORING_VERSION: &str = "megara.question-authoring/v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    Interview,
    Specification,
    Planning,
    Complete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelActionKind {
    DeltaAudit,
    FullAudit,
    GenerateSpec,
    GeneratePlan,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringRule {
    pub id: String,
    pub instruction: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QuestionAuthoring {
    pub version: String,
    pub rules: Vec<AuthoringRule>,
}

impl QuestionAuthoring {
    pub fn v1() -> Self {
        Self {
            version: QUESTION_AUTHORING_VERSION.to_string(),
            rules: vec![
                AuthoringRule {
                    id: "audience".to_string(),
                    instruction:
                        "Megara와 구현 기술을 모르는 소프트웨어 기획 초심자를 독자로 둔다."
                            .to_string(),
                },
                AuthoringRule {
                    id: "context".to_string(),
                    instruction: "쉬운 말 2~4문장으로 배경과 지금 결정할 이유를 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "one-decision".to_string(),
                    instruction: "한 번에 하나의 결정만 묻는다.".to_string(),
                },
                AuthoringRule {
                    id: "terms".to_string(),
                    instruction: "전문용어를 피하고, 불가피하면 뜻뿐 아니라 이 문맥의 역할과 중요성을 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "choices".to_string(),
                    instruction: "각 선택지의 진행 방향, 장점, 감수할 점을 서로 겹치지 않게 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "impact".to_string(),
                    instruction: "답에 따라 spec 또는 plan의 무엇이 달라지는지 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "recommendation".to_string(),
                    instruction: "유효한 근거를 연결할 수 있을 때만 추천한다.".to_string(),
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TechnicalTerm {
    pub term: String,
    pub plain_explanation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub direction: String,
    pub benefits: Vec<String>,
    pub tradeoffs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recommendation {
    pub choice_id: String,
    pub reason: String,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnswerMode {
    Choice {
        choices: Vec<Choice>,
        recommendation: Option<Recommendation>,
        freeform_hint: String,
    },
    Freeform {
        freeform_hint: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionProposal {
    pub context: String,
    pub question: String,
    pub why_it_matters: String,
    pub technical_terms: Vec<TechnicalTerm>,
    pub source_refs: Vec<SourceRef>,
    pub answer: AnswerMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceRef {
    InitialRequest {
        id: String,
    },
    Answer {
        id: String,
    },
    Evidence {
        id: String,
    },
    Entity {
        id: EntityId,
        revision: u64,
    },
    ApprovedSpec {
        id: CandidateId,
        semantic_hash: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Problem,
    Outcome,
    Fact,
    Decision,
    DecisionBoundary,
    Requirement,
    AcceptanceCriterion,
    Constraint,
    NonGoal,
    Assumption,
    Risk,
    PlanStep,
    Verification,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum EntityBody {
    Problem {
        statement: String,
    },
    Outcome {
        statement: String,
        observable_result: String,
    },
    Fact {
        statement: String,
        evidence_refs: Vec<String>,
    },
    Decision {
        statement: String,
        selected_option: String,
    },
    DecisionBoundary {
        autonomous_scope: Vec<String>,
        requires_user_approval: Vec<String>,
    },
    Requirement {
        statement: String,
        priority: RequirementPriority,
    },
    AcceptanceCriterion {
        statement: String,
    },
    Constraint {
        statement: String,
    },
    NonGoal {
        statement: String,
    },
    Assumption {
        statement: String,
        validation_status: ValidationStatus,
    },
    Risk {
        statement: String,
        impact: RiskImpact,
        mitigation: String,
    },
    PlanStep {
        objective: String,
        change_surface: Vec<String>,
        rollback_or_recovery: String,
    },
    Verification {
        method: VerificationMethod,
        procedure: String,
        expected_result: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RequirementPriority {
    Must,
    Should,
    Could,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    Unverified,
    Confirmed,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskImpact {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationMethod {
    Command,
    Assertion,
    Metric,
    Manual,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityDisposition {
    Current,
    Superseded,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EntityValidity {
    Valid,
    Stale {
        since_domain_revision: u64,
        causes: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRecord {
    pub entity_id: EntityId,
    pub revision: u64,
    pub kind: EntityKind,
    pub body: EntityBody,
    pub disposition: EntityDisposition,
    pub validity: EntityValidity,
    pub source_refs: Vec<SourceRef>,
    pub created_event_seq: u64,
    pub created_ordinal: u32,
}

impl EntityRecord {
    pub fn is_current(&self) -> bool {
        self.disposition == EntityDisposition::Current
            && matches!(self.validity, EntityValidity::Valid)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRevisionRef {
    pub id: EntityId,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub id: EntityId,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum EdgeTarget {
    Entity(EntityRef),
    Source(SourceRef),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    HasAcceptanceCriterion,
    Implements,
    Verifies,
    ExecutedBy,
    DependsOn,
    DerivedFrom,
    Supersedes,
    ConflictsWith,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub edge_id: EdgeId,
    pub revision: u64,
    pub kind: EdgeKind,
    pub from: EntityRef,
    pub to: EdgeTarget,
    pub source_refs: Vec<SourceRef>,
    pub retired: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EntityGraph {
    pub revisions: BTreeMap<EntityId, Vec<EntityRecord>>,
    pub edges: Vec<Edge>,
}

impl EntityGraph {
    pub fn current(&self, id: &str) -> Option<&EntityRecord> {
        self.revisions
            .get(id)
            .and_then(|records| records.iter().rev().find(|record| record.is_current()))
    }

    pub fn at_revision(&self, id: &str, revision: u64) -> Option<&EntityRecord> {
        self.revisions
            .get(id)
            .and_then(|records| records.iter().find(|record| record.revision == revision))
    }

    pub fn current_count(&self, kind: EntityKind) -> usize {
        self.revisions
            .values()
            .filter_map(|records| records.iter().rev().find(|record| record.is_current()))
            .filter(|record| record.kind == kind)
            .count()
    }

    pub fn current_requirements(&self) -> Vec<&EntityRecord> {
        self.revisions
            .values()
            .filter_map(|records| records.iter().rev().find(|record| record.is_current()))
            .filter(|record| record.kind == EntityKind::Requirement)
            .collect()
    }

    pub fn current_acceptance_criteria(&self) -> Vec<&EntityRecord> {
        self.revisions
            .values()
            .filter_map(|records| records.iter().rev().find(|record| record.is_current()))
            .filter(|record| record.kind == EntityKind::AcceptanceCriterion)
            .collect()
    }

    pub fn insert(&mut self, record: EntityRecord) -> Result<(), String> {
        if record.revision == 0 || record.entity_id.trim().is_empty() {
            return Err("entity ID and revision are required".to_string());
        }
        if record.source_refs.is_empty() {
            return Err("entity source_refs must not be empty".to_string());
        }
        if let Some(records) = self.revisions.get(&record.entity_id) {
            if records
                .iter()
                .any(|existing| existing.revision == record.revision)
            {
                return Err("duplicate entity revision".to_string());
            }
            if let Some(current) = records.iter().rev().find(|existing| existing.is_current()) {
                if record.revision <= current.revision {
                    return Err("entity revision must increase".to_string());
                }
            }
        }
        self.revisions
            .entry(record.entity_id.clone())
            .or_default()
            .push(record);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: Edge) -> Result<(), String> {
        if edge.source_refs.is_empty() {
            return Err("edge source_refs must not be empty".to_string());
        }
        let Some(from) = self.at_revision(&edge.from.id, edge.from.revision) else {
            return Err("edge from endpoint does not exist".to_string());
        };
        if !from.is_current() {
            return Err("edge from endpoint must be current".to_string());
        }
        match &edge.to {
            EdgeTarget::Entity(to) => {
                let Some(to_record) = self.at_revision(&to.id, to.revision) else {
                    return Err("edge to endpoint does not exist".to_string());
                };
                if edge.kind != EdgeKind::Supersedes && !to_record.is_current() {
                    return Err("edge to endpoint must be current".to_string());
                }
                if edge.kind == EdgeKind::Supersedes
                    && (edge.from.id != to.id || edge.from.revision <= to.revision)
                {
                    return Err(
                        "supersedes must point from a newer revision of the same entity"
                            .to_string(),
                    );
                }
                if !edge.kind.allows(from.kind, Some(to_record.kind)) {
                    return Err("edge direction is not allowed".to_string());
                }
            }
            EdgeTarget::Source(_) => {
                if !edge.kind.allows(from.kind, None) {
                    return Err("edge source direction is not allowed".to_string());
                }
            }
        }
        if self.edges.iter().any(|existing| {
            !existing.retired
                && existing.kind == edge.kind
                && existing.from == edge.from
                && existing.to == edge.to
        }) {
            return Err("duplicate current edge".to_string());
        }
        self.edges.push(edge);
        Ok(())
    }
}

impl EdgeKind {
    pub fn allows(self, from: EntityKind, to: Option<EntityKind>) -> bool {
        match self {
            Self::HasAcceptanceCriterion => {
                from == EntityKind::Requirement && to == Some(EntityKind::AcceptanceCriterion)
            }
            Self::Implements => from == EntityKind::PlanStep && to == Some(EntityKind::Requirement),
            Self::Verifies => {
                from == EntityKind::Verification && to == Some(EntityKind::AcceptanceCriterion)
            }
            Self::ExecutedBy => {
                from == EntityKind::Verification && to == Some(EntityKind::PlanStep)
            }
            Self::DependsOn => to.is_some(),
            Self::DerivedFrom => to.is_none(),
            Self::Supersedes | Self::ConflictsWith => to.is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    MissingProblem,
    MissingOutcome,
    MissingRequirement,
    MissingNonGoal,
    MissingDecisionBoundary,
    MissingAcceptanceCriterion,
    OpenDecision,
    Contradiction,
    EvidenceRequired,
    InvalidSource,
    ModelOutputInvalid,
    ManualReviewRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerSeverity {
    Blocking,
    Advisory,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Blocker {
    pub blocker_id: BlockerId,
    pub revision: u64,
    pub kind: BlockerKind,
    pub severity: BlockerSeverity,
    pub statement: String,
    pub source_refs: Vec<SourceRef>,
    pub resolved_at_revision: Option<u64>,
}

impl Blocker {
    pub fn is_blocking(&self) -> bool {
        self.severity == BlockerSeverity::Blocking && self.resolved_at_revision.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingQuestion {
    pub question_id: QuestionId,
    pub based_on_revision: u64,
    pub proposal: QuestionProposal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerRecord {
    pub answer_id: String,
    pub question_id: QuestionId,
    pub based_on_revision: u64,
    pub text: String,
    pub selected_choice_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptIndex {
    pub initial_request: String,
    pub answers: Vec<AnswerRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepoEvidenceSnapshot {
    pub evidence_hash: String,
    pub head_oid: Option<String>,
    pub status_hash: String,
    pub cited_files_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FullAuditRef {
    pub input_hash: String,
    pub base_domain_revision: u64,
    pub counterexample_review_performed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecCandidate {
    pub candidate_id: CandidateId,
    pub base_domain_revision: u64,
    pub audit_input_hash: String,
    pub semantic_hash: String,
    pub entity_refs: Vec<EntityRevisionRef>,
    pub content: Value,
    pub stale: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCandidate {
    pub candidate_id: CandidateId,
    pub base_plan_revision: u64,
    pub plan_input_hash: String,
    pub semantic_hash: String,
    pub spec_candidate_id: CandidateId,
    pub spec_semantic_hash: String,
    pub content: Value,
    pub stale: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRef {
    pub candidate_id: CandidateId,
    pub semantic_hash: String,
    pub base_revision: u64,
    pub approval_event_seq: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTrack<T> {
    pub current_candidate: Option<T>,
    pub approval: Option<ApprovalRef>,
}

impl<T> Default for ArtifactTrack<T> {
    fn default() -> Self {
        Self {
            current_candidate: None,
            approval: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelWorkItem {
    pub kind: ModelActionKind,
    pub work_item_id: WorkItemId,
    pub session_id: SessionId,
    pub base_revision: u64,
    pub base_domain_revision: u64,
    pub base_plan_revision: u64,
    pub input_hash: String,
    pub output_schema: String,
    pub context: Value,
    pub question_authoring: Option<QuestionAuthoring>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningState {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub revision: u64,
    pub domain_revision: u64,
    pub plan_revision: u64,
    pub phase: LifecyclePhase,
    pub pending_question: Option<PendingQuestion>,
    pub required_model_action: Option<ModelWorkItem>,
    pub blockers: BTreeMap<BlockerId, Blocker>,
    pub imported_legacy_context: bool,
    pub entities: EntityGraph,
    pub transcript: TranscriptIndex,
    pub repo_snapshot: Option<RepoEvidenceSnapshot>,
    pub full_audit: Option<FullAuditRef>,
    pub spec: ArtifactTrack<SpecCandidate>,
    pub plan: ArtifactTrack<PlanCandidate>,
}

impl PlanningState {
    pub fn new(session_id: SessionId, project_id: ProjectId, initial_request: String) -> Self {
        Self {
            session_id,
            project_id,
            revision: 0,
            domain_revision: 0,
            plan_revision: 0,
            phase: LifecyclePhase::Interview,
            pending_question: None,
            required_model_action: None,
            blockers: BTreeMap::new(),
            imported_legacy_context: false,
            entities: EntityGraph::default(),
            transcript: TranscriptIndex {
                initial_request,
                answers: Vec::new(),
            },
            repo_snapshot: None,
            full_audit: None,
            spec: ArtifactTrack::default(),
            plan: ArtifactTrack::default(),
        }
    }

    pub fn has_blocking_blocker(&self) -> bool {
        self.blockers.values().any(Blocker::is_blocking)
    }

    pub fn derived(&self) -> DerivedState {
        DerivedState {
            waiting_for_user: self.pending_question.is_some(),
            waiting_for_model: self.required_model_action.is_some(),
            blocked: self.has_blocking_blocker(),
            waiting_for_spec_approval: self.phase == LifecyclePhase::Specification
                && self.spec.current_candidate.is_some()
                && self.spec.approval.is_none(),
            waiting_for_plan_approval: self.phase == LifecyclePhase::Planning
                && self.plan.current_candidate.is_some()
                && self.plan.approval.is_none(),
            spec_stale: self
                .spec
                .current_candidate
                .as_ref()
                .is_some_and(|candidate| candidate.stale),
            plan_stale: self
                .plan
                .current_candidate
                .as_ref()
                .is_some_and(|candidate| candidate.stale),
        }
    }

    pub fn assert_invariants(&self) -> Result<(), String> {
        if self.pending_question.is_some() && self.required_model_action.is_some() {
            return Err(
                "pending_question and required_model_action are mutually exclusive".to_string(),
            );
        }
        if self.revision == 0 && (self.domain_revision != 0 || self.plan_revision != 0) {
            return Err("uncommitted state cannot have derived revisions".to_string());
        }
        if self.domain_revision > self.revision || self.plan_revision > self.revision {
            return Err("derived revision cannot exceed revision".to_string());
        }
        if self.phase == LifecyclePhase::Complete
            && (self.spec.approval.is_none() || self.plan.approval.is_none())
        {
            return Err("complete requires spec and plan approval".to_string());
        }
        if self.spec.approval.is_some()
            && self
                .spec
                .current_candidate
                .as_ref()
                .is_none_or(|candidate| candidate.stale)
        {
            return Err("spec approval requires a current candidate".to_string());
        }
        if self.plan.approval.is_some()
            && self
                .plan
                .current_candidate
                .as_ref()
                .is_none_or(|candidate| candidate.stale)
        {
            return Err("plan approval requires a current candidate".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedState {
    pub waiting_for_user: bool,
    pub waiting_for_model: bool,
    pub blocked: bool,
    pub waiting_for_spec_approval: bool,
    pub waiting_for_plan_approval: bool,
    pub spec_stale: bool,
    pub plan_stale: bool,
}

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
