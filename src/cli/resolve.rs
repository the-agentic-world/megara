use std::io::{self, IsTerminal, Write};

use anyhow::{bail, Result};

use crate::paths::{InstallScope, TargetRuntime};

use super::{ScopeArg, TargetArg};

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
        print!("Target runtime [codex/pi]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" | "codex" | "c" => return Ok(TargetRuntime::Codex),
            "pi" | "p" => return Ok(TargetRuntime::Pi),
            _ => eprintln!("Choose codex or pi."),
        }
    }
}
