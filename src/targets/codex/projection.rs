use std::{env, path::PathBuf};

use anyhow::{Context, Result};

use crate::{
    agents,
    installer::PlannedFile,
    paths::{InstallScope, TargetRuntime},
    templates::TemplateRegistry,
};

use super::{agent::agent_toml, agents_md::codex_agents_md, hooks::*};

pub(super) fn projection_files(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
) -> Result<Vec<PlannedFile>> {
    let megara_bin = env::current_exe().context("failed to resolve current megara executable")?;
    let mut files = vec![
        PlannedFile::new(root.join("AGENTS.md"), codex_agents_md(registry)?),
        PlannedFile::new(root.join("config.toml"), codex_config()),
        PlannedFile::new(
            root.join("hooks.json"),
            codex_hooks_json(scope, &root, &megara_bin, registry)?,
        ),
    ];

    if scope == InstallScope::Global {
        for skill in registry.workflows().into_iter().chain(registry.skills()) {
            files.push(PlannedFile::new(
                root.join("skills").join(&skill.name).join("SKILL.md"),
                skill.content.clone(),
            ));
        }
    }
    for fragment in registry.fragments() {
        files.push(PlannedFile::new(
            root.join(&fragment.relative_path),
            fragment.content.clone(),
        ));
    }
    for agent in registry.agents() {
        let policy = registry
            .config()
            .map(|config| {
                agents::effective_policy(scope, TargetRuntime::Codex, &agent.name, &config.content)
            })
            .transpose()?
            .unwrap_or_default();
        let (agent_id, agent_content) = agent_toml(agent, policy)?;
        files.push(PlannedFile::new(
            root.join("agents").join(format!("{agent_id}.toml")),
            agent_content,
        ));
    }

    Ok(files)
}

pub(super) fn obsolete_projection_files(
    root: PathBuf,
    scope: InstallScope,
    registry: &TemplateRegistry,
) -> Vec<PathBuf> {
    if scope != InstallScope::Project {
        return Vec::new();
    }
    registry
        .workflows()
        .into_iter()
        .chain(registry.skills())
        .map(|skill| root.join("skills").join(&skill.name).join("SKILL.md"))
        .collect()
}
