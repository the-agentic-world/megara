use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::{
    inventory::DiscoveredFile,
    journal::{safe_backup_root, safe_relative_path},
    types::MigrationFileRecord,
};

pub(crate) fn preflight_at(project: &Path, root: &Path, files: &[DiscoveredFile]) -> Result<()> {
    super::journal::safe_backup_root_parent(project)?;
    let relative_root = root
        .strip_prefix(project)
        .with_context(|| format!("backup root escaped project: {}", root.display()))?;
    super::journal::safe_relative_parent(
        project,
        &format!("{}/placeholder", relative_root.display()),
    )?;
    if fs::symlink_metadata(root)
        .map(|metadata| !metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        anyhow::bail!(
            "migration backup root is not a directory: {}",
            root.display()
        );
    }
    for file in files {
        let path = backup_path(root, &file.relative_path)?;
        if fs::symlink_metadata(&path).is_ok() {
            anyhow::bail!("migration backup target already exists: {}", path.display());
        }
    }
    Ok(())
}

pub(crate) fn write_at(
    _project: &Path,
    staging: &super::staging::OwnedStaging,
    files: &[DiscoveredFile],
) -> Result<()> {
    let root_directory = staging.directory.try_clone()?;
    let files_directory = super::safe_fs::ensure_directory_at(&root_directory, Path::new("files"))?;
    let result = (|| -> Result<()> {
        for file in files {
            let relative = Path::new(&file.relative_path);
            let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
            let parent = super::safe_fs::ensure_directory_at(&files_directory, parent_relative)?;
            let destination = std::ffi::CString::new(
                relative
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("backup file has no name"))?
                    .as_encoded_bytes(),
            )?;
            if super::safe_fs::create_linked_file_at(&parent, &destination, &file.bytes, file.mode)
                .with_context(|| {
                    format!("failed to create backup {}", file.relative_path.display())
                })?
                != super::safe_fs::CreateResult::Created
            {
                anyhow::bail!(
                    "migration backup target already exists: {}",
                    file.relative_path.display()
                );
            }
            let (metadata, actual) =
                super::safe_fs::read_file_at(&parent, &destination, usize::MAX)?;
            if super::inventory::sha256(&actual) != file.sha256
                || actual.len() as u64 != file.bytes.len() as u64
            {
                anyhow::bail!("migration backup digest verification failed")
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() != file.mode {
                    anyhow::bail!("migration backup mode verification failed")
                }
            }
        }
        Ok(())
    })();
    result
}

pub(crate) fn read(
    project: &Path,
    migration_id: &str,
    record: &MigrationFileRecord,
) -> Result<Vec<u8>> {
    let root = safe_backup_root(project, migration_id)?;
    let path = safe_relative_path(&root, &format!("files/{}", record.relative_path))?;
    let (metadata, bytes) = super::safe_fs::read_file_nofollow(&path)
        .with_context(|| format!("failed to read migration backup {}", path.display()))?;
    if super::inventory::sha256(&bytes) != record.sha256 || bytes.len() as u64 != record.size {
        anyhow::bail!(
            "migration backup digest verification failed: {}",
            path.display()
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() != record.mode {
            anyhow::bail!(
                "migration backup mode verification failed: {}",
                path.display()
            );
        }
    }
    Ok(bytes)
}

fn backup_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("unsafe migration backup path: {}", relative.display())
    }
    let relative = format!("files/{}", relative.display());
    super::journal::validate_relative_path(&relative)?;
    if fs::symlink_metadata(root).is_ok() {
        super::journal::safe_relative_parent(root, &relative)
    } else {
        Ok(root.join(relative))
    }
}
