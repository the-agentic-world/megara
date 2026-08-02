use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{entity::*, proposal::*};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingQuestion {
    pub question_id: QuestionId,
    pub created_event_seq: u64,
    pub created_ordinal: u32,
    pub based_on_revision: u64,
    pub proposal: QuestionProposal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerRecord {
    pub answer_id: String,
    pub created_event_seq: u64,
    pub created_ordinal: u32,
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

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRange {
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub path: String,
    pub ranges: Vec<EvidenceRange>,
    pub size: u64,
    pub sha256: String,
    pub tracked: bool,
    pub captured_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepoEvidenceSnapshot {
    pub evidence_hash: String,
    pub head_oid: Option<String>,
    pub head_ref: Option<String>,
    pub dirty: bool,
    pub status_hash: String,
    pub cited_files_hash: String,
    pub evidence: Vec<EvidenceRecord>,
}

impl RepoEvidenceSnapshot {
    pub fn has_evidence(&self, evidence_id: &str) -> bool {
        self.evidence
            .iter()
            .any(|record| record.evidence_id == evidence_id)
    }

    pub fn semantic_eq(&self, other: &Self) -> bool {
        self.evidence_hash == other.evidence_hash
            && self.head_oid == other.head_oid
            && self.head_ref == other.head_ref
            && self.dirty == other.dirty
            && self.status_hash == other.status_hash
            && self.cited_files_hash == other.cited_files_hash
            && self
                .evidence
                .iter()
                .zip(&other.evidence)
                .all(|(left, right)| {
                    left.evidence_id == right.evidence_id
                        && left.path == right.path
                        && left.ranges == right.ranges
                        && left.size == right.size
                        && left.sha256 == right.sha256
                        && left.tracked == right.tracked
                })
            && self.evidence.len() == other.evidence.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FullAuditRef {
    pub input_hash: String,
    pub base_domain_revision: u64,
    pub counterexample_review: CounterexampleReview,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecCandidate {
    pub candidate_id: CandidateId,
    pub created_event_seq: u64,
    pub created_ordinal: u32,
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
    pub created_event_seq: u64,
    pub created_ordinal: u32,
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
    pub created_event_seq: u64,
    pub created_ordinal: u32,
    pub session_id: SessionId,
    pub base_revision: u64,
    pub base_domain_revision: u64,
    pub base_plan_revision: u64,
    pub input_hash: String,
    pub output_schema: String,
    pub context: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningState {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub title: Option<String>,
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
            title: None,
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
        let spec_stale = match (
            self.spec.current_candidate.as_ref(),
            self.spec.approval.as_ref(),
        ) {
            (Some(candidate), Some(approval)) => {
                candidate.stale
                    || approval.base_revision != candidate.base_domain_revision
                    || approval.base_revision != self.domain_revision
            }
            (Some(candidate), None) => candidate.stale,
            _ => false,
        };
        let plan_stale = match (
            self.plan.current_candidate.as_ref(),
            self.plan.approval.as_ref(),
        ) {
            (Some(candidate), Some(approval)) => {
                candidate.stale
                    || approval.base_revision != candidate.base_plan_revision
                    || approval.base_revision != self.plan_revision
            }
            (Some(candidate), None) => candidate.stale,
            _ => false,
        };
        DerivedState {
            waiting_for_user: self.pending_question.is_some(),
            waiting_for_model: self.required_model_action.is_some(),
            blocked: self.has_blocking_blocker(),
            waiting_for_spec_approval: self.phase == LifecyclePhase::Specification
                && self
                    .spec
                    .current_candidate
                    .as_ref()
                    .is_some_and(|candidate| !candidate.stale)
                && self.spec.approval.is_none(),
            waiting_for_plan_approval: self.phase == LifecyclePhase::Planning
                && self
                    .plan
                    .current_candidate
                    .as_ref()
                    .is_some_and(|candidate| !candidate.stale)
                && self.plan.approval.is_none(),
            spec_stale,
            plan_stale,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedState {
    pub waiting_for_user: bool,
    pub waiting_for_model: bool,
    pub blocked: bool,
    pub waiting_for_spec_approval: bool,
    pub waiting_for_plan_approval: bool,
    pub spec_stale: bool,
    pub plan_stale: bool,
}
