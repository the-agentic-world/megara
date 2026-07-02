use super::trust::HookTrustState;

pub(super) fn merge_hook_trust_state(
    existing: &str,
    states: &[HookTrustState],
) -> (String, usize, usize) {
    let mut content = existing.to_string();
    let mut registered = 0;
    let mut unchanged = 0;

    if !states.is_empty() && !content.contains("[hooks.state]") {
        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }
        content.push_str("\n[hooks.state]\n");
    }

    for state in states {
        let header = hook_state_header(&state.key);
        if let Some((start, end)) = find_toml_section(&content, &header) {
            let section = &content[start..end];
            let desired = format!(
                "trusted_hash = \"{}\"",
                escape_toml_basic_string(&state.trusted_hash)
            );
            if section.lines().any(|line| line.trim() == desired) {
                unchanged += 1;
                continue;
            }
            let next_section = replace_or_insert_trusted_hash(section, &desired);
            content.replace_range(start..end, &next_section);
            registered += 1;
        } else {
            append_trusted_hash(&mut content, &header, &state.trusted_hash);
            registered += 1;
        }
    }

    (content, registered, unchanged)
}

fn append_trusted_hash(content: &mut String, header: &str, trusted_hash: &str) {
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push('\n');
    content.push_str(header);
    content.push('\n');
    content.push_str(&format!(
        "trusted_hash = \"{}\"\n",
        escape_toml_basic_string(trusted_hash)
    ));
}

fn hook_state_header(key: &str) -> String {
    format!("[hooks.state.\"{}\"]", escape_toml_basic_string(key))
}

fn escape_toml_basic_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn find_toml_section(content: &str, header: &str) -> Option<(usize, usize)> {
    let mut position = 0;
    let mut found_start = None;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(start) = found_start {
                return Some((start, position));
            }
            if trimmed == header {
                found_start = Some(position);
            }
        }
        position += line.len();
    }
    found_start.map(|start| (start, content.len()))
}

fn replace_or_insert_trusted_hash(section: &str, desired: &str) -> String {
    let mut replaced = false;
    let mut next = String::new();
    for line in section.split_inclusive('\n') {
        if line.trim_start().starts_with("trusted_hash") {
            next.push_str(desired);
            next.push('\n');
            replaced = true;
        } else {
            next.push_str(line);
        }
    }
    if !replaced {
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(desired);
        next.push('\n');
    }
    next
}
