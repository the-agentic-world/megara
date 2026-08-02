use serde::{de::DeserializeOwned, de::Deserializer, Deserialize};
use serde_json::Value;

use super::super::domain::{
    CounterexampleReview, EntityBody, EntityKind, ModelActionKind, PlanningState, QuestionProposal,
    RequirementPriority, RiskImpact, SourceRef, ValidationStatus,
};
use super::super::engine::{AuditCommand, AuditMode, AuditReadiness, BlockerOp, EdgeOp, EntityOp};
use super::error::ServiceError;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuditApplyParams {
    pub(crate) mode: AuditMode,
    pub(crate) proposal: AuditProposal,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuditProposal {
    schema: String,
    mode: AuditMode,
    work_item_id: String,
    base_revision: u64,
    base_domain_revision: u64,
    input_hash: String,
    readiness: AuditReadiness,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    next_question: Option<QuestionProposal>,
    entity_ops: Vec<Value>,
    edge_ops: Vec<EdgeOp>,
    blocker_ops: Vec<BlockerOp>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    counterexample_review: Option<CounterexampleReview>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum WireEntityOp {
    Create {
        temp_ref: String,
        kind: EntityKind,
        body: Value,
        source_refs: Vec<SourceRef>,
    },
    Revise {
        entity_id: String,
        base_entity_revision: u64,
        body: Value,
        source_refs: Vec<SourceRef>,
    },
    Reject {
        entity_id: String,
        base_entity_revision: u64,
        reason: String,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProblemBody {
    statement: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomeBody {
    statement: String,
    observable_result: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FactBody {
    statement: String,
    evidence_refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionBody {
    statement: String,
    selected_option: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionBoundaryBody {
    autonomous_scope: Vec<String>,
    requires_user_approval: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementBody {
    statement: String,
    priority: RequirementPriority,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatementBody {
    statement: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssumptionBody {
    statement: String,
    validation_status: ValidationStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RiskBody {
    statement: String,
    impact: RiskImpact,
    mitigation: String,
}

fn decode_body<T: DeserializeOwned>(body: Value) -> Result<T, ServiceError> {
    serde_json::from_value(body)
        .map_err(|error| ServiceError::proposal_schema(format!("entity body schema: {error}")))
}

fn decode_entity_body(kind: EntityKind, body: Value) -> Result<EntityBody, ServiceError> {
    Ok(match kind {
        EntityKind::Problem => {
            let body: ProblemBody = decode_body(body)?;
            EntityBody::Problem {
                statement: body.statement,
            }
        }
        EntityKind::Outcome => {
            let body: OutcomeBody = decode_body(body)?;
            EntityBody::Outcome {
                statement: body.statement,
                observable_result: body.observable_result,
            }
        }
        EntityKind::Fact => {
            let body: FactBody = decode_body(body)?;
            EntityBody::Fact {
                statement: body.statement,
                evidence_refs: body.evidence_refs,
            }
        }
        EntityKind::Decision => {
            let body: DecisionBody = decode_body(body)?;
            EntityBody::Decision {
                statement: body.statement,
                selected_option: body.selected_option,
            }
        }
        EntityKind::DecisionBoundary => {
            let body: DecisionBoundaryBody = decode_body(body)?;
            EntityBody::DecisionBoundary {
                autonomous_scope: body.autonomous_scope,
                requires_user_approval: body.requires_user_approval,
            }
        }
        EntityKind::Requirement => {
            let body: RequirementBody = decode_body(body)?;
            EntityBody::Requirement {
                statement: body.statement,
                priority: body.priority,
            }
        }
        EntityKind::AcceptanceCriterion => EntityBody::AcceptanceCriterion {
            statement: decode_body::<StatementBody>(body)?.statement,
        },
        EntityKind::Constraint => EntityBody::Constraint {
            statement: decode_body::<StatementBody>(body)?.statement,
        },
        EntityKind::NonGoal => EntityBody::NonGoal {
            statement: decode_body::<StatementBody>(body)?.statement,
        },
        EntityKind::Assumption => {
            let body: AssumptionBody = decode_body(body)?;
            EntityBody::Assumption {
                statement: body.statement,
                validation_status: body.validation_status,
            }
        }
        EntityKind::Risk => {
            let body: RiskBody = decode_body(body)?;
            EntityBody::Risk {
                statement: body.statement,
                impact: body.impact,
                mitigation: body.mitigation,
            }
        }
        EntityKind::PlanStep | EntityKind::Verification => {
            return Err(ServiceError::proposal_schema(
                "audit entity create does not allow plan entities",
            ))
        }
    })
}

impl WireEntityOp {
    fn into_core(self, state: &PlanningState) -> Result<EntityOp, ServiceError> {
        Ok(match self {
            Self::Create {
                temp_ref,
                kind,
                body,
                source_refs,
            } => EntityOp::Create {
                temp_ref,
                body: decode_entity_body(kind, body)?,
                source_refs,
            },
            Self::Revise {
                entity_id,
                base_entity_revision,
                body,
                source_refs,
            } => {
                let kind = state
                    .entities
                    .revisions
                    .get(&entity_id)
                    .and_then(|records| records.iter().max_by_key(|record| record.revision))
                    .map(|record| record.kind)
                    .ok_or_else(|| ServiceError::proposal_schema("revised entity was not found"))?;
                EntityOp::Revise {
                    entity_id,
                    base_entity_revision,
                    body: decode_entity_body(kind, body)?,
                    source_refs,
                }
            }
            Self::Reject {
                entity_id,
                base_entity_revision,
                reason,
                source_refs,
            } => EntityOp::Reject {
                entity_id,
                base_entity_revision,
                reason,
                source_refs,
            },
        })
    }
}

fn decode_wire_entity_op(value: Value, state: &PlanningState) -> Result<EntityOp, ServiceError> {
    let operation: WireEntityOp = serde_json::from_value(value).map_err(|error| {
        ServiceError::proposal_schema(format!("entity operation schema: {error}"))
    })?;
    operation.into_core(state)
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

impl AuditProposal {
    pub(crate) fn validate_binding(
        &self,
        state: &PlanningState,
        mode: AuditMode,
    ) -> Result<(), ServiceError> {
        if self.schema != "megara.audit-proposal/v1" || self.mode != mode {
            return Err(ServiceError::proposal_schema(
                "audit proposal schema or mode does not match the request",
            ));
        }
        let Some(work_item) = state.required_model_action.as_ref() else {
            return Err(ServiceError::with_code(
                "MODEL_ACTION_MISMATCH",
                "no required audit work item is active",
            ));
        };
        let expected_kind = match mode {
            AuditMode::Delta => ModelActionKind::DeltaAudit,
            AuditMode::Full => ModelActionKind::FullAudit,
        };
        if work_item.kind != expected_kind
            || work_item.work_item_id != self.work_item_id
            || work_item.base_revision != self.base_revision
            || work_item.base_domain_revision != self.base_domain_revision
            || work_item.input_hash != self.input_hash
        {
            return Err(ServiceError::with_code(
                "PROPOSAL_BASE_MISMATCH",
                "audit proposal does not match the current work item",
            ));
        }
        Ok(())
    }

    pub(crate) fn into_command(
        self,
        session_id: &str,
        expected_revision: u64,
        mode: AuditMode,
        state: &PlanningState,
    ) -> Result<AuditCommand, ServiceError> {
        if self.schema != "megara.audit-proposal/v1" || self.mode != mode {
            return Err(ServiceError::proposal_schema(
                "audit proposal schema or mode does not match the request",
            ));
        }
        Ok(AuditCommand {
            session_id: session_id.to_string(),
            expected_revision,
            work_item_id: self.work_item_id,
            mode: self.mode,
            base_revision: self.base_revision,
            base_domain_revision: self.base_domain_revision,
            input_hash: self.input_hash,
            readiness: self.readiness,
            next_question: self.next_question,
            entity_ops: self
                .entity_ops
                .into_iter()
                .map(|operation| decode_wire_entity_op(operation, state))
                .collect::<Result<Vec<_>, _>>()?,
            edge_ops: self.edge_ops,
            blocker_ops: self.blocker_ops,
            counterexample_review: self.counterexample_review,
        })
    }
}
