use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::MigrationFileRecord;

#[path = "staging_tree.rs"]
mod tree;

const STAGING_MARKER_MAX_BYTES: usize = 4 * 1024 * 1024;

pub(crate) struct OwnedStaging {
    pub path: PathBuf,
    pub directory: fs::File,
    pub namespace: fs::File,
    pub namespace_parent: fs::File,
}

pub(crate) fn staging_root(project: &Path, migration_id: &str) -> PathBuf {
    project
        .join(".megara/migration-backups/.staging")
        .join(migration_id)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StagingMarker {
    schema: String,
    migration_id: String,
    project_id: String,
    nonce: String,
    inventory_hash: String,
    files: Vec<StagingFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StagingFile {
    relative_path: String,
    sha256: String,
    size: u64,
    mode: u32,
    kind: String,
}

pub(crate) enum StagingState {
    Partial,
    Prepared(Box<super::types::MigrationManifest>),
}

pub(crate) fn prepare(
    project: &Path,
    migration_id: &str,
    project_id: &str,
    files: &[MigrationFileRecord],
) -> Result<OwnedStaging> {
    super::journal::validate_id(migration_id)?;
    let marker = StagingMarker {
        schema: "megara.planning-migration-staging/v1".to_string(),
        migration_id: migration_id.to_string(),
        project_id: project_id.to_string(),
        nonce: Uuid::now_v7().to_string(),
        inventory_hash: inventory_hash(files),
        files: files
            .iter()
            .map(|file| StagingFile {
                relative_path: file.relative_path.clone(),
                sha256: file.sha256.clone(),
                size: file.size,
                mode: file.mode,
                kind: file.kind.clone(),
            })
            .collect(),
    };
    validate_marker_files(&marker.files)?;
    let marker_bytes = serde_json::to_vec_pretty(&marker)?;
    if marker_bytes.len() > STAGING_MARKER_MAX_BYTES {
        anyhow::bail!("migration staging marker exceeds bounded size")
    }
    let staging = staging_root(project, migration_id);
    let namespace = staging
        .parent()
        .ok_or_else(|| anyhow::anyhow!("migration staging has no namespace"))?;
    super::journal::safe_relative_parent(
        project,
        ".megara/migration-backups/.staging/placeholder",
    )?;
    if fs::symlink_metadata(namespace)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        anyhow::bail!("migration staging namespace is a symlink")
    }
    let namespace_directory = super::safe_fs::ensure_directory_nofollow_open(namespace)?;
    let namespace_parent = namespace
        .parent()
        .ok_or_else(|| anyhow::anyhow!("migration staging has no parent"))?;
    let namespace_parent_directory = super::safe_fs::open_directory_nofollow(namespace_parent)?;
    let staging_directory =
        super::safe_fs::create_directory_at(&namespace_directory, migration_id, 0o700)
            .map_err(anyhow::Error::from)
            .with_context(|| format!("failed to create owned staging {}", staging.display()))?;
    let marker_name = std::ffi::CString::new("staging.json")?;
    match super::safe_fs::create_linked_file_at(
        &staging_directory,
        &marker_name,
        &marker_bytes,
        0o600,
    )
    .map_err(anyhow::Error::from)?
    {
        super::safe_fs::CreateResult::Created => {}
        super::safe_fs::CreateResult::Exists => anyhow::bail!("migration staging marker exists"),
    }
    staging_directory.sync_all()?;
    namespace_directory.sync_all()?;
    Ok(OwnedStaging {
        path: staging,
        directory: staging_directory,
        namespace: namespace_directory,
        namespace_parent: namespace_parent_directory,
    })
}

pub(crate) fn remove_owned_at(
    project: &Path,
    staging: &Path,
    migration_id: &str,
    project_id: &str,
    namespace: &fs::File,
    entry_name: &std::ffi::CStr,
    entry: &fs::File,
) -> Result<()> {
    let _ = validate_at(project, staging, migration_id, project_id, entry)?;
    super::remove_tree_at_validated(namespace, entry_name, entry, |held| {
        super::validate_staging_held(project, staging, migration_id, project_id, held)
            .map(|_| ())
            .map_err(|error| io::Error::other(format!("MIGRATION_INCOMPLETE: {error}")))
    })?;
    Ok(())
}

pub(crate) fn validate_at(
    project: &Path,
    staging: &Path,
    migration_id: &str,
    project_id: &str,
    directory: &fs::File,
) -> Result<StagingState> {
    super::safe_fs::verify_directory_identity(staging, directory)?;
    validate_held(project, staging, migration_id, project_id, directory)
}

pub(crate) fn validate_held(
    project: &Path,
    staging: &Path,
    migration_id: &str,
    project_id: &str,
    directory: &fs::File,
) -> Result<StagingState> {
    let relative = staging
        .strip_prefix(project)
        .with_context(|| format!("staging escaped project: {}", staging.display()))?;
    super::journal::safe_relative_parent(project, &format!("{}/marker", relative.display()))?;
    if !directory.metadata()?.file_type().is_dir() {
        anyhow::bail!("staging root is not a directory")
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = directory.metadata()?.permissions().mode() & 0o777;
        if mode != 0o700 {
            anyhow::bail!("staging root mode is not 0700")
        }
    }
    let marker_name = std::ffi::CString::new("staging.json")?;
    let (marker_metadata, bytes) =
        super::safe_fs::read_file_at(directory, &marker_name, STAGING_MARKER_MAX_BYTES)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if marker_metadata.permissions().mode() & 0o777 != 0o600 {
            anyhow::bail!("staging marker mode is not 0600")
        }
    }
    let marker: StagingMarker = serde_json::from_slice(&bytes)?;
    if marker.schema != "megara.planning-migration-staging/v1"
        || marker.migration_id != migration_id
        || marker.project_id != project_id
        || Uuid::parse_str(&marker.nonce).is_err()
        || marker.inventory_hash != inventory_hash_from_marker(&marker.files)
    {
        anyhow::bail!("staging marker identity mismatch")
    }
    validate_marker_files(&marker.files)?;
    let files = marker
        .files
        .iter()
        .map(|file| (file.relative_path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    if files.len() != marker.files.len() {
        anyhow::bail!("staging inventory contains duplicate paths")
    }
    let manifest_name = std::ffi::CString::new("manifest.json")?;
    let prepared_manifest =
        match super::safe_fs::read_file_at(directory, &manifest_name, STAGING_MARKER_MAX_BYTES) {
            Ok((metadata, _)) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o777 != 0o600 {
                        anyhow::bail!("staging manifest mode is not 0600")
                    }
                }
                let manifest = super::journal::read_manifest_from_directory(
                    migration_id,
                    staging,
                    project_id,
                    directory,
                )?;
                Some(manifest)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
    if let Some(manifest) = &prepared_manifest {
        if manifest.phase != super::types::MigrationPhase::Prepared
            || manifest.session_id.is_some()
            || manifest.revision.is_some()
            || manifest.rollback_export_sha256.is_some()
            || manifest.files.iter().any(|file| file.removed)
            || manifest.import_command_id != super::import::command_id(manifest)
            || inventory_hash(&manifest.files) != marker.inventory_hash
        {
            anyhow::bail!("staging manifest does not match marker inventory")
        }
    }
    let complete = prepared_manifest.is_some();
    let mut seen = BTreeSet::new();
    tree::validate_tree_at(directory, &files, complete, &mut seen)?;
    if complete && seen != files.keys().cloned().collect() {
        anyhow::bail!("staging backup inventory is incomplete")
    }
    Ok(match prepared_manifest {
        Some(manifest) => StagingState::Prepared(Box::new(manifest)),
        None => StagingState::Partial,
    })
}

fn validate_marker_files(files: &[StagingFile]) -> Result<()> {
    if files.len() > crate::planning::engine::LEGACY_MAX_FILES {
        anyhow::bail!("staging inventory contains too many files")
    }
    let mut previous = None;
    let mut declared_total = 0u64;
    for file in files {
        super::journal::validate_file_fields(
            &file.relative_path,
            &file.sha256,
            file.size,
            file.mode,
            &file.kind,
        )?;
        declared_total = declared_total
            .checked_add(file.size)
            .ok_or_else(|| anyhow::anyhow!("staging inventory size overflow"))?;
        if declared_total > crate::planning::engine::LEGACY_CONTEXT_MAX_BYTES as u64 {
            anyhow::bail!("staging inventory exceeds decoded byte limit")
        }
        if previous.is_some_and(|value: &str| value >= file.relative_path.as_str()) {
            anyhow::bail!("staging inventory must be sorted and unique")
        }
        previous = Some(file.relative_path.as_str());
    }
    Ok(())
}

fn inventory_hash(files: &[MigrationFileRecord]) -> String {
    let values = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "relative_path": file.relative_path,
                "sha256": file.sha256,
                "size": file.size,
                "mode": file.mode,
                "kind": file.kind,
            })
        })
        .collect::<Vec<_>>();
    crate::planning::canonical::canonical_hash(&values)
}

fn inventory_hash_from_marker(files: &[StagingFile]) -> String {
    let values = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "relative_path": file.relative_path,
                "sha256": file.sha256,
                "size": file.size,
                "mode": file.mode,
                "kind": file.kind,
            })
        })
        .collect::<Vec<_>>();
    crate::planning::canonical::canonical_hash(&values)
}
