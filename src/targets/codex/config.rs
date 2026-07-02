use anyhow::{Context, Result};
use serde::Deserialize;

use crate::templates::TemplateRegistry;

use super::DEFAULT_LOCALE;

#[derive(Debug, Deserialize)]
pub(super) struct HarnessConfig {
    #[serde(default = "default_locale")]
    pub(super) locale: String,
}

pub(super) fn default_locale() -> String {
    DEFAULT_LOCALE.to_string()
}

impl HarnessConfig {
    pub(super) fn from_registry(registry: &TemplateRegistry) -> Result<Self> {
        let Some(template) = registry.config() else {
            return Ok(Self {
                locale: default_locale(),
            });
        };
        let mut config: Self = toml::from_str(&template.content)
            .with_context(|| format!("failed to parse config SSOT {}", template.relative_path))?;
        if config.locale.trim().is_empty() {
            config.locale = default_locale();
        }
        Ok(config)
    }
}
