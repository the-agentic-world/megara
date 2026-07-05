use serde_json::{json, Value};

use crate::hook::runtime_input;

pub(crate) fn new_state(skill: &str, timestamp: &str, session_id: &str, payload: &Value) -> Value {
    let context = runtime_input::runtime_context(payload);
    json!({
        "version": 1,
        "skill": skill,
        "session_id": session_id,
        "runtime_session_id": payload.get("session_id").cloned().unwrap_or(Value::Null),
        "thread_id": payload.get("thread_id").cloned().unwrap_or(Value::Null),
        "turn_id": payload.get("turn_id").cloned().unwrap_or(Value::Null),
        "transcript_path": payload.get("transcript_path").cloned().unwrap_or(Value::Null),
        "surface": context.surface.as_str(),
        "transcript_source": context.transcript_source,
        "transcript_thread_source": context.transcript_thread_source,
        "transcript_originator": context.transcript_originator,
        "cwd": payload.get("cwd").cloned().unwrap_or(Value::Null),
        "active": true,
        "phase": "initialized",
        "pending_question": Value::Null,
        "questions": [],
        "updated_at": timestamp,
    })
}
