use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};

use super::{
    staging::OwnedStaging,
    types::{MigrationManifest, MigrationPhase, MIGRATION_SCHEMA},
};

pub(crate) const MIGRATION_MAX_PATH_DEPTH: usize = 64;

pub(crate) fn backup_root(project: &Path, migration_id: &str) -> PathBuf {
    project.join(".megara/migration-backups").join(migration_id)
}

pub(crate) fn read_manifest(project: &Path, migration_id: &str) -> Result<MigrationManifest> {
    validate_id(migration_id)?;
    let expected_project = crate::planning::store::canonical_project_identity(project)?.project_id;
    let root = safe_backup_root(project, migration_id)?;
    read_manifest_at(migration_id, &root, &expected_project)
}

pub(crate) fn read_manifest_at(
    migration_id: &str,
    root: &Path,
    expected_project: &str,
) -> Result<MigrationManifest> {
    let path = root.join("manifest.json");
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("failed to stat {}", path.display()))?;
    if !metadata.file_type().is_file() {
        anyhow::bail!("migration manifest is not a regular file")
    }
    let (_, bytes) = super::safe_fs::read_file_nofollow_limited(&path, 4 * 1024 * 1024)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: MigrationManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid migration manifest {}", path.display()))?;
    validate_manifest(migration_id, &manifest, expected_project, root)?;
    Ok(manifest)
}

pub(crate) fn read_manifest_from_directory(
    migration_id: &str,
    root: &Path,
    expected_project: &str,
    directory: &fs::File,
) -> Result<MigrationManifest> {
    let name = std::ffi::CString::new("manifest.json")?;
    let (_, bytes) = super::safe_fs::read_file_at(directory, &name, 4 * 1024 * 1024)
        .with_context(|| format!("failed to read staged manifest {}", root.display()))?;
    let manifest: MigrationManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid migration manifest {}", root.display()))?;
    validate_manifest_fields(migration_id, &manifest, expected_project)?;
    Ok(manifest)
}

pub(crate) fn validate_manifest_payload_at(
    manifest: &MigrationManifest,
    root: &fs::File,
) -> Result<()> {
    if manifest.phase == MigrationPhase::RolledBack {
        return Ok(());
    }
    let files_name = std::ffi::CString::new("files")?;
    let files_directory = super::safe_fs::open_directory_at(root, &files_name)?;
    for file in &manifest.files {
        let (parent, name) = open_backup_parent_at(&files_directory, &file.relative_path)?;
        let limit = usize::try_from(file.size)
            .map_err(|_| anyhow::anyhow!("migration backup file size is too large"))?;
        let (metadata, bytes) = super::safe_fs::read_file_at(&parent, &name, limit)?;
        if bytes.len() as u64 != file.size || super::inventory::sha256(&bytes) != file.sha256 {
            anyhow::bail!("migration backup source hash mismatch")
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() != file.mode {
                anyhow::bail!("migration backup mode mismatch")
            }
        }
    }
    Ok(())
}

pub(crate) fn write_manifest(project: &Path, manifest: &mut MigrationManifest) -> Result<()> {
    let root = safe_backup_root(project, &manifest.migration_id)?;
    write_manifest_path_at(project, manifest, &root)
}

pub(crate) fn write_manifest_at(
    project: &Path,
    manifest: &mut MigrationManifest,
    staging: &OwnedStaging,
) -> Result<()> {
    let _ = project;
    if manifest.schema != MIGRATION_SCHEMA {
        anyhow::bail!("unsupported migration manifest schema")
    }
    super::safe_fs::verify_directory_identity(&staging.path, &staging.directory)?;
    manifest.manifest_hash = integrity_hash(manifest)?;
    let bytes = serde_json::to_vec_pretty(manifest)?;
    let name = std::ffi::CString::new("manifest.json")?;
    match super::safe_fs::create_linked_file_at(&staging.directory, &name, &bytes, 0o600)
        .context("failed to write staging manifest")?
    {
        super::safe_fs::CreateResult::Created => {}
        super::safe_fs::CreateResult::Exists => {
            anyhow::bail!("migration staging manifest already exists")
        }
    }
    Ok(())
}

fn write_manifest_path_at(
    project: &Path,
    manifest: &mut MigrationManifest,
    root: &Path,
) -> Result<()> {
    if manifest.schema != MIGRATION_SCHEMA {
        anyhow::bail!("unsupported migration manifest schema")
    }
    let relative_root = root
        .strip_prefix(project)
        .with_context(|| format!("migration root escaped project: {}", root.display()))?;
    safe_relative_parent(
        project,
        &format!("{}/manifest.json", relative_root.display()),
    )?;
    if !fs::symlink_metadata(root)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        anyhow::bail!("migration backup root is not a regular directory")
    }
    manifest.manifest_hash = integrity_hash(manifest)?;
    let path = root.join("manifest.json");
    let bytes = serde_json::to_vec_pretty(manifest)?;
    #[cfg(unix)]
    {
        let (parent, name) = super::safe_fs::open_parent_nofollow(&path)?;
        super::safe_fs::replace_file_at(&parent, &name, &bytes)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        super::safe_fs::replace_file_nofollow(&path, &bytes)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn safe_backup_root(project: &Path, migration_id: &str) -> Result<PathBuf> {
    validate_id(migration_id)?;
    ensure_no_symlink_ancestors(project, Path::new(".megara/migration-backups"))?;
    let root = backup_root(project, migration_id);
    if fs::symlink_metadata(&root)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        anyhow::bail!("migration backup root is a symlink: {}", root.display())
    }
    if fs::symlink_metadata(root.join("manifest.json"))
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        anyhow::bail!("migration manifest is a symlink")
    }
    Ok(root)
}

pub(crate) fn safe_backup_root_parent(project: &Path) -> Result<()> {
    ensure_no_symlink_ancestors(project, Path::new(".megara/migration-backups"))
}

pub(crate) fn safe_relative_path(project: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        anyhow::bail!("unsafe migration relative path: {relative}")
    }
    ensure_no_symlink_ancestors(project, relative_path)?;
    Ok(project.join(relative_path))
}

pub(crate) fn safe_relative_parent(project: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let relative_path = Path::new(relative);
    let parent = relative_path.parent().unwrap_or_else(|| Path::new("."));
    ensure_no_symlink_ancestors(project, parent)?;
    Ok(project.join(relative_path))
}

fn validate_manifest(
    migration_id: &str,
    manifest: &MigrationManifest,
    expected_project: &str,
    backup_root: &Path,
) -> Result<()> {
    validate_manifest_fields(migration_id, manifest, expected_project)?;
    validate_manifest_payload_path(manifest, backup_root)
}

fn validate_manifest_fields(
    migration_id: &str,
    manifest: &MigrationManifest,
    expected_project: &str,
) -> Result<()> {
    if manifest.schema != MIGRATION_SCHEMA
        || manifest.migration_id != migration_id
        || manifest.project_id != expected_project
    {
        anyhow::bail!("migration manifest identity mismatch")
    }
    if manifest.manifest_hash != integrity_hash(manifest)? {
        anyhow::bail!("migration manifest integrity hash mismatch")
    }
    let all_hash = record_hash(&manifest.files, None);
    let opaque_hash = record_hash(&manifest.files, Some("opaque"));
    if !valid_sha256(&manifest.backup_bundle_hash)
        || !valid_sha256(&manifest.source_bundle_hash)
        || manifest.backup_bundle_hash != all_hash
        || manifest.source_bundle_hash != opaque_hash
    {
        anyhow::bail!("migration manifest source hash mismatch")
    }
    if manifest.files.len() > crate::planning::engine::LEGACY_MAX_FILES {
        anyhow::bail!("migration manifest contains too many files")
    }
    if manifest.import_command_id.is_empty()
        || manifest.import_command_id.len() > crate::planning::engine::LEGACY_MAX_METADATA_BYTES
        || manifest.session_id.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > crate::planning::engine::LEGACY_MAX_METADATA_BYTES
        })
        || manifest.session_id.is_some() != manifest.revision.is_some()
        || manifest
            .rollback_export_sha256
            .as_ref()
            .is_some_and(|value| !valid_sha256(value))
    {
        anyhow::bail!("migration manifest binding fields are invalid")
    }
    let mut previous = None;
    let mut declared_total = 0u64;
    for file in &manifest.files {
        validate_file_fields(
            &file.relative_path,
            &file.sha256,
            file.size,
            file.mode,
            &file.kind,
        )?;
        declared_total = declared_total
            .checked_add(file.size)
            .ok_or_else(|| anyhow::anyhow!("migration manifest size overflow"))?;
        if declared_total > crate::planning::engine::LEGACY_CONTEXT_MAX_BYTES as u64 {
            anyhow::bail!("migration manifest exceeds decoded byte limit")
        }
        if previous.is_some_and(|previous: &str| previous >= file.relative_path.as_str()) {
            anyhow::bail!("migration manifest paths must be sorted and unique")
        }
        previous = Some(file.relative_path.as_str());
    }
    Ok(())
}

pub(crate) fn validate_file_fields(
    relative_path: &str,
    sha256: &str,
    size: u64,
    mode: u32,
    kind: &str,
) -> Result<()> {
    validate_relative_path(relative_path)?;
    if relative_path.len() > crate::planning::engine::LEGACY_MAX_PATH_BYTES
        || Path::new(relative_path).components().count() > MIGRATION_MAX_PATH_DEPTH
        || kind.len() > crate::planning::engine::LEGACY_MAX_METADATA_BYTES
        || !valid_sha256(sha256)
        || size > crate::planning::engine::LEGACY_CONTEXT_MAX_BYTES as u64
        || mode == 0
        || !matches!(
            kind,
            "opaque" | "managed_skill" | "managed_fragment" | "managed_hook"
        )
    {
        anyhow::bail!("migration file field is invalid")
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_manifest_payload_path(manifest: &MigrationManifest, backup_root: &Path) -> Result<()> {
    for file in &manifest.files {
        let backup = safe_relative_path(backup_root, &format!("files/{}", file.relative_path))?;
        if fs::symlink_metadata(&backup)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            anyhow::bail!("migration backup file is a symlink")
        }
        if manifest.phase != MigrationPhase::RolledBack {
            let limit = usize::try_from(file.size)
                .map_err(|_| anyhow::anyhow!("migration backup file size is too large"))?;
            let (metadata, bytes) = super::safe_fs::read_file_nofollow_limited(&backup, limit)?;
            if bytes.len() as u64 != file.size || super::inventory::sha256(&bytes) != file.sha256 {
                anyhow::bail!("migration backup source hash mismatch")
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() != file.mode {
                    anyhow::bail!("migration backup mode mismatch")
                }
            }
        }
    }
    Ok(())
}

fn open_backup_parent_at(
    files_directory: &fs::File,
    relative: &str,
) -> Result<(fs::File, std::ffi::CString)> {
    let mut components = Path::new(relative).components().peekable();
    let mut directory = files_directory.try_clone()?;
    let mut final_name = None;
    while let Some(component) = components.next() {
        let Component::Normal(part) = component else {
            anyhow::bail!("migration backup path is not normalized")
        };
        if components.peek().is_none() {
            final_name = Some(part);
            break;
        }
        let name = std::ffi::CString::new(
            part.to_str()
                .ok_or_else(|| anyhow::anyhow!("migration backup path is not UTF-8"))?,
        )?;
        directory = super::safe_fs::open_directory_at(&directory, &name)?;
    }
    let final_name =
        final_name.ok_or_else(|| anyhow::anyhow!("migration backup path has no file name"))?;
    let final_name = std::ffi::CString::new(
        final_name
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("migration backup path is not UTF-8"))?,
    )?;
    Ok((directory, final_name))
}

fn integrity_hash(manifest: &MigrationManifest) -> Result<String> {
    let mut value = serde_json::to_value(manifest)?;
    value["manifest_hash"] = serde_json::Value::String(String::new());
    Ok(crate::planning::canonical::canonical_hash(&value))
}

fn record_hash(files: &[super::types::MigrationFileRecord], kind: Option<&str>) -> String {
    let values = files
        .iter()
        .filter(|file| kind.is_none_or(|expected| file.kind == expected))
        .map(|file| {
            serde_json::json!({
                "path": file.relative_path,
                "sha256": file.sha256,
                "size": file.size,
            })
        })
        .collect::<Vec<_>>();
    crate::planning::canonical::canonical_hash(&values)
}

fn ensure_no_symlink_ancestors(project: &Path, relative: &Path) -> Result<()> {
    if fs::symlink_metadata(project)?.file_type().is_symlink() {
        anyhow::bail!("project root is a symlink")
    }
    let mut current = project.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        current.push(part);
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            anyhow::bail!(
                "migration path ancestor is a symlink: {}",
                current.display()
            )
        }
    }
    Ok(())
}

pub(crate) fn validate_relative_path(value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.contains('\\')
        || value.contains('\0')
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains("//")
    {
        anyhow::bail!("migration path is not normalized")
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("migration path contains a non-normal component")
    }
    Ok(())
}

pub(crate) fn validate_id(value: &str) -> Result<()> {
    let suffix = value.strip_prefix("mig_").unwrap_or_default();
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

pub(crate) fn transition(manifest: &mut MigrationManifest, next: MigrationPhase) -> Result<()> {
    let legal = matches!(
        (manifest.phase, next),
        (MigrationPhase::Prepared, MigrationPhase::PlanningImported)
            | (
                MigrationPhase::PlanningImported,
                MigrationPhase::ProjectionRemoved
            )
            | (MigrationPhase::ProjectionRemoved, MigrationPhase::Applied)
            | (MigrationPhase::Applied, MigrationPhase::RolledBack)
            | (
                MigrationPhase::ProjectionRemoved,
                MigrationPhase::RolledBack
            )
            | (MigrationPhase::PlanningImported, MigrationPhase::RolledBack)
            | (MigrationPhase::Prepared, MigrationPhase::RolledBack)
    );
    if !legal {
        anyhow::bail!("illegal migration phase transition")
    }
    manifest.phase = next;
    Ok(())
}
