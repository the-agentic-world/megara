use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Block {
    pub(crate) fields: BTreeMap<String, String>,
    lists: BTreeMap<String, Vec<String>>,
}

pub(crate) fn parse_block(text: &str, marker: &str) -> Option<Block> {
    parse_blocks(text, marker).into_iter().next()
}

pub(crate) fn parse_blocks(text: &str, marker: &str) -> Vec<Block> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        if lines[index].trim() != marker {
            index += 1;
            continue;
        }
        index += 1;
        let mut fields = BTreeMap::new();
        let mut lists = BTreeMap::<String, Vec<String>>::new();
        let mut current_key = String::new();
        let mut saw_field = false;

        while index < lines.len() {
            let raw = lines[index];
            if raw.trim().is_empty() {
                index += 1;
                if saw_field {
                    break;
                }
                continue;
            }
            if (raw.starts_with("  - ") || raw.starts_with("    - ")) && !current_key.is_empty() {
                if let Some((_, value)) = raw.split_once("- ") {
                    lists
                        .entry(current_key.clone())
                        .or_default()
                        .push(clean_block_value(value));
                }
                index += 1;
                continue;
            }

            let stripped = raw.trim();
            if !stripped.starts_with("- ") {
                break;
            }
            let Some((key, value)) = stripped[2..].split_once(':') else {
                index += 1;
                continue;
            };
            let key = key.trim().to_ascii_lowercase().replace('-', "_");
            current_key = key.clone();
            saw_field = true;
            if value.trim().is_empty() {
                lists.entry(key).or_default();
            } else {
                fields.insert(key, clean_block_value(value));
            }
            index += 1;
        }
        if saw_field {
            blocks.push(Block { fields, lists });
        }
    }
    blocks
}

pub(crate) fn block_list(block: &Block, key: &str) -> Vec<String> {
    block
        .lists
        .get(key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect()
}

fn clean_block_value(value: &str) -> String {
    let mut value = value.trim();
    while let Some(stripped) = value
        .strip_suffix("</input>")
        .or_else(|| value.strip_suffix("</codex_delegation>"))
    {
        value = stripped.trim_end();
    }
    value.to_string()
}
