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

#[derive(Clone, Debug, Serialize)]
pub struct InstallOptions {
    pub scope: InstallScope,
    pub target: TargetRuntime,
    pub dry_run: bool,
    pub force: bool,
    pub json: bool,
}

impl InstallOptions {
    pub fn resolve(args: InstallArgs, interactive: bool) -> Result<Self> {
        Ok(Self {
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
        let mut files = ssot_files(paths.ssot_root.clone(), self.registry);
        match self.options.target {
            TargetRuntime::Codex => {
                files.extend(codex::projection_files(
                    paths.target_root.clone(),
                    self.registry,
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

        let verb = if self.options.dry_run {
            "planned"
        } else {
            "installed"
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
    let mut files = vec![
        PlannedFile::new(root.join("megara.toml"), ssot_config(registry)),
        PlannedFile::new(root.join("README.md"), ssot_readme(registry)),
        PlannedFile::new(root.join("rules").join("planning.md"), planning_rule()),
    ];

    for workflow in registry.workflows() {
        files.push(PlannedFile::new(
            root.join("skills").join(workflow.name).join("SKILL.md"),
            workflow.content,
        ));
    }

    for agent in registry.agents() {
        files.push(PlannedFile::new(
            root.join("agents").join(format!("{}.md", agent.name)),
            agent.content,
        ));
    }

    files
}

fn ssot_config(registry: &TemplateRegistry) -> String {
    let workflows = registry
        .workflows()
        .iter()
        .map(|workflow| format!("\"{}\"", workflow.name))
        .collect::<Vec<_>>()
        .join(", ");
    let agents = registry
        .agents()
        .iter()
        .map(|agent| format!("\"{}\"", agent.name))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"schema_version = 1
targets = ["codex"]
enabled_workflows = [{workflows}]
enabled_agents = [{agents}]

[target.codex]
enabled = true
"#
    )
}

fn ssot_readme(registry: &TemplateRegistry) -> String {
    let workflows = registry
        .workflows()
        .iter()
        .map(|workflow| format!("- {}", workflow.name))
        .collect::<Vec<_>>()
        .join("\n");
    let agents = registry
        .agents()
        .iter()
        .map(|agent| format!("- {}", agent.name))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# Megara Harness

This directory is the Megara source of truth.

Run `megara sync` after editing these files.

## Workflows

{workflows}

## Agents

{agents}
"#
    )
}

fn planning_rule() -> &'static str {
    r#"# Planning Rule

- Ask clarifying questions only when the answer changes implementation.
- Convert ambiguous work into concrete acceptance criteria.
- Keep execution scoped to the accepted plan.
"#
}

fn add_marker(path: &std::path::Path, content: String) -> String {
    if path
        .extension()
        .is_some_and(|extension| extension == "toml")
    {
        format!(
            "# {MANAGED_MARKER} generated by Megara. Edit SSOT and run `megara sync`.\n{content}"
        )
    } else {
        format!("<!-- {MANAGED_MARKER} generated by Megara. Edit SSOT and run `megara sync`. -->\n{content}")
    }
}
