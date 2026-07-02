use serde_json::Value;

#[path = "mutation/redirection.rs"]
mod redirection;
#[path = "mutation/tools.rs"]
mod tools;

#[derive(Debug)]
pub(crate) struct MutationSignal {
    pub(crate) kind: &'static str,
    pub(crate) value: String,
}

pub(crate) fn mutation_signal(payload: &Value) -> Option<MutationSignal> {
    let tool_input = payload.get("tool_input").and_then(Value::as_object);
    let command = tool_input
        .and_then(|input| input.get("command").or_else(|| input.get("cmd")))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if mutating_command(command) {
        return Some(MutationSignal {
            kind: "command",
            value: command.to_string(),
        });
    }

    for name in tools::tool_names(payload) {
        if matches!(
            name.as_str(),
            "applypatch" | "edit" | "multiedit" | "notebookedit" | "write"
        ) {
            return Some(MutationSignal {
                kind: "tool",
                value: name,
            });
        }
    }
    None
}

pub(crate) fn protected_workflow_state_mutation(payload: &Value) -> Option<MutationSignal> {
    let mutation = mutation_signal(payload)?;
    payload_contains_protected_workflow_path(payload).then_some(mutation)
}

pub(crate) fn mutating_command(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    if redirection::has_mutating_redirection(command) {
        return true;
    }

    command
        .split([';', '&', '|'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .any(mutating_command_segment)
}

fn payload_contains_protected_workflow_path(value: &Value) -> bool {
    match value {
        Value::String(text) => is_protected_workflow_state_path(text),
        Value::Array(items) => items.iter().any(payload_contains_protected_workflow_path),
        Value::Object(object) => object
            .values()
            .any(payload_contains_protected_workflow_path),
        _ => false,
    }
}

fn is_protected_workflow_state_path(value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    [
        ".agents/state/workflows/deep-interview/",
        ".agents/state/workflows/ralplan/",
    ]
    .iter()
    .any(|path| normalized.contains(path))
}

fn mutating_command_segment(segment: &str) -> bool {
    let tokens = segment.split_whitespace().collect::<Vec<_>>();
    let Some(first) = tokens.first().copied() else {
        return false;
    };

    if first == "apply_patch" {
        return true;
    }
    if matches!(
        first,
        "rm" | "mv" | "cp" | "mkdir" | "touch" | "chmod" | "chown" | "ln" | "install" | "tee"
    ) {
        return true;
    }
    if first == "git"
        && tokens.get(1).is_some_and(|verb| {
            matches!(
                *verb,
                "add"
                    | "commit"
                    | "push"
                    | "tag"
                    | "checkout"
                    | "switch"
                    | "reset"
                    | "merge"
                    | "rebase"
                    | "restore"
            )
        })
    {
        return true;
    }
    if matches!(first, "npm" | "pnpm" | "yarn" | "bun")
        && tokens
            .get(1)
            .is_some_and(|verb| matches!(*verb, "install" | "add" | "remove" | "update"))
    {
        return true;
    }
    if first == "cargo" && tokens.get(1) == Some(&"fmt") {
        return true;
    }
    if first == "sed" && tokens.get(1).is_some_and(|arg| arg.starts_with("-i")) {
        return true;
    }
    first == "perl" && tokens.get(1).is_some_and(|arg| arg.starts_with("-pi"))
}
