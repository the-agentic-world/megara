use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::proposal::*;

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
    pub internal_uuid: String,
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
            if let Some(max_revision) = records.iter().map(|existing| existing.revision).max() {
                if record.revision <= max_revision {
                    return Err("entity revision must exceed history maximum".to_string());
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
    pub created_event_seq: u64,
    pub created_ordinal: u32,
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
