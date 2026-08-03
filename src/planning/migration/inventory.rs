use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::bindings;
use super::types::MigrationFileRecord;

pub(crate) const MAX_VISITED_ENTRIES: usize = 4_096;
pub(crate) const MAX_WARNINGS: usize = 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LegacyFileKind {
    Opaque,
    ManagedSkill,
    ManagedFragment,
    ManagedHook,
}

impl LegacyFileKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::ManagedSkill => "managed_skill",
            Self::ManagedFragment => "managed_fragment",
            Self::ManagedHook => "managed_hook",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveredFile {
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub kind: LegacyFileKind,
    pub removable: bool,
    pub mode: u32,
}

impl DiscoveredFile {
    pub(crate) fn record(&self) -> MigrationFileRecord {
        MigrationFileRecord {
            relative_path: self.relative_path.display().to_string(),
            sha256: self.sha256.clone(),
            size: self.bytes.len() as u64,
            mode: self.mode,
            kind: self.kind.as_str().to_string(),
            removable: self.removable,
            removed: false,
        }
    }
}

pub(crate) struct Discovery {
    pub files: Vec<DiscoveredFile>,
    pub warnings: Vec<String>,
}

pub(crate) fn discover(root: &Path) -> Result<Discovery> {
    let mut paths = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut declared_total = 0u64;
    let mut visited_entries = 0usize;
    for relative in [
        ".agents/state/hooks",
        ".agents/state/workflows",
        ".agents/state/team",
        ".agents/artifacts/deep-interview",
        ".agents/artifacts/ralplan",
        ".agents/artifacts/ultragoal",
        ".megara/state/hooks",
        ".megara/state/workflows",
        ".megara/state/team",
        ".megara/artifacts/deep-interview",
        ".megara/artifacts/ralplan",
        ".megara/artifacts/ultragoal",
    ] {
        let path = super::journal::safe_relative_parent(root, relative)?;
        if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_dir())
            .unwrap_or(false)
        {
            collect_tree(
                root,
                &path,
                &mut paths,
                &mut warnings,
                &mut declared_total,
                &mut visited_entries,
            )?;
        } else if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            push_warning(
                &mut warnings,
                format!("legacy symlink root preserved without following: {relative}"),
            )?;
        }
    }
    for relative in managed_paths() {
        let path = super::journal::safe_relative_parent(root, relative)?;
        if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_file())
            .unwrap_or(false)
        {
            let metadata = fs::symlink_metadata(&path)?;
            account_visited(&mut visited_entries)?;
            account_candidate(
                relative,
                &path,
                metadata.len(),
                &mut paths,
                &mut declared_total,
            )?;
        } else if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            account_visited(&mut visited_entries)?;
            push_warning(
                &mut warnings,
                format!("legacy symlink candidate preserved without following: {relative}"),
            )?;
        }
    }

    let mut remaining = crate::planning::engine::LEGACY_CONTEXT_MAX_BYTES as u64;
    paths
        .into_iter()
        .map(|(relative, path)| {
            let limit = usize::try_from(remaining)
                .map_err(|_| anyhow::anyhow!("legacy inventory size bound is too large"))?;
            let (metadata, bytes) = super::safe_fs::read_file_nofollow_limited(&path, limit)
                .with_context(|| format!("failed to read legacy file {}", path.display()))?;
            remaining = remaining
                .checked_sub(bytes.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("legacy inventory exceeds decoded byte limit"))?;
            let kind = classify(&relative);
            let sha256 = sha256(&bytes);
            let removable = matches!(kind, LegacyFileKind::Opaque)
                || bindings::is_exact_legacy(&relative, &bytes);
            let mode = file_mode(&metadata)?;
            Ok(DiscoveredFile {
                relative_path: PathBuf::from(relative),
                bytes,
                sha256,
                kind,
                removable,
                mode,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(|files| Discovery { files, warnings })
}

pub(crate) fn source_bundle_hash(files: &[DiscoveredFile]) -> String {
    let value = files
        .iter()
        .map(|file| {
            json!({
                "path": file.relative_path,
                "sha256": file.sha256,
                "size": file.bytes.len(),
            })
        })
        .collect::<Vec<_>>();
    crate::planning::canonical::canonical_hash(&value)
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

pub(crate) fn record_paths(files: &[DiscoveredFile]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect()
}

fn collect_tree(
    root: &Path,
    path: &Path,
    output: &mut BTreeMap<String, PathBuf>,
    warnings: &mut Vec<String>,
    declared_total: &mut u64,
    visited_entries: &mut usize,
) -> Result<()> {
    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to inventory legacy directory {}", path.display()))?
    {
        let entry = entry?;
        account_visited(visited_entries)?;
        let current = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let relative = current.strip_prefix(root).with_context(|| {
                format!("legacy path escaped project root: {}", current.display())
            })?;
            validate_candidate_path(relative.to_str().ok_or_else(|| {
                anyhow::anyhow!("legacy path is not UTF-8: {}", current.display())
            })?)?;
            collect_tree(
                root,
                &current,
                output,
                warnings,
                declared_total,
                visited_entries,
            )?;
        } else if file_type.is_file() {
            let relative = current.strip_prefix(root).with_context(|| {
                format!("legacy path escaped project root: {}", current.display())
            })?;
            let relative = relative.to_str().ok_or_else(|| {
                anyhow::anyhow!("legacy path is not UTF-8: {}", current.display())
            })?;
            let metadata = fs::symlink_metadata(&current)?;
            account_candidate(relative, &current, metadata.len(), output, declared_total)?;
        } else if file_type.is_symlink() {
            let relative = current
                .strip_prefix(root)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| current.display().to_string());
            push_warning(
                warnings,
                format!("legacy symlink preserved without following: {relative}"),
            )?;
        }
    }
    Ok(())
}

fn push_warning(warnings: &mut Vec<String>, warning: String) -> Result<()> {
    if warnings.len() >= MAX_WARNINGS {
        anyhow::bail!("legacy inventory warning budget exceeded")
    }
    warnings.push(warning);
    Ok(())
}

fn account_visited(visited_entries: &mut usize) -> Result<()> {
    *visited_entries = visited_entries
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("legacy inventory visited-entry count overflow"))?;
    if *visited_entries > MAX_VISITED_ENTRIES {
        anyhow::bail!("legacy inventory visited-entry budget exceeded")
    }
    Ok(())
}

pub(crate) fn validate_candidate_path(relative: &str) -> Result<()> {
    super::journal::validate_relative_path(relative)?;
    if relative.len() > crate::planning::engine::LEGACY_MAX_PATH_BYTES
        || Path::new(relative).components().count() > super::journal::MIGRATION_MAX_PATH_DEPTH
    {
        anyhow::bail!("legacy inventory path is too large or deep")
    }
    Ok(())
}

fn account_candidate(
    relative: &str,
    path: &Path,
    size: u64,
    output: &mut BTreeMap<String, PathBuf>,
    declared_total: &mut u64,
) -> Result<()> {
    validate_candidate_path(relative)?;
    if size > crate::planning::engine::LEGACY_CONTEXT_MAX_BYTES as u64 {
        anyhow::bail!("legacy inventory file exceeds decoded byte limit: {relative}")
    }
    if !output.contains_key(relative) {
        if output.len() >= crate::planning::engine::LEGACY_MAX_FILES {
            anyhow::bail!("legacy inventory contains too many files")
        }
        *declared_total = declared_total
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("legacy inventory size overflow"))?;
        if *declared_total > crate::planning::engine::LEGACY_CONTEXT_MAX_BYTES as u64 {
            anyhow::bail!("legacy inventory exceeds decoded byte limit")
        }
        output.insert(relative.to_string(), path.to_path_buf());
    }
    Ok(())
}

fn managed_paths() -> &'static [&'static str] {
    &[
        ".codex/hooks.json",
        ".agents/skills/deep-interview/SKILL.md",
        ".agents/skills/ralplan/SKILL.md",
        ".agents/skills/team/SKILL.md",
        ".agents/skills/ultragoal/SKILL.md",
        ".codex/skills/deep-interview/SKILL.md",
        ".codex/skills/ralplan/SKILL.md",
        ".codex/skills/team/SKILL.md",
        ".codex/skills/ultragoal/SKILL.md",
        ".agents/skill-fragments/deep-interview/auto-answer-uncertain.md",
        ".agents/skill-fragments/deep-interview/auto-research-greenfield.md",
        ".agents/skill-fragments/deep-interview/lateral-review-panel.md",
        ".agents/skill-fragments/ultragoal/ai-slop-cleaner.md",
        ".codex/skill-fragments/deep-interview/auto-answer-uncertain.md",
        ".codex/skill-fragments/deep-interview/auto-research-greenfield.md",
        ".codex/skill-fragments/deep-interview/lateral-review-panel.md",
        ".codex/skill-fragments/ultragoal/ai-slop-cleaner.md",
    ]
}

fn classify(path: &str) -> LegacyFileKind {
    if path == ".codex/hooks.json" {
        LegacyFileKind::ManagedHook
    } else if path.contains("/skills/") {
        LegacyFileKind::ManagedSkill
    } else if path.contains("/skill-fragments/") {
        LegacyFileKind::ManagedFragment
    } else {
        LegacyFileKind::Opaque
    }
}

fn file_mode(metadata: &fs::Metadata) -> Result<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(metadata.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(0)
    }
}
