use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::limits::validate_wire_value;
use super::params::normalize_typed_params;

pub const PROTOCOL_VERSION: u32 = 1;
pub const RESULT_SCHEMA: &str = "megara.result/v1";
pub const LOGICAL_OPERATIONS: &[&str] = &[
    "planning.start",
    "planning.answer",
    "planning.status",
    "planning.current",
    "planning.list",
    "planning.evidence.refresh",
    "planning.audit.apply",
    "planning.spec.generate",
    "planning.spec.show",
    "planning.spec.approve",
    "planning.spec.revise",
    "planning.plan.generate",
    "planning.plan.show",
    "planning.plan.approve",
    "planning.plan.revise",
    "planning.export",
    "planning.purge",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Mutation,
    Query,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidRequest(String),
    SessionRequired,
    UnsupportedVersion(u32),
    FrameTooLarge,
    InvalidUtf8,
    InvalidJson(String),
    Io(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            Self::SessionRequired => write!(f, "session_id is required"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported protocol version: {version}")
            }
            Self::FrameTooLarge => write!(f, "JSONL frame exceeds 4 MiB"),
            Self::InvalidUtf8 => write!(f, "request is not UTF-8"),
            Self::InvalidJson(message) => write!(f, "invalid JSON: {message}"),
            Self::Io(message) => write!(f, "I/O error: {message}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: String,
    pub command_id: Option<String>,
    pub session_id: Option<String>,
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub params: Option<Value>,
}

pub(crate) fn validate_raw_wire_shape(raw: &Map<String, Value>) -> Result<(), ProtocolError> {
    let operation = raw
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| ProtocolError::InvalidRequest("operation must be a string".to_string()))?;
    let contract = operation_contract(operation)
        .ok_or_else(|| ProtocolError::InvalidRequest(format!("unknown operation: {operation}")))?;
    for field in ["actor", "adapter", "phase", "approval"] {
        if raw.contains_key(field) {
            return Err(ProtocolError::InvalidRequest(format!(
                "forbidden field: {field}"
            )));
        }
    }
    if contract.forbids_session && raw.contains_key("session_id") {
        return Err(ProtocolError::InvalidRequest(
            "forbidden field: session_id".to_string(),
        ));
    }
    if contract.forbids_revision && raw.contains_key("expected_revision") {
        return Err(ProtocolError::InvalidRequest(
            "forbidden field: expected_revision".to_string(),
        ));
    }
    if contract.kind == OperationKind::Query && raw.contains_key("command_id") {
        return Err(ProtocolError::InvalidRequest(
            "forbidden field: command_id".to_string(),
        ));
    }
    Ok(())
}

impl LogicalRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let wire = serde_json::to_value(self)
            .map_err(|error| ProtocolError::InvalidRequest(error.to_string()))?;
        validate_wire_value(&wire)?;
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(self.protocol_version));
        }
        if self.request_id.trim().is_empty() {
            return Err(ProtocolError::InvalidRequest(
                "request_id must not be blank".to_string(),
            ));
        }
        let contract = operation_contract(&self.operation).ok_or_else(|| {
            ProtocolError::InvalidRequest(format!("unknown operation: {}", self.operation))
        })?;
        match contract.kind {
            OperationKind::Mutation if self.command_id.as_deref().is_none_or(str::is_empty) => {
                return Err(ProtocolError::InvalidRequest(
                    "mutation requires command_id".to_string(),
                ));
            }
            OperationKind::Query if self.command_id.is_some() => {
                return Err(ProtocolError::InvalidRequest(
                    "query forbids command_id".to_string(),
                ));
            }
            _ => {}
        }
        if contract.requires_session && self.session_id.as_deref().is_none_or(str::is_empty) {
            return Err(ProtocolError::SessionRequired);
        }
        if contract.forbids_session && self.session_id.is_some() {
            return Err(ProtocolError::InvalidRequest(
                "operation forbids session_id".to_string(),
            ));
        }
        if contract.requires_revision && self.expected_revision.is_none() {
            return Err(ProtocolError::InvalidRequest(
                "operation requires expected_revision".to_string(),
            ));
        }
        if contract.forbids_revision && self.expected_revision.is_some() {
            return Err(ProtocolError::InvalidRequest(
                "operation forbids expected_revision".to_string(),
            ));
        }

        let params = match self.params.as_ref() {
            Some(Value::Object(params)) => params,
            Some(_) => {
                return Err(ProtocolError::InvalidRequest(
                    "params must be a JSON object".to_string(),
                ));
            }
            None if contract.required_params.is_empty() => return Ok(()),
            None => {
                return Err(ProtocolError::InvalidRequest(
                    "operation requires params".to_string(),
                ));
            }
        };
        for required in contract.required_params {
            if !params.get(*required).is_some_and(|value| !value.is_null()) {
                return Err(ProtocolError::InvalidRequest(format!(
                    "missing params.{required}"
                )));
            }
        }
        for key in params.keys() {
            if !contract.optional_params.contains(&key.as_str())
                && !contract.required_params.contains(&key.as_str())
            {
                return Err(ProtocolError::InvalidRequest(format!(
                    "unknown params field: {key}"
                )));
            }
        }
        super::params::validate_typed_params(
            &self.operation,
            typed_params_input(&self.operation, Value::Object(params.clone())),
        )?;
        validate_semantic_hash_field(&self.operation, params)?;
        Ok(())
    }

    pub fn canonical_request_hash(&self, project_id: &str) -> Result<String, ProtocolError> {
        self.validate()?;
        let mut params = normalized_params(self)?;
        if matches!(
            self.operation.as_str(),
            "planning.spec.generate" | "planning.plan.generate"
        ) {
            params.remove("projection_policy");
        }
        let canonical = serde_json::json!({
            "protocol_version": self.protocol_version,
            "project_id": project_id,
            "operation": self.operation,
            "session_id": self.session_id,
            "expected_revision": self.expected_revision,
            "params": Value::Object(params),
        });
        let mut hasher = Sha256::new();
        hasher.update(super::super::canonical::canonical_json_bytes(&canonical));
        Ok(format!("sha256:{:x}", hasher.finalize()))
    }
}

fn normalized_params(request: &LogicalRequest) -> Result<Map<String, Value>, ProtocolError> {
    request.validate()?;
    let value = normalize_typed_params(
        &request.operation,
        typed_params_input(
            &request.operation,
            request
                .params
                .clone()
                .unwrap_or_else(|| Value::Object(Map::new())),
        ),
    )?;
    match value {
        Value::Object(params) => Ok(params),
        _ => unreachable!("typed params always serialize as object"),
    }
}

fn typed_params_input(operation: &str, params: Value) -> Value {
    let Some(contract) = operation_contract(operation) else {
        return params;
    };
    let Value::Object(mut object) = params else {
        return params;
    };
    for optional in contract.optional_params {
        if object.get(*optional).is_some_and(Value::is_null) {
            object.remove(*optional);
        }
    }
    Value::Object(object)
}

fn validate_semantic_hash_field(
    operation: &str,
    params: &Map<String, Value>,
) -> Result<(), ProtocolError> {
    if matches!(operation, "planning.spec.approve" | "planning.plan.approve") {
        let value = params
            .get("semantic_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ProtocolError::InvalidRequest("semantic_hash must be a string".to_string())
            })?;
        let hex = value.strip_prefix("sha256:").unwrap_or_default();
        if hex.len() != 64
            || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            || hex.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(ProtocolError::InvalidRequest(
                "semantic_hash must be sha256:<64 lowercase hex>".to_string(),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct OperationContract {
    kind: OperationKind,
    requires_session: bool,
    forbids_session: bool,
    requires_revision: bool,
    forbids_revision: bool,
    required_params: &'static [&'static str],
    optional_params: &'static [&'static str],
}

fn operation_contract(operation: &str) -> Option<OperationContract> {
    use OperationKind::Mutation as M;
    Some(match operation {
        "planning.start" => OperationContract {
            kind: M,
            requires_session: false,
            forbids_session: true,
            requires_revision: false,
            forbids_revision: true,
            required_params: &["request"],
            optional_params: &["title"],
        },
        "planning.answer" => mutation(&["question_id", "text"], &["selected_choice_ids"]),
        "planning.status" | "planning.current" => query_with_session(&[], &[]),
        "planning.list" => query_without_session(&[], &["phase"]),
        "planning.evidence.refresh" => mutation(&["citations"], &[]),
        "planning.audit.apply" => mutation(&["mode", "proposal"], &[]),
        "planning.spec.generate" | "planning.plan.generate" => {
            mutation(&["proposal"], &["projection_policy"])
        }
        "planning.spec.show" | "planning.plan.show" => {
            query_with_session(&[], &["candidate_id", "format"])
        }
        "planning.spec.approve" => mutation(
            &["candidate_id", "semantic_hash", "base_domain_revision"],
            &[],
        ),
        "planning.spec.revise" | "planning.plan.revise" => mutation(&["candidate_id", "text"], &[]),
        "planning.plan.approve" => mutation(
            &["candidate_id", "semantic_hash", "base_plan_revision"],
            &[],
        ),
        "planning.export" => OperationContract {
            kind: M,
            requires_session: false,
            forbids_session: false,
            requires_revision: false,
            forbids_revision: true,
            required_params: &["out", "format"],
            optional_params: &["include_transcript", "force"],
        },
        "planning.purge" => mutation(&["confirm"], &[]),
        _ => return None,
    })
}

fn mutation(
    required_params: &'static [&'static str],
    optional_params: &'static [&'static str],
) -> OperationContract {
    OperationContract {
        kind: OperationKind::Mutation,
        requires_session: true,
        forbids_session: false,
        requires_revision: true,
        forbids_revision: false,
        required_params,
        optional_params,
    }
}

fn query_with_session(
    required_params: &'static [&'static str],
    optional_params: &'static [&'static str],
) -> OperationContract {
    OperationContract {
        kind: OperationKind::Query,
        requires_session: false,
        forbids_session: false,
        requires_revision: false,
        forbids_revision: true,
        required_params,
        optional_params,
    }
}

fn query_without_session(
    required_params: &'static [&'static str],
    optional_params: &'static [&'static str],
) -> OperationContract {
    OperationContract {
        kind: OperationKind::Query,
        requires_session: false,
        forbids_session: true,
        requires_revision: false,
        forbids_revision: true,
        required_params,
        optional_params,
    }
}

pub fn supported_operations() -> &'static [&'static str] {
    LOGICAL_OPERATIONS
}
