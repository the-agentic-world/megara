use std::path::Path;

use serde_json::{json, Value};

use super::{block_list, parse_block};

pub(crate) fn question_from_text(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
) -> Option<Value> {
    let block = parse_block(text, "Megara Question Gate:")?;
    let question_id = block.fields.get("id")?.trim();
    let question = block.fields.get("question")?.trim();
    if question_id.is_empty() || question.is_empty() {
        return None;
    }

    Some(json!({
        "id": question_id,
        "round": normalize_round(block.fields.get("round").map(String::as_str)),
        "component": block.fields.get("component").map(String::as_str).unwrap_or("").trim(),
        "dimension": block.fields.get("dimension").map(String::as_str).unwrap_or("").trim(),
        "question": question,
        "options": block_list(&block, "options"),
        "free_text": parse_bool(block.fields.get("free_text").map(String::as_str).unwrap_or("false")),
        "status": "pending",
        "asked_at": timestamp,
        "payload": payload_file,
    }))
}

fn normalize_round(value: Option<&str>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    value
        .trim()
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| json!(value))
}

pub(super) fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}
