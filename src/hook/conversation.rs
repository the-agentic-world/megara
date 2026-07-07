use super::*;

pub(super) fn record_conversation_event(
    state_dir: &Path,
    timestamp: &str,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
    payload_bytes: usize,
) -> Result<()> {
    let Some(role) = conversation_role(&options.event) else {
        return Ok(());
    };

    append_jsonl(
        &state_dir.join("conversation-events.jsonl"),
        &json!({
            "timestamp": timestamp,
            "runtime": options.runtime,
            "event": options.event,
            "role": role,
            "surface": runtime_input::runtime_context(payload).surface.as_str(),
            "payload": payload_file,
            "payload_bytes": payload_bytes,
        }),
    )?;

    let context = runtime_input::runtime_context(payload);
    let (field, content) = if role == "user" {
        let Some(content) = context.effective_prompt.clone() else {
            return Ok(());
        };
        ("prompt", content)
    } else {
        let Some(content) = runtime_input::assistant_message_from_payload(payload) else {
            return Ok(());
        };
        ("last_assistant_message", content)
    };
    if content.trim().is_empty() {
        return Ok(());
    }

    let mut entry = Map::new();
    entry.insert("timestamp".to_string(), json!(timestamp));
    entry.insert("runtime".to_string(), json!(options.runtime));
    entry.insert("event".to_string(), json!(options.event));
    entry.insert("role".to_string(), json!(role));
    entry.insert("content".to_string(), json!(content.clone()));
    entry.insert("surface".to_string(), json!(context.surface.as_str()));
    entry.insert("payload".to_string(), json!(payload_file));
    if let Some(raw_content) = payload.get(field).and_then(Value::as_str) {
        if raw_content != content && !runtime_input::contains_internal_hook_feedback(raw_content) {
            entry.insert("raw_content".to_string(), json!(raw_content));
        }
    }
    if let Some(source) = context.transcript_source {
        entry.insert("transcript_source".to_string(), json!(source));
    }
    if let Some(thread_source) = context.transcript_thread_source {
        entry.insert("transcript_thread_source".to_string(), json!(thread_source));
    }
    if let Some(originator) = context.transcript_originator {
        entry.insert("transcript_originator".to_string(), json!(originator));
    }

    for key in ["session_id", "turn_id", "transcript_path", "cwd", "model"] {
        if let Some(value) = payload.get(key) {
            entry.insert(key.to_string(), value.clone());
        }
    }

    append_jsonl(&state_dir.join("conversation.jsonl"), &Value::Object(entry))
}

fn conversation_role(event: &str) -> Option<&'static str> {
    match event {
        "UserPromptSubmit" => Some("user"),
        "Stop" => Some("assistant"),
        _ => None,
    }
}
