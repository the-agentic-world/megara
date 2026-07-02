use std::{env, path::PathBuf};

use anyhow::{Context, Result};

use crate::{
    paths::{InstallPaths, TargetRuntime},
    targets::codex,
    templates::TemplateRegistry,
    writer::write_files,
};

use super::model::*;

pub struct Planner<'a> {
    registry: &'a TemplateRegistry,
    options: InstallOptions,
}

impl<'a> Planner<'a> {
    pub fn new(registry: &'a TemplateRegistry, options: InstallOptions) -> Self {
        Self { registry, options }
    }

    pub fn plan(&self) -> Result<InstallPlan> {
        let paths = InstallPaths::resolve(self.options.scope, self.options.target)?;
        let mut files = Vec::new();
        files.extend(runtime_support_files(paths.ssot_root.clone())?);
        let projection_registry = match self.options.action {
            InstallAction::Install => {
                files.extend(ssot_files(paths.ssot_root.clone(), self.registry));
                self.registry.clone()
            }
            InstallAction::Sync => TemplateRegistry::from_ssot_root(&paths.ssot_root)?,
        };

        match self.options.target {
            TargetRuntime::Codex => files.extend(codex::projection_files(
                paths.target_root.clone(),
                self.options.scope,
                &projection_registry,
            )?),
        }

        Ok(InstallPlan {
            scope: self.options.scope,
            target: self.options.target,
            ssot_root: paths.ssot_root,
            target_root: paths.target_root,
            files,
        })
    }

    pub fn execute(&self) -> Result<InstallResult> {
        let plan = self.plan()?;
        let summary = write_files(&plan.files, self.options.dry_run, self.options.force)?;
        let hook_trust = match self.options.target {
            TargetRuntime::Codex => Some(codex::ensure_hook_trust(
                &plan.target_root.join("hooks.json"),
                self.options.dry_run,
            )?),
        };
        let mut warnings = runtime_dependency_issues(self.options.target);
        if matches!(self.options.action, InstallAction::Install)
            && self.options.target == TargetRuntime::Codex
        {
            warnings.push(
                "Codex App loads hooks when a session starts; open a new session after install for hooks to take effect."
                    .to_string(),
            );
        }
        Ok(InstallResult {
            options: self.options.clone(),
            plan,
            summary,
            hook_trust,
            warnings,
        })
    }
}

fn runtime_dependency_issues(target: TargetRuntime) -> Vec<String> {
    match target {
        TargetRuntime::Codex => codex::runtime_dependency_issues(),
    }
}

fn ssot_files(root: PathBuf, registry: &TemplateRegistry) -> Vec<PlannedFile> {
    registry
        .ssot_files()
        .iter()
        .map(|template| {
            PlannedFile::new(root.join(&template.relative_path), template.content.clone())
        })
        .collect()
}

pub(crate) fn runtime_support_files(root: PathBuf) -> Result<Vec<PlannedFile>> {
    let megara_bin = env::current_exe().context("failed to resolve current megara executable")?;
    Ok(vec![PlannedFile::new_executable_shell(
        root.join("bin").join("megara"),
        format!(
            "#!/bin/sh\nexec {} \"$@\"\n",
            shell_quote(&megara_bin.display().to_string())
        ),
    )])
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
