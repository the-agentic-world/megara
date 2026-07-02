use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use super::{trust_hash::command_hook_hash, trust_toml::merge_hook_trust_state};

#[derive(Clone, Debug, Serialize)]
pub struct HookTrustSummary {
    pub config_path: PathBuf,
    pub registered: usize,
    pub unchanged: usize,
    pub skipped: bool,
}

#[derive(Clone, Debug)]
pub(super) struct HookTrustState {
    pub(super) key: String,
    pub(super) trusted_hash: String,
}

pub(super) fn ensure_hook_trust(hooks_path: &Path, dry_run: bool) -> Result<HookTrustSummary> {
    let config_path = codex_home_dir()?.join("config.toml");
    if dry_run {
        return Ok(HookTrustSummary {
            config_path,
            registered: 0,
            unchanged: 0,
            skipped: true,
        });
    }

    let hooks_content = fs::read_to_string(hooks_path)
        .with_context(|| format!("failed to read hooks config {}", hooks_path.display()))?;
    let trust_states = trusted_hook_states(hooks_path, &hooks_content)?;
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    let (next, registered, unchanged) = merge_hook_trust_state(&existing, &trust_states);

    if next != existing {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&config_path, next)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }

    Ok(HookTrustSummary {
        config_path,
        registered,
        unchanged,
        skipped: false,
    })
}

fn codex_home_dir() -> Result<PathBuf> {
    if let Some(value) = env::var_os("CODEX_HOME") {
        if !value.is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    crate::paths::home_dir().map(|home| home.join(".codex"))
}

fn trusted_hook_states(hooks_path: &Path, hooks_content: &str) -> Result<Vec<HookTrustState>> {
    let hooks_path = fs::canonicalize(hooks_path).unwrap_or_else(|_| hooks_path.to_path_buf());
    let root: Value = serde_json::from_str(hooks_content)
        .with_context(|| format!("failed to parse hooks config {}", hooks_path.display()))?;
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };

    let mut states = Vec::new();
    for (event_name, groups) in hooks {
        let Some(event_label) = hook_event_label(event_name) else {
            continue;
        };
        collect_event_states(&hooks_path, event_label, groups, &mut states);
    }
    Ok(states)
}

fn collect_event_states(
    hooks_path: &Path,
    event_label: &str,
    groups: &Value,
    states: &mut Vec<HookTrustState>,
) {
    let Some(groups) = groups.as_array() else {
        return;
    };
    for (group_index, group) in groups.iter().enumerate() {
        let matcher = group.get("matcher").and_then(Value::as_str);
        let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
            continue;
        };
        for (handler_index, handler) in handlers.iter().enumerate() {
            if handler.get("type").and_then(Value::as_str) != Some("command")
                || handler.get("async").and_then(Value::as_bool) == Some(true)
                || handler
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                continue;
            }
            let key = format!(
                "{}:{event_label}:{group_index}:{handler_index}",
                hooks_path.display()
            );
            states.push(HookTrustState {
                key,
                trusted_hash: command_hook_hash(event_label, matcher, handler),
            });
        }
    }
}

fn hook_event_label(event_name: &str) -> Option<&'static str> {
    match event_name {
        "PreToolUse" => Some("pre_tool_use"),
        "PermissionRequest" => Some("permission_request"),
        "PostToolUse" => Some("post_tool_use"),
        "PreCompact" => Some("pre_compact"),
        "PostCompact" => Some("post_compact"),
        "SessionStart" => Some("session_start"),
        "UserPromptSubmit" => Some("user_prompt_submit"),
        "SubagentStart" => Some("subagent_start"),
        "SubagentStop" => Some("subagent_stop"),
        "Stop" => Some("stop"),
        _ => None,
    }
}
