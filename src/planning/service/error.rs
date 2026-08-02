use serde_json::{json, Value};

use super::super::engine::CoreError;
use super::super::evidence::EvidenceError;
use super::super::protocol::ProtocolError;
use super::super::store::StoreError;

#[derive(Debug)]
pub(crate) struct ServiceError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) details: Value,
    pub(crate) retryable: bool,
}

impl ServiceError {
    pub(crate) fn with_code(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Value::Null,
            retryable: false,
        }
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::with_code("INVALID_REQUEST", message)
    }

    pub(crate) fn proposal_schema(message: impl Into<String>) -> Self {
        Self::with_code("PROPOSAL_SCHEMA_INVALID", message)
    }

    pub(crate) fn session_ambiguous() -> Self {
        Self::with_code("SESSION_AMBIGUOUS", "a session must be selected explicitly")
    }

    pub(crate) fn protocol(error: ProtocolError) -> Self {
        let (code, message) = match error {
            ProtocolError::SessionRequired => ("SESSION_REQUIRED", "session_id is required".into()),
            ProtocolError::UnsupportedVersion(version) => (
                "PROTOCOL_VERSION_UNSUPPORTED",
                format!("unsupported protocol version: {version}"),
            ),
            ProtocolError::FrameTooLarge => ("INVALID_REQUEST", "JSONL frame exceeds 4 MiB".into()),
            ProtocolError::InvalidUtf8 => ("INVALID_REQUEST", "request is not UTF-8".into()),
            ProtocolError::InvalidJson(message) => ("INVALID_REQUEST", message),
            ProtocolError::InvalidRequest(message) => ("INVALID_REQUEST", message),
            ProtocolError::Io(message) => ("IO_ERROR", message),
        };
        Self::with_code(code, message)
    }

    pub(crate) fn revision_conflict(expected: u64, actual: u64) -> Self {
        Self {
            code: "REVISION_CONFLICT",
            message: format!("expected revision {expected}, current revision is {actual}"),
            details: json!({"expected_revision":expected, "actual_revision":actual}),
            retryable: false,
        }
    }

    pub(crate) fn evidence(error: EvidenceError) -> Self {
        let code = match error {
            EvidenceError::Git(_) | EvidenceError::Io(_) => "IO_ERROR",
            EvidenceError::InvalidRequest(_)
            | EvidenceError::PathOutsideRoot(_)
            | EvidenceError::SensitivePath(_)
            | EvidenceError::IgnoredPath(_)
            | EvidenceError::MissingFile(_)
            | EvidenceError::InvalidRange(_) => "INVALID_REQUEST",
        };
        Self::with_code(code, error.to_string())
    }
}

impl From<StoreError> for ServiceError {
    fn from(error: StoreError) -> Self {
        let (code, retryable, details) = match &error {
            StoreError::DbBusy => ("DB_BUSY", true, Value::Null),
            StoreError::DbCorrupt(_) => ("DB_CORRUPT", false, Value::Null),
            StoreError::ProjectionDiverged(_) => ("PROJECTION_DIVERGED", false, Value::Null),
            StoreError::SessionNotFound(_) => ("SESSION_NOT_FOUND", false, Value::Null),
            StoreError::SessionPurged(_) => ("SESSION_PURGED", false, Value::Null),
            StoreError::CommandIdReuse => ("COMMAND_ID_REUSE", false, Value::Null),
            StoreError::CommandIdRetired => ("COMMAND_ID_RETIRED", false, Value::Null),
            StoreError::PurgeConfirmationMismatch => {
                ("PURGE_CONFIRMATION_MISMATCH", false, Value::Null)
            }
            StoreError::ProjectIdMismatch { expected, actual } => (
                "INVALID_REQUEST",
                false,
                json!({"reason":"project_id_mismatch", "expected":expected, "actual":actual}),
            ),
            StoreError::SchemaUpgradeRequired { actual, expected } => (
                "SCHEMA_UPGRADE_REQUIRED",
                false,
                json!({"actual":actual, "expected":expected}),
            ),
            StoreError::SchemaVersionUnsupported { actual, expected } => (
                "SCHEMA_VERSION_UNSUPPORTED",
                false,
                json!({"actual":actual, "expected":expected}),
            ),
            StoreError::Core(core) => return core_error(core),
            StoreError::InvalidRequest(_) | StoreError::ProjectIdentity(_) => {
                ("INVALID_REQUEST", false, Value::Null)
            }
            StoreError::Sqlite(_) | StoreError::Io(_) => ("IO_ERROR", false, Value::Null),
            StoreError::Json(_) => ("DB_CORRUPT", false, Value::Null),
        };
        Self {
            code,
            message: error.to_string(),
            details,
            retryable,
        }
    }
}

fn core_error(error: &CoreError) -> ServiceError {
    let (code, details) = match error {
        CoreError::InvalidRequest(_) => ("INVALID_REQUEST", Value::Null),
        CoreError::SessionExists(id) => (
            "INVALID_REQUEST",
            json!({"reason":"session_exists", "session_id":id}),
        ),
        CoreError::SessionNotFound(_) => ("SESSION_NOT_FOUND", Value::Null),
        CoreError::RevisionConflict { expected, actual } => (
            "REVISION_CONFLICT",
            json!({"expected_revision":expected, "actual_revision":actual}),
        ),
        CoreError::InvalidPhase(_) => ("INVALID_PHASE", Value::Null),
        CoreError::QuestionMismatch => ("QUESTION_MISMATCH", Value::Null),
        CoreError::ModelActionMismatch => ("MODEL_ACTION_MISMATCH", Value::Null),
        CoreError::ProposalSchemaInvalid(_) => ("PROPOSAL_SCHEMA_INVALID", Value::Null),
        CoreError::ProposalBaseMismatch => ("PROPOSAL_BASE_MISMATCH", Value::Null),
        CoreError::InvalidSourceReference => ("INVALID_SOURCE_REFERENCE", Value::Null),
        CoreError::BlockersPresent => ("BLOCKERS_PRESENT", Value::Null),
        CoreError::CandidateNotFound(_) => ("CANDIDATE_NOT_FOUND", Value::Null),
        CoreError::CandidateStale => ("CANDIDATE_STALE", Value::Null),
        CoreError::ApprovalBindingMismatch => ("APPROVAL_BINDING_MISMATCH", Value::Null),
        CoreError::Invariant(_) => ("DB_CORRUPT", Value::Null),
    };
    ServiceError {
        code,
        message: error.to_string(),
        details,
        retryable: false,
    }
}
