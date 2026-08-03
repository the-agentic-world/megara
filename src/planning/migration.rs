#[path = "migration/apply.rs"]
mod apply;
#[path = "migration/backup.rs"]
mod backup;
#[path = "migration/bindings.rs"]
mod bindings;
#[path = "migration/import.rs"]
mod import;
#[path = "migration/inventory.rs"]
pub(crate) mod inventory;
#[path = "migration/journal.rs"]
mod journal;
#[path = "migration/lock.rs"]
mod lock;
#[path = "migration/recovery.rs"]
mod recovery;
#[path = "migration/rollback.rs"]
mod rollback;
#[path = "migration/safe_fs.rs"]
mod safe_fs;
#[path = "migration/staging.rs"]
mod staging;
#[path = "migration/types.rs"]
mod types;

pub(crate) use lock::acquire as acquire_project_lock;
pub(crate) use safe_fs::remove_tree_at_validated;
pub(crate) use staging::validate_held as validate_staging_held;
pub use types::{MigrationOptions, MigrationPhase, MigrationReport, MigrationReportAction};

pub(crate) fn remove_tree_nofollow(path: &std::path::Path) -> std::io::Result<bool> {
    safe_fs::remove_tree_nofollow(path)
}

pub fn run(options: MigrationOptions) -> anyhow::Result<MigrationReport> {
    apply::run(options)
}
