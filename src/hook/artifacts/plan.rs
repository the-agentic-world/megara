use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::hook::{
    fsutil::{append_jsonl, sha256_hex, write_text_atomic},
    parser::{text_before_first_workflow_block, TerminalState},
    ralplan_input::LinkedSpec,
    state_paths::WorkflowPaths,
    RALPLAN,
};

use super::{
    path::{unique_plan_path, yaml_string},
    types::PersistedPlan,
};

pub(crate) fn persist_pending_plan(
    timestamp: &str,
    paths: &WorkflowPaths,
    plan_id: &str,
    terminal: &TerminalState,
    text: &str,
    payload_file: &Path,
    linked_spec: Option<&LinkedSpec>,
) -> Result<Option<PersistedPlan>> {
    if terminal.status != "pending_approval" || text.trim().is_empty() {
        return Ok(None);
    }
    let plan_body = text_before_first_workflow_block(text);
    if plan_body.is_empty() {
        return Ok(None);
    }

    let mut header = vec![
        "---".to_string(),
        "skill: \"ralplan\"".to_string(),
        format!("session_id: {}", yaml_string(&paths.session_id)),
        format!("plan_id: {}", yaml_string(plan_id)),
        "status: \"pending_approval\"".to_string(),
        format!("next: {}", yaml_string(&terminal.next)),
    ];
    if let Some(spec) = linked_spec {
        header.push(format!("input_spec_path: {}", yaml_string(&spec.path)));
        header.push(format!("input_spec_sha256: {}", yaml_string(&spec.sha256)));
    }
    header.extend([
        format!("persisted_at: {}", yaml_string(timestamp)),
        format!("payload: {}", yaml_string(payload_file.display())),
        "---".to_string(),
        String::new(),
        plan_body,
    ]);
    let mut content = header.join("\n");
    content.push('\n');

    let plan_path = unique_plan_path(&paths.workflow_dir, &paths.session_id, plan_id, timestamp);
    write_text_atomic(&plan_path, &content)?;
    let sha256 = sha256_hex(content.as_bytes());
    append_jsonl(
        &paths.workflow_dir.join("plans").join("index.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "plan_persisted",
            "session_id": paths.session_id,
            "skill": RALPLAN,
            "status": "pending_approval",
            "plan_id": plan_id,
            "path": plan_path,
            "sha256": sha256,
            "input_spec_path": linked_spec.map(|spec| spec.path.as_str()),
            "input_spec_sha256": linked_spec.map(|spec| spec.sha256.as_str()),
            "payload": payload_file,
        }),
    )?;

    Ok(Some(PersistedPlan {
        path: plan_path.display().to_string(),
        sha256,
        persisted_at: timestamp.to_string(),
        payload: payload_file.display().to_string(),
    }))
}
