use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::paths::{InstallScope, TargetRuntime};

#[derive(Debug, Parser)]
#[command(name = "megara", version, about = "Install portable agent harnesses")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run the harness installer wizard.
    Install(InstallArgs),
    /// Reproject managed runtime files from the Megara SSOT.
    Sync(InstallArgs),
    /// Inspect installation health and drift.
    Doctor(DoctorArgs),
    /// Inspect bundled harness templates.
    Templates {
        #[command(subcommand)]
        command: TemplateCommands,
    },
    /// Inspect supported agent runtimes.
    Targets {
        #[command(subcommand)]
        command: TargetCommands,
    },
    /// Internal runtime hook entrypoint.
    #[command(hide = true)]
    Hook(HookArgs),
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[arg(long, hide = true)]
    pub managed_marker: Option<String>,
    #[arg(long, value_enum, default_value = "project")]
    pub scope: ScopeArg,
    #[arg(long)]
    pub project_root: Option<PathBuf>,
    #[arg(long, default_value = "codex")]
    pub runtime: String,
    #[arg(long)]
    pub event: String,
    #[arg(long)]
    pub matcher: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommands {
    /// List bundled workflow and agent templates.
    List(JsonArgs),
    /// Print a bundled template by name.
    Show(ShowTemplateArgs),
}

#[derive(Debug, Subcommand)]
pub enum TargetCommands {
    /// List supported target runtimes.
    List(JsonArgs),
}

#[derive(Debug, Args)]
pub struct JsonArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ShowTemplateArgs {
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ScopeArg {
    Global,
    Project,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum TargetArg {
    Codex,
}

impl From<ScopeArg> for InstallScope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::Global => InstallScope::Global,
            ScopeArg::Project => InstallScope::Project,
        }
    }
}

impl From<TargetArg> for TargetRuntime {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Codex => TargetRuntime::Codex,
        }
    }
}

pub fn resolve_scope(scope: Option<ScopeArg>, interactive: bool) -> Result<InstallScope> {
    match scope {
        Some(scope) => Ok(scope.into()),
        None if interactive && io::stdin().is_terminal() => prompt_scope(),
        None => bail!("missing --scope in non-interactive mode"),
    }
}

pub fn resolve_target(target: Option<TargetArg>, interactive: bool) -> Result<TargetRuntime> {
    match target {
        Some(target) => Ok(target.into()),
        None if interactive && io::stdin().is_terminal() => prompt_target(),
        None => bail!("missing --target in non-interactive mode"),
    }
}

fn prompt_scope() -> Result<InstallScope> {
    loop {
        print!("Install scope [project/global]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().to_ascii_lowercase().as_str() {
            "project" | "p" => return Ok(InstallScope::Project),
            "global" | "g" => return Ok(InstallScope::Global),
            _ => eprintln!("Choose project or global."),
        }
    }
}

fn prompt_target() -> Result<TargetRuntime> {
    loop {
        print!("Target runtime [codex]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" | "codex" | "c" => return Ok(TargetRuntime::Codex),
            _ => eprintln!("V1 supports codex only."),
        }
    }
}
