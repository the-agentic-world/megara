use std::path::Path;

use anyhow::Result;

use crate::{paths::InstallScope, templates::TemplateRegistry};

use super::config::HarnessConfig;

pub(super) fn codex_config() -> String {
    r#"# Megara Codex projection.
# Codex discovers skills, agents, and hooks from this directory.
"#
    .to_string()
}

pub(super) fn codex_hooks_json(
    scope: InstallScope,
    root: &Path,
    megara_bin: &Path,
    registry: &TemplateRegistry,
) -> Result<String> {
    let config = HarnessConfig::from_registry(registry)?;
    let mut hook_events = serde_json::Map::new();

    if default_active_skill(&config, "caveman") {
        hook_events.insert(
            "SessionStart".to_string(),
            serde_json::json!([{
                "matcher": "startup|resume",
                "hooks": [{
                    "name": "megara-caveman-SessionStart",
                    "type": "command",
                    "command": CAVEMAN_SESSION_START_COMMAND,
                    "timeout": 5,
                    "statusMessage": "Loading caveman mode"
                }]
            }]),
        );
    }
    hook_events.insert(
        "UserPromptSubmit".to_string(),
        serde_json::json!([{
            "hooks": [{
                "name": "megara-hook-UserPromptSubmit",
                "type": "command",
                "command": hook_command(megara_bin, scope, root, "UserPromptSubmit"),
                "timeout": 21
            }]
        }]),
    );
    hook_events.insert(
        "PreToolUse".to_string(),
        serde_json::json!([{
            "hooks": [{
                "name": "megara-hook-PreToolUse",
                "type": "command",
                "command": hook_command(megara_bin, scope, root, "PreToolUse"),
                "timeout": 10
            }]
        }]),
    );
    hook_events.insert(
        "PostToolUse".to_string(),
        serde_json::json!([{
            "hooks": [{
                "name": "megara-hook-PostToolUse",
                "type": "command",
                "command": hook_command(megara_bin, scope, root, "PostToolUse"),
                "timeout": 10
            }]
        }]),
    );
    hook_events.insert(
        "Stop".to_string(),
        serde_json::json!([{
            "hooks": [{
                "name": "megara-hook-Stop",
                "type": "command",
                "command": hook_command(megara_bin, scope, root, "Stop"),
                "timeout": 10
            }]
        }]),
    );
    hook_events.insert(
        "SubagentStart".to_string(),
        serde_json::json!([{
            "hooks": [{
                "name": "megara-hook-SubagentStart",
                "type": "command",
                "command": hook_command(megara_bin, scope, root, "SubagentStart"),
                "timeout": 10
            }]
        }]),
    );
    hook_events.insert(
        "SubagentStop".to_string(),
        serde_json::json!([{
            "hooks": [{
                "name": "megara-hook-SubagentStop",
                "type": "command",
                "command": hook_command(megara_bin, scope, root, "SubagentStop"),
                "timeout": 10
            }]
        }]),
    );

    let hooks = serde_json::json!({ "hooks": hook_events });
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&hooks).expect("hooks json is serializable")
    ))
}

const CAVEMAN_SESSION_START_COMMAND: &str = "echo 'CAVEMAN MODE ACTIVE. Rules: Drop articles/filler/pleasantries/hedging. Fragments OK. Short synonyms. Pattern: [thing] [action] [reason]. [next step]. Off only: stop caveman / normal mode. Code, commands, commits, PRs: write normal.'";

fn default_active_skill(config: &HarnessConfig, name: &str) -> bool {
    config
        .default_active_skills
        .iter()
        .any(|skill| skill == name)
}

fn hook_command(megara_bin: &Path, scope: InstallScope, root: &Path, event: &str) -> String {
    let scope = scope.to_string();
    let project_root = match scope.as_str() {
        "project" => root
            .parent()
            .map(|path| {
                format!(
                    " --project-root {}",
                    shell_quote(&path.display().to_string())
                )
            })
            .unwrap_or_default(),
        _ => String::new(),
    };
    format!(
        "{} hook --managed-marker MEGARA:MANAGED --scope {scope}{project_root} --runtime codex --event {event}",
        shell_quote(&megara_bin.display().to_string())
    )
}

fn shell_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}
