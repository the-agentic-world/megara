use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::super::domain::{AggregateEvent, Blocker, EventEffect};
use super::super::engine::{
    CoreError, EvidenceRefreshResult, InMemoryPlanningCore, MutationResult,
};
use super::hash::normalized_state_hash;
use super::*;

pub const EVENT_ENVELOPE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventActor {
    System,
    User,
    Model,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAdapter {
    Core,
    Cli,
    CodexMcp,
    Pi,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventMetadata {
    pub occurred_at: String,
    pub actor: EventActor,
    pub adapter: EventAdapter,
    pub request_id: Option<String>,
    pub command_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EventType {
    #[serde(rename = "planning.start")]
    Start,
    #[serde(rename = "planning.answer")]
    Answer,
    #[serde(rename = "planning.evidence.refresh")]
    EvidenceRefresh,
    #[serde(rename = "planning.audit.apply")]
    AuditApply,
    #[serde(rename = "planning.spec.generate")]
    SpecGenerate,
    #[serde(rename = "planning.spec.approve")]
    SpecApprove,
    #[serde(rename = "planning.spec.revise")]
    SpecRevise,
    #[serde(rename = "planning.plan.generate")]
    PlanGenerate,
    #[serde(rename = "planning.plan.approve")]
    PlanApprove,
    #[serde(rename = "planning.plan.revise")]
    PlanRevise,
}

impl EventType {
    pub fn from_operation(operation: &str) -> Option<Self> {
        Some(match operation {
            "planning.start" => Self::Start,
            "planning.answer" => Self::Answer,
            "planning.evidence.refresh" => Self::EvidenceRefresh,
            "planning.audit.apply" => Self::AuditApply,
            "planning.spec.generate" => Self::SpecGenerate,
            "planning.spec.approve" => Self::SpecApprove,
            "planning.spec.revise" => Self::SpecRevise,
            "planning.plan.generate" => Self::PlanGenerate,
            "planning.plan.approve" => Self::PlanApprove,
            "planning.plan.revise" => Self::PlanRevise,
            _ => return None,
        })
    }

    pub fn operation(self) -> &'static str {
        match self {
            Self::Start => "planning.start",
            Self::Answer => "planning.answer",
            Self::EvidenceRefresh => "planning.evidence.refresh",
            Self::AuditApply => "planning.audit.apply",
            Self::SpecGenerate => "planning.spec.generate",
            Self::SpecApprove => "planning.spec.approve",
            Self::SpecRevise => "planning.spec.revise",
            Self::PlanGenerate => "planning.plan.generate",
            Self::PlanApprove => "planning.plan.approve",
            Self::PlanRevise => "planning.plan.revise",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event_id: String,
    pub session_id: SessionId,
    pub seq: u64,
    pub revision_after: u64,
    pub domain_revision_after: u64,
    pub plan_revision_after: u64,
    pub event_type: EventType,
    pub metadata: EventMetadata,
    pub semantic_payload: AggregateEvent,
    pub semantic_payload_hash: String,
    pub state_hash_after: String,
}

pub fn replay_events(events: &[EventEnvelope]) -> Result<InMemoryPlanningCore, StoreError> {
    let mut core = InMemoryPlanningCore::default();
    for envelope in events {
        if envelope.schema_version != EVENT_ENVELOPE_SCHEMA_VERSION
            || Uuid::parse_str(&envelope.event_id)
                .ok()
                .is_none_or(|id| id.get_version_num() != 7)
        {
            return Err(StoreError::DbCorrupt("invalid event envelope".to_string()));
        }
        if envelope.metadata.occurred_at.trim().is_empty()
            || envelope.metadata.command_id.trim().is_empty()
        {
            return Err(StoreError::DbCorrupt(
                "event command id missing".to_string(),
            ));
        }
        let event = &envelope.semantic_payload;
        if event.schema != EVENT_SCHEMA_VERSION
            || envelope.event_type.operation() != event.operation
            || envelope.session_id != event.session_id
            || envelope.seq != event.seq
            || envelope.revision_after != event.revision_after
            || envelope.domain_revision_after != event.domain_revision_after
            || envelope.plan_revision_after != event.plan_revision_after
        {
            return Err(StoreError::DbCorrupt(
                "unsupported event schema".to_string(),
            ));
        }
        if envelope.semantic_payload_hash != semantic_payload_hash(event)? {
            return Err(StoreError::DbCorrupt(
                "event payload hash mismatch".to_string(),
            ));
        }
        if event.seq != core.events.len() as u64 + 1 || event.seq != event.revision_after {
            return Err(StoreError::DbCorrupt(format!(
                "event sequence mismatch at seq {}",
                event.seq
            )));
        }
        let before = core.state(&event.session_id).cloned();
        let result = reduce_event(&mut core, event)?;
        compare_event(event, &result.event)?;
        reconcile_generated_ids(&mut core, event, &result.event)?;
        if let Some(replayed_event) = core.events.last_mut() {
            *replayed_event = event.clone();
        }
        let state = core
            .state(&event.session_id)
            .ok_or_else(|| StoreError::DbCorrupt("reducer produced no state".to_string()))?;
        if normalized_state_hash(state) != envelope.state_hash_after {
            return Err(StoreError::DbCorrupt(format!(
                "event seq {} hash mismatch",
                event.seq
            )));
        }
        if let Some(previous) = before {
            if event.revision_after != previous.revision + 1 {
                return Err(StoreError::DbCorrupt(format!(
                    "revision gap at event seq {}",
                    event.seq
                )));
            }
        } else if event.seq != 1 || event.revision_after != 1 {
            return Err(StoreError::DbCorrupt(
                "first event revision is invalid".to_string(),
            ));
        }
    }
    Ok(core)
}

fn reduce_event(
    core: &mut InMemoryPlanningCore,
    event: &AggregateEvent,
) -> Result<MutationResult, StoreError> {
    let command = event
        .primary
        .get("command")
        .cloned()
        .ok_or_else(|| StoreError::DbCorrupt("event command payload missing".to_string()))?;
    match event.operation.as_str() {
        "planning.start" => Ok(core.start(decode(command)?).map_err(core_corrupt)?),
        "planning.answer" => Ok(core.answer(decode(command)?).map_err(core_corrupt)?),
        "planning.evidence.refresh" => match core
            .refresh_evidence(decode(command)?)
            .map_err(core_corrupt)?
        {
            EvidenceRefreshResult::Changed(result) => Ok(result),
            EvidenceRefreshResult::Unchanged { .. } => Err(StoreError::DbCorrupt(
                "stored evidence event reduced to no-op".to_string(),
            )),
        },
        "planning.audit.apply" => Ok(core.apply_audit(decode(command)?).map_err(core_corrupt)?),
        "planning.spec.generate" => {
            Ok(core.generate_spec(decode(command)?).map_err(core_corrupt)?)
        }
        "planning.spec.approve" => Ok(core.approve_spec(decode(command)?).map_err(core_corrupt)?),
        "planning.spec.revise" => Ok(core.revise_spec(decode(command)?).map_err(core_corrupt)?),
        "planning.plan.generate" => {
            Ok(core.generate_plan(decode(command)?).map_err(core_corrupt)?)
        }
        "planning.plan.approve" => Ok(core.approve_plan(decode(command)?).map_err(core_corrupt)?),
        "planning.plan.revise" => Ok(core.revise_plan(decode(command)?).map_err(core_corrupt)?),
        operation => Err(StoreError::DbCorrupt(format!(
            "unknown event operation: {operation}"
        ))),
    }
}

fn compare_event(stored: &AggregateEvent, generated: &AggregateEvent) -> Result<(), StoreError> {
    if stored.schema != generated.schema
        || stored.operation != generated.operation
        || stored.session_id != generated.session_id
        || stored.seq != generated.seq
        || stored.revision_after != generated.revision_after
        || stored.domain_revision_after != generated.domain_revision_after
        || stored.plan_revision_after != generated.plan_revision_after
        || stored.primary != generated.primary
        || normalized_effects(&stored.effects)? != normalized_effects(&generated.effects)?
    {
        return Err(StoreError::DbCorrupt(format!(
            "event semantic mismatch at seq {}",
            stored.seq
        )));
    }
    Ok(())
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, StoreError> {
    serde_json::from_value(value)
        .map_err(|error| StoreError::DbCorrupt(format!("event command payload: {error}")))
}

fn core_corrupt(error: CoreError) -> StoreError {
    StoreError::DbCorrupt(format!("event command application: {error}"))
}

pub(crate) fn semantic_payload_hash(event: &AggregateEvent) -> Result<String, StoreError> {
    let value = serde_json::to_value(event)?;
    let bytes = super::hash::canonical_json_bytes(&value);
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn normalized_effects(effects: &[EventEffect]) -> Result<Vec<Value>, StoreError> {
    effects
        .iter()
        .map(|effect| match effect {
            EventEffect::QuestionSet { question_id } => {
                validate_generated_id(question_id, "qst_")?;
                Ok(serde_json::json!({"kind":"question_set"}))
            }
            EventEffect::AnswerSubmitted { answer_id } => {
                validate_generated_id(answer_id, "ans_")?;
                Ok(serde_json::json!({"kind":"answer_submitted"}))
            }
            EventEffect::EntityCreated {
                entity_id,
                revision,
                internal_uuid,
            } => {
                validate_uuid_v7(internal_uuid)?;
                Ok(serde_json::json!({"kind":"entity_created","entity_id":entity_id,"revision":revision}))
            }
            EventEffect::BlockerCreated { blocker_id } => {
                validate_generated_id(blocker_id, "blk_")?;
                Ok(serde_json::json!({"kind":"blocker_created"}))
            }
            other => Ok(serde_json::to_value(other)
                .expect("event effect serialization is infallible")),
        })
        .collect()
}

fn validate_generated_id(id: &str, prefix: &str) -> Result<(), StoreError> {
    let suffix = id.strip_prefix(prefix).unwrap_or_default();
    if suffix.is_empty()
        || Uuid::parse_str(suffix)
            .ok()
            .is_none_or(|uuid| uuid.get_version_num() != 7)
    {
        return Err(StoreError::DbCorrupt(format!(
            "generated id does not use UUIDv7 {prefix} format"
        )));
    }
    Ok(())
}

fn validate_uuid_v7(id: &str) -> Result<(), StoreError> {
    if Uuid::parse_str(id)
        .ok()
        .is_none_or(|uuid| uuid.get_version_num() != 7)
    {
        return Err(StoreError::DbCorrupt(
            "generated entity uuid is not UUIDv7".to_string(),
        ));
    }
    Ok(())
}

fn reconcile_generated_ids(
    core: &mut InMemoryPlanningCore,
    stored: &AggregateEvent,
    generated: &AggregateEvent,
) -> Result<(), StoreError> {
    let state = core.sessions.get_mut(&stored.session_id).ok_or_else(|| {
        StoreError::DbCorrupt("reducer state missing for reconciliation".to_string())
    })?;
    for (stored_effect, generated_effect) in stored.effects.iter().zip(generated.effects.iter()) {
        match (stored_effect, generated_effect) {
            (
                EventEffect::QuestionSet {
                    question_id: stored_id,
                },
                EventEffect::QuestionSet {
                    question_id: generated_id,
                },
            ) => {
                let question = state.pending_question.as_mut().ok_or_else(|| {
                    StoreError::DbCorrupt("question effect target missing".to_string())
                })?;
                if question.question_id != *generated_id {
                    return Err(StoreError::DbCorrupt(
                        "generated question effect target mismatch".to_string(),
                    ));
                }
                question.question_id = stored_id.clone();
            }
            (
                EventEffect::AnswerSubmitted {
                    answer_id: stored_id,
                },
                EventEffect::AnswerSubmitted {
                    answer_id: generated_id,
                },
            ) => {
                let matches = state
                    .transcript
                    .answers
                    .iter()
                    .filter(|answer| answer.answer_id == *generated_id)
                    .count();
                if matches != 1 {
                    return Err(StoreError::DbCorrupt(
                        "generated answer effect target mismatch".to_string(),
                    ));
                }
                let answer = state
                    .transcript
                    .answers
                    .iter_mut()
                    .find(|answer| answer.answer_id == *generated_id)
                    .expect("count checked above");
                answer.answer_id = stored_id.clone();
            }
            (
                EventEffect::EntityCreated {
                    entity_id,
                    revision,
                    internal_uuid,
                },
                EventEffect::EntityCreated { .. },
            ) => {
                let record = state
                    .entities
                    .revisions
                    .get_mut(entity_id)
                    .and_then(|records| {
                        records
                            .iter_mut()
                            .find(|record| record.revision == *revision)
                    })
                    .ok_or_else(|| {
                        StoreError::DbCorrupt("entity effect target missing".to_string())
                    })?;
                record.internal_uuid = internal_uuid.clone();
            }
            (
                EventEffect::BlockerCreated {
                    blocker_id: stored_id,
                },
                EventEffect::BlockerCreated {
                    blocker_id: generated_id,
                },
            ) if stored_id != generated_id => {
                let blocker = state.blockers.remove(generated_id).ok_or_else(|| {
                    StoreError::DbCorrupt("blocker effect target missing".to_string())
                })?;
                state.blockers.insert(
                    stored_id.clone(),
                    Blocker {
                        blocker_id: stored_id.clone(),
                        ..blocker
                    },
                );
            }
            _ => {}
        }
    }
    let state = core
        .state(&stored.session_id)
        .ok_or_else(|| StoreError::DbCorrupt("reconciled state missing".to_string()))?;
    for effect in &stored.effects {
        match effect {
            EventEffect::QuestionSet { question_id } => {
                if state
                    .pending_question
                    .as_ref()
                    .is_none_or(|question| question.question_id != *question_id)
                {
                    return Err(StoreError::DbCorrupt(
                        "stored question id not present in state".to_string(),
                    ));
                }
            }
            EventEffect::AnswerSubmitted { answer_id } => {
                if state
                    .transcript
                    .answers
                    .iter()
                    .filter(|answer| answer.answer_id == *answer_id)
                    .count()
                    != 1
                {
                    return Err(StoreError::DbCorrupt(
                        "stored answer id not present exactly once".to_string(),
                    ));
                }
            }
            EventEffect::EntityCreated {
                entity_id,
                revision,
                internal_uuid,
            } => {
                if state
                    .entities
                    .revisions
                    .get(entity_id)
                    .into_iter()
                    .flat_map(|records| records.iter())
                    .filter(|record| {
                        record.revision == *revision && record.internal_uuid == *internal_uuid
                    })
                    .count()
                    != 1
                {
                    return Err(StoreError::DbCorrupt(
                        "stored entity id is not bound to state".to_string(),
                    ));
                }
            }
            EventEffect::BlockerCreated { blocker_id }
                if state
                    .blockers
                    .get(blocker_id)
                    .is_none_or(|blocker| blocker.blocker_id != *blocker_id) =>
            {
                return Err(StoreError::DbCorrupt(
                    "stored blocker id is not bound to state".to_string(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}
