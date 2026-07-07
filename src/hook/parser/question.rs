use std::path::Path;

use serde_json::{json, Value};

use super::{block_list, parse_block};

pub(crate) fn question_from_text(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
) -> Option<Value> {
    question_from_gate(timestamp, text, payload_file)
        .or_else(|| visible_question_from_text(timestamp, text, payload_file))
}

fn question_from_gate(timestamp: &str, text: &str, payload_file: &Path) -> Option<Value> {
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

fn visible_question_from_text(timestamp: &str, text: &str, payload_file: &Path) -> Option<Value> {
    let lines = text.lines().collect::<Vec<_>>();
    let question_index = lines
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, line)| is_visible_question(line).then_some(index))?;
    let question = clean_visible_question(lines[question_index])?;
    let options = visible_options_after(&lines[question_index + 1..]);
    if !looks_like_deep_interview_turn(text, &options) {
        return None;
    }

    Some(json!({
        "id": format!("di-visible-{}", sanitize_id(timestamp)),
        "round": Value::Null,
        "component": "",
        "dimension": "",
        "question": question,
        "options": options,
        "free_text": true,
        "status": "pending",
        "asked_at": timestamp,
        "payload": payload_file,
    }))
}

fn looks_like_deep_interview_turn(text: &str, options: &[String]) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("ambiguity")
        || text.contains("모호성")
        || text.contains("모호도")
        || options
            .last()
            .is_some_and(|option| is_free_text_catch_all(option))
}

fn is_free_text_catch_all(option: &str) -> bool {
    let lower = option.to_ascii_lowercase();
    lower.contains("direct input")
        || lower.contains("not listed")
        || option.contains("직접 입력")
        || option.contains("목록에 없음")
}

fn is_visible_question(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.starts_with('-') || line.starts_with('<') || is_workflow_marker(line)
    {
        return false;
    }
    let line = line.trim_end_matches(['"', '\'', '`', ')', ']', '}', '”', '’']);
    line.ends_with('?') || line.ends_with('？')
}

fn is_workflow_marker(line: &str) -> bool {
    matches!(
        line,
        "Megara Question Gate:"
            | "Megara Workflow State:"
            | "Megara Plan Gate:"
            | "Megara Approval Gate:"
            | "Megara Review Pass:"
            | "Megara Blocker Gate:"
    )
}

fn clean_visible_question(line: &str) -> Option<String> {
    let line = line
        .trim()
        .trim_start_matches(['"', '\'', '`', '“', '‘'])
        .trim_end_matches(['"', '\'', '`', '”', '’'])
        .trim();
    (!line.is_empty()).then(|| line.to_string())
}

fn visible_options_after(lines: &[&str]) -> Vec<String> {
    let mut options = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if options.is_empty() {
                continue;
            }
            break;
        }
        let Some(option) = visible_option_text(trimmed) else {
            if options.is_empty() {
                continue;
            }
            break;
        };
        let option = option.trim();
        if !option.is_empty() {
            options.push(option.to_string());
        }
    }
    options
}

fn visible_option_text(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| numbered_option_text(line))
}

fn numbered_option_text(line: &str) -> Option<&str> {
    let split_at = line
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .last()
        .map(|(index, ch)| index + ch.len_utf8())?;
    let rest = line.get(split_at..)?;
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    rest.strip_prefix(' ')
}

fn sanitize_id(value: &str) -> String {
    let id = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(24)
        .collect::<String>()
        .to_ascii_lowercase();
    if id.is_empty() {
        "question".to_string()
    } else {
        id
    }
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
