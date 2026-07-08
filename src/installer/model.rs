use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Serialize;

use crate::{
    cli::{resolve_scope, resolve_target, DoctorArgs, InstallArgs},
    paths::{InstallScope, TargetRuntime},
    targets::codex,
    writer::WriteSummary,
};

use super::marker::{add_marker, add_shell_marker};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallAction {
    Install,
    Sync,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallOptions {
    pub action: InstallAction,
    pub scope: InstallScope,
    pub target: TargetRuntime,
    pub locale: Option<String>,
    pub dry_run: bool,
    pub force: bool,
    pub json: bool,
}

impl InstallOptions {
    pub fn resolve(args: InstallArgs, interactive: bool, action: InstallAction) -> Result<Self> {
        Ok(Self {
            action,
            scope: resolve_scope(args.scope, interactive)?,
            target: resolve_target(args.target, interactive)?,
            locale: normalize_locale(args.locale)?,
            dry_run: args.dry_run,
            force: args.force,
            json: args.json,
        })
    }
}

fn normalize_locale(locale: Option<String>) -> Result<Option<String>> {
    let Some(locale) = locale else {
        return Ok(None);
    };
    let locale = locale.trim().to_string();
    if locale.is_empty() {
        bail!("--locale must not be empty");
    }
    if locale.chars().any(char::is_control) {
        bail!("--locale must not contain control characters");
    }
    Ok(Some(locale))
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorOptions {
    pub scope: InstallScope,
    pub target: TargetRuntime,
    pub json: bool,
}

impl DoctorArgs {
    pub fn resolve(self) -> Result<DoctorOptions> {
        Ok(DoctorOptions {
            scope: resolve_scope(self.scope, false)?,
            target: resolve_target(self.target, false)?,
            json: self.json,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub content: String,
    pub executable: bool,
}

impl PlannedFile {
    pub fn new(path: PathBuf, content: impl Into<String>) -> Self {
        let content = add_marker(&path, content.into());
        Self {
            path,
            content,
            executable: false,
        }
    }

    pub fn new_executable_shell(path: PathBuf, content: impl Into<String>) -> Self {
        let content = add_shell_marker(content.into());
        Self {
            path,
            content,
            executable: true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallPlan {
    pub scope: InstallScope,
    pub target: TargetRuntime,
    pub ssot_root: PathBuf,
    pub runtime_root: PathBuf,
    pub target_root: PathBuf,
    pub files: Vec<PlannedFile>,
    pub obsolete_files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct StateMigrationSummary {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub moved: Vec<PathBuf>,
    pub conflicts: Vec<PathBuf>,
    pub removed_source: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallResult {
    pub options: InstallOptions,
    pub plan: InstallPlan,
    pub summary: WriteSummary,
    pub migrations: Vec<StateMigrationSummary>,
    pub hook_trust: Option<codex::HookTrustSummary>,
    pub warnings: Vec<String>,
}
