use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::installer::{PlannedFile, MANAGED_MARKER};

#[derive(Clone, Debug, Default, Serialize)]
pub struct WriteSummary {
    pub created: Vec<PathBuf>,
    pub updated: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
    pub conflicts: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

pub fn write_files(files: &[PlannedFile], dry_run: bool, force: bool) -> Result<WriteSummary> {
    let mut summary = WriteSummary::default();
    let mut actions = Vec::new();

    for file in files {
        match classify(&file.path, &file.content, force)? {
            WriteAction::Create => {
                summary.created.push(file.path.clone());
                actions.push((file, WriteAction::Create));
            }
            WriteAction::Update => {
                summary.updated.push(file.path.clone());
                actions.push((file, WriteAction::Update));
            }
            WriteAction::Unchanged => {
                summary.unchanged.push(file.path.clone());
                actions.push((file, WriteAction::Unchanged));
            }
            WriteAction::Conflict => {
                summary.conflicts.push(file.path.clone());
                actions.push((file, WriteAction::Conflict));
            }
        }
    }

    if !summary.conflicts.is_empty() && !dry_run {
        bail!(
            "refusing to overwrite {} unmanaged file(s); rerun with --force",
            summary.conflicts.len()
        );
    }

    if !dry_run {
        for (file, action) in actions {
            if matches!(action, WriteAction::Create | WriteAction::Update) {
                write_one(file)?;
            }
            if !matches!(action, WriteAction::Conflict) {
                ensure_mode(file)?;
            }
        }
    }

    Ok(summary)
}

pub fn remove_managed_files(paths: &[PathBuf], dry_run: bool) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        let current = fs::read_to_string(path)
            .with_context(|| format!("failed to read existing file {}", path.display()))?;
        if !current.contains(MANAGED_MARKER) {
            continue;
        }
        removed.push(path.clone());
    }

    if !dry_run {
        for path in &removed {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            remove_empty_parent_dirs(path);
        }
    }

    Ok(removed)
}

enum WriteAction {
    Create,
    Update,
    Unchanged,
    Conflict,
}

fn classify(path: &Path, desired: &str, force: bool) -> Result<WriteAction> {
    if !path.exists() {
        return Ok(WriteAction::Create);
    }

    let current = fs::read_to_string(path)
        .with_context(|| format!("failed to read existing file {}", path.display()))?;
    if current == desired {
        return Ok(WriteAction::Unchanged);
    }
    if force || current.contains(MANAGED_MARKER) {
        return Ok(WriteAction::Update);
    }
    Ok(WriteAction::Conflict)
}

fn write_one(file: &PlannedFile) -> Result<()> {
    if let Some(parent) = file.path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let mut handle = fs::File::create(&file.path)
        .with_context(|| format!("failed to create {}", file.path.display()))?;
    handle
        .write_all(file.content.as_bytes())
        .with_context(|| format!("failed to write {}", file.path.display()))?;
    Ok(())
}

fn ensure_mode(file: &PlannedFile) -> Result<()> {
    if !file.executable {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(&file.path)
            .with_context(|| format!("failed to stat {}", file.path.display()))?;
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 == 0o111 {
            return Ok(());
        }
        permissions.set_mode(mode | 0o755);
        fs::set_permissions(&file.path, permissions)
            .with_context(|| format!("failed to chmod {}", file.path.display()))?;
    }

    Ok(())
}

fn remove_empty_parent_dirs(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    let _ = fs::remove_dir(parent);
    if let Some(grandparent) = parent.parent() {
        let _ = fs::remove_dir(grandparent);
    }
}
