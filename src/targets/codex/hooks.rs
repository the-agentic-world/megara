use std::path::Path;

use crate::paths::InstallScope;

pub(super) fn codex_config() -> String {
    r#"# Megara Codex projection.
# Codex discovers skills, agents, and hooks from this directory.
"#
    .to_string()
}

pub(super) fn codex_hooks_json(scope: InstallScope, root: &Path, megara_bin: &Path) -> String {
    let hooks = serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "name": "megara-hook-UserPromptSubmit",
                    "type": "command",
                    "command": hook_command(megara_bin, scope, root, "UserPromptSubmit"),
                    "timeout": 21
                }]
            }],
            "PreToolUse": [{
                "hooks": [{
                    "name": "megara-hook-PreToolUse",
                    "type": "command",
                    "command": hook_command(megara_bin, scope, root, "PreToolUse"),
                    "timeout": 10
                }]
            }],
            "Stop": [{
                "hooks": [{
                    "name": "megara-hook-Stop",
                    "type": "command",
                    "command": hook_command(megara_bin, scope, root, "Stop"),
                    "timeout": 10
                }]
            }]
        }
    });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&hooks).expect("hooks json is serializable")
    )
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
