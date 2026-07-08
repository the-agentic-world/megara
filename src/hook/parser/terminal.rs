use super::parse_block;
use crate::hook::{DEEP_INTERVIEW, RALPLAN, TEAM, WORKFLOWS};

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

    if looks_like_visible_team_complete(text) {
        return Some(TerminalState {
            skill: TEAM.to_string(),
            status: "complete".to_string(),
            ambiguity: String::new(),
            next: String::new(),
            plan_id: None,
        });
    }

    None
}

fn looks_like_visible_ralplan_pending(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_approval =
        lower.contains("approve") || lower.contains("approval") || text.contains("승인");
    let options = visible_options(text);
    has_approval
        && looks_like_ralplan_approval_options(&options)
        && visible_section_score(text) >= 3
}

fn looks_like_ralplan_approval_options(options: &[String]) -> bool {
    if options.len() < 3 {
        return false;
    }
    let joined = options.join("\n").to_ascii_lowercase();
    let has_ultragoal = joined.contains("ultragoal");
    let has_team = joined.contains("team") || options.iter().any(|option| option.contains("팀"));
    let has_refine = joined.contains("refine")
        || options
            .iter()
            .any(|option| option.contains("보완") || option.contains("수정"));
    let has_pending = joined.contains("pending")
        || options
            .iter()
            .any(|option| option.contains("보류") || option.contains("대기"));
    let has_approval = joined.contains("approve")
        || joined.contains("approval")
        || options.iter().any(|option| option.contains("승인"));
    has_ultragoal && has_team && has_approval && (has_refine || has_pending)
}

fn visible_options(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| visible_option_text(line.trim()))
        .map(|option| option.trim().to_string())
        .filter(|option| !option.is_empty())
        .collect()
}

fn visible_option_text(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| numbered_option_text(line))
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

fn looks_like_visible_team_complete(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let has_team = lower.contains("team")
        || lower.contains("teammate")
        || text.contains("팀")
        || text.contains("팀메이트");
    let has_synthesis = lower.contains("synthesis")
        || lower.contains("integration")
        || text.contains("합성")
        || text.contains("통합");
    let has_verification =
        lower.contains("verification") || text.contains("검증") || text.contains("확인");
    let has_completion =
        lower.contains("complete") || lower.contains("done") || text.contains("완료");
    has_team && has_synthesis && has_verification && has_completion
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

fn extract_ambiguity(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("ambiguity") || line.contains("모호성") || line.contains("모호도")
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
