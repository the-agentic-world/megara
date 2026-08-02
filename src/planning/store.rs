use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Error as SqliteError, ErrorCode};
use serde::{Deserialize, Serialize};

use super::domain::*;
use super::engine::*;

pub const STORE_SCHEMA_VERSION: i64 = 1;
pub const PLANNING_DB_RELATIVE_PATH: &str = ".megara/planning/planning.db";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectIdentity {
    pub canonical_root: String,
    pub project_id: String,
}

#[path = "store/artifacts.rs"]
mod artifacts;
#[path = "store/command_cache.rs"]
mod command_cache;
#[path = "store/hash.rs"]
mod hash;
#[path = "store/persistence.rs"]
mod persistence;
#[path = "store/purge.rs"]
mod purge;
#[path = "store/reconcile.rs"]
mod reconcile;
#[path = "store/replay.rs"]
mod replay;
#[path = "store/schema.rs"]
mod schema;
#[path = "store/transaction.rs"]
mod transaction;

pub use hash::normalized_state_hash;
pub use purge::PurgeReceipt;
pub use replay::{
    replay_events, EventActor, EventAdapter, EventEnvelope, EventMetadata, EventType,
    EVENT_ENVELOPE_SCHEMA_VERSION,
};
pub use schema::canonical_project_identity;
pub(crate) use transaction::EventContext;
pub use transaction::StoredOutcome;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(SqliteError),
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidRequest(String),
    ProjectIdentity(String),
    ProjectIdMismatch { expected: String, actual: String },
    SchemaUpgradeRequired { actual: i64, expected: i64 },
    SchemaVersionUnsupported { actual: i64, expected: i64 },
    DbBusy,
    DbCorrupt(String),
    ProjectionDiverged(String),
    SessionNotFound(SessionId),
    SessionPurged(SessionId),
    CommandIdReuse,
    CommandIdRetired,
    PurgeConfirmationMismatch,
    Core(CoreError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "sqlite error: {error}"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Json(error) => write!(f, "json error: {error}"),
            Self::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            Self::ProjectIdentity(message) => write!(f, "project identity error: {message}"),
            Self::ProjectIdMismatch { expected, actual } => {
                write!(
                    f,
                    "project id mismatch: expected {expected}, actual {actual}"
                )
            }
            Self::SchemaUpgradeRequired { actual, expected } => {
                write!(
                    f,
                    "schema upgrade required: actual {actual}, expected {expected}"
                )
            }
            Self::SchemaVersionUnsupported { actual, expected } => {
                write!(
                    f,
                    "schema version unsupported: actual {actual}, expected {expected}"
                )
            }
            Self::DbBusy => write!(f, "database busy"),
            Self::DbCorrupt(message) => write!(f, "database corrupt: {message}"),
            Self::ProjectionDiverged(message) => write!(f, "projection diverged: {message}"),
            Self::SessionNotFound(id) => write!(f, "session not found: {id}"),
            Self::SessionPurged(id) => write!(f, "session purged: {id}"),
            Self::CommandIdReuse => write!(f, "command id reuse"),
            Self::CommandIdRetired => write!(f, "command id retired"),
            Self::PurgeConfirmationMismatch => write!(f, "purge confirmation mismatch"),
            Self::Core(error) => write!(f, "core error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<SqliteError> for StoreError {
    fn from(error: SqliteError) -> Self {
        match &error {
            SqliteError::SqliteFailure(error, _)
                if matches!(
                    error.code,
                    ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
                ) =>
            {
                Self::DbBusy
            }
            SqliteError::SqliteFailure(error, _)
                if matches!(
                    error.code,
                    ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
                ) =>
            {
                Self::DbCorrupt(error.to_string())
            }
            _ => Self::Sqlite(error),
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<CoreError> for StoreError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

pub struct PlanningStore {
    pub(crate) conn: Connection,
    pub(crate) identity: ProjectIdentity,
    pub(crate) database_path: PathBuf,
}

impl PlanningStore {
    pub fn open_project(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        schema::open_project(root)
    }

    pub fn open(
        database_path: impl AsRef<Path>,
        identity: ProjectIdentity,
    ) -> Result<Self, StoreError> {
        schema::open(database_path, identity)
    }

    pub fn project_id(&self) -> &str {
        &self.identity.project_id
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn project_root(&self) -> &Path {
        self.database_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
    }
}
