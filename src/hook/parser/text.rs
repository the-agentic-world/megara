use super::{plan_gate_metadata_from_text, workflow_state_metadata_from_text};

pub(crate) fn text_before_block(text: &str, marker: &str) -> String {
    for (index, line) in text.lines().enumerate() {
        if line.trim() == marker {
            return text
                .lines()
                .take(index)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
        }
    }
    text.trim().to_string()
}

pub(crate) fn text_before_first_workflow_block(text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() == "Megara Plan Gate:" && marker_has_immediate_fields(&lines, index) {
            let tail = lines[index..].join("\n");
            if plan_gate_metadata_from_text(&tail).is_some()
                && workflow_state_metadata_from_text(&tail).is_some()
            {
                end = metadata_body_end(&lines, index);
                break;
            }
        }
    }
    lines
        .into_iter()
        .take(end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn metadata_body_end(lines: &[&str], marker_index: usize) -> usize {
    let mut index = marker_index;
    while index > 0 && lines[index - 1].trim().is_empty() {
        index -= 1;
    }
    if index > 0 && lines[index - 1].trim() == "<!--" {
        index - 1
    } else {
        marker_index
    }
}

fn marker_has_immediate_fields(lines: &[&str], marker_index: usize) -> bool {
    let mut index = marker_index + 1;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }
    lines
        .get(index)
        .is_some_and(|line| line.trim_start().starts_with("- "))
}
