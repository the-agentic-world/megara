use anyhow::{bail, Result};

pub(crate) struct ParsedGoal {
    pub(crate) title: String,
    pub(crate) objective: String,
}

pub(crate) fn parse_goals(brief: &str) -> Result<Vec<ParsedGoal>> {
    let brief = strip_approval_tail(&strip_html_comments(strip_yaml_frontmatter(brief)));
    let brief = brief.trim();
    let mut sections = Vec::<(String, Vec<String>)>::new();
    for line in brief.lines() {
        if let Some(title) = goal_marker(line) {
            sections.push((title, Vec::new()));
        } else if let Some((_, body)) = sections.last_mut() {
            body.push(line.to_string());
        }
    }

    if sections.is_empty() {
        if let Some(goals) = parse_steps_section(brief)? {
            return Ok(goals);
        }

        let objective = brief.trim();
        if objective.is_empty() {
            bail!("ultragoal brief has no goal content");
        }
        return Ok(vec![ParsedGoal {
            title: title_from_objective(objective),
            objective: objective.to_string(),
        }]);
    }

    sections
        .into_iter()
        .enumerate()
        .map(|(index, (title, body_lines))| {
            let body = body_lines.join("\n").trim().to_string();
            let title = if title.trim().is_empty() {
                title_from_objective(&body)
            } else {
                title.trim().to_string()
            };
            let objective = if body.is_empty() { title.clone() } else { body };
            if title.trim().is_empty() || objective.trim().is_empty() {
                bail!("ultragoal @goal block {} is empty", index + 1);
            }
            Ok(ParsedGoal { title, objective })
        })
        .collect()
}

fn parse_steps_section(brief: &str) -> Result<Option<Vec<ParsedGoal>>> {
    let lines = brief.lines().collect::<Vec<_>>();
    let Some(start) = lines
        .iter()
        .position(|line| is_steps_section_heading(line))
        .map(|index| index + 1)
    else {
        return Ok(None);
    };

    let mut items = Vec::<Vec<String>>::new();
    let mut current = Vec::<String>::new();
    for line in &lines[start..] {
        if !current.is_empty() && is_section_heading(line) {
            break;
        }
        if let Some(item) = ordered_item_text(line) {
            if !current.is_empty() {
                items.push(std::mem::take(&mut current));
            }
            current.push(item);
        } else if !current.is_empty() {
            current.push((*line).to_string());
        }
    }
    if !current.is_empty() {
        items.push(current);
    }

    if items.len() < 2 {
        return Ok(None);
    }

    items
        .into_iter()
        .enumerate()
        .map(|(index, lines)| {
            let objective = lines.join("\n").trim().to_string();
            if objective.is_empty() {
                bail!("ultragoal step {} is empty", index + 1);
            }
            Ok(ParsedGoal {
                title: title_from_objective(&objective),
                objective,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

fn strip_approval_tail(input: &str) -> String {
    let lines = input.lines().collect::<Vec<_>>();
    let Some(cut_at) = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| approval_question_at(&lines, index, line).then_some(index))
        .next_back()
    else {
        return input.to_string();
    };
    let cut_at = approval_section_start(&lines, cut_at);
    lines[..cut_at].join("\n").trim_end().to_string()
}

fn approval_section_start(lines: &[&str], cut_at: usize) -> usize {
    let mut index = cut_at;
    while index > 0 {
        let previous = lines[index - 1].trim();
        if previous.is_empty() {
            index -= 1;
            continue;
        }
        if is_approval_section_heading(previous) {
            return index - 1;
        }
        break;
    }
    cut_at
}

fn approval_question_at(lines: &[&str], index: usize, line: &str) -> bool {
    let normalized = line.trim().to_ascii_lowercase();
    let looks_like_question = normalized.contains("approve this plan")
        || is_approval_section_heading(line)
        || line.contains("이 계획")
        || line.contains("다음 단계")
        || line.contains("어떻게 할까");
    if !looks_like_question {
        return false;
    }
    let tail = lines[index + 1..].join("\n").to_ascii_lowercase();
    let numbered_choice_count = tail
        .lines()
        .filter(|tail_line| {
            let trimmed = tail_line.trim_start();
            matches!(
                trimmed.as_bytes(),
                [b'1'..=b'4', b'.', ..] | [b'1'..=b'4', b')', ..]
            )
        })
        .count();
    numbered_choice_count >= 2
        && (tail.contains("ultragoal")
            || tail.contains("team")
            || tail.contains("refine")
            || tail.contains("pending")
            || tail.contains("승인")
            || tail.contains("보류")
            || tail.contains("다듬"))
}

fn is_approval_section_heading(line: &str) -> bool {
    let normalized = normalize_title_line(line)
        .trim_end_matches(':')
        .trim()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "approval" | "approval question" | "approval questions" | "승인" | "승인 질문"
    )
}

fn is_steps_section_heading(line: &str) -> bool {
    let normalized = normalize_title_line(line)
        .trim_end_matches(':')
        .trim()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "steps"
            | "execution steps"
            | "implementation steps"
            | "작업 순서"
            | "작업 단계"
            | "실행 단계"
    )
}

fn is_section_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.starts_with('#')
        || (trimmed.starts_with("**") && trimmed.ends_with("**"))
        || is_generic_title_line(&normalize_title_line(trimmed))
}

fn strip_html_comments(input: &str) -> String {
    let mut output = String::new();
    let mut rest = input;
    loop {
        let Some(start) = rest.find("<!--") else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 4..];
        let Some(end) = after_start.find("-->") else {
            break;
        };
        rest = &after_start[end + 3..];
    }
    output
}

fn goal_marker(line: &str) -> Option<String> {
    let rest = line.strip_prefix("@goal")?;
    if rest.is_empty() {
        return Some(String::new());
    }
    let mut chars = rest.chars();
    match chars.next()? {
        ':' => Some(chars.as_str().trim().to_string()),
        ' ' | '\t' => Some(chars.as_str().trim().to_string()),
        _ => None,
    }
}

fn ordered_item_text(line: &str) -> Option<String> {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces > 3 {
        return None;
    }
    let trimmed = line.trim_start();
    let marker_end = trimmed
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .map(|(index, ch)| index + ch.len_utf8())
        .last()?;
    let mut chars = trimmed[marker_end..].chars();
    match chars.next()? {
        '.' | ')' => {
            let rest = chars.as_str().trim();
            (!rest.is_empty()).then(|| rest.to_string())
        }
        _ => None,
    }
}

fn title_from_objective(objective: &str) -> String {
    let first = objective
        .lines()
        .map(normalize_title_line)
        .find(|line| !line.is_empty() && !is_generic_title_line(line))
        .unwrap_or_else(|| "Complete ultragoal brief".to_string());
    compact_goal_title(&first)
}

fn compact_goal_title(title: &str) -> String {
    const MAX_TITLE_CHARS: usize = 96;
    const SUFFIX: &str = "...";

    if let Some(label) = compact_colon_label(title) {
        return label;
    }

    if title.chars().count() <= MAX_TITLE_CHARS {
        return title.to_string();
    }

    let title = title.replace(['`', '"'], "");
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title;
    }

    let prefix = title
        .chars()
        .take(MAX_TITLE_CHARS - SUFFIX.len())
        .collect::<String>();
    let boundary = prefix
        .char_indices()
        .rev()
        .find(|(index, ch)| *index >= 32 && ch.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(prefix.len());
    let mut compact = prefix[..boundary]
        .trim_end_matches(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | ':' | '-'))
        .to_string();
    if compact.is_empty() {
        compact = prefix;
    }
    compact.push_str(SUFFIX);
    compact
}

fn compact_colon_label(title: &str) -> Option<String> {
    let (label, detail) = title.split_once(':')?;
    let label = label.trim();
    let detail = detail.trim();
    if label.is_empty() || detail.is_empty() {
        return None;
    }
    if label.chars().any(|ch| matches!(ch, '`' | '"' | '\'')) {
        return None;
    }
    let label_len = label.chars().count();
    if label_len > 32 || is_generic_title_line(label) {
        return None;
    }
    let detail_len = detail.chars().count();
    let looks_like_execution_detail =
        detail_len >= 24 || detail.contains('`') || detail.contains(',') || detail.contains("한다");
    looks_like_execution_detail.then(|| label.to_string())
}

fn is_generic_title_line(line: &str) -> bool {
    let normalized = line.trim_end_matches(':').trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "pending execution plan"
            | "execution plan"
            | "plan"
            | "summary"
            | "summary and goal"
            | "scope"
            | "decisions"
            | "steps"
            | "acceptance criteria"
            | "risks"
            | "실행 계획"
            | "계획"
            | "요약"
            | "요약과 목표"
            | "범위"
            | "결정 사항"
            | "작업 순서"
            | "인수 기준"
            | "승인 기준"
            | "위험과 대응"
    )
}

fn normalize_title_line(line: &str) -> String {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
        .to_string()
}

fn strip_yaml_frontmatter(input: &str) -> &str {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let Some(after_open) = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))
    else {
        return input;
    };

    let mut cursor = input.len() - after_open.len();
    for line in after_open.split_inclusive('\n') {
        cursor += line.len();
        if line.trim() == "---" {
            return &input[cursor..];
        }
    }
    input
}
