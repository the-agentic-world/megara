use super::{block_list, parse_block, question::parse_bool};

pub(crate) struct ApprovalGate {
    pub(crate) plan_id: String,
    pub(crate) plan_sha256: String,
    pub(crate) handoff_target: String,
}

pub(crate) struct PlanGate {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) question: String,
    pub(crate) options: Vec<String>,
    pub(crate) free_text: bool,
}

pub(crate) fn plan_gate_from_text(text: &str) -> Option<PlanGate> {
    let block = parse_block(text, "Megara Plan Gate:")?;
    let id = block.fields.get("id")?.trim();
    if id.is_empty() {
        return None;
    }
    Some(PlanGate {
        id: id.to_string(),
        status: block
            .fields
            .get("status")
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "pending_approval".to_string()),
        question: block
            .fields
            .get("question")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        options: block_list(&block, "options"),
        free_text: parse_bool(
            block
                .fields
                .get("free_text")
                .map(String::as_str)
                .unwrap_or("false"),
        ),
    })
}

pub(crate) fn approval_gate_from_text(text: &str) -> Option<ApprovalGate> {
    let block = parse_block(text, "Megara Approval Gate:")?;
    let plan_id = block.fields.get("plan_id")?.trim();
    let plan_sha256 = block.fields.get("plan_sha256")?.trim();
    let handoff_target = block.fields.get("handoff_target")?.trim();
    if plan_id.is_empty() || plan_sha256.len() != 64 || handoff_target.is_empty() {
        return None;
    }
    Some(ApprovalGate {
        plan_id: plan_id.to_string(),
        plan_sha256: plan_sha256.to_string(),
        handoff_target: handoff_target.to_ascii_lowercase(),
    })
}
