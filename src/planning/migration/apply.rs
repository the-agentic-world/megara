use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use uuid::Uuid;

use super::{
    backup, import,
    inventory::{self, Discovery},
    journal, rollback,
    types::{
        MigrationFileRecord, MigrationManifest, MigrationOptions, MigrationPhase, MigrationReport,
        MigrationReportAction, MigrationReportEntry, MIGRATION_SCHEMA,
    },
};
use crate::planning::store::canonical_project_identity;

pub(crate) fn run(options: MigrationOptions) -> Result<MigrationReport> {
    validate_options(&options)?;
    let project = options
        .project
        .canonicalize()
        .with_context(|| format!("failed to resolve project {}", options.project.display()))?;
    let _lock = super::lock::acquire(&project)?;
    if options.dry_run {
        return dry_run(&project);
    }
    if let Some(migration_id) = options.resume.as_deref() {
        return resume(&project, migration_id);
    }
    if let Some(migration_id) = options.rollback.as_deref() {
        return rollback::run(&project, migration_id, options.force);
    }
    super::recovery::ensure_no_incomplete_migration(&project)?;
    let migration_id = format!("mig_{}", Uuid::now_v7());
    let discovery = inventory::discover(&project)?;
    if discovery.files.is_empty() {
        return Ok(MigrationReport {
            migration_id: "noop".to_string(),
            phase: MigrationPhase::Applied,
            dry_run: false,
            source_files: Vec::new(),
            removed_files: Vec::new(),
            session_id: None,
            revision: None,
            entries: Vec::new(),
            warnings: discovery.warnings,
        });
    }
    create_and_resume(&project, &migration_id, discovery, options.force)
}

pub(crate) fn resume(project: &Path, migration_id: &str) -> Result<MigrationReport> {
    validate_id(migration_id)?;
    let mut manifest = journal::read_manifest(project, migration_id)?;
    if manifest.phase.is_terminal() {
        return Ok(report(&manifest, false, Vec::new()));
    }
    advance(project, &mut manifest, false)
}

fn create_and_resume(
    project: &Path,
    migration_id: &str,
    discovery: Discovery,
    force: bool,
) -> Result<MigrationReport> {
    let identity = canonical_project_identity(project)?;
    let records = discovery
        .files
        .iter()
        .map(DiscoveryRecord::record)
        .collect();
    let opaque_files = discovery
        .files
        .iter()
        .filter(|file| file.kind == inventory::LegacyFileKind::Opaque)
        .cloned()
        .collect::<Vec<_>>();
    let mut manifest = MigrationManifest {
        schema: MIGRATION_SCHEMA.to_string(),
        manifest_hash: String::new(),
        migration_id: migration_id.to_string(),
        project_id: identity.project_id,
        source_bundle_hash: inventory::source_bundle_hash(&opaque_files),
        backup_bundle_hash: inventory::source_bundle_hash(&discovery.files),
        phase: MigrationPhase::Prepared,
        files: records,
        session_id: None,
        revision: None,
        rollback_export_sha256: None,
        import_command_id: String::new(),
        warnings: discovery.warnings,
    };
    manifest.import_command_id = import::command_id(&manifest);
    let staging =
        super::staging::prepare(project, migration_id, &manifest.project_id, &manifest.files)?;
    let mut manifest_written = false;
    let staging_result = (|| -> Result<()> {
        backup::preflight_at(project, &staging.path, &discovery.files)?;
        backup::write_at(project, &staging, &discovery.files)?;
        journal::write_manifest_at(project, &mut manifest, &staging)?;
        manifest_written = true;
        match super::staging::validate_at(
            project,
            &staging.path,
            migration_id,
            &manifest.project_id,
            &staging.directory,
        )? {
            super::staging::StagingState::Prepared(_) => {}
            super::staging::StagingState::Partial => {
                anyhow::bail!("staging manifest was not prepared")
            }
        }
        super::recovery::publish_staging(project, migration_id, &staging)
    })();
    if staging_result.is_err() && !manifest_written {
        if let Ok(entry_name) = std::ffi::CString::new(migration_id) {
            let _ = super::staging::remove_owned_at(
                project,
                &staging.path,
                migration_id,
                &manifest.project_id,
                &staging.namespace,
                &entry_name,
                &staging.directory,
            );
        }
    }
    staging_result?;
    advance(project, &mut manifest, force)
}

fn advance(
    project: &Path,
    manifest: &mut MigrationManifest,
    force: bool,
) -> Result<MigrationReport> {
    if manifest.phase == MigrationPhase::Prepared {
        if manifest.files.iter().any(|file| file.kind == "opaque") {
            let mut store = crate::planning::store::PlanningStore::open_project(project)?;
            let outcome = import::import(project, &mut store, manifest)?;
            manifest.session_id = Some(outcome.state.session_id);
            manifest.revision = Some(outcome.state.revision);
        }
        journal::transition(manifest, MigrationPhase::PlanningImported)?;
        journal::write_manifest(project, manifest)?;
    }
    if manifest.phase == MigrationPhase::PlanningImported {
        remove_candidates(project, manifest, force)?;
        journal::transition(manifest, MigrationPhase::ProjectionRemoved)?;
        journal::write_manifest(project, manifest)?;
    }
    if manifest.phase == MigrationPhase::ProjectionRemoved {
        journal::transition(manifest, MigrationPhase::Applied)?;
        journal::write_manifest(project, manifest)?;
    }
    Ok(report(manifest, false, Vec::new()))
}

fn remove_candidates(project: &Path, manifest: &mut MigrationManifest, force: bool) -> Result<()> {
    // Empty parent directories are intentionally retained: v1 does not journal
    // directory modes, so removing them would make rollback lossy.
    for file in &mut manifest.files {
        if !file.removable || file.removed {
            continue;
        }
        let path = journal::safe_relative_parent(project, &file.relative_path)?;
        match super::safe_fs::read_file_nofollow(&path) {
            Ok((_, bytes)) => {
                let removed = if force {
                    super::safe_fs::remove_file_nofollow(&path)?
                } else if inventory::sha256(&bytes) == file.sha256 {
                    super::safe_fs::remove_file_if_matches_nofollow(&path, &file.sha256, file.mode)?
                } else {
                    false
                };
                if removed {
                    file.removed = true;
                } else {
                    manifest
                        .warnings
                        .push(format!("user_modified_managed: {}", file.relative_path));
                }
            }
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                manifest
                    .warnings
                    .push(format!("legacy symlink preserved: {}", file.relative_path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                file.removed = true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => manifest
                .warnings
                .push(format!("legacy non-file preserved: {}", file.relative_path)),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn dry_run(project: &Path) -> Result<MigrationReport> {
    let discovery = inventory::discover(project)?;
    Ok(MigrationReport {
        migration_id: "dry-run".to_string(),
        phase: MigrationPhase::Prepared,
        dry_run: true,
        source_files: inventory::record_paths(&discovery.files),
        removed_files: Vec::new(),
        session_id: None,
        revision: None,
        entries: discovery_entries(&discovery.files),
        warnings: discovery.warnings,
    })
}

fn report(manifest: &MigrationManifest, dry_run: bool, removed: Vec<PathBuf>) -> MigrationReport {
    MigrationReport {
        migration_id: manifest.migration_id.clone(),
        phase: manifest.phase,
        dry_run,
        source_files: manifest
            .files
            .iter()
            .map(|file| PathBuf::from(&file.relative_path))
            .collect(),
        removed_files: if removed.is_empty() {
            manifest
                .files
                .iter()
                .filter(|file| file.removed)
                .map(|file| PathBuf::from(&file.relative_path))
                .collect()
        } else {
            removed
        },
        session_id: manifest.session_id.clone(),
        revision: manifest.revision,
        entries: manifest_entries(&manifest.files),
        warnings: manifest.warnings.clone(),
    }
}

fn discovery_entries(files: &[inventory::DiscoveredFile]) -> Vec<MigrationReportEntry> {
    let mut entries = Vec::with_capacity(files.len() * 2);
    for file in files {
        entries.push(MigrationReportEntry {
            relative_path: file.relative_path.clone(),
            action: MigrationReportAction::Backup,
            reason: None,
        });
        entries.push(MigrationReportEntry {
            relative_path: file.relative_path.clone(),
            action: if file.removable {
                MigrationReportAction::Remove
            } else {
                MigrationReportAction::Preserve
            },
            reason: (!file.removable).then(|| "not_managed".to_string()),
        });
    }
    entries
}

fn manifest_entries(files: &[MigrationFileRecord]) -> Vec<MigrationReportEntry> {
    let mut entries = Vec::with_capacity(files.len() * 2);
    for file in files {
        entries.push(MigrationReportEntry {
            relative_path: file.relative_path.clone().into(),
            action: MigrationReportAction::Backup,
            reason: None,
        });
        entries.push(MigrationReportEntry {
            relative_path: file.relative_path.clone().into(),
            action: if file.removed {
                MigrationReportAction::Remove
            } else {
                MigrationReportAction::Preserve
            },
            reason: if file.removed {
                None
            } else if file.removable {
                Some("user_modified_or_unavailable".to_string())
            } else {
                Some("not_managed".to_string())
            },
        });
    }
    entries
}

fn validate_options(options: &MigrationOptions) -> Result<()> {
    let selected = usize::from(options.dry_run)
        + usize::from(options.apply)
        + usize::from(options.resume.is_some())
        + usize::from(options.rollback.is_some());
    if selected != 1 {
        anyhow::bail!("choose exactly one of --dry-run, --apply, --resume, or --rollback")
    }
    if options.force && options.rollback.is_none() {
        anyhow::bail!("--force is valid only with --rollback")
    }
    if let Some(id) = options.resume.as_deref().or(options.rollback.as_deref()) {
        validate_id(id)?;
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<()> {
    let Some(suffix) = value.strip_prefix("mig_") else {
        anyhow::bail!("migration id must use the generated mig_<lowercase-id> form")
    };
    if suffix.is_empty()
        || value.len() > 128
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'z' | b'-'))
    {
        anyhow::bail!("migration id must use the generated mig_<lowercase-id> form")
    }
    Ok(())
}

struct DiscoveryRecord;

impl DiscoveryRecord {
    fn record(file: &inventory::DiscoveredFile) -> MigrationFileRecord {
        file.record()
    }
}
