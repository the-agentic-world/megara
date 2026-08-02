use std::collections::BTreeMap;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::super::domain::PlanningState;

pub fn normalized_state_hash(state: &PlanningState) -> String {
    let aliases = generated_aliases(state);
    let value = serde_json::to_value(state).expect("planning state serialization is infallible");
    let canonical = canonical_value(&value, None, Some(&aliases));
    hash_bytes(&serde_json::to_vec(&canonical).expect("canonical JSON serialization is infallible"))
}

pub(crate) fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&canonical_value(value, None, None))
        .expect("canonical JSON serialization is infallible")
}

fn generated_aliases(state: &PlanningState) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    aliases.insert(state.session_id.clone(), "SESSION@1:0".to_string());
    if let Some(work_item) = &state.required_model_action {
        aliases.insert(
            work_item.work_item_id.clone(),
            format!(
                "WORK_ITEM@{}:{}",
                work_item.created_event_seq, work_item.created_ordinal
            ),
        );
    }

    if let Some(question) = &state.pending_question {
        aliases.insert(
            question.question_id.clone(),
            format!(
                "QUESTION@{}:{}",
                question.created_event_seq, question.created_ordinal
            ),
        );
    }
    for answer in &state.transcript.answers {
        aliases
            .entry(answer.question_id.clone())
            .or_insert_with(|| format!("QUESTION@{}:0", answer.based_on_revision));
        aliases.insert(
            answer.answer_id.clone(),
            format!(
                "ANSWER@{}:{}",
                answer.created_event_seq, answer.created_ordinal
            ),
        );
    }
    for (map_id, blocker) in &state.blockers {
        let alias = format!(
            "BLOCKER@{}:{}",
            blocker.created_event_seq, blocker.created_ordinal
        );
        aliases.insert(map_id.clone(), alias.clone());
        aliases.insert(blocker.blocker_id.clone(), alias);
    }
    for records in state.entities.revisions.values() {
        for record in records {
            aliases.insert(
                record.internal_uuid.clone(),
                format!(
                    "ENTITY@{}:{}",
                    record.created_event_seq, record.created_ordinal
                ),
            );
        }
    }
    aliases
}

fn canonical_value(
    value: &Value,
    key: Option<&str>,
    aliases: Option<&BTreeMap<String, String>>,
) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = BTreeMap::new();
            for (name, child) in object {
                let normalized_name = aliases
                    .and_then(|map| map.get(name))
                    .cloned()
                    .unwrap_or_else(|| normalize_text(name));
                sorted.insert(normalized_name, canonical_value(child, Some(name), aliases));
            }
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(values) => {
            let mut normalized = values
                .iter()
                .map(|child| canonical_value(child, key, aliases))
                .collect::<Vec<_>>();
            if is_set_array(key) {
                normalized.sort_by(|left, right| {
                    serde_json::to_string(left)
                        .expect("canonical JSON serialization is infallible")
                        .cmp(
                            &serde_json::to_string(right)
                                .expect("canonical JSON serialization is infallible"),
                        )
                });
            }
            Value::Array(normalized)
        }
        Value::String(text) => {
            if let Some(alias) = aliases.and_then(|map| map.get(text)) {
                Value::String(alias.clone())
            } else {
                Value::String(normalize_text(text))
            }
        }
        other => other.clone(),
    }
}

fn is_set_array(key: Option<&str>) -> bool {
    matches!(
        key,
        Some(
            "selected_choice_ids"
                | "source_refs"
                | "entity_refs"
                | "technical_terms"
                | "autonomous_scope"
                | "requires_user_approval"
                | "change_surface"
                | "requirement_refs"
                | "verification_refs"
                | "dependencies"
                | "edges"
        )
    ) || key.is_some_and(|name| name.ends_with("_ids") || name.ends_with("_refs"))
}

fn normalize_text(text: &str) -> String {
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect::<String>();
    let lines = normalized
        .lines()
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>();
    let first = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(first, |index| index + 1);
    lines[first..last].join("\n")
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}
