use serde_json::Value;

pub(super) fn tool_names(payload: &Value) -> Vec<String> {
    let mut names = Vec::new();
    collect_tool_names(payload, &mut names);
    if let Some(tool_input) = payload.get("tool_input") {
        collect_tool_names(tool_input, &mut names);
    }
    names
}

fn collect_tool_names(value: &Value, names: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };
    for key in ["tool_name", "toolName", "tool", "name"] {
        match object.get(key) {
            Some(Value::String(name)) => names.push(normalized_tool_name(name)),
            Some(Value::Object(nested)) => {
                if let Some(Value::String(name)) = nested.get("name") {
                    names.push(normalized_tool_name(name));
                }
            }
            _ => {}
        }
    }
}

fn normalized_tool_name(name: &str) -> String {
    name.rsplit('.')
        .next()
        .unwrap_or(name)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
