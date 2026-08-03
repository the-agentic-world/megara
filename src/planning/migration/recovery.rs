use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{journal, staging};
use crate::planning::store::canonical_project_identity;

pub(crate) fn ensure_no_incomplete_migration(project: &Path) -> Result<()> {
    journal::safe_backup_root_parent(project)?;
    let root = project.join(".megara/migration-backups");
    let backup_parent = match super::safe_fs::open_directory_nofollow(&root) {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let project_id = canonical_project_identity(project)?.project_id;
    let root_entries = super::safe_fs::read_directory_names(&backup_parent)?;
    for entry_name in root_entries {
        let migration_id = entry_name
            .to_str()
            .map_err(|_| anyhow::anyhow!("MIGRATION_INCOMPLETE: migration entry is not UTF-8"))?
            .to_string();
        let entry_directory = super::safe_fs::open_directory_at(&backup_parent, &entry_name)
            .with_context(|| {
                format!("MIGRATION_INCOMPLETE: migration entry is not a directory: {migration_id}")
            })?;
        if migration_id == ".staging" {
            recover_staging(
                project,
                &backup_parent,
                &entry_name,
                &entry_directory,
                &project_id,
            )?;
            continue;
        }
        if journal::validate_id(&migration_id).is_err() {
            anyhow::bail!(
                "MIGRATION_INCOMPLETE: unexpected migration backup {}",
                migration_id
            );
        }
        let entry_path = root.join(&migration_id);
        let manifest = journal::read_manifest_from_directory(
            &migration_id,
            &entry_path,
            &project_id,
            &entry_directory,
        )?;
        journal::validate_manifest_payload_at(&manifest, &entry_directory)?;
        if !manifest.phase.is_terminal() {
            anyhow::bail!("MIGRATION_INCOMPLETE: {migration_id}")
        }
    }
    Ok(())
}

pub(crate) fn publish_staging(
    project: &Path,
    migration_id: &str,
    staging: &staging::OwnedStaging,
) -> Result<()> {
    journal::safe_backup_root_parent(project)?;
    let staging_root = staging::staging_root(project, migration_id);
    #[cfg(not(unix))]
    let final_root = journal::backup_root(project, migration_id);
    if staging.path != staging_root {
        anyhow::bail!("migration staging path binding is inconsistent")
    }
    super::safe_fs::verify_directory_identity(&staging_root, &staging.directory)?;
    let project_id = canonical_project_identity(project)?.project_id;
    match staging::validate_at(
        project,
        &staging_root,
        migration_id,
        &project_id,
        &staging.directory,
    )? {
        staging::StagingState::Prepared(_) => {}
        staging::StagingState::Partial => {
            anyhow::bail!("MIGRATION_INCOMPLETE: staging manifest is not prepared")
        }
    }
    staging.directory.sync_all()?;
    staging.namespace.sync_all()?;
    #[cfg(unix)]
    {
        let destination_parent = staging.namespace_parent.try_clone()?;
        let destination_name = std::ffi::CString::new(migration_id)?;
        let source_name = std::ffi::CString::new(migration_id)?;
        super::safe_fs::rename_directory_at_noreplace(
            &staging.namespace,
            &source_name,
            &staging.directory,
            &destination_parent,
            &destination_name,
        )
        .with_context(|| {
            format!(
                "failed to publish migration staging {}",
                staging_root.display()
            )
        })?;
        let namespace_name = std::ffi::CString::new(".staging")?;
        let removed_namespace = super::safe_fs::remove_empty_directory_at(
            &staging.namespace_parent,
            &namespace_name,
            &staging.namespace,
        )?;
        if !removed_namespace {
            anyhow::bail!("MIGRATION_INCOMPLETE: staging namespace contains unprocessed entries");
        }
    }
    #[cfg(not(unix))]
    {
        super::safe_fs::rename_directory_nofollow(&staging_root, &final_root).with_context(
            || {
                format!(
                    "failed to publish migration staging {}",
                    staging_root.display()
                )
            },
        )?;
        let staging_namespace = staging_root
            .parent()
            .ok_or_else(|| anyhow::anyhow!("migration staging has no namespace"))?;
        let _ = super::safe_fs::remove_empty_directory_nofollow(staging_namespace)?;
    }
    Ok(())
}

fn recover_staging(
    project: &Path,
    backup_parent: &fs::File,
    namespace_name: &std::ffi::CStr,
    namespace_directory: &fs::File,
    project_id: &str,
) -> Result<()> {
    let staging_namespace = project.join(".megara/migration-backups/.staging");
    for entry_name in super::safe_fs::read_directory_names(namespace_directory)? {
        let migration_id = entry_name
            .to_str()
            .map_err(|_| anyhow::anyhow!("MIGRATION_INCOMPLETE: staging entry is not UTF-8"))?
            .to_string();
        if journal::validate_id(&migration_id).is_err() {
            anyhow::bail!(
                "MIGRATION_INCOMPLETE: unexpected staging entry {}",
                migration_id
            );
        }
        let entry_path = staging_namespace.join(&migration_id);
        let entry_directory =
            super::safe_fs::open_directory_at(namespace_directory, &entry_name)
                .with_context(|| format!("failed to open owned staging {migration_id}"))?;
        let state = match staging::validate_at(
            project,
            &entry_path,
            &migration_id,
            project_id,
            &entry_directory,
        ) {
            Ok(state) => state,
            Err(error) => anyhow::bail!(
                "MIGRATION_INCOMPLETE: staging {} is not product-owned: {}",
                migration_id,
                error
            ),
        };
        match state {
            staging::StagingState::Prepared(manifest) => {
                if manifest.migration_id != migration_id {
                    anyhow::bail!(
                        "MIGRATION_INCOMPLETE: staged manifest identity mismatch for {}",
                        migration_id
                    );
                }
                #[cfg(not(unix))]
                let final_root = journal::backup_root(project, &migration_id);
                #[cfg(unix)]
                {
                    match staging::validate_at(
                        project,
                        &entry_path,
                        &migration_id,
                        project_id,
                        &entry_directory,
                    )? {
                        staging::StagingState::Prepared(_) => {}
                        staging::StagingState::Partial => anyhow::bail!(
                            "MIGRATION_INCOMPLETE: staged migration {} is no longer prepared",
                            migration_id
                        ),
                    }
                    let final_name = std::ffi::CString::new(migration_id.as_bytes())?;
                    let source_name = std::ffi::CString::new(migration_id.as_bytes())?;
                    super::safe_fs::rename_directory_at_noreplace(
                        namespace_directory,
                        &source_name,
                        &entry_directory,
                        backup_parent,
                        &final_name,
                    )?;
                    namespace_directory.sync_all()?;
                    backup_parent.sync_all()?;
                    let removed_namespace = super::safe_fs::remove_empty_directory_at(
                        backup_parent,
                        namespace_name,
                        namespace_directory,
                    )?;
                    if !removed_namespace {
                        anyhow::bail!(
                            "MIGRATION_INCOMPLETE: staging namespace contains unprocessed entries"
                        );
                    }
                    backup_parent.sync_all()?;
                }
                #[cfg(not(unix))]
                {
                    super::safe_fs::rename_directory_nofollow(&entry_path, &final_root)?;
                }
                anyhow::bail!(
                    "MIGRATION_INCOMPLETE: staged migration {} is ready; resume it",
                    migration_id
                );
            }
            staging::StagingState::Partial => {
                staging::remove_owned_at(
                    project,
                    &entry_path,
                    &migration_id,
                    project_id,
                    namespace_directory,
                    &entry_name,
                    &entry_directory,
                )
                .with_context(|| {
                    format!("failed to remove owned migration staging {migration_id}")
                })?;
            }
        }
    }
    #[cfg(unix)]
    {
        let removed_namespace = super::safe_fs::remove_empty_directory_at(
            backup_parent,
            namespace_name,
            namespace_directory,
        )?;
        if !removed_namespace {
            anyhow::bail!("MIGRATION_INCOMPLETE: staging namespace contains unprocessed entries");
        }
    }
    #[cfg(not(unix))]
    {
        let _ = super::safe_fs::remove_empty_directory_nofollow(&staging_namespace)?;
    }
    Ok(())
}
