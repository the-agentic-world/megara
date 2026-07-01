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
        }
    }

    Ok(summary)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn protects_unmanaged_files_without_force() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        fs::write(&path, "manual").unwrap();

        let file = PlannedFile::new(path.clone(), "generated");
        let summary = write_files(&[file], true, false).unwrap();

        assert_eq!(summary.conflicts, vec![path]);
    }

    #[test]
    fn conflicts_do_not_partially_write() {
        let dir = tempdir().unwrap();
        let conflict = dir.path().join("AGENTS.md");
        let created = dir.path().join("new.md");
        fs::write(&conflict, "manual").unwrap();

        let files = vec![
            PlannedFile::new(conflict, "generated"),
            PlannedFile::new(created.clone(), "generated"),
        ];
        let err = write_files(&files, false, false).unwrap_err();

        assert!(err.to_string().contains("refusing to overwrite"));
        assert!(!created.exists());
    }

    #[test]
    fn force_updates_unmanaged_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        fs::write(&path, "manual").unwrap();

        let file = PlannedFile::new(path.clone(), "generated");
        let summary = write_files(&[file], false, true).unwrap();

        assert_eq!(summary.updated, vec![path.clone()]);
        assert!(fs::read_to_string(path).unwrap().contains(MANAGED_MARKER));
    }
}
