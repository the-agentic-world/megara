use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{installer::PlannedFile, paths::InstallScope, templates::TemplateRegistry};

#[path = "codex/agent.rs"]
mod agent;
#[path = "codex/agents_md.rs"]
mod agents_md;
#[path = "codex/config.rs"]
mod config;
#[path = "codex/hooks.rs"]
mod hooks;
#[path = "codex/projection.rs"]
mod projection;
#[path = "codex/trust.rs"]
mod trust;
#[path = "codex/trust_hash.rs"]
mod trust_hash;
#[path = "codex/trust_toml.rs"]
mod trust_toml;

pub use trust::HookTrustSummary;

const DEFAULT_LOCALE: &str = "ko-KR";

pub fn projection_files(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
) -> Result<Vec<PlannedFile>> {
    projection::projection_files(root, scope, registry)
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

pub fn ensure_hook_trust(hooks_path: &Path, dry_run: bool) -> Result<HookTrustSummary> {
    trust::ensure_hook_trust(hooks_path, dry_run)
}
