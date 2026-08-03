use std::fs::File;
use std::io::{self, Read};

use serde_json::Value;

use crate::planning::protocol::{validate_wire_value, ProtocolError};

pub(crate) fn read_json_input(source: &str, limit: usize) -> Result<Value, ProtocolError> {
    let bytes = read_source_bytes(source, limit + 1)
        .map_err(|error| ProtocolError::Io(error.to_string()))?;
    if bytes.len() > limit {
        return Err(ProtocolError::FrameTooLarge);
    }
    let text = String::from_utf8(bytes).map_err(|_| ProtocolError::InvalidUtf8)?;
    let value = serde_json::from_str(&text)
        .map_err(|error| ProtocolError::InvalidJson(error.to_string()))?;
    validate_wire_value(&value)?;
    Ok(value)
}

fn read_source_bytes(source: &str, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if source == "-" {
        io::stdin().take(limit as u64).read_to_end(&mut bytes)?;
    } else {
        File::open(source)?
            .take(limit as u64)
            .read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

pub(crate) fn read_text_input(source: &str, limit: usize) -> Result<String, ProtocolError> {
    let bytes = read_source_bytes(source, limit + 1)
        .map_err(|error| ProtocolError::Io(error.to_string()))?;
    if bytes.len() > limit {
        return Err(ProtocolError::InvalidRequest(format!(
            "input exceeds {limit} bytes"
        )));
    }
    String::from_utf8(bytes).map_err(|_| ProtocolError::InvalidUtf8)
}
