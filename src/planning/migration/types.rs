use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const MIGRATION_SCHEMA: &str = "megara.planning-migration/v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    Prepared,
    PlanningImported,
    ProjectionRemoved,
    Applied,
    RolledBack,
}

impl MigrationPhase {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Applied | Self::RolledBack)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationOptions {
    pub project: PathBuf,
    pub dry_run: bool,
    pub apply: bool,
    pub resume: Option<String>,
    pub rollback: Option<String>,
    pub force: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MigrationFileRecord {
    pub relative_path: String,
    pub sha256: String,
    pub size: u64,
    pub mode: u32,
    pub kind: String,
    pub removable: bool,
    pub removed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MigrationManifest {
    pub schema: String,
    pub manifest_hash: String,
    pub migration_id: String,
    pub project_id: String,
    pub source_bundle_hash: String,
    pub backup_bundle_hash: String,
    pub phase: MigrationPhase,
    pub files: Vec<MigrationFileRecord>,
    pub session_id: Option<String>,
    pub revision: Option<u64>,
    #[serde(default)]
    pub rollback_export_sha256: Option<String>,
    pub import_command_id: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationReportAction {
    Backup,
    Remove,
    Preserve,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationReportEntry {
    pub relative_path: PathBuf,
    pub action: MigrationReportAction,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationReport {
    pub migration_id: String,
    pub phase: MigrationPhase,
    pub dry_run: bool,
    pub source_files: Vec<PathBuf>,
    pub removed_files: Vec<PathBuf>,
    pub session_id: Option<String>,
    pub revision: Option<u64>,
    pub entries: Vec<MigrationReportEntry>,
    pub warnings: Vec<String>,
}
