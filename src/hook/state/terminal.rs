use std::collections::HashSet;

use serde_json::{json, Value};

use crate::hook::{artifacts::PersistedSpec, parser::TerminalState};

pub(crate) fn update_terminal_state(
    timestamp: &str,
    state: &mut Value,
    terminal: &TerminalState,
    spec: Option<&PersistedSpec>,
) {
    let terminal_statuses = HashSet::from([
        "approved",
        "crystallized",
        "cancelled",
        "canceled",
        "complete",
        "completed",
        "rejected",
    ]);
    let active = !terminal_statuses.contains(terminal.status.as_str());
    state["active"] = json!(active);
    state["phase"] = json!(terminal.status);
    state["status"] = json!(terminal.status);
    if !terminal.ambiguity.is_empty() {
        state["ambiguity"] = json!(terminal.ambiguity);
    }
    if !terminal.next.is_empty() {
        state["next"] = json!(terminal.next);
    }
    if let Some(plan_id) = &terminal.plan_id {
        state["plan_id"] = json!(plan_id);
    }
    if let Some(spec) = spec {
        state["spec_path"] = json!(spec.path);
        state["spec_sha256"] = json!(spec.sha256);
        state["spec_persisted_at"] = json!(spec.persisted_at);
        state["spec_payload"] = json!(spec.payload);
    }
    if !active {
        state["pending_question"] = Value::Null;
        state["closed_at"] = json!(timestamp);
    }
    state["updated_at"] = json!(timestamp);
}
