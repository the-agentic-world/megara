use serde::Serialize;
use serde_json::Value;

use super::request::{validate_raw_wire_shape, LogicalRequest, ProtocolError};

pub const MAX_JSONL_FRAME_BYTES: usize = 4 * 1024 * 1024;

pub fn decode_jsonl_frame(frame: &[u8]) -> Result<LogicalRequest, ProtocolError> {
    if frame.len() > MAX_JSONL_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    prescan_depth(frame)?;
    let text = std::str::from_utf8(frame).map_err(|_| ProtocolError::InvalidUtf8)?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let text = text.strip_suffix('\r').unwrap_or(text);
    if text.contains('\n') || text.contains('\r') {
        return Err(ProtocolError::InvalidRequest(
            "JSONL frame must contain one line".to_string(),
        ));
    }
    let raw: Value = serde_json::from_str(text)
        .map_err(|error| ProtocolError::InvalidJson(error.to_string()))?;
    if !raw.is_object() {
        return Err(ProtocolError::InvalidRequest(
            "request must be a JSON object".to_string(),
        ));
    }
    validate_wire_value(&raw)?;
    validate_raw_wire_shape(raw.as_object().expect("request object checked above"))?;
    let request: LogicalRequest = serde_json::from_value(raw)
        .map_err(|error| ProtocolError::InvalidJson(error.to_string()))?;
    request.validate()?;
    Ok(request)
}

pub(crate) fn validate_wire_value(value: &Value) -> Result<(), ProtocolError> {
    let mut operation_count = 0;
    validate_wire_limits(value, 0, None, &mut operation_count)
}

pub fn encode_jsonl<T: Serialize>(value: &T) -> Result<String, ProtocolError> {
    serde_json::to_string(value)
        .map(|json| format!("{json}\n"))
        .map_err(|error| ProtocolError::InvalidJson(error.to_string()))
}

pub(crate) fn validate_wire_limits(
    value: &Value,
    depth: usize,
    key: Option<&str>,
    operation_count: &mut usize,
) -> Result<(), ProtocolError> {
    if depth > 64 && matches!(value, Value::Object(_) | Value::Array(_)) {
        return Err(ProtocolError::InvalidRequest(
            "JSON nesting exceeds depth 64".to_string(),
        ));
    }
    match value {
        Value::Object(object) => {
            for (name, child) in object {
                validate_wire_limits(child, depth + 1, Some(name), operation_count)?;
            }
        }
        Value::Array(values) => {
            if matches!(
                key,
                Some("entity_ops" | "edge_ops" | "blocker_ops" | "operations" | "citations")
            ) {
                *operation_count = operation_count.saturating_add(values.len());
                if *operation_count > 10_000 {
                    return Err(ProtocolError::InvalidRequest(
                        "operation count exceeds 10000".to_string(),
                    ));
                }
            }
            for child in values {
                validate_wire_limits(child, depth + 1, key, operation_count)?;
            }
        }
        Value::String(text) => {
            let max = if key.is_some_and(is_path_field) {
                4 * 1024
            } else if key.is_some_and(is_id_field) {
                128
            } else {
                64 * 1024
            };
            if text.len() > max {
                return Err(ProtocolError::InvalidRequest(format!(
                    "field exceeds {} bytes",
                    max
                )));
            }
            if key.is_some_and(is_id_field) && text.trim().is_empty() {
                return Err(ProtocolError::InvalidRequest(
                    "identifier must not be blank".to_string(),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_id_field(key: &str) -> bool {
    key == "id"
        || key == "request_id"
        || key == "command_id"
        || key == "session_id"
        || key == "question_id"
        || key == "candidate_id"
        || key == "temp_ref"
        || key.ends_with("_id")
        || key.ends_with("_ids")
}

fn is_path_field(key: &str) -> bool {
    matches!(
        key,
        "path" | "out" | "change_surface" | "cited_paths" | "paths"
    )
}

fn prescan_depth(frame: &[u8]) -> Result<(), ProtocolError> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in frame {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > 64 {
                    return Err(ProtocolError::InvalidRequest(
                        "JSON nesting exceeds depth 64".to_string(),
                    ));
                }
            }
            b'}' | b']' => {
                if depth == 0 {
                    return Err(ProtocolError::InvalidJson(
                        "closing delimiter without opening delimiter".to_string(),
                    ));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    Ok(())
}
