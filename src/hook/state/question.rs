use std::path::Path;

use serde_json::{json, Value};

use crate::hook::state_paths::value_to_string;

pub(crate) fn upsert_question(timestamp: &str, state: &mut Value, question: Value) {
    supersede_pending_question(timestamp, state);
    let question_id = question.get("id").map(value_to_string).unwrap_or_default();
    let mut questions = state
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|existing| existing.get("id").map(value_to_string) != Some(question_id.clone()))
        .collect::<Vec<_>>();
    questions.push(question.clone());

    state["questions"] = json!(questions);
    state["pending_question"] = question;
    state["active"] = json!(true);
    state["phase"] = json!("question_pending");
    state["updated_at"] = json!(timestamp);
}

pub(crate) fn answer_pending_question(
    timestamp: &str,
    state: &mut Value,
    prompt: &str,
    payload_file: &Path,
) -> Option<String> {
    let pending = state.get("pending_question")?;
    if pending.get("status").and_then(Value::as_str) != Some("pending") {
        return None;
    }
    let pending_id = pending.get("id").map(value_to_string)?;
    let answer = json!({
        "content": prompt,
        "answered_at": timestamp,
        "payload": payload_file,
    });

    if let Some(questions) = state.get_mut("questions").and_then(Value::as_array_mut) {
        if let Some(existing) = questions.iter_mut().find(|existing| {
            existing.get("id").map(value_to_string) == Some(pending_id.clone())
                && existing.get("status").and_then(Value::as_str) == Some("pending")
        }) {
            existing["status"] = json!("answered");
            existing["answer"] = answer;
        }
    }
    state["pending_question"] = Value::Null;
    state["phase"] = json!("interviewing");
    state["updated_at"] = json!(timestamp);
    Some(pending_id)
}

fn supersede_pending_question(timestamp: &str, state: &mut Value) {
    let pending_id = state.get("pending_question").and_then(|pending| {
        (pending.get("status").and_then(Value::as_str) == Some("pending"))
            .then(|| pending.get("id").map(value_to_string))
            .flatten()
    });
    let Some(pending_id) = pending_id else {
        return;
    };
    if let Some(questions) = state.get_mut("questions").and_then(Value::as_array_mut) {
        if let Some(existing) = questions.iter_mut().find(|existing| {
            existing.get("id").map(value_to_string) == Some(pending_id.clone())
                && existing.get("status").and_then(Value::as_str) == Some("pending")
        }) {
            existing["status"] = json!("superseded");
            existing["superseded_at"] = json!(timestamp);
        }
    }
}
