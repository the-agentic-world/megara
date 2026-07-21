use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    installer::PlannedFile,
    paths::InstallScope,
    templates::{HarnessTemplate, TemplateRegistry},
};

const MIN_VERSION: (u32, u32, u32) = (0, 80, 10);
const MAX_MINOR: u32 = 81;

#[derive(Deserialize)]
struct AgentSpec {
    id: String,
    name: String,
    description: String,
    #[serde(default)]
    thinking_level: String,
    instructions: String,
}

#[derive(Default, Deserialize)]
struct MegaraConfig {
    #[serde(default)]
    target: TargetConfig,
}

#[derive(Default, Deserialize)]
struct TargetConfig {
    #[serde(default)]
    pi: PiTargetConfig,
}

#[derive(Default, Deserialize)]
struct PiTargetConfig {
    #[serde(default)]
    roles: BTreeMap<String, PiRoleOverride>,
}

#[derive(Clone, Default, Deserialize)]
struct PiRoleOverride {
    model: Option<String>,
    thinking_level: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct ProjectTrust {
    project_root: String,
    agents_sha256: String,
}

pub fn projection_files(
    root: PathBuf,
    _scope: InstallScope,
    registry: &TemplateRegistry,
) -> Result<Vec<PlannedFile>> {
    let extension = registry
        .find("pi-extension")
        .context("bundled Pi extension template is missing")?;
    let mut files = vec![
        PlannedFile::new(root.join("settings.json"), pi_settings()),
        PlannedFile::new(root.join("extensions/megara.ts"), extension.content.clone()),
    ];
    for agent in registry.agents() {
        let role_override = role_override(registry, &agent.name)?;
        let (id, content) = agent_markdown(agent, role_override.as_ref())?;
        files.push(PlannedFile::new(
            root.join("agents").join(format!("{id}.md")),
            content,
        ));
    }
    Ok(files)
}

pub fn obsolete_projection_files(
    _root: PathBuf,
    _scope: InstallScope,
    _registry: &TemplateRegistry,
) -> Vec<PathBuf> {
    Vec::new()
}

pub fn runtime_dependency_issues() -> Vec<String> {
    let output = match Command::new("pi").arg("--version").output() {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return vec![format!(
                "Pi is not usable: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )]
        }
        Err(_) => {
            return vec![
                "Pi is not installed; install @earendil-works/pi-coding-agent >=0.80.10."
                    .to_string(),
            ]
        }
    };
    let version = String::from_utf8_lossy(&output.stdout);
    match parse_version(&version) {
        Some((major, minor, patch))
            if (major, minor, patch) >= MIN_VERSION && minor < MAX_MINOR =>
        {
            Vec::new()
        }
        Some((major, minor, patch)) => vec![format!(
            "Pi version {major}.{minor}.{patch} is unsupported; require >=0.80.10 and <0.81.0."
        )],
        None => vec![format!(
            "could not parse Pi version from {}",
            version.trim()
        )],
    }
}

pub fn ensure_project_trust(
    runtime_root: &Path,
    project_root: &Path,
    registry: &TemplateRegistry,
    dry_run: bool,
) -> Result<()> {
    let project_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let trust = ProjectTrust {
        project_root: project_root.display().to_string(),
        agents_sha256: agents_sha256(registry),
    };
    let path = runtime_root.join("trust/pi-project.toml");
    if !dry_run {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, toml::to_string_pretty(&trust)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub fn inspect_trust(
    runtime_root: &Path,
    project_root: &Path,
    registry: &TemplateRegistry,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let path = runtime_root.join("trust/pi-project.toml");
    if !path.exists() {
        warnings.push(
            "Pi project role agents are not trusted; rerun install with --trust-project."
                .to_string(),
        );
        return Ok(());
    }
    let trust: ProjectTrust = toml::from_str(&fs::read_to_string(&path)?)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let project_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    if trust.project_root != project_root.display().to_string()
        || trust.agents_sha256 != agents_sha256(registry)
    {
        warnings.push(
            "Pi project role agent trust no longer matches the installed agents; rerun install with --trust-project."
                .to_string(),
        );
    }
    Ok(())
}

pub fn has_project_trust(runtime_root: &Path) -> bool {
    runtime_root.join("trust/pi-project.toml").is_file()
}

pub fn is_project_trusted(
    runtime_root: &Path,
    project_root: &Path,
    registry: &TemplateRegistry,
) -> bool {
    let path = runtime_root.join("trust/pi-project.toml");
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(trust) = toml::from_str::<ProjectTrust>(&content) else {
        return false;
    };
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    trust.project_root == root.display().to_string()
        && trust.agents_sha256 == agents_sha256(registry)
}

fn pi_settings() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "megara_managed": "MEGARA:MANAGED",
        "extensions": ["./extensions/megara.ts"]
    }))
    .expect("Pi settings are serializable")
        + "\n"
}

fn agent_markdown(
    template: &HarnessTemplate,
    override_config: Option<&PiRoleOverride>,
) -> Result<(String, String)> {
    let agent: AgentSpec = toml::from_str(&template.content)
        .with_context(|| format!("failed to parse agent SSOT {}", template.relative_path))?;
    let thinking = override_config
        .and_then(|config| config.thinking_level.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            (!agent.thinking_level.trim().is_empty()).then_some(agent.thinking_level.as_str())
        })
        .unwrap_or("medium");
    let model = override_config
        .and_then(|config| config.model.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("model: {value}\n"))
        .unwrap_or_default();
    Ok((
        agent.id.clone(),
        format!(
            "---\nname: {}\ndescription: {}\n{}thinking_level: {}\n---\n\n# {}\n\n{}\n",
            agent.id, agent.description, model, thinking, agent.name, agent.instructions
        ),
    ))
}

fn role_override(registry: &TemplateRegistry, role: &str) -> Result<Option<PiRoleOverride>> {
    let Some(config) = registry.config() else {
        return Ok(None);
    };
    let config: MegaraConfig = toml::from_str(&config.content)
        .context("failed to parse Megara configuration for Pi role projection")?;
    Ok(config.target.pi.roles.get(role).cloned())
}

fn agents_sha256(registry: &TemplateRegistry) -> String {
    let mut hasher = Sha256::new();
    for agent in registry.agents() {
        hasher.update(agent.relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(agent.content.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn parse_version(value: &str) -> Option<(u32, u32, u32)> {
    value
        .split(|value: char| !value.is_ascii_digit() && value != '.')
        .find_map(|token| {
            let mut parts = token.split('.');
            Some((
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            ))
        })
}
