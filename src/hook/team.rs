use super::*;

pub(super) fn register_requirement(
    timestamp: &str,
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
    let transport = if surface == "cli" && crate::team::warp_is_supported_by_default() {
        "warp"
    } else {
        "subagent-fallback"
    };
    let warp_layout = crate::team::warp_layout(teammate_count)
        .map(|layout| {
            json!({
                "columns": 2,
                "left_column": layout.left_column,
                "right_rows": layout.right_rows,
            })
        })
        .unwrap_or(Value::Null);
    let message_contract_example =
        crate::team::message_contract_example(&request_id(timestamp, payload_file), "executor-1");

    state["team"] = json!({
        "surface": surface,
        "leader": "current-session",
        "transport": transport,
        "teammate_count": teammate_count,
        "roles": role_names,
        "warp_layout": warp_layout,
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
        "request_id": request_id(timestamp, payload_file),
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
    match surface {
        "app" => format!(
            "Internal Megara team instruction: this Codex App session is the team leader. Use Codex subagents as teammates for exactly these roles: {roles}. For each teammate, send a bounded assignment with scope, allowed files or surfaces, acceptance criteria, evidence required, a correlation id, and a teammate id. Required message kinds are assignment, teammate status, teammate result, teammate failure, leader synthesis, and fallback notice. Wait for every teammate result or teammate failure before final synthesis. Do not expose Megara runtime metadata."
        ),
        "cli" => format!(
            "Internal Megara team instruction: this Codex CLI session is the team leader. Prefer Warp panes only if stable pane creation and message exchange are available in the current environment. The target layout is two columns: left column leader, right column split into one row per teammate role ({roles}). If Warp pane creation or message exchange is unavailable, emit exactly this one-line notice once and use Codex subagents instead: {fallback}. For fallback, use Codex subagents as teammates for exactly these roles: {roles}. Each assignment and result must carry a correlation id and teammate id. Wait for every teammate result or teammate failure before final synthesis. Do not expose Megara runtime metadata."
        ),
        _ => format!(
            "Internal Megara team instruction: runtime surface is ambiguous, so use Codex subagents as teammates. This session is the team leader. Required roles: {roles}. Each assignment and result must carry a correlation id and teammate id. Wait for every teammate result or teammate failure before final synthesis. Do not expose Megara runtime metadata."
        ),
    }
}

fn request_id(timestamp: &str, payload_file: &Path) -> String {
    format!("{TEAM}:{timestamp}:{}", payload_file.display())
}
