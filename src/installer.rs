use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::{
    cli::{resolve_scope, resolve_target, DoctorArgs, InstallArgs},
    paths::{InstallPaths, InstallScope, TargetRuntime},
    targets::codex,
    templates::TemplateRegistry,
    writer::{write_files, WriteSummary},
};

pub const MANAGED_MARKER: &str = "MEGARA:MANAGED";

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
            dry_run: args.dry_run,
            force: args.force,
            json: args.json,
        })
    }
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
}

impl PlannedFile {
    pub fn new(path: PathBuf, content: impl Into<String>) -> Self {
        let content = add_marker(&path, content.into());
        Self { path, content }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallPlan {
    pub scope: InstallScope,
    pub target: TargetRuntime,
    pub ssot_root: PathBuf,
    pub target_root: PathBuf,
    pub files: Vec<PlannedFile>,
}

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

        let projection_registry = match self.options.action {
            InstallAction::Install => {
                files.extend(ssot_files(paths.ssot_root.clone(), self.registry));
                self.registry.clone()
            }
            InstallAction::Sync => TemplateRegistry::from_ssot_root(&paths.ssot_root)?,
        };

        match self.options.target {
            TargetRuntime::Codex => {
                files.extend(codex::projection_files(
                    paths.target_root.clone(),
                    &projection_registry,
                ));
            }
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
        Ok(InstallResult {
            options: self.options.clone(),
            plan,
            summary,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallResult {
    pub options: InstallOptions,
    pub plan: InstallPlan,
    pub summary: WriteSummary,
}

impl InstallResult {
    pub fn print(&self) -> Result<()> {
        if self.options.json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }

        let verb = match (self.options.action, self.options.dry_run) {
            (InstallAction::Install, true) => "install planned",
            (InstallAction::Install, false) => "installed",
            (InstallAction::Sync, true) => "sync planned",
            (InstallAction::Sync, false) => "synced",
        };
        println!(
            "megara {verb}: scope={}, target={}, ssot={}, projection={}",
            self.plan.scope,
            self.plan.target,
            self.plan.ssot_root.display(),
            self.plan.target_root.display()
        );
        println!(
            "created={}, updated={}, unchanged={}, conflicts={}",
            self.summary.created.len(),
            self.summary.updated.len(),
            self.summary.unchanged.len(),
            self.summary.conflicts.len()
        );

        if !self.summary.conflicts.is_empty() {
            println!("conflicts:");
            for path in &self.summary.conflicts {
                println!("- {}", path.display());
            }
        }

        Ok(())
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

fn add_marker(path: &std::path::Path, content: String) -> String {
    if path
        .extension()
        .is_some_and(|extension| extension == "toml")
    {
        format!(
            "# {MANAGED_MARKER} generated by Megara. Edit SSOT and run `megara sync`.\n{content}"
        )
    } else if let Some(stripped) = content.strip_prefix("---\n") {
        if let Some(frontmatter_end) = stripped.find("\n---\n") {
            let split_at = 4 + frontmatter_end + "\n---\n".len();
            return format!(
                "{}<!-- {MANAGED_MARKER} generated by Megara. Edit SSOT and run `megara sync`. -->\n{}",
                &content[..split_at],
                &content[split_at..]
            );
        }
        format!("<!-- {MANAGED_MARKER} generated by Megara. Edit SSOT and run `megara sync`. -->\n{content}")
    } else {
        format!("<!-- {MANAGED_MARKER} generated by Megara. Edit SSOT and run `megara sync`. -->\n{content}")
    }
}

pub fn strip_managed_marker(content: &str) -> String {
    let mut stripped = String::new();
    for line in content.split_inclusive('\n') {
        if !line.contains(MANAGED_MARKER) {
            stripped.push_str(line);
        }
    }
    stripped
}
