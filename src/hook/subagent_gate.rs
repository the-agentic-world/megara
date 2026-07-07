use super::*;

pub(super) fn workflow_start_from_prompt(prompt: &str) -> Option<&'static str> {
    let lower = prompt_after_optional_plan_prefix(prompt)
        .trim_start()
        .to_ascii_lowercase();
    if starts_workflow(&lower, DEEP_INTERVIEW) {
        return Some(DEEP_INTERVIEW);
    }
    if starts_workflow(&lower, RALPLAN) {
        return Some(RALPLAN);
    }
    None
}

pub(super) fn register_requirement(
    timestamp: &str,
    state: &mut Value,
    workflow: &str,
    payload_file: &Path,
) {
    let roles = required_roles(workflow);
    if roles.is_empty() {
        return;
    }
    state["subagent_orchestration"] = json!({
        "status": "required",
        "workflow": workflow,
        "roles": roles,
        "requested_at": timestamp,
        "request_id": request_id(workflow, timestamp, payload_file),
        "payload": payload_file,
    });
    state["subagent_receipts"] = json!([]);
    state["subagent_in_flight"] = json!([]);
    state["updated_at"] = json!(timestamp);
}

pub(super) fn additional_context(workflow: &str) -> Option<&'static str> {
    match workflow {
        DEEP_INTERVIEW => Some(
            "Internal Megara workflow instruction: this deep-interview run must ask the Round 0 topology question before broad repository inspection or source-file reads unless the user explicitly requested a repository audit before interviewing. Use the user request and already-loaded context for Round 0. After Round 0, do not block the immediate next question on repository inspection; ask one compact follow-up from the confirmed topology first. On later turns, use a minimal brownfield fact pass only when the next decision depends on repository facts: inspect file names/manifests and at most five focused source/test files, avoid broad full-file dumps, and return to one compact question. This run must use one Codex subagent before final crystallization. Spawn exactly one architect subagent for lateral review of assumptions, hidden risks, and missing acceptance criteria after the interview has enough context for a useful review. The subagent prompt must include the compact interview context needed for review and must explicitly forbid tool calls, forbid Megara workflow/skill invocation, forbid nested subagents, forbid file reads, forbid file writes, forbid implementation, forbid long exploration, and forbid progress/commentary output. Ask the subagent to decide only from the prompt context and return a final-answer-only short direct verdict. Do not send main-session commentary/progress updates while spawning or waiting for subagents; wait silently and answer only with the compact next question or final spec. Wait for the subagent result, fold only the useful conclusions into the main-session interview or final spec, and keep all Megara runtime details out of user-facing prose.",
        ),
        RALPLAN => Some(
            "Internal Megara instruction for this ralplan turn. Do not inspect more source files before review unless the locked spec is missing. First write a short internal draft plan with scope, files, ordered tasks, acceptance criteria, verification, risks, and a baseline-failure policy. If baseline tests already fail, classify them as pre-existing, avoid expanding scope to fix unrelated failures, and verify no new failures plus targeted evidence for this plan. Do not block on verification details that can be closed conservatively, such as baseline semantics, key-input no-op behavior, status widgets, accessibility labels, report surfaces, or review-only versus execution stages; pick the stricter product-facing criterion and state it in the plan. Do not put workflow or handoff names such as ralplan, ultragoal, or team inside the draft plan body; reserve approval targets for the final numbered choices only. Then use Codex subagents for exactly these review roles: planner, architect, critic. Include the same draft plan in every subagent prompt. Each subagent prompt must forbid tools, file reads, file writes, Megara workflows, nested subagents, implementation, and progress output; ask for a short final verdict only. Wait for all three receipts, revise once if the critic asks for iteration, convert verification-detail feedback into concrete plan criteria, then answer only with the final approval-ready plan. Use a user-friendly blocker only for contradictions or missing facts that cannot be safely planned around. Keep runtime metadata hidden.",
        ),
        _ => None,
    }
}

pub(super) fn print_user_prompt_context(workflow: &str) -> Result<()> {
    let Some(context) = additional_context(workflow) else {
        return Ok(());
    };
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

pub(super) fn answer_continuation_context(workflow: &str, state: &Value) -> Option<String> {
    let missing = missing_roles_for_state(state);
    if missing.is_empty() {
        return None;
    }
    let in_flight = in_flight_missing_roles_for_state(state, &missing);
    let spawn_roles = missing
        .iter()
        .copied()
        .filter(|role| !in_flight.contains(role))
        .collect::<Vec<_>>()
        .join(", ");
    let waiting_roles = in_flight.join(", ");
    let spawn = spawn_instruction(&spawn_roles, &waiting_roles);
    match workflow {
        DEEP_INTERVIEW => Some(format!(
            "Internal Megara workflow instruction: continue deep-interview in the configured locale. Ask exactly one compact next question if more user input is still needed. Do not delay an ordinary interview question turn to spawn subagents. Before final crystallization, complete the required context-only, tool-free subagent review. Missing receipt roles: {}. If you are about to crystallize, complete the missing review first; otherwise keep the interview moving. The subagent prompt must include only the compact interview context and must forbid tool calls, file reads, file writes, Megara workflows, nested subagents, implementation, long exploration, and progress output. Keep this instruction internal and do not expose runtime metadata.",
            missing.join(", ")
        )),
        RALPLAN => Some(format!(
            "Internal Megara workflow instruction: continue ralplan in the configured locale. Before approval-ready output, complete required context-only, tool-free subagent reviews. Missing receipt roles: {}. {} Keep this instruction internal and do not expose runtime metadata.",
            missing.join(", "),
            spawn
        )),
        _ => None,
    }
}

pub(super) fn record_start(
    timestamp: &str,
    state: &mut Value,
    payload: &Value,
    payload_file: &Path,
) {
    let Some(role) = role_from_payload(payload) else {
        return;
    };
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return;
    };
    if !roles_from_orchestration(&orchestration).contains(&role) {
        return;
    }

    let mut entry = json!({
        "role": role,
        "started_at": timestamp,
        "payload": payload_file,
        "workflow": orchestration.get("workflow").cloned().unwrap_or(Value::Null),
        "orchestration_requested_at": orchestration
            .get("requested_at")
            .cloned()
            .unwrap_or(Value::Null),
        "orchestration_request_id": orchestration
            .get("request_id")
            .cloned()
            .unwrap_or(Value::Null),
    });
    copy_subagent_identity(payload, &mut entry);

    let mut in_flight = state
        .get("subagent_in_flight")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    in_flight.retain(|existing| !same_subagent_identity(existing, &entry));
    if !role_already_satisfied(state, role, &orchestration) {
        in_flight.push(entry);
    }
    state["subagent_in_flight"] = json!(in_flight);

    update_orchestration_waiting(timestamp, state);
}

pub(super) fn record_stop_receipt(
    timestamp: &str,
    state: &mut Value,
    payload: &Value,
    payload_file: &Path,
) {
    let Some(role) = role_from_payload(payload) else {
        return;
    };
    let orchestration = state.get("subagent_orchestration").cloned();
    let mut receipt = json!({
        "role": role,
        "stopped_at": timestamp,
        "payload": payload_file,
    });
    if let Some(orchestration) = orchestration.as_ref() {
        receipt["workflow"] = orchestration
            .get("workflow")
            .cloned()
            .unwrap_or(Value::Null);
        receipt["orchestration_requested_at"] = orchestration
            .get("requested_at")
            .cloned()
            .unwrap_or(Value::Null);
        receipt["orchestration_request_id"] = orchestration
            .get("request_id")
            .cloned()
            .unwrap_or(Value::Null);
    }
    for key in [
        "agent_id",
        "subagent_id",
        "agent_type",
        "subagent_name",
        "name",
    ] {
        if let Some(value) = payload.get(key) {
            receipt[key] = value.clone();
        }
    }
    let mut receipts = state
        .get("subagent_receipts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    receipts.retain(|existing| {
        if let Some(orchestration) = orchestration.as_ref() {
            !receipt_matches_orchestration(existing, orchestration)
                || existing.get("role").and_then(Value::as_str) != Some(role)
        } else {
            existing.get("role").and_then(Value::as_str) != Some(role)
        }
    });
    receipts.push(receipt);
    state["subagent_receipts"] = json!(receipts);
    clear_in_flight_for_role(state, role, orchestration.as_ref());

    update_orchestration_waiting(timestamp, state);
}

pub(super) fn reset_after_refine(timestamp: &str, state: &mut Value, payload_file: &Path) {
    if state.get("subagent_orchestration").is_none() {
        return;
    }
    state["subagent_receipts"] = json!([]);
    state["subagent_in_flight"] = json!([]);
    let missing = missing_roles_from_orchestration(state);
    if let Some(orchestration) = state.get("subagent_orchestration") {
        let mut updated = orchestration.clone();
        updated["status"] = json!("required");
        updated["missing_roles"] = json!(missing);
        if let Some(workflow) = updated
            .get("workflow")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            updated["request_id"] = json!(request_id(&workflow, timestamp, payload_file));
        }
        updated["requested_at"] = json!(timestamp);
        updated["payload"] = json!(payload_file);
        updated["updated_at"] = json!(timestamp);
        state["subagent_orchestration"] = updated;
    }
    state["updated_at"] = json!(timestamp);
}

pub(super) fn block_terminal_if_missing_receipts(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<Option<String>> {
    if !terminal_status_requires_receipts(terminal) {
        return Ok(None);
    }
    let missing = missing_roles_for_state(state);
    let in_flight_roles = in_flight_missing_roles_for_state(state, &missing);
    if missing.is_empty() {
        return Ok(None);
    }

    state["active"] = json!(true);
    state["phase"] = json!("subagent_review_required");
    state["status"] = json!("subagent_review_required");
    if terminal.skill == RALPLAN {
        state["approval_status"] = json!("blocked");
    }
    if let Some(orchestration) = state.get("subagent_orchestration") {
        let mut updated = orchestration.clone();
        updated["status"] = json!("waiting_for_receipts");
        updated["missing_roles"] = json!(missing.clone());
        updated["in_flight_roles"] = json!(in_flight_roles.clone());
        updated["updated_at"] = json!(timestamp);
        state["subagent_orchestration"] = updated;
    }
    state["updated_at"] = json!(timestamp);
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "subagent_receipts_missing",
            "session_id": paths.session_id,
            "skill": terminal.skill,
            "status": terminal.status,
            "missing_roles": missing,
            "in_flight_roles": in_flight_roles,
            "payload": payload_file,
        }),
    )?;
    Ok(Some(continuation_prompt(
        &terminal.skill,
        &missing,
        &in_flight_roles,
    )))
}

pub(super) fn role_from_payload(payload: &Value) -> Option<&'static str> {
    [
        "role",
        "agent_type",
        "subagent_name",
        "name",
        "agent_id",
        "subagent_id",
    ]
    .into_iter()
    .filter_map(|key| payload.get(key).and_then(Value::as_str))
    .find_map(role_from_text)
}

fn prompt_after_optional_plan_prefix(prompt: &str) -> &str {
    let trimmed = prompt.trim_start();
    let Some(rest) = trimmed.get(5..) else {
        return prompt;
    };
    if !trimmed[..5].eq_ignore_ascii_case("/plan") {
        return prompt;
    }
    if rest
        .chars()
        .next()
        .is_some_and(|first| first.is_whitespace() || first == '$' || first == '[')
    {
        rest.trim_start()
    } else {
        prompt
    }
}

fn starts_workflow(prompt: &str, workflow: &str) -> bool {
    prompt.starts_with(&format!("${workflow}"))
        || prompt.starts_with(&format!("[$${workflow}]").replace("$$", "$"))
}

fn terminal_status_requires_receipts(terminal: &TerminalState) -> bool {
    matches!(
        (terminal.skill.as_str(), terminal.status.as_str()),
        (DEEP_INTERVIEW, "crystallized") | (RALPLAN, "pending_approval")
    )
}

fn missing_roles_for_state(state: &Value) -> Vec<&'static str> {
    let Some(orchestration) = state.get("subagent_orchestration") else {
        return Vec::new();
    };
    if orchestration.get("status").and_then(Value::as_str) != Some("required")
        && orchestration.get("status").and_then(Value::as_str) != Some("waiting_for_receipts")
    {
        return Vec::new();
    }
    let roles = roles_from_orchestration(orchestration);
    if roles.is_empty() {
        return Vec::new();
    }
    let receipts = state
        .get("subagent_receipts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    roles
        .into_iter()
        .filter(|role| {
            !receipts.iter().any(|receipt| {
                receipt_matches_orchestration(receipt, orchestration)
                    && receipt.get("role").and_then(Value::as_str) == Some(*role)
            })
        })
        .collect()
}

fn in_flight_missing_roles_for_state(state: &Value, missing: &[&'static str]) -> Vec<&'static str> {
    let Some(orchestration) = state.get("subagent_orchestration") else {
        return Vec::new();
    };
    let in_flight = state
        .get("subagent_in_flight")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    missing
        .iter()
        .copied()
        .filter(|role| {
            in_flight.iter().any(|entry| {
                receipt_matches_orchestration(entry, orchestration)
                    && entry.get("role").and_then(Value::as_str) == Some(*role)
            })
        })
        .collect()
}

fn missing_roles_from_orchestration(state: &Value) -> Vec<&'static str> {
    state
        .get("subagent_orchestration")
        .map(roles_from_orchestration)
        .unwrap_or_default()
}

fn roles_from_orchestration(orchestration: &Value) -> Vec<&'static str> {
    orchestration
        .get("roles")
        .and_then(Value::as_array)
        .map(|roles| {
            roles
                .iter()
                .filter_map(Value::as_str)
                .filter_map(role_from_text)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn continuation_prompt(
    workflow: &str,
    missing: &[&'static str],
    in_flight: &[&'static str],
) -> String {
    let roles = missing.join(", ");
    let spawn_roles = missing
        .iter()
        .copied()
        .filter(|role| !in_flight.contains(role))
        .collect::<Vec<_>>()
        .join(", ");
    let waiting_roles = in_flight.join(", ");
    let spawn = spawn_instruction(&spawn_roles, &waiting_roles);
    match workflow {
        DEEP_INTERVIEW => format!(
            "Megara requires context-only, tool-free subagent review before deep-interview can crystallize. Missing receipt roles: {roles}. {spawn} The subagent prompt must include the compact interview context and forbid tool calls or file reads. Wait for in-flight roles to finish, fold the useful findings into the final spec or next question, then retry crystallization. Keep this runtime instruction internal and do not show Megara metadata to the user."
        ),
        RALPLAN => format!(
            "Megara requires ralplan review receipts before approval. Missing roles: {roles}. {spawn} Use the pending plan as the reviewed draft in every missing-role prompt. Wait for receipts, fold useful findings into the plan, then retry the approval-ready response. Keep this instruction and all runtime metadata hidden."
        ),
        _ => format!(
            "Megara requires missing Codex subagent receipts before this workflow can continue: {roles}. {spawn} Complete the subagent review and retry the workflow response."
        ),
    }
}

fn spawn_instruction(spawn_roles: &str, waiting_roles: &str) -> String {
    match (spawn_roles.is_empty(), waiting_roles.is_empty()) {
        (false, false) => format!(
            "Spawn only the missing roles that are not already in-flight: {spawn_roles}. Do not spawn duplicate/replacement subagents for in-flight roles: {waiting_roles}."
        ),
        (false, true) => {
            format!("Spawn only these missing roles now: {spawn_roles}. Do not spawn duplicates.")
        }
        (true, false) => format!(
            "Do not spawn duplicate/replacement subagents. The required roles are already in-flight: {waiting_roles}."
        ),
        (true, true) => "Do not spawn duplicate/replacement subagents.".to_string(),
    }
}

fn required_roles(workflow: &str) -> &'static [&'static str] {
    match workflow {
        DEEP_INTERVIEW => &["architect"],
        RALPLAN => &["planner", "architect", "critic"],
        _ => &[],
    }
}

fn request_id(workflow: &str, timestamp: &str, payload_file: &Path) -> String {
    format!("{workflow}:{timestamp}:{}", payload_file.display())
}

fn receipt_matches_orchestration(receipt: &Value, orchestration: &Value) -> bool {
    let Some(request_id) = orchestration.get("request_id").and_then(Value::as_str) else {
        return false;
    };
    receipt
        .get("orchestration_request_id")
        .and_then(Value::as_str)
        == Some(request_id)
}

fn update_orchestration_waiting(timestamp: &str, state: &mut Value) {
    let Some(orchestration) = state.get("subagent_orchestration") else {
        state["updated_at"] = json!(timestamp);
        return;
    };
    let missing = missing_roles_for_state(state);
    let in_flight_roles = in_flight_missing_roles_for_state(state, &missing);
    let next_status = if missing.is_empty() {
        "satisfied"
    } else {
        "waiting_for_receipts"
    };
    let mut updated = orchestration.clone();
    updated["status"] = json!(next_status);
    updated["missing_roles"] = json!(missing);
    updated["in_flight_roles"] = json!(in_flight_roles);
    updated["updated_at"] = json!(timestamp);
    state["subagent_orchestration"] = updated;
    state["updated_at"] = json!(timestamp);
}

fn copy_subagent_identity(payload: &Value, target: &mut Value) {
    for key in [
        "agent_id",
        "subagent_id",
        "agent_type",
        "subagent_name",
        "name",
        "turn_id",
    ] {
        if let Some(value) = payload.get(key) {
            target[key] = value.clone();
        }
    }
}

fn same_subagent_identity(left: &Value, right: &Value) -> bool {
    ["agent_id", "subagent_id", "turn_id"].iter().any(|key| {
        let left_value = left.get(*key).and_then(Value::as_str);
        left_value.is_some() && left_value == right.get(*key).and_then(Value::as_str)
    })
}

fn role_already_satisfied(state: &Value, role: &str, orchestration: &Value) -> bool {
    state
        .get("subagent_receipts")
        .and_then(Value::as_array)
        .is_some_and(|receipts| {
            receipts.iter().any(|receipt| {
                receipt_matches_orchestration(receipt, orchestration)
                    && receipt.get("role").and_then(Value::as_str) == Some(role)
            })
        })
}

fn clear_in_flight_for_role(state: &mut Value, role: &str, orchestration: Option<&Value>) {
    let Some(orchestration) = orchestration else {
        return;
    };
    let mut in_flight = state
        .get("subagent_in_flight")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    in_flight.retain(|entry| {
        !receipt_matches_orchestration(entry, orchestration)
            || entry.get("role").and_then(Value::as_str) != Some(role)
    });
    state["subagent_in_flight"] = json!(in_flight);
}

fn role_from_text(value: &str) -> Option<&'static str> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("planner") {
        return Some("planner");
    }
    if lower.contains("architect") {
        return Some("architect");
    }
    if lower.contains("critic") {
        return Some("critic");
    }
    None
}
