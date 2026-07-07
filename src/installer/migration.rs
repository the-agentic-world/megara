use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::model::StateMigrationSummary;

pub(super) fn migrate_legacy_project_state(
    ssot_root: &Path,
    runtime_root: &Path,
    dry_run: bool,
) -> Result<Option<StateMigrationSummary>> {
    if ssot_root == runtime_root {
        return Ok(None);
    }

    let source = ssot_root.join("state");
    if !source.exists() {
        return Ok(None);
    }

    let destination = runtime_root.join("state");
    let mut summary = StateMigrationSummary {
        source: source.clone(),
        destination: destination.clone(),
        moved: Vec::new(),
        conflicts: Vec::new(),
        removed_source: false,
    };

    migrate_dir(&source, &destination, &source, dry_run, &mut summary)?;
    summary.removed_source = !dry_run && !source.exists();
    Ok(Some(summary))
}

fn migrate_dir(
    source: &Path,
    destination: &Path,
    root: &Path,
    dry_run: bool,
    summary: &mut StateMigrationSummary,
) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    if destination.exists() && !destination.is_dir() {
        summary.conflicts.push(relative_to(root, source));
        return Ok(());
    }

    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read legacy state {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            migrate_dir(&source_path, &destination_path, root, dry_run, summary)?;
            remove_empty_dir(&source_path, dry_run)?;
            continue;
        }

        migrate_file(&source_path, &destination_path, root, dry_run, summary)?;
    }

    remove_empty_dir(source, dry_run)?;
    Ok(())
}

fn migrate_file(
    source: &Path,
    destination: &Path,
    root: &Path,
    dry_run: bool,
    summary: &mut StateMigrationSummary,
) -> Result<()> {
    if destination.exists() {
        if same_file_content(source, destination)? {
            summary.moved.push(relative_to(root, source));
            if !dry_run {
                fs::remove_file(source)
                    .with_context(|| format!("failed to remove {}", source.display()))?;
            }
        } else {
            summary.conflicts.push(relative_to(root, source));
        }
        return Ok(());
    }

    summary.moved.push(relative_to(root, source));
    if dry_run {
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::rename(source, destination).with_context(|| {
        format!(
            "failed to move {} to {}",
            source.display(),
            destination.display()
        )
    })
}

fn same_file_content(left: &Path, right: &Path) -> Result<bool> {
    let left = fs::read(left).with_context(|| format!("failed to read {}", left.display()))?;
    let right = fs::read(right).with_context(|| format!("failed to read {}", right.display()))?;
    Ok(left == right)
}

fn remove_empty_dir(path: &Path, dry_run: bool) -> Result<()> {
    if dry_run || !path.exists() || fs::read_dir(path)?.next().is_some() {
        return Ok(());
    }
    fs::remove_dir(path).with_context(|| format!("failed to remove {}", path.display()))
}

fn relative_to(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}
