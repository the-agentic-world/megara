use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde_json::json;

use super::{
    backup, import, journal,
    types::{MigrationManifest, MigrationPhase, MigrationReport},
};
use crate::planning::{
    domain::PlanningState,
    store::{PlanningStore, StoreError},
};

pub(crate) fn run(project: &Path, migration_id: &str, force: bool) -> Result<MigrationReport> {
    let mut manifest = journal::read_manifest(project, migration_id)?;
    if manifest.files.iter().any(|file| file.kind == "opaque")
        && manifest.import_command_id != import::command_id(&manifest)
    {
        anyhow::bail!("ROLLBACK_CONFLICT: migration import command binding is inconsistent")
    }
    if manifest.session_id.is_some() != manifest.revision.is_some() {
        anyhow::bail!("ROLLBACK_CONFLICT: migration session binding is incomplete")
    }
    let prepared_import = if manifest.phase == MigrationPhase::Prepared
        && manifest.session_id.is_none()
        && manifest.files.iter().any(|file| file.kind == "opaque")
    {
        resolve_prepared_import(project, &manifest)?
    } else {
        None
    };
    if let Some((session_id, revision)) = prepared_import.as_ref() {
        manifest.session_id = Some(session_id.clone());
        manifest.revision = Some(*revision);
    }
    let rollback_command = rollback_command_id(&manifest);
    if manifest.phase == MigrationPhase::RolledBack {
        finish_terminal_cleanup(project, &manifest, &rollback_command)?;
        return Ok(report(&manifest));
    }
    let current = current_session(project, &manifest)?;
    let purge_committed = if current.is_none() && manifest.session_id.is_some() {
        let store = PlanningStore::open_project(project)?;
        store.purge_receipt_exists(&rollback_command)?
    } else {
        false
    };
    if current.is_none() && manifest.session_id.is_some() && !purge_committed {
        anyhow::bail!("ROLLBACK_CONFLICT: imported session is missing without rollback receipt")
    }
    let expected = manifest.revision.unwrap_or_default();
    let current_revision = current.as_ref().map(|state| state.revision);
    let changed = current_revision.is_some_and(|revision| revision != expected);
    if changed && !force {
        anyhow::bail!(
            "ROLLBACK_CONFLICT: imported session revision changed from {} to {}",
            expected,
            current_revision.unwrap_or_default()
        )
    }
    preflight_restore(project, &manifest)?;
    if prepared_import.is_some() {
        journal::write_manifest(project, &mut manifest)?;
    }
    if changed {
        let state = current.as_ref().expect("changed implies a current session");
        let store = PlanningStore::open_project(project)?;
        let export_hash = write_changed_session_export(project, &manifest, state, &store)?;
        manifest.rollback_export_sha256 = Some(export_hash);
        journal::write_manifest(project, &mut manifest)?;
    }
    if let Some(session_id) = manifest.session_id.as_deref() {
        let purge_revision = current_revision.unwrap_or(expected);
        if !purge_committed {
            let mut store = PlanningStore::open_project(project)?;
            let request_hash = crate::planning::canonical::canonical_hash(&json!([
                manifest.project_id,
                manifest.migration_id,
                "rollback-purge"
            ]));
            store.purge_for_rollback(
                session_id,
                &rollback_command,
                &request_hash,
                purge_revision,
                session_id,
            )?;
        }
    }
    restore(project, &manifest)?;
    journal::transition(&mut manifest, MigrationPhase::RolledBack)?;
    journal::write_manifest(project, &mut manifest)?;
    finish_terminal_cleanup(project, &manifest, &rollback_command)?;
    Ok(report(&manifest))
}

fn resolve_prepared_import(
    project: &Path,
    manifest: &MigrationManifest,
) -> Result<Option<(String, u64)>> {
    let Some(mut store) = PlanningStore::open_existing_project(project)? else {
        return Ok(None);
    };
    let Some(outcome) =
        store.cached_command(&manifest.import_command_id, &import::request_hash(manifest))?
    else {
        return Ok(None);
    };
    let event = outcome
        .event
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ROLLBACK_CONFLICT: import receipt has no event"))?;
    if event.operation != "planning.migration.import"
        || event.session_id != outcome.state.session_id
        || event.revision_after != outcome.state.revision
    {
        anyhow::bail!("ROLLBACK_CONFLICT: import receipt event binding is invalid")
    }
    let Some(legacy) = outcome.state.legacy_import.as_ref() else {
        anyhow::bail!("ROLLBACK_CONFLICT: import receipt has no legacy reference")
    };
    if !outcome.state.imported_legacy_context
        || legacy.migration_id != manifest.migration_id
        || legacy.source_backup_id != manifest.migration_id
        || legacy.source_bundle_hash != manifest.source_bundle_hash
    {
        anyhow::bail!("ROLLBACK_CONFLICT: import receipt does not match migration")
    }
    Ok(Some((outcome.state.session_id, outcome.state.revision)))
}

fn rollback_command_id(manifest: &MigrationManifest) -> String {
    format!(
        "cmd_mig_rollback_{}",
        crate::planning::canonical::canonical_hash(&json!([
            manifest.project_id,
            manifest.migration_id,
            "rollback-purge"
        ]))
        .trim_start_matches("sha256:")
    )
}

fn current_session(project: &Path, manifest: &MigrationManifest) -> Result<Option<PlanningState>> {
    if let Some(session_id) = manifest.session_id.as_deref() {
        let store = PlanningStore::open_project(project)?;
        return match store.current(session_id) {
            Ok(state) => Ok(Some(state)),
            Err(StoreError::SessionNotFound(_) | StoreError::SessionPurged(_)) => Ok(None),
            Err(error) => Err(error.into()),
        };
    }
    Ok(None)
}

fn preflight_restore(project: &Path, manifest: &MigrationManifest) -> Result<()> {
    for record in &manifest.files {
        let path = super::journal::safe_relative_parent(project, &record.relative_path)?;
        let needs_restore = record.removed
            || matches!(
                fs::symlink_metadata(&path),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            );
        if !needs_restore {
            continue;
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                anyhow::bail!("ROLLBACK_CONFLICT: symlink appeared at {}", path.display())
            }
            Ok(_) => {
                if !file_matches(&path, record)? {
                    anyhow::bail!("ROLLBACK_CONFLICT: user file changed at {}", path.display())
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let _ = backup::read(project, &manifest.migration_id, record)?;
    }
    Ok(())
}

fn restore(project: &Path, manifest: &MigrationManifest) -> Result<()> {
    for record in &manifest.files {
        let path = super::journal::safe_relative_parent(project, &record.relative_path)?;
        let missing = matches!(
            fs::symlink_metadata(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        );
        if !record.removed && !missing {
            continue;
        }
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("ROLLBACK_CONFLICT: symlink appeared at {}", path.display())
            }
            if metadata.file_type().is_file() {
                if file_matches(&path, record)? {
                    continue;
                }
                anyhow::bail!("ROLLBACK_CONFLICT: user file changed at {}", path.display())
            }
            anyhow::bail!(
                "ROLLBACK_CONFLICT: destination is not a file: {}",
                path.display()
            )
        }
        let bytes = backup::read(project, &manifest.migration_id, record)?;
        restore_missing(&path, &bytes, record)?;
    }
    Ok(())
}

fn file_matches(path: &Path, record: &super::types::MigrationFileRecord) -> Result<bool> {
    let (metadata, bytes) = match super::safe_fs::read_file_nofollow(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if super::inventory::sha256(&bytes) != record.sha256 {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(metadata.permissions().mode() == record.mode)
    }
    #[cfg(not(unix))]
    {
        let _ = record;
        Ok(true)
    }
}

fn restore_missing(
    path: &Path,
    bytes: &[u8],
    record: &super::types::MigrationFileRecord,
) -> Result<()> {
    match super::safe_fs::create_linked_file(path, bytes, record.mode)? {
        super::safe_fs::CreateResult::Created => Ok(()),
        super::safe_fs::CreateResult::Exists if file_matches(path, record)? => Ok(()),
        super::safe_fs::CreateResult::Exists => {
            anyhow::bail!("ROLLBACK_CONFLICT: user file changed at {}", path.display())
        }
    }
}

fn write_changed_session_export(
    project: &Path,
    manifest: &MigrationManifest,
    state: &PlanningState,
    store: &PlanningStore,
) -> Result<String> {
    let root = journal::safe_backup_root(project, &manifest.migration_id)?;
    let path = root.join("rollback-export.json");
    let value = serde_json::json!({
        "session_id": state.session_id,
        "state": state,
        "events": store.event_envelopes(&state.session_id)?,
    });
    let bytes = serde_json::to_vec_pretty(&value)?;
    let expected_hash = super::inventory::sha256(&bytes);
    if let Ok((_, existing)) = super::safe_fs::read_file_nofollow(&path) {
        if super::inventory::sha256(&existing) != expected_hash {
            anyhow::bail!("ROLLBACK_CONFLICT: rollback export already differs")
        }
        return Ok(expected_hash);
    }
    match super::safe_fs::create_linked_file(&path, &bytes, 0o600)? {
        super::safe_fs::CreateResult::Created => {}
        super::safe_fs::CreateResult::Exists => {
            let (_, existing) = super::safe_fs::read_file_nofollow(&path)?;
            if super::inventory::sha256(&existing) != expected_hash {
                anyhow::bail!("ROLLBACK_CONFLICT: rollback export already differs")
            }
            return Ok(expected_hash);
        }
    }
    let (_, actual) = super::safe_fs::read_file_nofollow(&path)?;
    if super::inventory::sha256(&actual) != expected_hash {
        anyhow::bail!("ROLLBACK export digest verification failed")
    }
    Ok(expected_hash)
}

fn finish_terminal_cleanup(
    project: &Path,
    manifest: &MigrationManifest,
    rollback_command: &str,
) -> Result<()> {
    let root = journal::safe_backup_root(project, &manifest.migration_id)?;
    let files_root = root.join("files");
    if fs::symlink_metadata(&files_root)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        anyhow::bail!("migration backup files directory is a symlink")
    }
    if fs::symlink_metadata(&files_root).is_ok() {
        super::safe_fs::remove_tree_nofollow(&files_root)
            .with_context(|| format!("failed to remove {}", files_root.display()))?;
        fs::File::open(&root)?.sync_all()?;
    }
    if manifest.session_id.is_some() {
        let mut store = PlanningStore::open_project(project)?;
        store.complete_purge_cleanup(rollback_command)?;
    }
    Ok(())
}

fn report(manifest: &MigrationManifest) -> MigrationReport {
    MigrationReport {
        migration_id: manifest.migration_id.clone(),
        phase: manifest.phase,
        dry_run: false,
        source_files: manifest
            .files
            .iter()
            .map(|file| file.relative_path.clone().into())
            .collect(),
        removed_files: Vec::new(),
        session_id: manifest.session_id.clone(),
        revision: manifest.revision,
        entries: Vec::new(),
        warnings: manifest.warnings.clone(),
    }
}
