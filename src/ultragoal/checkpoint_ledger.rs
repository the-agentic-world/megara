use super::{model, store::*, *};

pub(super) struct RecordInput<'a> {
    pub(super) paths: &'a UltragoalPaths,
    pub(super) plan: &'a model::UltragoalPlan,
    pub(super) goal_id: &'a str,
    pub(super) status: &'a str,
    pub(super) evidence: &'a str,
    pub(super) receipt: &'a Option<model::CompletionReceipt>,
    pub(super) next_started: &'a Option<Value>,
    pub(super) timestamp: &'a str,
}

pub(super) fn record(input: RecordInput<'_>) -> Result<()> {
    append_ledger(
        input.paths,
        &json!({
            "timestamp": input.timestamp,
            "event": "goal_checkpointed",
            "session_id": input.plan.session_id,
            "goal_id": input.goal_id,
            "status": input.status,
            "evidence": input.evidence,
            "completion_receipt": input.receipt,
            "next_goal_started": input.next_started.clone(),
        }),
    )?;
    if let Some(goal) = input.next_started {
        append_ledger(
            input.paths,
            &json!({
                "timestamp": input.timestamp,
                "event": "goal_started",
                "session_id": input.plan.session_id,
                "goal_id": goal["id"],
                "title": goal["title"],
            }),
        )?;
    }
    Ok(())
}
