use super::*;

pub(super) fn handle_ralplan_terminal(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    let plan_gate = plan_gate_from_text(text);
    let plan_id = terminal
        .plan_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| plan_gate.as_ref().map(|gate| gate.id.as_str()))
        .unwrap_or("rp-plan")
        .to_string();

    if terminal.status == "pending_approval" {
        if let Some(blocker) = ralplan_input::active_deep_interview_state(paths) {
            reject_ralplan_handoff_not_ready(timestamp, state, &plan_id, &blocker);
            append_jsonl(
                &paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "handoff_blocked",
                    "session_id": paths.session_id,
                    "plan_id": plan_id,
                    "blocked_by": DEEP_INTERVIEW,
                    "blocked_phase": blocker.get("phase").cloned().unwrap_or(Value::Null),
                    "blocked_status": blocker.get("status").cloned().unwrap_or(Value::Null),
                    "payload": payload_file,
                }),
            )?;
            return Ok(());
        }
    }

    let linked_spec = ralplan_input::linked_deep_interview_spec(paths);
    if terminal.status == "pending_approval" {
        if let Some(reason) =
            ralplan_input::ralplan_input_lock_blocker(state, linked_spec.as_ref(), text)
        {
            reject_ralplan_input_lock(timestamp, state, &plan_id, reason);
            append_jsonl(
                &paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "input_lock_blocked",
                    "session_id": paths.session_id,
                    "plan_id": plan_id,
                    "reason": reason,
                    "payload": payload_file,
                }),
            )?;
            return Ok(());
        }
    }

    if terminal.status == "pending_approval" {
        ralplan_reviews::infer_ready_from_visible_plan(timestamp, payload_file, state, text);
    }

    if terminal.status == "pending_approval" && !ralplan_reviews::ready(state) {
        reject_ralplan_without_reviews(timestamp, state, &plan_id);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "review_incomplete",
                "session_id": paths.session_id,
                "plan_id": plan_id,
                "status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        return Ok(());
    }

    let plan = persist_pending_plan(
        timestamp,
        paths,
        &plan_id,
        terminal,
        text,
        payload_file,
        linked_spec.as_ref(),
    )?;
    if terminal.status == "pending_approval" && plan.is_none() {
        reject_ralplan_without_plan(timestamp, state, &plan_id);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "plan_missing",
                "session_id": paths.session_id,
                "plan_id": plan_id,
                "status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        return Ok(());
    }

    ralplan_state::persist(
        ralplan_state::PersistInput {
            timestamp,
            payload_file,
            paths,
            terminal,
            plan_gate,
            linked_spec,
            plan,
            plan_id,
        },
        state,
    )
}
