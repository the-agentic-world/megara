use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeSurface {
    Cli,
    App,
    Unknown,
}

impl RuntimeSurface {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RuntimeSurface::Cli => "cli",
            RuntimeSurface::App => "app",
            RuntimeSurface::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeContext {
    pub(crate) effective_prompt: Option<String>,
    pub(crate) surface: RuntimeSurface,
    pub(crate) transcript_source: Option<String>,
    pub(crate) transcript_thread_source: Option<String>,
    pub(crate) transcript_originator: Option<String>,
}

#[derive(Clone, Debug)]
struct TranscriptMeta {
    source: Option<String>,
    thread_source: Option<String>,
    originator: Option<String>,
}

pub(crate) fn runtime_context(payload: &Value) -> RuntimeContext {
    let effective_prompt = effective_prompt_from_payload(payload);
    let transcript_meta = payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .and_then(|path| transcript_meta(Path::new(path)));
    let surface = classify_surface(transcript_meta.as_ref(), payload);

    RuntimeContext {
        effective_prompt,
        surface,
        transcript_source: transcript_meta
            .as_ref()
            .and_then(|meta| meta.source.clone()),
        transcript_thread_source: transcript_meta
            .as_ref()
            .and_then(|meta| meta.thread_source.clone()),
        transcript_originator: transcript_meta.and_then(|meta| meta.originator),
    }
}

pub(crate) fn effective_prompt_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("prompt")
        .and_then(Value::as_str)
        .map(effective_prompt_text)
        .filter(|prompt| !prompt.trim().is_empty())
        .filter(|prompt| !is_internal_hook_feedback(prompt))
}

pub(crate) fn assistant_message_from_payload(payload: &Value) -> Option<String> {
    if let Some(raw) = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
    {
        return assistant_message_text(raw);
    }
    assistant_message_from_transcript(payload).and_then(|text| assistant_message_text(&text))
}

pub(crate) fn effective_prompt_text(prompt: &str) -> String {
    if let Some(input) = extract_delegated_input(prompt) {
        return clean_effective_prompt(input);
    }
    let unescaped = html_unescape_basic(prompt);
    if unescaped != prompt {
        if let Some(input) = extract_delegated_input(&unescaped) {
            return clean_effective_prompt(input);
        }
    }
    clean_effective_prompt(prompt)
}

fn clean_effective_prompt(prompt: &str) -> String {
    strip_hook_prompt_blocks(&html_unescape_basic(prompt))
        .trim()
        .to_string()
}

fn assistant_message_text(text: &str) -> Option<String> {
    let cleaned = clean_effective_prompt(text);
    if cleaned.trim().is_empty() || is_internal_hook_feedback(&cleaned) {
        return None;
    }
    Some(cleaned)
}

fn extract_delegated_input(prompt: &str) -> Option<&str> {
    extract_tag_body(prompt, "<input>", "</input>")
}

fn extract_tag_body<'a>(text: &'a str, start_tag: &str, end_tag: &str) -> Option<&'a str> {
    let start = text.find(start_tag)? + start_tag.len();
    let end = text[start..]
        .find(end_tag)
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    text.get(start..end)
}

fn html_unescape_basic(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

pub(crate) fn is_internal_hook_feedback(text: &str) -> bool {
    let unescaped = html_unescape_basic(text);
    let stripped = strip_hook_prompt_blocks(&unescaped);
    if stripped.trim().is_empty() && contains_hook_prompt_tag(&unescaped) {
        return true;
    }

    let lowered = stripped.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return false;
    }

    INTERNAL_HOOK_FEEDBACK_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
}

pub(crate) fn contains_internal_hook_feedback(text: &str) -> bool {
    let unescaped = html_unescape_basic(text);
    if contains_hook_prompt_tag(&unescaped) {
        return true;
    }
    let lowered = unescaped.to_ascii_lowercase();
    INTERNAL_HOOK_FEEDBACK_PREFIXES
        .iter()
        .any(|prefix| lowered.contains(prefix))
}

const INTERNAL_HOOK_FEEDBACK_PREFIXES: &[&str] = &[
    "megara git guard:",
    "megara mutation guard:",
    "megara deep-interview reached",
    "megara needs an internal git cleanup pass before the final response",
    "megara internal guard feedback must stay hidden",
    "megara runtime artifact or state paths are internal",
    "internal megara workflow instruction",
    "keep this runtime instruction internal",
];

fn contains_hook_prompt_tag(text: &str) -> bool {
    text.to_ascii_lowercase().contains("<hook_prompt")
}

fn strip_hook_prompt_blocks(text: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    loop {
        let lowered = text[cursor..].to_ascii_lowercase();
        let Some(relative_start) = lowered.find("<hook_prompt") else {
            output.push_str(&text[cursor..]);
            break;
        };
        let start = cursor + relative_start;
        output.push_str(&text[cursor..start]);
        let Some(relative_tag_end) = text[start..].find('>') else {
            break;
        };
        let body_start = start + relative_tag_end + 1;
        let lowered_after_tag = text[body_start..].to_ascii_lowercase();
        let Some(relative_end) = lowered_after_tag.find("</hook_prompt>") else {
            break;
        };
        cursor = body_start + relative_end + "</hook_prompt>".len();
    }
    output
}

fn classify_surface(meta: Option<&TranscriptMeta>, payload: &Value) -> RuntimeSurface {
    if let Some(source) = meta.and_then(|meta| meta.source.as_deref()) {
        if source.eq_ignore_ascii_case("exec") {
            return RuntimeSurface::Cli;
        }
        if source.eq_ignore_ascii_case("vscode") {
            return RuntimeSurface::App;
        }
    }
    if payload
        .get("prompt")
        .and_then(Value::as_str)
        .is_some_and(|prompt| prompt.contains("<codex_delegation"))
    {
        return RuntimeSurface::App;
    }
    RuntimeSurface::Unknown
}

fn transcript_meta(path: &Path) -> Option<TranscriptMeta> {
    let file = File::open(path).ok()?;
    for line in BufReader::new(file).lines().map_while(Result::ok).take(32) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("session_meta") {
            continue;
        }
        let payload = value.get("payload")?;
        return Some(TranscriptMeta {
            source: string_field(payload, "source"),
            thread_source: string_field(payload, "thread_source"),
            originator: string_field(payload, "originator"),
        });
    }
    None
}

fn assistant_message_from_transcript(payload: &Value) -> Option<String> {
    let transcript_path = payload.get("transcript_path").and_then(Value::as_str)?;
    let requested_turn_id = string_field(payload, "turn_id");
    let file = File::open(transcript_path).ok()?;
    let mut active_turn_id = None::<String>;
    let mut latest = None::<String>;
    let mut latest_for_turn = None::<String>;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(turn_id) = record_turn_id(&record) {
            active_turn_id = Some(turn_id);
        }
        let Some(message) = assistant_message_from_record(&record) else {
            continue;
        };
        latest = Some(message.clone());
        if requested_turn_id.as_deref().is_some_and(|turn_id| {
            record_belongs_to_turn(&record, active_turn_id.as_deref(), turn_id)
        }) {
            latest_for_turn = Some(message);
        }
    }

    if requested_turn_id.is_some() {
        latest_for_turn
    } else {
        latest
    }
}

fn record_belongs_to_turn(record: &Value, active_turn_id: Option<&str>, turn_id: &str) -> bool {
    record_turn_id(record)
        .as_deref()
        .is_some_and(|candidate| candidate == turn_id)
        || active_turn_id.is_some_and(|candidate| candidate == turn_id)
}

fn record_turn_id(record: &Value) -> Option<String> {
    string_field(record, "turn_id")
        .or_else(|| {
            record
                .get("payload")
                .and_then(|payload| string_field(payload, "turn_id"))
        })
        .or_else(|| {
            record
                .pointer("/payload/internal_chat_message_metadata_passthrough/turn_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn assistant_message_from_record(record: &Value) -> Option<String> {
    let payload = record.get("payload").unwrap_or(record);
    let payload_type = payload.get("type").and_then(Value::as_str);

    if payload_type.is_some_and(|value| value == "agent_message") {
        return payload
            .get("message")
            .and_then(content_text)
            .filter(|text| !text.trim().is_empty());
    }

    if payload_type.is_some_and(|value| value == "message")
        && payload
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|role| role == "assistant")
    {
        return payload
            .get("content")
            .and_then(content_text)
            .filter(|text| !text.trim().is_empty());
    }

    let item = payload.get("item")?;
    if item
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "message")
        && item
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|role| role == "assistant")
    {
        return item
            .get("content")
            .and_then(content_text)
            .filter(|text| !text.trim().is_empty());
    }

    None
}

fn content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(content_text)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join(""))
        }
        Value::Object(map) => ["text", "message", "content"]
            .iter()
            .find_map(|key| map.get(*key).and_then(content_text)),
        _ => None,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}
