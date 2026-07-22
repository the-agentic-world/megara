use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{agents::RolePolicy, templates::HarnessTemplate};

#[derive(Debug, Deserialize)]
struct AgentSpec {
    id: String,
    name: String,
    description: String,
    codex: Option<CodexRuntimeSpec>,
    instructions: String,
}

#[derive(Debug, Deserialize)]
struct CodexRuntimeSpec {
    model: Option<String>,
    model_reasoning_effort: Option<String>,
}

#[derive(Debug, Serialize)]
struct CodexAgentSpec<'a> {
    name: &'a str,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_reasoning_effort: Option<&'a str>,
    developer_instructions: &'a str,
}

pub(super) fn agent_toml(
    template: &HarnessTemplate,
    policy: RolePolicy,
) -> Result<(String, String)> {
    let agent: AgentSpec = toml::from_str(&template.content)
        .with_context(|| format!("failed to parse agent SSOT {}", template.relative_path))?;
    let codex_agent = CodexAgentSpec {
        name: &agent.name,
        description: &agent.description,
        model: policy.model.as_deref().or_else(|| {
            agent
                .codex
                .as_ref()
                .and_then(|codex| codex.model.as_deref())
        }),
        model_reasoning_effort: policy.reasoning_effort.as_deref().or_else(|| {
            agent
                .codex
                .as_ref()
                .and_then(|codex| codex.model_reasoning_effort.as_deref())
        }),
        developer_instructions: &agent.instructions,
    };
    let content = toml::to_string_pretty(&codex_agent)
        .with_context(|| format!("failed to render Codex agent {}", agent.id))?;
    Ok((agent.id, content))
}
