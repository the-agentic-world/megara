use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::super::domain::*;

pub const LEGACY_CONTEXT_MAX_BYTES: usize = 4 * 1024 * 1024;
pub const LEGACY_EVENT_MAX_BYTES: usize = 12 * 1024 * 1024;
pub const LEGACY_MAX_FILES: usize = 1_024;
pub const LEGACY_MAX_PATH_BYTES: usize = 1_024;
pub const LEGACY_MAX_METADATA_BYTES: usize = 1_024;
pub const LEGACY_MAX_INITIAL_REQUEST_BYTES: usize = 64 * 1024;
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
pub struct LegacyImportCommand {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub initial_request: String,
    pub legacy_bundle: LegacyContextBundle,
}

impl LegacyContextBundle {
    pub fn validate(&self) -> Result<(), String> {
        if self.migration_id.trim().is_empty()
            || self.source_backup_id.trim().is_empty()
            || self.source_bundle_hash.trim().is_empty()
            || self.source_path.trim().is_empty()
        {
            return Err("legacy bundle metadata must not be blank".to_string());
        }
        for (label, value) in [
            ("migration_id", self.migration_id.as_str()),
            ("source_backup_id", self.source_backup_id.as_str()),
            ("source_bundle_hash", self.source_bundle_hash.as_str()),
        ] {
            if value.len() > LEGACY_MAX_METADATA_BYTES {
                return Err(format!("legacy bundle {label} is too large"));
            }
        }
        if self.source_path.len() > LEGACY_MAX_PATH_BYTES {
            return Err("legacy bundle source_path is too large".to_string());
        }
        validate_sha256("source_bundle_hash", &self.source_bundle_hash)?;
        validate_relative_path(&self.source_path)?;
        if self.files.is_empty() {
            return Err("legacy bundle must contain at least one file".to_string());
        }
        if self.files.len() > LEGACY_MAX_FILES {
            return Err("legacy bundle contains too many files".to_string());
        }
        let mut paths = BTreeSet::new();
        let mut decoded_total = 0usize;
        for file in &self.files {
            if file.relative_path.len() > LEGACY_MAX_PATH_BYTES {
                return Err(format!(
                    "legacy bundle path is too large: {}",
                    file.relative_path
                ));
            }
            validate_relative_path(&file.relative_path)?;
            if !paths.insert(file.relative_path.clone()) {
                return Err("legacy bundle file paths must be unique".to_string());
            }
            let declared_size = file.declared_size()?;
            decoded_total = decoded_total
                .checked_add(declared_size as usize)
                .ok_or_else(|| "legacy bundle size overflow".to_string())?;
            if decoded_total > LEGACY_CONTEXT_MAX_BYTES {
                return Err("legacy bundle decoded bytes exceed 4MiB".to_string());
            }
            file.decode_bytes()?;
        }
        let sorted = self
            .files
            .windows(2)
            .all(|pair| pair[0].relative_path < pair[1].relative_path);
        if !sorted {
            return Err("legacy bundle files must be sorted by relative_path".to_string());
        }
        if !paths.contains(&self.source_path) {
            return Err("legacy bundle source_path must name an opaque file".to_string());
        }
        if self.source_bundle_hash != self.computed_source_bundle_hash() {
            return Err("legacy bundle source_bundle_hash does not match files".to_string());
        }
        Ok(())
    }

    pub fn computed_source_bundle_hash(&self) -> String {
        let basis = self
            .files
            .iter()
            .map(|file| {
                json!({
                    "path": file.relative_path,
                    "sha256": file.byte_sha256,
                    "size": file.size,
                })
            })
            .collect::<Vec<_>>();
        crate::planning::canonical::canonical_hash(&basis)
    }
}

impl OpaqueLegacyFile {
    fn declared_size(&self) -> Result<u64, String> {
        validate_sha256("byte_sha256", &self.byte_sha256)?;
        match self.encoding {
            LegacyContextEncoding::Utf8 => {
                if self.payload.len() as u64 != self.size {
                    return Err(format!("legacy file size mismatch: {}", self.relative_path));
                }
            }
            LegacyContextEncoding::Hex => {
                if !self.payload.len().is_multiple_of(2)
                    || !self
                        .payload
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                    || self.payload.len() as u64 != self.size.saturating_mul(2)
                {
                    return Err(format!(
                        "invalid lowercase hex payload: {}",
                        self.relative_path
                    ));
                }
            }
        }
        Ok(self.size)
    }

    fn decode_bytes(&self) -> Result<Vec<u8>, String> {
        let bytes = match self.encoding {
            LegacyContextEncoding::Utf8 => self.payload.as_bytes().to_vec(),
            LegacyContextEncoding::Hex => decode_hex(&self.payload)?,
        };
        if bytes.len() as u64 != self.size {
            return Err(format!("legacy file size mismatch: {}", self.relative_path));
        }
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = format!("sha256:{:x}", hasher.finalize());
        if actual != self.byte_sha256 {
            return Err(format!(
                "legacy file digest mismatch: {}",
                self.relative_path
            ));
        }
        Ok(bytes)
    }
}

fn validate_relative_path(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.contains('\0') || value.contains('\\') {
        return Err("legacy paths must be normalized project-relative paths".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(format!("legacy path is not safely relative: {value}"));
    }
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("hex legacy payload must be even-length lowercase hex".to_string());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| "invalid hex legacy payload".to_string())
        })
        .collect()
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be sha256: followed by 64 lowercase hex digits"
        ));
    }
    Ok(())
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
