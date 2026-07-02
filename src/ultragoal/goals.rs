use anyhow::{bail, Result};

pub(crate) struct ParsedGoal {
    pub(crate) title: String,
    pub(crate) objective: String,
}

pub(crate) fn parse_goals(brief: &str) -> Result<Vec<ParsedGoal>> {
    let brief = strip_yaml_frontmatter(brief).trim();
    let mut sections = Vec::<(String, Vec<String>)>::new();
    for line in brief.lines() {
        if let Some(title) = goal_marker(line) {
            sections.push((title, Vec::new()));
        } else if let Some((_, body)) = sections.last_mut() {
            body.push(line.to_string());
        }
    }

    if sections.is_empty() {
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

fn title_from_objective(objective: &str) -> String {
    let first = objective
        .lines()
        .map(normalize_title_line)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| "Complete ultragoal brief".to_string());
    first.chars().take(96).collect()
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
