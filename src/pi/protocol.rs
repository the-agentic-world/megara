use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PiEventRequest {
    pub protocol_version: u32,
    pub action: PiAction,
    pub event_id: String,
    pub workflow: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PiAction {
    Activate,
    NextAction,
    PrepareAttempt,
    AttemptFinished,
    Shutdown,
}

#[derive(Clone, Debug, Serialize)]
pub struct PiEventResponse {
    pub status: String,
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required_roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PiEventResponse {
    pub fn new(status: impl Into<String>, event_id: &str) -> Self {
        Self {
            status: status.into(),
            event_id: event_id.to_string(),
            attempt_id: None,
            required_roles: Vec::new(),
            retry_after_ms: None,
            model: None,
            output: None,
            message: None,
        }
    }
}
