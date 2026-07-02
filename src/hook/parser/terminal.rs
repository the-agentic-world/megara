use super::parse_block;
use crate::hook::WORKFLOWS;

#[derive(Debug)]
pub(crate) struct TerminalState {
    pub(crate) skill: String,
    pub(crate) status: String,
    pub(crate) ambiguity: String,
    pub(crate) next: String,
    pub(crate) plan_id: Option<String>,
}

pub(crate) fn workflow_state_from_text(text: &str) -> Option<TerminalState> {
    let block = parse_block(text, "Megara Workflow State:")?;
    let skill = block.fields.get("skill")?.trim();
    if !WORKFLOWS.contains(&skill) {
        return None;
    }
    let status = block.fields.get("status")?.trim().to_ascii_lowercase();
    if status.is_empty() {
        return None;
    }
    Some(TerminalState {
        skill: skill.to_string(),
        status,
        ambiguity: block
            .fields
            .get("ambiguity")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        next: block
            .fields
            .get("next")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        plan_id: block
            .fields
            .get("plan_id")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}
