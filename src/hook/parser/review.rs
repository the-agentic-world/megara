use super::{block_list, parse_blocks, Block};

pub(crate) struct ReviewPass {
    pub(crate) role: String,
    pub(crate) round: i64,
    pub(crate) verdict: String,
    pub(crate) summary: String,
    pub(crate) required_fixes: Vec<String>,
}

pub(crate) fn review_passes_from_text(text: &str) -> Vec<ReviewPass> {
    parse_blocks(text, "Megara Review Pass:")
        .into_iter()
        .filter_map(review_pass_from_block)
        .collect()
}

fn review_pass_from_block(block: Block) -> Option<ReviewPass> {
    let role = normalize_review_role(block.fields.get("role")?.trim())?;
    let verdict = normalize_review_verdict(block.fields.get("verdict")?.trim())?;
    let summary = block.fields.get("summary")?.trim();
    if role.is_empty() || verdict.is_empty() || summary.is_empty() {
        return None;
    }
    let round = block
        .fields
        .get("round")
        .and_then(|round| round.trim().parse::<i64>().ok())
        .filter(|round| *round > 0)?;
    Some(ReviewPass {
        role,
        round,
        verdict,
        summary: summary.to_string(),
        required_fixes: default_required_fixes(block_list(&block, "required_fixes")),
    })
}

fn default_required_fixes(fixes: Vec<String>) -> Vec<String> {
    if fixes.is_empty() {
        vec!["none".to_string()]
    } else {
        fixes
    }
}

fn normalize_review_role(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "planner" | "architect" | "critic").then_some(normalized)
}

fn normalize_review_verdict(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_uppercase();
    matches!(
        normalized.as_str(),
        "DRAFT" | "CLEAR" | "WATCH" | "BLOCK" | "OKAY" | "ITERATE" | "REJECT"
    )
    .then_some(normalized)
}
