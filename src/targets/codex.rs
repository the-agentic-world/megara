use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    installer::{ManagedTomlEdit, PlannedFile},
    paths::InstallScope,
    templates::TemplateRegistry,
};

#[path = "codex/agent.rs"]
mod agent;
#[path = "codex/agents_md.rs"]
mod agents_md;
#[path = "codex/config.rs"]
mod config;
#[path = "codex/mcp_config.rs"]
mod mcp_config;
#[path = "codex/projection.rs"]
mod projection;
const DEFAULT_LOCALE: &str = "ko-KR";

pub fn projection_files(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
) -> Result<Vec<PlannedFile>> {
    projection::projection_plan(root, scope, registry, false, false).map(|(files, _)| files)
}

pub fn projection_files_with_force(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
    force: bool,
) -> Result<Vec<PlannedFile>> {
    projection::projection_plan(root, scope, registry, force, false).map(|(files, _)| files)
}

pub(crate) fn projection_plan_with_force(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
    force: bool,
) -> Result<(Vec<PlannedFile>, Option<ManagedTomlEdit>)> {
    projection::projection_plan(root, scope, registry, force, true)
}

pub(crate) fn plan_remove_mcp_config(root: &Path, force: bool) -> Result<Option<ManagedTomlEdit>> {
    mcp_config::plan_remove(root, force)
}

pub fn obsolete_projection_files(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
) -> Vec<PathBuf> {
    projection::obsolete_projection_files(root, scope, registry)
}

pub fn runtime_dependency_issues() -> Vec<String> {
    Vec::new()
}
