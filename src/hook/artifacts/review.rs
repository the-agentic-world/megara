use std::path::Path;

use anyhow::Result;
use serde_json::{json, Value};

use crate::hook::{
    fsutil::{append_jsonl, sha256_hex, write_text_atomic},
    parser::ReviewPass,
    state_paths::WorkflowPaths,
    RALPLAN,
};

use super::path::{unique_review_path, yaml_string};

pub(crate) fn persist_ralplan_review(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    review: ReviewPass,
    state: &mut Value,
) -> Result<()> {
    let review_path = unique_review_path(
        &paths.workflow_dir,
        &paths.session_id,
        &review.role,
        review.round,
        timestamp,
    );
    let mut content = review_markdown(timestamp, payload_file, paths, &review);
    content.push('\n');

    write_text_atomic(&review_path, &content)?;
    let sha256 = sha256_hex(content.as_bytes());
    let review_entry = json!({
        "timestamp": timestamp,
        "event": "review_persisted",
        "session_id": paths.session_id,
        "skill": RALPLAN,
        "role": review.role,
        "round": review.round,
        "verdict": review.verdict,
        "summary": review.summary,
        "required_fixes": review.required_fixes,
        "path": review_path,
        "sha256": sha256,
        "payload": payload_file,
    });
    append_jsonl(
        &paths.workflow_dir.join("reviews").join("index.jsonl"),
        &review_entry,
    )?;
    append_jsonl(&paths.events_file, &review_entry)?;
    push_review_state(timestamp, payload_file, state, &review_entry);
    Ok(())
}

fn review_markdown(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    review: &ReviewPass,
) -> String {
    [
        "---".to_string(),
        "skill: \"ralplan\"".to_string(),
        format!("session_id: {}", yaml_string(&paths.session_id)),
        format!("role: {}", yaml_string(&review.role)),
        format!("round: {}", review.round),
        format!("verdict: {}", yaml_string(&review.verdict)),
        format!("persisted_at: {}", yaml_string(timestamp)),
        format!("payload: {}", yaml_string(payload_file.display())),
        "---".to_string(),
        String::new(),
        format!("# {} review", review.role),
        String::new(),
        format!("Verdict: `{}`", review.verdict),
        String::new(),
        "## Summary".to_string(),
        String::new(),
        review.summary.clone(),
        String::new(),
        "## Required Fixes".to_string(),
        String::new(),
        review
            .required_fixes
            .iter()
            .map(|fix| format!("- {fix}"))
            .collect::<Vec<_>>()
            .join("\n"),
    ]
    .join("\n")
}

fn push_review_state(timestamp: &str, payload_file: &Path, state: &mut Value, entry: &Value) {
    let mut reviews = state
        .get("reviews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    reviews.push(json!({
        "role": entry["role"].clone(),
        "round": entry["round"].clone(),
        "verdict": entry["verdict"].clone(),
        "summary": entry["summary"].clone(),
        "required_fixes": entry["required_fixes"].clone(),
        "path": entry["path"].clone(),
        "sha256": entry["sha256"].clone(),
        "persisted_at": timestamp,
        "payload": payload_file,
    }));
    state["reviews"] = json!(reviews);
    state["active"] = json!(true);
    state["phase"] = json!("reviewing");
    state["status"] = json!("reviewing");
    state["updated_at"] = json!(timestamp);
}
