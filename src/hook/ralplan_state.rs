use super::*;
use super::{artifacts::PersistedPlan, parser::PlanGate, ralplan_input::LinkedSpec};

pub(super) struct PersistInput<'a> {
    pub(super) timestamp: &'a str,
    pub(super) payload_file: &'a Path,
    pub(super) paths: &'a WorkflowPaths,
    pub(super) terminal: &'a TerminalState,
    pub(super) plan_gate: Option<PlanGate>,
    pub(super) linked_spec: Option<LinkedSpec>,
    pub(super) plan: Option<PersistedPlan>,
    pub(super) plan_id: String,
}

pub(super) fn persist(input: PersistInput<'_>, state: &mut Value) -> Result<()> {
    let PersistInput {
        timestamp,
        payload_file,
        paths,
        terminal,
        plan_gate,
        linked_spec,
        plan,
        plan_id,
    } = input;

    update_terminal_state(timestamp, state, terminal, None);
    state["plan_id"] = json!(plan_id);
    if terminal.status == "pending_approval" {
        state["active"] = json!(true);
        state["phase"] = json!("pending_approval");
        state["approval_status"] = json!("pending");
    }
    if let Some(gate) = plan_gate {
        state["plan_gate"] = json!({
            "id": gate.id,
            "status": gate.status,
            "question": gate.question,
            "options": gate.options,
            "free_text": gate.free_text,
        });
    }
    if let Some(spec) = &linked_spec {
        state["input_spec_path"] = json!(spec.path);
        state["input_spec_sha256"] = json!(spec.sha256);
        state["input_spec_persisted_at"] = json!(spec.persisted_at);
    }

    let mut entry = workflow_entry(timestamp, payload_file, paths, terminal, &plan_id);
    if let Some(spec) = &linked_spec {
        entry["input_spec_path"] = json!(spec.path);
        entry["input_spec_sha256"] = json!(spec.sha256);
    }
    if let Some(plan) = plan {
        let record = PlanRecord {
            timestamp,
            payload_file,
            paths,
            linked_spec: &linked_spec,
            plan_id: &plan_id,
        };
        record_plan(state, &mut entry, record, plan)?;
    }
    append_jsonl(&paths.events_file, &entry)
}

fn workflow_entry(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    plan_id: &str,
) -> Value {
    json!({
        "timestamp": timestamp,
        "event": "workflow_state",
        "session_id": paths.session_id,
        "skill": terminal.skill,
        "status": terminal.status,
        "plan_id": plan_id,
        "payload": payload_file,
    })
}

struct PlanRecord<'a> {
    timestamp: &'a str,
    payload_file: &'a Path,
    paths: &'a WorkflowPaths,
    linked_spec: &'a Option<LinkedSpec>,
    plan_id: &'a str,
}

fn record_plan(
    state: &mut Value,
    entry: &mut Value,
    record: PlanRecord<'_>,
    plan: PersistedPlan,
) -> Result<()> {
    state["plan_path"] = json!(plan.path);
    state["plan_sha256"] = json!(plan.sha256);
    state["plan_persisted_at"] = json!(plan.persisted_at);
    state["plan_payload"] = json!(plan.payload);
    entry["plan_path"] = json!(plan.path);
    entry["plan_sha256"] = json!(plan.sha256);
    append_jsonl(
        &record.paths.events_file,
        &json!({
            "timestamp": record.timestamp,
            "event": "plan_persisted",
            "session_id": record.paths.session_id,
            "plan_id": record.plan_id,
            "path": plan.path,
            "sha256": plan.sha256,
            "input_spec_path": record.linked_spec.as_ref().map(|spec| spec.path.as_str()),
            "input_spec_sha256": record.linked_spec.as_ref().map(|spec| spec.sha256.as_str()),
            "payload": record.payload_file,
        }),
    )
}
