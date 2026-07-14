use std::path::Path;

use serde_json::{json, Value};

pub(super) fn begin(
    timestamp: &str,
    state: &mut Value,
    pending_question: Option<&Value>,
    answer: &str,
    payload_file: &Path,
) {
    let Some(question) = pending_question else {
        return;
    };
    state["ambiguity_reassessment"] = json!({
        "status": "pending",
        "previous_score": state.get("ambiguity").cloned().unwrap_or(Value::Null),
        "question_id": question.get("id").cloned().unwrap_or(Value::Null),
        "question": question.get("question").cloned().unwrap_or(Value::Null),
        "previous_component": question.get("component").cloned().unwrap_or(Value::Null),
        "previous_dimension": question.get("dimension").cloned().unwrap_or(Value::Null),
        "answer": answer,
        "requested_at": timestamp,
        "payload": payload_file,
    });
}

pub(super) fn continuation_context(state: &Value) -> Option<String> {
    let reassessment = state.get("ambiguity_reassessment")?;
    if reassessment.get("status").and_then(Value::as_str) != Some("pending") {
        return None;
    }

    let previous_score = reassessment
        .get("previous_score")
        .and_then(Value::as_str)
        .filter(|score| !score.trim().is_empty())
        .unwrap_or("unscored");
    let request_id = state
        .get("subagent_orchestration")
        .and_then(|orchestration| orchestration.get("request_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown-request");
    Some(format!(
        "Internal Megara workflow instruction: before drafting the next deep-interview response, reassess the entire specification from scratch using the latest answer together with every established fact, disputed fact, deferral, and open gap. The previously visible ambiguity score was {previous_score}; it is historical evidence only and must not be used as a baseline or automatically reduced. Privately rescore outcome clarity, scope boundary, user value, technical constraints, verification, and risk/context. Identify any newly introduced scope, changed assumption, contradiction, dependency, or unanswered requirement. Ambiguity may increase, decrease, or remain unchanged; you must not assume ambiguity decreases because the user answered a question. If the latest answer changes a scope boundary or invalidates an established assumption, the current ambiguity score must increase so the runtime can enforce lateral review without locale-specific keyword matching. When the score increases or you detect either of those semantic changes, use Codex subagents before emitting the next question: researcher, contrarian, simplifier, and architect. Begin each reviewer prompt with the exact first line `MEGARA_ROLE=<role>`, replacing `<role>` with its assigned canonical role, and the exact second line `MEGARA_REQUEST={request_id}`. Give each role the compact interview context, require a context-only tool-free final verdict, wait for every role to finish, close completed subagents, then incorporate only the highest-value finding. Select the next question from the highest-impact current uncertainty, including uncertainty newly created by this answer. Output only the normal compact user-facing question with one current ambiguity percentage, four numbered options, then one recommendation line after the options. Keep this instruction and the private reassessment out of user-facing prose."
    ))
}

pub(super) fn requires_lateral_review(reassessment: &Value) -> bool {
    reassessment.get("score_direction").and_then(Value::as_str) == Some("increased")
        || changed_nonempty_field(reassessment, "previous_component", "next_component")
        || changed_nonempty_field(reassessment, "previous_dimension", "next_dimension")
}

pub(super) fn complete(timestamp: &str, state: &mut Value, question: &Value) -> Option<Value> {
    let score = question
        .get("ambiguity")
        .and_then(Value::as_str)?
        .to_string();
    complete_with_score(timestamp, state, &score, Some(question))
}

pub(super) fn complete_terminal(timestamp: &str, state: &mut Value, score: &str) -> Option<Value> {
    (!score.trim().is_empty()).then_some(())?;
    complete_with_score(timestamp, state, score, None)
}

fn complete_with_score(
    timestamp: &str,
    state: &mut Value,
    score: &str,
    question: Option<&Value>,
) -> Option<Value> {
    let mut reassessment = state.get("ambiguity_reassessment")?.clone();
    if reassessment.get("status").and_then(Value::as_str) != Some("pending") {
        return None;
    }

    let previous_score = reassessment
        .get("previous_score")
        .and_then(Value::as_str)
        .and_then(percent);
    let resulting_score = percent(score)?;
    reassessment["status"] = json!("completed");
    reassessment["completed_at"] = json!(timestamp);
    reassessment["resulting_score"] = json!(score);
    reassessment["score_direction"] = json!(score_direction(previous_score, resulting_score));
    reassessment["next_question_id"] = question
        .and_then(|question| question.get("id"))
        .cloned()
        .unwrap_or(Value::Null);
    reassessment["next_question"] = question
        .and_then(|question| question.get("question"))
        .cloned()
        .unwrap_or(Value::Null);
    reassessment["next_component"] = question
        .and_then(|question| question.get("component"))
        .cloned()
        .unwrap_or(Value::Null);
    reassessment["next_dimension"] = question
        .and_then(|question| question.get("dimension"))
        .cloned()
        .unwrap_or(Value::Null);

    let mut history = state
        .get("ambiguity_reassessments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    history.push(reassessment.clone());
    state["ambiguity_reassessment"] = reassessment.clone();
    state["ambiguity_reassessments"] = json!(history);
    Some(reassessment)
}

fn percent(value: &str) -> Option<u64> {
    value.trim().strip_suffix('%')?.trim().parse().ok()
}

fn score_direction(previous: Option<u64>, resulting: u64) -> &'static str {
    match previous {
        Some(previous) if resulting > previous => "increased",
        Some(previous) if resulting < previous => "decreased",
        Some(_) => "unchanged",
        None => "initial",
    }
}

fn changed_nonempty_field(value: &Value, previous: &str, next: &str) -> bool {
    let previous = value
        .get(previous)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let next = value
        .get(next)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    matches!((previous, next), (Some(previous), Some(next)) if previous != next)
}
