#[path = "pi/dispatch.rs"]
mod dispatch;
#[path = "pi/protocol.rs"]
mod protocol;
#[path = "pi/receipt.rs"]
mod receipt;

use std::{
    env,
    io::{self, Read},
};

use anyhow::{Context, Result};

use crate::{
    cli::PiEventArgs,
    paths::{InstallPaths, InstallScope, TargetRuntime},
    templates::TemplateRegistry,
};

pub fn run_event(args: PiEventArgs, _registry: &TemplateRegistry) -> Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: protocol::PiEventRequest =
        serde_json::from_str(&input).context("Pi event input must be a JSON object")?;
    let scope: InstallScope = args.scope.into();
    let project_root = args.project_root.unwrap_or(env::current_dir()?);
    let paths = match scope {
        InstallScope::Project => InstallPaths {
            ssot_root: project_root.join(".agents"),
            runtime_root: project_root.join(".megara"),
            target_root: project_root.join(".pi"),
        },
        InstallScope::Global => InstallPaths::resolve(scope, TargetRuntime::Pi)?,
    };
    let registry = TemplateRegistry::from_ssot_root(&paths.ssot_root)
        .context("Pi harness is not installed; run megara install first")?;
    let response = dispatch::dispatch(
        request,
        scope,
        &project_root,
        &paths.runtime_root,
        &registry,
    )?;
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}
