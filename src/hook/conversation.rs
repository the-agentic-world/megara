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
            "payload": payload_file,
            "payload_bytes": payload_bytes,
        }),
    )?;

    let field = if role == "user" {
        "prompt"
    } else {
        "last_assistant_message"
    };
    let Some(content) = payload.get(field).and_then(Value::as_str) else {
        return Ok(());
    };
    if content.trim().is_empty() {
        return Ok(());
    }

    let mut entry = Map::new();
    entry.insert("timestamp".to_string(), json!(timestamp));
    entry.insert("runtime".to_string(), json!(options.runtime));
    entry.insert("event".to_string(), json!(options.event));
    entry.insert("role".to_string(), json!(role));
    entry.insert("content".to_string(), json!(content));
    entry.insert("payload".to_string(), json!(payload_file));

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
