use std::collections::BTreeMap;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub(crate) fn canonical_hash<T: serde::Serialize>(value: &T) -> String {
    canonical_hash_with_aliases(value, None)
}

pub(crate) fn canonical_hash_with_aliases<T: serde::Serialize>(
    value: &T,
    aliases: Option<&BTreeMap<String, String>>,
) -> String {
    let value = serde_json::to_value(value).expect("canonical serialization is infallible");
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&canonical_value(&value, None, aliases))
            .expect("canonical JSON serialization is infallible"),
    );
    format!("sha256:{:x}", hasher.finalize())
}

pub(crate) fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&canonical_value(value, None, None))
        .expect("canonical JSON serialization is infallible")
}

fn canonical_value(
    value: &Value,
    key: Option<&str>,
    aliases: Option<&BTreeMap<String, String>>,
) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = BTreeMap::new();
            for (name, child) in object {
                if name == "captured_at" {
                    continue;
                }
                let normalized_name = aliases
                    .and_then(|map| map.get(name))
                    .cloned()
                    .unwrap_or_else(|| normalize_text(name));
                sorted.insert(normalized_name, canonical_value(child, Some(name), aliases));
            }
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(values) => {
            let mut normalized = values
                .iter()
                .map(|child| canonical_value(child, key, aliases))
                .collect::<Vec<_>>();
            if is_set_array(key) {
                normalized.sort_by(|left, right| {
                    serde_json::to_string(left)
                        .expect("canonical JSON serialization is infallible")
                        .cmp(
                            &serde_json::to_string(right)
                                .expect("canonical JSON serialization is infallible"),
                        )
                });
            }
            Value::Array(normalized)
        }
        Value::String(text) => Value::String(
            aliases
                .and_then(|map| map.get(text))
                .cloned()
                .unwrap_or_else(|| normalize_text(text)),
        ),
        other => other.clone(),
    }
}

fn is_set_array(key: Option<&str>) -> bool {
    matches!(
        key,
        Some(
            "selected_choice_ids"
                | "source_refs"
                | "entity_refs"
                | "autonomous_scope"
                | "requires_user_approval"
                | "change_surface"
                | "requirement_refs"
                | "verification_refs"
                | "dependencies"
                | "edges"
        )
    ) || key.is_some_and(|name| name.ends_with("_ids") || name.ends_with("_refs"))
}

fn normalize_text(text: &str) -> String {
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect::<String>();
    let lines = normalized
        .lines()
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>();
    let first = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(first, |index| index + 1);
    lines[first..last].join("\n")
}
