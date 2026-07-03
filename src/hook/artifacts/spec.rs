use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::hook::{
    fsutil::{append_jsonl, sha256_hex, write_text_atomic},
    parser::{text_before_block, TerminalState},
    DEEP_INTERVIEW,
};

use super::{
    path::{unique_spec_path, yaml_string},
    types::PersistedSpec,
};

pub(crate) fn persist_crystallized_spec(
    timestamp: &str,
    workflow_dir: &Path,
    session_id: &str,
    terminal: &TerminalState,
    text: &str,
    payload_file: &Path,
) -> Result<Option<PersistedSpec>> {
    if terminal.status != "crystallized" || text.trim().is_empty() {
        return Ok(None);
    }
    let visible_text = visible_spec_text(text);
    if visible_text.is_empty() {
        return Ok(None);
    }

    let mut content = [
        "---".to_string(),
        "skill: \"deep-interview\"".to_string(),
        format!("session_id: {}", yaml_string(session_id)),
        "status: \"crystallized\"".to_string(),
        format!("ambiguity: {}", yaml_string(&terminal.ambiguity)),
        format!("next: {}", yaml_string(&terminal.next)),
        format!("persisted_at: {}", yaml_string(timestamp)),
        format!("payload: {}", yaml_string(payload_file.display())),
        "---".to_string(),
        String::new(),
        visible_text,
    ]
    .join("\n");
    content.push('\n');

    let spec_path = unique_spec_path(workflow_dir, session_id, timestamp);
    write_text_atomic(&spec_path, &content)?;
    let sha256 = sha256_hex(content.as_bytes());
    append_jsonl(
        &workflow_dir.join("specs").join("index.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "spec_persisted",
            "session_id": session_id,
            "skill": DEEP_INTERVIEW,
            "status": "crystallized",
            "path": spec_path,
            "sha256": sha256,
            "payload": payload_file,
        }),
    )?;

    Ok(Some(PersistedSpec {
        path: spec_path.display().to_string(),
        sha256,
        persisted_at: timestamp.to_string(),
        payload: payload_file.display().to_string(),
    }))
}

fn visible_spec_text(text: &str) -> String {
    strip_html_comments(&text_before_block(text, "Megara Workflow State:"))
        .trim()
        .to_string()
}

fn strip_html_comments(text: &str) -> String {
    let mut output = String::new();
    let mut rest = text;

    while let Some(start) = rest.find("<!--") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + "<!--".len()..];
        let Some(end) = after_start.find("-->") else {
            return output;
        };
        rest = &after_start[end + "-->".len()..];
    }

    output.push_str(rest);
    output
}
