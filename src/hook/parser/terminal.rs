use super::parse_block;
use crate::hook::{DEEP_INTERVIEW, RALPLAN, WORKFLOWS};

#[derive(Debug)]
pub(crate) struct TerminalState {
    pub(crate) skill: String,
    pub(crate) status: String,
    pub(crate) ambiguity: String,
    pub(crate) next: String,
    pub(crate) plan_id: Option<String>,
}

pub(crate) fn workflow_state_from_text(text: &str) -> Option<TerminalState> {
    workflow_state_metadata_from_text(text).or_else(|| workflow_state_from_visible_text(text))
}

pub(crate) fn workflow_state_metadata_from_text(text: &str) -> Option<TerminalState> {
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

fn workflow_state_from_visible_text(text: &str) -> Option<TerminalState> {
    if looks_like_visible_ralplan_pending(text) {
        return Some(TerminalState {
            skill: RALPLAN.to_string(),
            status: "pending_approval".to_string(),
            ambiguity: String::new(),
            next: "approval".to_string(),
            plan_id: None,
        });
    }

    if looks_like_visible_deep_interview_crystallized(text) {
        return Some(TerminalState {
            skill: DEEP_INTERVIEW.to_string(),
            status: "crystallized".to_string(),
            ambiguity: extract_ambiguity(text).unwrap_or_default(),
            next: RALPLAN.to_string(),
            plan_id: None,
        });
    }

    None
}

fn looks_like_visible_ralplan_pending(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_execution_choice =
        lower.contains("ultragoal") || lower.contains("team") || text.contains("팀");
    let has_approval =
        lower.contains("approve") || lower.contains("approval") || text.contains("승인");
    has_execution_choice
        && has_approval
        && visible_section_score(text) >= 3
        && has_numbered_options(text)
}

fn looks_like_visible_deep_interview_crystallized(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_pending_question = text
        .lines()
        .rev()
        .take(10)
        .any(|line| line.trim_end().ends_with('?') || line.trim_end().ends_with('？'));
    lower.contains("ralplan") && visible_section_score(text) >= 3 && !has_pending_question
}

fn visible_section_score(text: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    [
        lower.contains("goal") || text.contains("목표"),
        lower.contains("scope") || text.contains("범위"),
        lower.contains("decision") || text.contains("결정"),
        lower.contains("acceptance") || text.contains("수용") || text.contains("인수"),
        lower.contains("constraint")
            || lower.contains("risk")
            || text.contains("제약")
            || text.contains("리스크")
            || text.contains("위험"),
        lower.contains("next") || text.contains("다음"),
        lower.contains("verification") || text.contains("검증"),
    ]
    .into_iter()
    .filter(|matched| *matched)
    .count()
}

fn has_numbered_options(text: &str) -> bool {
    text.lines()
        .filter(|line| numbered_option_text(line.trim()).is_some())
        .count()
        >= 2
}

fn extract_ambiguity(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("ambiguity") || line.contains("모호성")
        })
        .find_map(percent_from_line)
}

fn percent_from_line(line: &str) -> Option<String> {
    let percent_index = line.find('%')?;
    let before = &line[..percent_index];
    let digits = before
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    (!digits.is_empty()).then(|| format!("{digits}%"))
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
