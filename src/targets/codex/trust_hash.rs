use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub(super) fn command_hook_hash(
    event_label: &str,
    matcher: Option<&str>,
    handler: &Value,
) -> String {
    let timeout = handler
        .get("timeout")
        .and_then(Value::as_i64)
        .unwrap_or(600)
        .max(1);
    let command = handler
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut normalized_handler = serde_json::Map::new();
    normalized_handler.insert("type".to_string(), json!("command"));
    normalized_handler.insert("command".to_string(), json!(command));
    normalized_handler.insert("timeout".to_string(), json!(timeout));
    normalized_handler.insert("async".to_string(), json!(false));
    if let Some(status_message) = handler.get("statusMessage").and_then(Value::as_str) {
        normalized_handler.insert("statusMessage".to_string(), json!(status_message));
    }

    let mut identity = serde_json::Map::new();
    identity.insert("event_name".to_string(), json!(event_label));
    identity.insert(
        "hooks".to_string(),
        Value::Array(vec![Value::Object(normalized_handler)]),
    );
    if let Some(matcher) = matcher {
        identity.insert("matcher".to_string(), json!(matcher));
    }

    let canonical = canonical_json(&Value::Object(identity));
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let items = items.iter().map(canonical_json).collect::<Vec<_>>();
            format!("[{}]", items.join(","))
        }
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let fields = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("json key is serializable"),
                        canonical_json(&object[key])
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", fields.join(","))
        }
        _ => serde_json::to_string(value).expect("json value is serializable"),
    }
}
