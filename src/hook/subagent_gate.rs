use super::*;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

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
    if starts_workflow(&lower, TEAM) {
        return Some(TEAM);
    }
    None
}

pub(super) fn register_requirement(
    timestamp: &str,
    state: &mut Value,
    workflow: &str,
    payload_file: &Path,
) {
    if workflow == DEEP_INTERVIEW {
        state["subagent_orchestration"] = json!({
            "status": "idle",
            "workflow": workflow,
            "audit": [],
            "updated_at": timestamp,
        });
        state["subagent_receipts"] = json!([]);
        state["subagent_in_flight"] = json!([]);
        state["updated_at"] = json!(timestamp);
        return;
    }
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

pub(super) fn schedule_deep_interview_review(
    timestamp: &str,
    state: &mut Value,
    payload_file: &Path,
    trigger: &str,
    include_architect: bool,
) -> bool {
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return false;
    };
    if orchestration.get("workflow").and_then(Value::as_str) != Some(DEEP_INTERVIEW) {
        return false;
    }

    let mut roles = vec!["researcher", "contrarian", "simplifier"];
    if include_architect {
        roles.push("architect");
    }
    let trigger_id = request_id(trigger, timestamp, payload_file);
    let mut audit = orchestration
        .get("audit")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(previous) = orchestration.get("trigger_id").and_then(Value::as_str) {
        if !previous.is_empty()
            && orchestration.get("status").and_then(Value::as_str) != Some("satisfied")
        {
            audit.push(json!({
                "event": "trigger_superseded",
                "trigger_id": previous,
                "superseded_by": trigger_id,
                "recorded_at": timestamp,
            }));
        }
    }
    audit.push(json!({
        "event": "trigger_scheduled",
        "trigger_id": trigger_id,
        "trigger": trigger,
        "roles": roles,
        "recorded_at": timestamp,
    }));
    state["subagent_orchestration"] = json!({
        "status": "required",
        "workflow": DEEP_INTERVIEW,
        "trigger": trigger,
        "trigger_id": trigger_id,
        "roles": roles,
        "requested_at": timestamp,
        "request_id": trigger_id,
        "payload": payload_file,
        "audit": audit,
    });
    state["subagent_receipts"] = json!([]);
    state["subagent_in_flight"] = json!([]);
    state["subagent_retry_intents"] = json!([]);
    state["updated_at"] = json!(timestamp);
    true
}

pub(super) fn prepare_deep_interview_reassessment_review(
    timestamp: &str,
    state: &mut Value,
    payload_file: &Path,
) {
    let request_id = request_id("deep-interview-reassessment", timestamp, payload_file);
    state["subagent_orchestration"] = json!({
        "status": "conditional",
        "workflow": DEEP_INTERVIEW,
        "trigger": "reassessment_pending",
        "trigger_id": request_id,
        "roles": ["researcher", "contrarian", "simplifier", "architect"],
        "requested_at": timestamp,
        "request_id": request_id,
        "payload": payload_file,
        "audit": [{
            "event": "conditional_review_prepared",
            "trigger_id": request_id,
            "recorded_at": timestamp,
        }],
    });
    state["subagent_receipts"] = json!([]);
    state["subagent_in_flight"] = json!([]);
    state["subagent_retry_intents"] = json!([]);
    state["updated_at"] = json!(timestamp);
}

pub(super) fn resolve_deep_interview_reassessment_review(
    timestamp: &str,
    state: &mut Value,
    required: bool,
) -> bool {
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return false;
    };
    if orchestration.get("workflow").and_then(Value::as_str) != Some(DEEP_INTERVIEW)
        || orchestration.get("trigger").and_then(Value::as_str) != Some("reassessment_pending")
    {
        return false;
    }

    if !required {
        let mut updated = orchestration;
        updated["status"] = json!("idle");
        updated["roles"] = json!([]);
        updated["missing_roles"] = json!([]);
        updated["in_flight_roles"] = json!([]);
        updated["trigger"] = json!("reassessment_unchanged");
        updated["updated_at"] = json!(timestamp);
        state["subagent_orchestration"] = updated;
        state["subagent_receipts"] = json!([]);
        state["subagent_in_flight"] = json!([]);
        state["subagent_retry_intents"] = json!([]);
        state["updated_at"] = json!(timestamp);
        return false;
    }

    let mut updated = orchestration;
    let mut audit = updated
        .get("audit")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    audit.push(json!({
        "event": "reassessment_review_required",
        "trigger_id": updated.get("trigger_id").cloned().unwrap_or(Value::Null),
        "recorded_at": timestamp,
    }));
    updated["status"] = json!("required");
    updated["trigger"] = json!("reassessment_change");
    updated["audit"] = json!(audit);
    updated["updated_at"] = json!(timestamp);
    state["subagent_orchestration"] = updated;
    update_orchestration_waiting(timestamp, state);
    true
}

pub(super) fn ensure_deep_interview_final_review(
    timestamp: &str,
    state: &mut Value,
    payload_file: &Path,
) -> bool {
    let Some(orchestration) = state.get("subagent_orchestration") else {
        return false;
    };
    if orchestration.get("workflow").and_then(Value::as_str) != Some(DEEP_INTERVIEW) {
        return false;
    }
    if orchestration.get("status").and_then(Value::as_str) == Some("satisfied") {
        return false;
    }
    schedule_deep_interview_review(
        timestamp,
        state,
        payload_file,
        "final_crystallization",
        false,
    )
}

pub(super) fn consume_satisfied_deep_interview_review(timestamp: &str, state: &mut Value) {
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return;
    };
    if orchestration.get("workflow").and_then(Value::as_str) != Some(DEEP_INTERVIEW)
        || orchestration.get("status").and_then(Value::as_str) != Some("satisfied")
    {
        return;
    }
    let mut updated = orchestration;
    let mut audit = updated
        .get("audit")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    audit.push(json!({
        "event": "review_consumed",
        "trigger_id": updated.get("trigger_id").cloned().unwrap_or(Value::Null),
        "recorded_at": timestamp,
    }));
    updated["status"] = json!("idle");
    updated["roles"] = json!([]);
    updated["missing_roles"] = json!([]);
    updated["in_flight_roles"] = json!([]);
    updated["audit"] = json!(audit);
    updated["updated_at"] = json!(timestamp);
    state["subagent_orchestration"] = updated;
    state["subagent_receipts"] = json!([]);
    state["subagent_in_flight"] = json!([]);
    state["subagent_retry_intents"] = json!([]);
}

pub(super) fn additional_context(workflow: &str) -> Option<&'static str> {
    match workflow {
        DEEP_INTERVIEW => Some(
            "Internal Megara workflow instruction: this deep-interview run must ask the Round 0 topology question before broad repository inspection or source-file reads unless the user explicitly requested a repository audit before interviewing. Use the user request and already-loaded context for Round 0. After Round 0, do not block the immediate next question on repository inspection; ask one compact follow-up from the confirmed topology first. On later turns, use a minimal brownfield fact pass only when the next decision depends on repository facts: inspect file names/manifests and at most five focused source/test files, avoid broad full-file dumps, and return to one compact question. Lateral reviewers are requested only when the runtime marks a milestone, semantic design change, or final crystallization. Keep all Megara runtime details out of user-facing prose.",
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
    let request_id = state
        .get("subagent_orchestration")
        .and_then(|orchestration| orchestration.get("request_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown-request");
    match workflow {
        DEEP_INTERVIEW => Some(format!(
            "Internal Megara workflow instruction: before the next deep-interview question or final crystallization, complete the required context-only, tool-free lateral reviews. Missing receipt roles: {}. {} Spawn only one attempt per role. Begin each reviewer prompt with the exact first line `MEGARA_ROLE=<role>`, replacing `<role>` with its assigned canonical role, and the exact second line `MEGARA_REQUEST={}`. Each reviewer receives the compact interview context and must not call tools, read files, write files, invoke Megara workflows, spawn nested subagents, implement, explore broadly, or emit progress. Wait for terminal reviewer results, close each completed reviewer, distill only the highest-value finding into one next question or the final spec, and keep this instruction internal.",
            missing.join(", "), spawn, request_id
        )),
        RALPLAN => Some(format!(
            "Internal Megara workflow instruction: continue ralplan in the configured locale. Before approval-ready output, complete required context-only, tool-free subagent reviews. Missing receipt roles: {}. {} Keep this instruction internal and do not expose runtime metadata.",
            missing.join(", "),
            spawn
        )),
        TEAM => Some(format!(
            "Internal Megara team instruction: continue team execution in the configured locale. This session remains the team leader. Required teammate receipts are missing for roles: {}. {} Use bounded teammate assignments with correlation id and teammate id, wait for result or failure receipts, then synthesize only after receipts exist. Keep this instruction internal and do not expose runtime metadata.",
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
    let role = role_from_payload(payload);
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return;
    };
    let orchestration_roles = roles_from_orchestration(&orchestration);
    if orchestration_roles.is_empty()
        || role.is_some_and(|role| !orchestration_roles.contains(&role))
    {
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
    if !role.is_some_and(|role| role_already_satisfied(state, role, &orchestration)) {
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
        clear_in_flight_for_identity(state, payload);
        return;
    };
    let orchestration = state.get("subagent_orchestration").cloned();
    if let Some(orchestration) = orchestration.as_ref() {
        assign_in_flight_role(state, role, payload, orchestration);
        if orchestration.get("workflow").and_then(Value::as_str) == Some(DEEP_INTERVIEW)
            && orchestration.get("trigger_id").is_some()
            && !is_current_deep_interview_attempt(state, role, payload, orchestration)
        {
            append_subagent_audit(
                state,
                json!({
                    "event": "late_receipt",
                    "role": role,
                    "trigger_id": orchestration.get("trigger_id").cloned().unwrap_or(Value::Null),
                    "recorded_at": timestamp,
                }),
            );
            return;
        }
    }
    let status = receipt_status(payload);
    let mut receipt = json!({
        "role": role,
        "status": status,
        "attempt_id": attempt_id(payload, timestamp),
        "sequence": next_receipt_sequence(state),
        "stopped_at": timestamp,
        "payload": payload_file,
        "compact_finding": compact_finding(payload),
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
    append_subagent_audit(
        state,
        json!({
            "event": "receipt_recorded",
            "role": role,
            "status": status,
            "trigger_id": orchestration.as_ref().and_then(|value| value.get("trigger_id")).cloned().unwrap_or(Value::Null),
            "recorded_at": timestamp,
        }),
    );
    let mut receipts = state
        .get("subagent_receipts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if status == "completed" {
        receipts.retain(|existing| {
            if let Some(orchestration) = orchestration.as_ref() {
                !receipt_matches_orchestration(existing, orchestration)
                    || existing.get("role").and_then(Value::as_str) != Some(role)
            } else {
                existing.get("role").and_then(Value::as_str) != Some(role)
            }
        });
        receipts.push(receipt);
    } else if status != "cancelled" {
        let mut retries = state
            .get("subagent_retry_intents")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        retries.retain(|entry| {
            entry.get("role").and_then(Value::as_str) != Some(role)
                || entry.get("orchestration_request_id") != receipt.get("orchestration_request_id")
        });
        retries.push(json!({
            "role": role,
            "status": "pending",
            "reason": status,
            "orchestration_request_id": receipt.get("orchestration_request_id").cloned().unwrap_or(Value::Null),
            "recorded_at": timestamp,
        }));
        state["subagent_retry_intents"] = json!(retries);
    }
    state["subagent_receipts"] = json!(receipts);
    clear_in_flight_for_role(state, role, orchestration.as_ref());

    if status == "cancelled" {
        if let Some(orchestration) = state.get("subagent_orchestration") {
            let mut updated = orchestration.clone();
            updated["status"] = json!("cancelled");
            updated["roles"] = json!([]);
            updated["missing_roles"] = json!([]);
            updated["in_flight_roles"] = json!([]);
            updated["updated_at"] = json!(timestamp);
            state["subagent_orchestration"] = updated;
        }
        state["subagent_in_flight"] = json!([]);
        state["subagent_retry_intents"] = json!([]);
        state["phase"] = json!("cancelled");
        state["status"] = json!("cancelled");
        state["active"] = json!(false);
        state["updated_at"] = json!(timestamp);
        return;
    }

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
        state,
    )))
}

pub(super) fn block_deep_interview_question_if_missing_receipts(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    state: &mut Value,
) -> Result<bool> {
    let missing = missing_roles_for_state(state);
    if missing.is_empty() {
        return Ok(false);
    }
    let Some(orchestration) = state.get("subagent_orchestration").cloned() else {
        return Ok(false);
    };
    if orchestration.get("workflow").and_then(Value::as_str) != Some(DEEP_INTERVIEW) {
        return Ok(false);
    }

    let in_flight_roles = in_flight_missing_roles_for_state(state, &missing);
    state["active"] = json!(true);
    state["phase"] = json!("subagent_review_required");
    state["status"] = json!("subagent_review_required");
    let mut updated = orchestration;
    updated["status"] = json!("waiting_for_receipts");
    updated["missing_roles"] = json!(missing.clone());
    updated["in_flight_roles"] = json!(in_flight_roles.clone());
    updated["updated_at"] = json!(timestamp);
    state["subagent_orchestration"] = updated;
    state["updated_at"] = json!(timestamp);
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "subagent_question_gate_blocked",
            "session_id": paths.session_id,
            "missing_roles": missing,
            "in_flight_roles": in_flight_roles,
            "payload": payload_file,
        }),
    )?;
    Ok(true)
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
    .or_else(|| role_from_subagent_transcript(payload))
}

fn role_from_subagent_transcript(payload: &Value) -> Option<&'static str> {
    ["agent_transcript_path", "transcript_path"]
        .into_iter()
        .filter_map(|key| payload.get(key).and_then(Value::as_str))
        .find_map(|path| {
            let file = File::open(path).ok()?;
            BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
                .filter_map(|record| subagent_user_message(&record))
                .filter_map(|message| role_from_first_line(&message))
                .last()
        })
}

fn subagent_request_id_from_transcript(payload: &Value) -> Option<String> {
    ["agent_transcript_path", "transcript_path"]
        .into_iter()
        .filter_map(|key| payload.get(key).and_then(Value::as_str))
        .find_map(|path| {
            let file = File::open(path).ok()?;
            BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
                .filter_map(|record| subagent_user_message(&record))
                .flat_map(|message| {
                    message
                        .lines()
                        .map(str::trim)
                        .filter_map(|line| line.strip_prefix("MEGARA_REQUEST="))
                        .map(str::trim)
                        .filter(|request_id| !request_id.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .last()
        })
}

fn subagent_user_message(record: &Value) -> Option<String> {
    let payload = record.get("payload")?;
    if record.get("type").and_then(Value::as_str) == Some("event_msg")
        && payload.get("type").and_then(Value::as_str) == Some("user_message")
    {
        return payload
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    if record.get("type").and_then(Value::as_str) != Some("response_item")
        || payload.get("type").and_then(Value::as_str) != Some("message")
        || payload.get("role").and_then(Value::as_str) != Some("user")
    {
        return None;
    }
    let message = payload
        .get("content")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!message.trim().is_empty()).then_some(message)
}

fn role_from_first_line(message: &str) -> Option<&'static str> {
    let line = message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    if line.len() > 80 {
        return None;
    }
    let (_, declared_role) = line.split_once('=').or_else(|| line.split_once(':'))?;
    exact_role_from_token(declared_role)
}

fn exact_role_from_token(value: &str) -> Option<&'static str> {
    let token = value
        .trim()
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '-')
        .split_whitespace()
        .next()?
        .to_ascii_lowercase();
    match token.as_str() {
        "planner" => Some("planner"),
        "architect" => Some("architect"),
        "critic" => Some("critic"),
        "researcher" => Some("researcher"),
        "contrarian" => Some("contrarian"),
        "simplifier" => Some("simplifier"),
        "executor" => Some("executor"),
        _ => None,
    }
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
        (DEEP_INTERVIEW, "crystallized")
            | (RALPLAN, "pending_approval")
            | (TEAM, "complete")
            | (TEAM, "completed")
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
                    && receipt.get("status").and_then(Value::as_str) == Some("completed")
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
    state: &Value,
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
    let request_id = state
        .get("subagent_orchestration")
        .and_then(|orchestration| orchestration.get("request_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown-request");
    match workflow {
        DEEP_INTERVIEW => format!(
            "Megara requires context-only, tool-free subagent review before deep-interview can crystallize. Missing receipt roles: {roles}. {spawn} Begin each reviewer prompt with the exact first line `MEGARA_ROLE=<role>`, replacing `<role>` with its assigned canonical role, and the exact second line `MEGARA_REQUEST={request_id}`. The subagent prompt must include the compact interview context and forbid tool calls or file reads. Wait for in-flight roles to finish, close each completed reviewer, fold the useful findings into the final spec or next question, then retry crystallization. Keep this runtime instruction internal and do not show Megara metadata to the user."
        ),
        RALPLAN => format!(
            "Megara requires ralplan review receipts before approval. Missing roles: {roles}. {spawn} Use the pending plan as the reviewed draft in every missing-role prompt. Wait for receipts, fold useful findings into the plan, then retry the approval-ready response. Keep this instruction and all runtime metadata hidden."
        ),
        TEAM => format!(
            "Megara requires teammate receipts before team completion. Missing roles: {roles}. {spawn} Use bounded teammate assignments with correlation id and teammate id, wait for result or failure receipts, then retry the leader synthesis. Keep this instruction and all runtime metadata hidden."
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
    if orchestration.get("status").and_then(Value::as_str) == Some("conditional") {
        let mut updated = orchestration.clone();
        updated["updated_at"] = json!(timestamp);
        state["subagent_orchestration"] = updated;
        state["updated_at"] = json!(timestamp);
        return;
    }
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
                    && receipt.get("status").and_then(Value::as_str) == Some("completed")
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

fn clear_in_flight_for_identity(state: &mut Value, payload: &Value) {
    let mut in_flight = state
        .get("subagent_in_flight")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    in_flight.retain(|entry| !same_subagent_identity(entry, payload));
    state["subagent_in_flight"] = json!(in_flight);
}

fn assign_in_flight_role(state: &mut Value, role: &str, payload: &Value, orchestration: &Value) {
    let Some(in_flight) = state
        .get_mut("subagent_in_flight")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for entry in in_flight {
        if receipt_matches_orchestration(entry, orchestration)
            && same_subagent_identity(entry, payload)
            && entry.get("role").is_none_or(Value::is_null)
        {
            entry["role"] = json!(role);
        }
    }
}

fn is_current_deep_interview_attempt(
    state: &Value,
    role: &str,
    payload: &Value,
    orchestration: &Value,
) -> bool {
    let tracked_start = state
        .get("subagent_in_flight")
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                receipt_matches_orchestration(entry, orchestration)
                    && entry.get("role").and_then(Value::as_str) == Some(role)
                    && same_subagent_identity(entry, payload)
            })
        });
    tracked_start
        || subagent_request_id_from_transcript(payload).as_deref()
            == orchestration.get("request_id").and_then(Value::as_str)
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
    if lower.contains("researcher") || lower.contains("research") {
        return Some("researcher");
    }
    if lower.contains("contrarian") {
        return Some("contrarian");
    }
    if lower.contains("simplifier") || lower.contains("simplify") {
        return Some("simplifier");
    }
    if lower.contains("executor") || lower.contains("delivery") {
        return Some("executor");
    }
    None
}

fn receipt_status(payload: &Value) -> &'static str {
    let text = ["status", "reason", "error"]
        .into_iter()
        .filter_map(|key| payload.get(key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if text.contains("cancel") || text.contains("interrupt") {
        "cancelled"
    } else if text.contains("fail")
        || text.contains("error")
        || payload
            .get("exit_code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0)
    {
        "failed"
    } else {
        "completed"
    }
}

fn compact_finding(payload: &Value) -> Value {
    let text = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    let hint = text.chars().take(240).collect::<String>();
    json!({
        "severity": if text.to_ascii_lowercase().contains("block") { "block" } else if text.to_ascii_lowercase().contains("risk") { "risk" } else { "simplify" },
        "question_hint": hint,
        "rationales": [],
    })
}

fn attempt_id(payload: &Value, timestamp: &str) -> String {
    ["agent_id", "subagent_id", "turn_id"]
        .into_iter()
        .find_map(|key| payload.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| format!("attempt:{timestamp}"))
}

fn next_receipt_sequence(state: &Value) -> usize {
    state
        .get("subagent_audit")
        .and_then(Value::as_array)
        .map_or(1, |audit| audit.len() + 1)
}

fn append_subagent_audit(state: &mut Value, entry: Value) {
    let mut audit = state
        .get("subagent_audit")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    audit.push(entry);
    state["subagent_audit"] = json!(audit);
}
