use crate::domain::Provider;
use anyhow::{Context, Result, bail};

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "sisyphus-provider-token";

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

fn credential_account(provider: &Provider) -> &'static str {
    provider.as_str()
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
}
