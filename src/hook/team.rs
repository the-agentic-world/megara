use super::*;

pub(super) fn register_requirement(
    timestamp: &str,
    state_dir: &Path,
    state: &mut Value,
    payload: &Value,
    prompt: &str,
    payload_file: &Path,
) {
    let roles = crate::team::select_teammates(prompt);
    let role_names = crate::team::role_names(&roles);
    let runtime = runtime_input::runtime_context(payload);
    let surface = runtime.surface.as_str();
    let teammate_count = role_names.len();
    let split_correlation_id = crate::team::team_correlation_id(timestamp);
    let runtime_root = runtime_root_from_state_dir(state_dir);
    let split_receipt_dir = crate::team::split::receipt_dir(&runtime_root, &split_correlation_id);
    if let Err(error) = crate::team::split::write_task(&runtime_root, &split_correlation_id, prompt)
    {
        state["team_split_task_error"] = json!(error.to_string());
    }
    let transport = if surface == "cli" {
        "split-pane"
    } else {
        "subagent-fallback"
    };
    let split_layout = crate::team::split_layout(teammate_count)
        .map(|layout| {
            json!({
                "columns": 2,
                "left_column": layout.left_column,
                "right_rows": layout.right_rows,
            })
        })
        .unwrap_or(Value::Null);
    let message_contract_example =
        crate::team::message_contract_example(&split_correlation_id, "executor-1");

    state["team"] = json!({
        "surface": surface,
        "leader": "current-session",
        "transport": transport,
        "correlation_id": split_correlation_id,
        "teammate_count": teammate_count,
        "roles": role_names,
        "split_layout": split_layout,
        "split_transports": crate::team::cli_split_transports(),
        "split_receipt_dir": split_receipt_dir,
        "fallback_notice": crate::team::FALLBACK_NOTICE,
        "message_contract": crate::team::message_contract_kinds(),
        "message_contract_example": message_contract_example,
        "requires_correlation_id": true,
        "requires_teammate_id": true,
    });
    state["subagent_orchestration"] = json!({
        "status": "required",
        "workflow": TEAM,
        "roles": role_names,
        "requested_at": timestamp,
        "request_id": split_correlation_id,
        "payload": payload_file,
        "transport": transport,
    });
    state["subagent_receipts"] = json!([]);
    state["subagent_in_flight"] = json!([]);
    state["updated_at"] = json!(timestamp);
}

pub(super) fn print_user_prompt_context(
    payload: &Value,
    prompt: &str,
    state: &Value,
) -> Result<()> {
    let context = additional_context(payload, prompt, state);
    println!(
        "{}",
        serde_json::to_string(&json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": context,
            }
        }))?
    );
    Ok(())
}

fn additional_context(payload: &Value, prompt: &str, state: &Value) -> String {
    let roles = state
        .get("subagent_orchestration")
        .and_then(|value| value.get("roles"))
        .and_then(Value::as_array)
        .map(|roles| {
            roles
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| {
            crate::team::role_names(&crate::team::select_teammates(prompt)).join(", ")
        });
    let surface = runtime_input::runtime_context(payload).surface.as_str();
    let fallback = crate::team::FALLBACK_NOTICE;
    let correlation_id = state
        .get("team")
        .and_then(|value| value.get("correlation_id"))
        .and_then(Value::as_str)
        .unwrap_or("team-current");
    let split_command = format!(
        "MEGARA_BIN=\"${{MEGARA_BIN:-.agents/bin/megara}}\"; \"$MEGARA_BIN\" team split --transport auto --roles {} --correlation-id {} --open",
        roles.replace(", ", ","),
        shell_quote(correlation_id)
    );
    match surface {
        "app" => format!(
            "Internal Megara team instruction: this Codex App session is the team leader. Use Codex subagents as teammates for exactly these roles: {roles}. For each teammate, send a bounded assignment with scope, allowed files or surfaces, acceptance criteria, evidence required, a correlation id, and a teammate id. Required message kinds are assignment, teammate status, teammate result, teammate failure, leader synthesis, and fallback notice. Wait for every teammate result or teammate failure before final synthesis. Do not expose Megara runtime metadata."
        ),
        "cli" => format!(
            "Internal Megara team instruction: this Codex CLI session is the team leader. First try the executable CLI split pane path by running this command silently from the project root: `{split_command}`. Supported split transports are cmux, tmux, and orca only. The target layout is two columns: left column leader guidance, right column split into one row per teammate role ({roles}). Wait for every teammate result or teammate failure before final synthesis. If cmux, tmux, and orca split pane creation, Codex CLI execution, or receipt collection is unavailable or fails, emit exactly this one-line notice once and use Codex subagents instead: {fallback}. Do not include split failure details or runtime metadata in the notice. For fallback, use Codex subagents as teammates for exactly these roles: {roles}. Each assignment and result must carry a correlation id and teammate id. Do not expose Megara runtime metadata."
        ),
        _ => format!(
            "Internal Megara team instruction: runtime surface is ambiguous, so use Codex subagents as teammates. This session is the team leader. Required roles: {roles}. Each assignment and result must carry a correlation id and teammate id. Wait for every teammate result or teammate failure before final synthesis. Do not expose Megara runtime metadata."
        ),
    }
}

pub(super) fn sync_split_receipts(timestamp: &str, state: &mut Value) {
    let Some(team_state) = state.get("team") else {
        return;
    };
    let Some(receipt_dir) = team_state
        .get("split_receipt_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
    else {
        return;
    };
    let Some(correlation_id) = team_state.get("correlation_id").and_then(Value::as_str) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&receipt_dir) else {
        return;
    };
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return;
    };
    let mut receipts = state
        .get("subagent_receipts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Some(mut receipt) = load_json(&path) else {
            continue;
        };
        if receipt.get("correlation_id").and_then(Value::as_str) != Some(correlation_id) {
            continue;
        }
        let Some(role) = receipt
            .get("role")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        receipt["workflow"] = json!(TEAM);
        if receipt.get("transport").and_then(Value::as_str).is_none() {
            receipt["transport"] = json!("split-pane");
        }
        receipt["orchestration_request_id"] = orchestration
            .get("request_id")
            .cloned()
            .unwrap_or_else(|| json!(correlation_id));
        receipt["synced_at"] = json!(timestamp);
        receipt["receipt_file"] = json!(path);
        receipts.retain(|existing| {
            existing
                .get("orchestration_request_id")
                .and_then(Value::as_str)
                != receipt
                    .get("orchestration_request_id")
                    .and_then(Value::as_str)
                || existing.get("role").and_then(Value::as_str) != Some(role.as_str())
        });
        receipts.push(receipt);
    }
    state["subagent_receipts"] = json!(receipts);
    mark_split_receipts_satisfied(timestamp, state);
}

fn runtime_root_from_state_dir(state_dir: &Path) -> PathBuf {
    if state_dir.file_name().is_some_and(|name| name == "hooks") {
        return state_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| state_dir.to_path_buf());
    }
    if state_dir.file_name().is_some_and(|name| name == "state") {
        return state_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| state_dir.to_path_buf());
    }
    state_dir.to_path_buf()
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn mark_split_receipts_satisfied(timestamp: &str, state: &mut Value) {
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return;
    };
    let Some(request_id) = orchestration.get("request_id").and_then(Value::as_str) else {
        return;
    };
    let roles = orchestration
        .get("roles")
        .and_then(Value::as_array)
        .map(|roles| roles.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if roles.is_empty() {
        return;
    }
    let receipts = state
        .get("subagent_receipts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let satisfied = roles.iter().all(|role| {
        receipts.iter().any(|receipt| {
            receipt
                .get("orchestration_request_id")
                .and_then(Value::as_str)
                == Some(request_id)
                && receipt.get("role").and_then(Value::as_str) == Some(*role)
        })
    });
    if satisfied {
        let mut updated = orchestration;
        updated["status"] = json!("satisfied");
        updated["missing_roles"] = json!([]);
        updated["in_flight_roles"] = json!([]);
        updated["updated_at"] = json!(timestamp);
        state["subagent_orchestration"] = updated;
    }
}
