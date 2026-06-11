use crate::domain::{Provider, WorkItem};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub base_dir: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub socket_path: PathBuf,
    pub stdout_log_path: PathBuf,
    pub stderr_log_path: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self> {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        let base_dir = home.join(".sisyphus");

        Ok(Self {
            config_path: base_dir.join("config.toml"),
            db_path: base_dir.join("sisyphus.db"),
            socket_path: base_dir.join("sisyphus.sock"),
            stdout_log_path: base_dir.join("sisyphus.out.log"),
            stderr_log_path: base_dir.join("sisyphus.err.log"),
            base_dir,
        })
    }

    pub fn ensure_base_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)
            .with_context(|| format!("failed to create {}", self.base_dir.display()))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default)]
    pub polling: PollingConfig,
    #[serde(default)]
    pub dispatch: DispatchConfig,
    #[serde(default)]
    pub providers: Vec<ProviderTargetConfig>,
    #[serde(default)]
    pub repositories: Vec<RepositoryConfig>,
}

impl Config {
    pub fn repository_path_for(&self, work_item: &WorkItem) -> Option<PathBuf> {
        self.repositories
            .iter()
            .find(|repository| repository.matches(work_item))
            .map(|repository| repository.path.clone())
    }

    pub fn provider_target_for(&self, work_item: &WorkItem) -> Option<&ProviderTargetConfig> {
        self.providers
            .iter()
            .find(|provider| provider.matches(work_item))
    }

    pub fn upsert_repository(&mut self, repository: RepositoryConfig) {
        if let Some(existing) = self.repositories.iter_mut().find(|existing| {
            existing.kind == repository.kind
                && existing.owner_or_namespace == repository.owner_or_namespace
                && existing.repo == repository.repo
                && existing.resolved_instance_url() == repository.resolved_instance_url()
        }) {
            *existing = repository;
            return;
        }

        self.repositories.push(repository);
    }

    pub fn upsert_provider_target(&mut self, provider: ProviderTargetConfig) {
        if let Some(existing) = self.providers.iter_mut().find(|existing| {
            existing.kind == provider.kind
                && existing.owner_or_namespace == provider.owner_or_namespace
                && existing.repo == provider.repo
                && existing.resolved_instance_url() == provider.resolved_instance_url()
        }) {
            *existing = provider;
            return;
        }

        self.providers.push(provider);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollingConfig {
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: u64,
    #[serde(default = "default_max_backoff_seconds")]
    pub max_backoff_seconds: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 5,
            max_backoff_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchConfig {
    #[serde(default = "default_auto_dispatch")]
    pub auto_dispatch: bool,
    #[serde(default = "default_require_open")]
    pub require_open: bool,
    #[serde(default = "default_trigger_labels")]
    pub trigger_labels: Vec<String>,
    #[serde(default = "default_ignore_labels")]
    pub ignore_labels: Vec<String>,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            auto_dispatch: true,
            require_open: true,
            trigger_labels: vec!["sisyphus".to_string()],
            ignore_labels: vec!["wontfix".to_string(), "blocked".to_string()],
        }
    }
}

impl DispatchConfig {
    pub fn should_dispatch(&self, issue_state: &str, labels: &[String]) -> bool {
        if self.require_open && issue_state != "open" {
            return false;
        }

        if labels
            .iter()
            .any(|label| self.ignore_labels.iter().any(|ignored| ignored == label))
        {
            return false;
        }

        labels.iter().any(|label| {
            self.trigger_labels
                .iter()
                .any(|trigger_label| trigger_label == label)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTargetConfig {
    pub kind: Provider,
    pub owner_or_namespace: String,
    pub repo: String,
    #[serde(default)]
    pub instance_url: Option<String>,
    #[serde(default)]
    pub token_env: Option<String>,
}

impl ProviderTargetConfig {
    pub fn resolved_instance_url(&self) -> String {
        self.instance_url
            .clone()
            .unwrap_or_else(|| match &self.kind {
                Provider::GitHub => "https://github.com".to_string(),
                Provider::GitLab => "https://gitlab.com".to_string(),
            })
    }

    pub fn auth_token(&self) -> Result<Option<String>> {
        self.auth_token_with_resolvers(|key| std::env::var(key), crate::auth::load_provider_token)
    }

    fn auth_token_with_resolvers<Env, Stored>(
        &self,
        env: Env,
        stored: Stored,
    ) -> Result<Option<String>>
    where
        Env: FnOnce(&str) -> std::result::Result<String, std::env::VarError>,
        Stored: FnOnce(&Provider) -> Result<Option<String>>,
    {
        let Some(env_key) = &self.token_env else {
            return stored(&self.kind);
        };

        let token =
            env(env_key).with_context(|| format!("provider token env {env_key} is not set"))?;
        Ok(Some(token))
    }

    fn matches(&self, work_item: &WorkItem) -> bool {
        self.kind == work_item.provider
            && self.owner_or_namespace == work_item.owner_or_namespace
            && self.repo == work_item.repo
            && trim_trailing_slash(&self.resolved_instance_url())
                == trim_trailing_slash(&work_item.instance_url)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryConfig {
    pub kind: Provider,
    pub owner_or_namespace: String,
    pub repo: String,
    pub path: PathBuf,
    #[serde(default)]
    pub instance_url: Option<String>,
}

impl RepositoryConfig {
    pub fn resolved_instance_url(&self) -> String {
        self.instance_url
            .clone()
            .unwrap_or_else(|| match &self.kind {
                Provider::GitHub => "https://github.com".to_string(),
                Provider::GitLab => "https://gitlab.com".to_string(),
            })
    }

    fn matches(&self, work_item: &WorkItem) -> bool {
        self.kind == work_item.provider
            && self.owner_or_namespace == work_item.owner_or_namespace
            && self.repo == work_item.repo
            && trim_trailing_slash(&self.resolved_instance_url())
                == trim_trailing_slash(&work_item.instance_url)
    }
}

fn trim_trailing_slash(value: &str) -> &str {
    value.trim_end_matches('/')
}

fn default_interval_seconds() -> u64 {
    5
}

fn default_max_backoff_seconds() -> u64 {
    60
}

fn default_require_open() -> bool {
    true
}

fn default_auto_dispatch() -> bool {
    true
}

fn default_trigger_labels() -> Vec<String> {
    vec!["sisyphus".to_string()]
}

fn default_ignore_labels() -> Vec<String> {
    vec!["wontfix".to_string(), "blocked".to_string()]
}

pub fn load_or_create(paths: &Paths) -> Result<Config> {
    paths.ensure_base_dir()?;

    if !paths.config_path.exists() {
        let config = Config::default();
        let rendered = toml::to_string_pretty(&config)?;
        fs::write(&paths.config_path, rendered)
            .with_context(|| format!("failed to write {}", paths.config_path.display()))?;
        return Ok(config);
    }

    let raw = fs::read_to_string(&paths.config_path)
        .with_context(|| format!("failed to read {}", paths.config_path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse {}", paths.config_path.display()))
}

pub fn save(paths: &Paths, config: &Config) -> Result<()> {
    paths.ensure_base_dir()?;
    let rendered = toml::to_string_pretty(config)?;
    fs::write(&paths.config_path, rendered)
        .with_context(|| format!("failed to write {}", paths.config_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_product_plan() {
        let config = Config::default();
        assert_eq!(config.polling.interval_seconds, 5);
        assert_eq!(config.polling.max_backoff_seconds, 60);
        assert!(config.dispatch.auto_dispatch);
        assert!(config.dispatch.require_open);
        assert_eq!(config.dispatch.trigger_labels, vec!["sisyphus"]);
        assert_eq!(config.dispatch.ignore_labels, vec!["wontfix", "blocked"]);
        assert!(config.providers.is_empty());
        assert!(config.repositories.is_empty());
    }

    #[test]
    fn dispatch_requires_open_issue_and_trigger_label() {
        let dispatch = DispatchConfig::default();
        assert!(dispatch.should_dispatch("open", &[String::from("sisyphus")]));
        assert!(!dispatch.should_dispatch("closed", &[String::from("sisyphus")]));
        assert!(!dispatch.should_dispatch("open", &[String::from("bug")]));
        assert!(
            !dispatch.should_dispatch("open", &[String::from("sisyphus"), String::from("blocked")])
        );
    }

    #[test]
    fn partial_toml_uses_nested_defaults() {
        let config: Config = toml::from_str(
            r#"
            [polling]
            interval_seconds = 10

            [dispatch]
            trigger_labels = ["agent"]
            "#,
        )
        .unwrap();

        assert_eq!(config.polling.interval_seconds, 10);
        assert_eq!(config.polling.max_backoff_seconds, 60);
        assert!(config.dispatch.auto_dispatch);
        assert!(config.dispatch.require_open);
        assert_eq!(config.dispatch.trigger_labels, vec!["agent"]);
        assert_eq!(config.dispatch.ignore_labels, vec!["wontfix", "blocked"]);
    }

    #[test]
    fn parses_provider_targets() {
        let config: Config = toml::from_str(
            r#"
            [[providers]]
            kind = "github"
            owner_or_namespace = "acme"
            repo = "widgets"
            token_env = "GITHUB_TOKEN"

            [[providers]]
            kind = "gitlab"
            owner_or_namespace = "acme/platform"
            repo = "widgets"
            "#,
        )
        .unwrap();

        assert_eq!(config.providers.len(), 2);
        assert_eq!(config.providers[0].kind, Provider::GitHub);
        assert_eq!(
            config.providers[0].resolved_instance_url(),
            "https://github.com"
        );
        assert_eq!(config.providers[1].kind, Provider::GitLab);
        assert_eq!(
            config.providers[1].resolved_instance_url(),
            "https://gitlab.com"
        );
    }

    #[test]
    fn matches_provider_target_for_work_item() {
        let config: Config = toml::from_str(
            r#"
            [[providers]]
            kind = "github"
            owner_or_namespace = "acme"
            repo = "widgets"
            "#,
        )
        .unwrap();
        let work_item = WorkItem {
            provider: Provider::GitHub,
            source_url: "https://github.com/acme/widgets/issues/42".to_string(),
            instance_url: "https://github.com".to_string(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: vec!["sisyphus".to_string()],
            comments: vec![],
        };

        assert!(config.provider_target_for(&work_item).is_some());
    }

    #[test]
    fn parses_and_matches_repository_targets() {
        let config: Config = toml::from_str(
            r#"
            [[repositories]]
            kind = "github"
            owner_or_namespace = "acme"
            repo = "widgets"
            path = "/work/widgets"
            "#,
        )
        .unwrap();
        let work_item = WorkItem {
            provider: Provider::GitHub,
            source_url: "https://github.com/acme/widgets/issues/42".to_string(),
            instance_url: "https://github.com".to_string(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: vec!["sisyphus".to_string()],
            comments: vec![],
        };

        assert_eq!(
            config.repository_path_for(&work_item),
            Some(PathBuf::from("/work/widgets"))
        );
    }

    #[test]
    fn upsert_repository_replaces_existing_mapping() {
        let mut config = Config::default();
        config.upsert_repository(RepositoryConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            path: PathBuf::from("/old"),
            instance_url: None,
        });
        config.upsert_repository(RepositoryConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            path: PathBuf::from("/new"),
            instance_url: None,
        });

        assert_eq!(config.repositories.len(), 1);
        assert_eq!(config.repositories[0].path, PathBuf::from("/new"));
    }

    #[test]
    fn upsert_provider_target_replaces_existing_mapping() {
        let mut config = Config::default();
        config.upsert_provider_target(ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: Some("OLD_TOKEN".to_string()),
        });
        config.upsert_provider_target(ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: Some("NEW_TOKEN".to_string()),
        });

        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].token_env.as_deref(), Some("NEW_TOKEN"));
    }

    #[test]
    fn auth_token_prefers_token_env() {
        let target = ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: Some("GITHUB_TOKEN".to_string()),
        };

        let token = target
            .auth_token_with_resolvers(
                |key| {
                    assert_eq!(key, "GITHUB_TOKEN");
                    Ok("env-token".to_string())
                },
                |_| panic!("stored credential should not be read when token_env is set"),
            )
            .unwrap();

        assert_eq!(token.as_deref(), Some("env-token"));
    }

    #[test]
    fn auth_token_uses_stored_credential_without_token_env() {
        let target = ProviderTargetConfig {
            kind: Provider::GitLab,
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: None,
        };

        let token = target
            .auth_token_with_resolvers(
                |_| panic!("environment should not be read without token_env"),
                |provider| {
                    assert_eq!(provider, &Provider::GitLab);
                    Ok(Some("stored-token".to_string()))
                },
            )
            .unwrap();

        assert_eq!(token.as_deref(), Some("stored-token"));
    }

    #[test]
    fn save_persists_config_for_daemon_reload() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-config-reload-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        let mut config = Config::default();
        config.upsert_provider_target(ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: Some("GITHUB_TOKEN".to_string()),
        });
        save(&paths, &config).unwrap();

        let reloaded = load_or_create(&paths).unwrap();
        assert_eq!(reloaded.providers.len(), 1);
        assert_eq!(reloaded.providers[0].repo, "widgets");

        let _ = fs::remove_dir_all(root);
    }
}
