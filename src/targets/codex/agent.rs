use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::templates::HarnessTemplate;

#[derive(Debug, Deserialize)]
struct AgentSpec {
    id: String,
    name: String,
    description: String,
    instructions: String,
}

#[derive(Debug, Serialize)]
struct CodexAgentSpec<'a> {
    name: &'a str,
    description: &'a str,
    developer_instructions: &'a str,
}

pub(super) fn agent_toml(template: &HarnessTemplate) -> Result<(String, String)> {
    let agent: AgentSpec = toml::from_str(&template.content)
        .with_context(|| format!("failed to parse agent SSOT {}", template.relative_path))?;
    let codex_agent = CodexAgentSpec {
        name: &agent.name,
        description: &agent.description,
        developer_instructions: &agent.instructions,
    };
    let content = toml::to_string_pretty(&codex_agent)
        .with_context(|| format!("failed to render Codex agent {}", agent.id))?;
    Ok((agent.id, content))
}
