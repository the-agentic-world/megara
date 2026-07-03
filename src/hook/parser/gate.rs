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
    plan_gate_metadata_from_text(text).or_else(|| plan_gate_from_visible_text(text))
}

pub(crate) fn plan_gate_metadata_from_text(text: &str) -> Option<PlanGate> {
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

fn plan_gate_from_visible_text(text: &str) -> Option<PlanGate> {
    let options = visible_options(text);
    if options.len() < 2 || !looks_like_approval_options(&options) {
        return None;
    }
    Some(PlanGate {
        id: "rp-plan".to_string(),
        status: "pending_approval".to_string(),
        question: visible_approval_question(text)
            .unwrap_or_else(|| "Approve this plan?".to_string()),
        options,
        free_text: false,
    })
}

fn looks_like_approval_options(options: &[String]) -> bool {
    let joined = options.join("\n").to_ascii_lowercase();
    (joined.contains("ultragoal") || joined.contains("team") || joined.contains("팀"))
        && (joined.contains("approve") || joined.contains("승인"))
}

fn visible_approval_question(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .find(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with(|character: char| character.is_ascii_digit())
                && (line.ends_with('?') || line.ends_with('？'))
        })
        .map(|line| line.trim().to_string())
}

fn visible_options(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| visible_option_text(line.trim()))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn visible_option_text(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| numbered_option_text(line))
}

fn numbered_option_text(line: &str) -> Option<&str> {
    let split_at = line
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .last()
        .map(|(index, ch)| index + ch.len_utf8())?;
    let rest = line.get(split_at..)?;
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    rest.strip_prefix(' ')
}
