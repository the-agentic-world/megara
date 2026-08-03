use serde_json::Value;
use uuid::Uuid;

use super::super::domain::{Blocker, EventEffect};
use super::super::engine::InMemoryPlanningCore;
use super::StoreError;

pub(super) fn normalized_effects(effects: &[EventEffect]) -> Result<Vec<Value>, StoreError> {
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

pub(super) fn reconcile_generated_ids(
    core: &mut InMemoryPlanningCore,
    stored: &super::super::domain::AggregateEvent,
    generated: &super::super::domain::AggregateEvent,
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
                replace_work_item_reference(state, generated_id, stored_id);
            }
            (
                EventEffect::EntityCreated {
                    entity_id,
                    revision,
                    internal_uuid: stored_uuid,
                },
                EventEffect::EntityCreated {
                    internal_uuid: generated_uuid,
                    ..
                },
            ) => {
                {
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
                    if record.internal_uuid != *generated_uuid {
                        return Err(StoreError::DbCorrupt(
                            "generated entity UUID target mismatch".to_string(),
                        ));
                    }
                    record.internal_uuid = stored_uuid.clone();
                }
                replace_work_item_reference(state, generated_uuid, stored_uuid);
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
                replace_work_item_reference(state, generated_id, stored_id);
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

fn replace_work_item_reference(
    state: &mut super::super::domain::PlanningState,
    from: &str,
    to: &str,
) {
    if let Some(work_item) = state.required_model_action.as_mut() {
        replace_json_string(&mut work_item.context, from, to);
    }
}

fn replace_json_string(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::Object(object) => {
            let entries = std::mem::take(object);
            for (key, mut child) in entries {
                replace_json_string(&mut child, from, to);
                object.insert(if key == from { to.to_string() } else { key }, child);
            }
        }
        Value::Array(values) => {
            for child in values {
                replace_json_string(child, from, to);
            }
        }
        Value::String(text) if text == from => *text = to.to_string(),
        _ => {}
    }
}
