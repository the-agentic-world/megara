use crate::domain::Provider;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "sisyphus-provider-token";
const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const DEFAULT_GITHUB_SCOPE: &str = "repo";

pub fn prompt_for_provider_token(provider: &Provider) -> Result<String> {
    let token = rpassword::prompt_password(format!("{} token: ", provider.as_str()))
        .context("failed to read provider token")?;
    if token.trim().is_empty() {
        bail!("provider token cannot be empty");
    }

    Ok(token)
}

pub fn store_provider_token(provider: &Provider, token: &str) -> Result<()> {
    store_provider_token_for_account(credential_account(provider), token)
}

pub fn load_provider_token(provider: &Provider) -> Result<Option<String>> {
    load_provider_token_for_account(credential_account(provider))
}

pub fn require_github_oauth_client_id(cli_client_id: Option<String>) -> Result<String> {
    cli_client_id
        .and_then(trimmed_non_empty)
        .context("GitHub OAuth requires --client-id; use provider-add --token-env to authenticate with a GitHub PAT instead")
}

pub fn github_oauth_scopes(scopes: &[String]) -> String {
    if scopes.is_empty() {
        DEFAULT_GITHUB_SCOPE.to_string()
    } else {
        scopes.join(" ")
    }
}

pub async fn authenticate_github_device_flow(client_id: &str, scopes: &[String]) -> Result<String> {
    let client = reqwest::Client::new();
    let scope = github_oauth_scopes(scopes);
    let device = request_github_device_code(&client, client_id, &scope).await?;
    let verification_url = device
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&device.verification_uri);

    println!("Open this URL to authorize Sisyphus: {verification_url}");
    println!("GitHub device code: {}", device.user_code);
    open_browser(verification_url);

    poll_github_device_token(&client, client_id, &device).await
}

fn credential_account(provider: &Provider) -> &'static str {
    provider.as_str()
}

fn trimmed_non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

#[derive(Debug, Deserialize)]
struct GitHubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GitHubAccessTokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

async fn request_github_device_code(
    client: &reqwest::Client,
    client_id: &str,
    scope: &str,
) -> Result<GitHubDeviceCodeResponse> {
    client
        .post(GITHUB_DEVICE_CODE_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&github_device_code_form(client_id, scope))
        .send()
        .await?
        .error_for_status()?
        .json::<GitHubDeviceCodeResponse>()
        .await
        .context("failed to request GitHub device code")
}

async fn poll_github_device_token(
    client: &reqwest::Client,
    client_id: &str,
    device: &GitHubDeviceCodeResponse,
) -> Result<String> {
    let mut interval = Duration::from_secs(device.interval.unwrap_or(5).max(1));
    let deadline = Instant::now() + Duration::from_secs(device.expires_in);

    loop {
        if Instant::now() >= deadline {
            bail!("GitHub device authorization expired");
        }

        tokio::time::sleep(interval).await;

        let response = client
            .post(GITHUB_ACCESS_TOKEN_URL)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&github_access_token_form(client_id, &device.device_code))
            .send()
            .await?
            .error_for_status()?
            .json::<GitHubAccessTokenResponse>()
            .await
            .context("failed to poll GitHub device authorization")?;

        if let Some(token) = response.access_token {
            return Ok(token);
        }

        match response.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += Duration::from_secs(5),
            Some("expired_token") => bail!("GitHub device authorization expired"),
            Some("access_denied") => bail!("GitHub device authorization was denied"),
            Some(error) => bail!(
                "GitHub device authorization failed: {}",
                response.error_description.as_deref().unwrap_or(error)
            ),
            None => bail!("GitHub device authorization response did not include an access token"),
        }
    }
}

fn github_device_code_form(client_id: &str, scope: &str) -> Vec<(&'static str, String)> {
    vec![
        ("client_id", client_id.to_string()),
        ("scope", scope.to_string()),
    ]
}

fn github_access_token_form(client_id: &str, device_code: &str) -> Vec<(&'static str, String)> {
    vec![
        ("client_id", client_id.to_string()),
        ("device_code", device_code.to_string()),
        ("grant_type", GITHUB_DEVICE_GRANT_TYPE.to_string()),
    ]
}

fn open_browser(url: &str) {
    if let Err(error) = open_browser_result(url) {
        eprintln!("warning: failed to open browser: {error}");
    }
}

#[cfg(target_os = "macos")]
fn open_browser_result(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(url)
        .status()
        .map(|_| ())
}

#[cfg(target_os = "linux")]
fn open_browser_result(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .status()
        .map(|_| ())
}

#[cfg(target_os = "windows")]
fn open_browser_result(url: &str) -> std::io::Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .status()
        .map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_browser_result(_url: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn store_provider_token_for_account(account: &str, token: &str) -> Result<()> {
    security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, account, token.as_bytes())
        .with_context(|| format!("failed to store {account} token in macOS Keychain"))
}

#[cfg(not(target_os = "macos"))]
fn store_provider_token_for_account(_account: &str, _token: &str) -> Result<()> {
    bail!("provider credential storage is only supported on macOS Keychain")
}

#[cfg(target_os = "macos")]
fn load_provider_token_for_account(account: &str) -> Result<Option<String>> {
    let options = security_framework::passwords::PasswordOptions::new_generic_password(
        KEYCHAIN_SERVICE,
        account,
    );
    match security_framework::passwords::generic_password(options) {
        Ok(token) => String::from_utf8(token)
            .map(Some)
            .context("stored provider token is not valid UTF-8"),
        Err(error) if error.code() == security_framework_sys::base::errSecItemNotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("failed to read {account} token from macOS Keychain")),
    }
}

#[cfg(not(target_os = "macos"))]
fn load_provider_token_for_account(_account: &str) -> Result<Option<String>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_account_uses_provider_name() {
        assert_eq!(credential_account(&Provider::GitHub), "github");
        assert_eq!(credential_account(&Provider::GitLab), "gitlab");
    }

    #[test]
    fn require_github_oauth_client_id_trims_cli_value() {
        assert_eq!(
            require_github_oauth_client_id(Some(" client-1 ".to_string())).unwrap(),
            "client-1"
        );
    }

    #[test]
    fn require_github_oauth_client_id_rejects_missing_or_blank_value() {
        assert!(require_github_oauth_client_id(None).is_err());
        assert!(require_github_oauth_client_id(Some(" ".to_string())).is_err());
    }

    #[test]
    fn github_oauth_scopes_defaults_to_repo() {
        assert_eq!(github_oauth_scopes(&[]), "repo");
    }

    #[test]
    fn github_oauth_scopes_joins_requested_scopes() {
        assert_eq!(
            github_oauth_scopes(&["repo".to_string(), "read:user".to_string()]),
            "repo read:user"
        );
    }

    #[test]
    fn github_device_code_form_includes_client_and_scope() {
        assert_eq!(
            github_device_code_form("client-1", "repo"),
            vec![
                ("client_id", "client-1".to_string()),
                ("scope", "repo".to_string())
            ]
        );
    }

    #[test]
    fn github_access_token_form_uses_device_grant() {
        assert_eq!(
            github_access_token_form("client-1", "device-1"),
            vec![
                ("client_id", "client-1".to_string()),
                ("device_code", "device-1".to_string()),
                ("grant_type", GITHUB_DEVICE_GRANT_TYPE.to_string())
            ]
        );
    }
}
