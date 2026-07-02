use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    cli::{Commands, UpdateArgs, UpdateScopeArg},
    paths::{home_dir, InstallPaths, InstallScope, TargetRuntime},
};

const REPO: &str = "the-agentic-world/megara";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const NO_UPDATE_CHECK_ENV: &str = "MEGARA_NO_UPDATE_CHECK";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct UpdateCheckState {
    checked_at: u64,
    latest_tag: Option<String>,
}

pub fn maybe_notify(command: &Commands) {
    if matches!(command, Commands::Hook(_) | Commands::Update(_)) || update_check_disabled() {
        return;
    }
    let _ = maybe_notify_inner();
}

pub fn run(args: UpdateArgs) -> Result<()> {
    let latest = fetch_latest_release()?;
    let current = current_tag();
    let install_dir = current_install_dir()?;

    if is_newer_tag(&latest.tag_name, &current) || args.force {
        println!(
            "Updating Megara binary: current={}, latest={}, install_dir={}",
            current,
            latest.tag_name,
            install_dir.display()
        );
        install_release(&latest.tag_name, &install_dir)?;
    } else {
        println!("Megara binary is current: {current}");
    }

    if let Ok(path) = state_path() {
        let _ = write_check_state(
            &path,
            &UpdateCheckState {
                checked_at: now_epoch_secs(),
                latest_tag: Some(latest.tag_name.clone()),
            },
        );
    }

    refresh_harnesses(args, &install_dir.join("megara"))?;
    println!("Megara update complete.");
    Ok(())
}

fn maybe_notify_inner() -> Result<()> {
    let path = state_path()?;
    if !check_due(&path)? {
        return Ok(());
    }

    let checked_at = now_epoch_secs();
    let latest = fetch_latest_release();
    let latest_tag = latest.as_ref().ok().map(|release| release.tag_name.clone());
    let _ = write_check_state(
        &path,
        &UpdateCheckState {
            checked_at,
            latest_tag: latest_tag.clone(),
        },
    );

    let latest = latest?;
    let current = current_tag();
    if is_newer_tag(&latest.tag_name, &current) {
        eprintln!(
            "Megara update available: {} (current {}). Run `megara update`.",
            latest.tag_name, current
        );
    }
    Ok(())
}

fn fetch_latest_release() -> Result<GitHubRelease> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let output = Command::new("curl")
        .args(["-fsSL", "--connect-timeout", "2", "--max-time", "5", &url])
        .output()
        .context("failed to run curl for latest Megara release")?;
    if !output.status.success() {
        bail!(
            "failed to fetch latest Megara release: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("failed to parse latest Megara release")
}

fn install_release(tag: &str, install_dir: &Path) -> Result<()> {
    let temp_dir = env::temp_dir().join(format!(
        "megara-update-{}-{}",
        std::process::id(),
        now_epoch_secs()
    ));
    fs::create_dir_all(&temp_dir)
        .with_context(|| format!("failed to create {}", temp_dir.display()))?;
    let script = temp_dir.join("install.sh");
    let script_url = format!("https://github.com/{REPO}/releases/download/{tag}/install.sh");

    let download = Command::new("curl")
        .args(["-fsSL", &script_url, "-o"])
        .arg(&script)
        .status()
        .context("failed to run curl for Megara installer")?;
    if !download.success() {
        let _ = fs::remove_dir_all(&temp_dir);
        bail!("failed to download Megara installer");
    }

    let status = Command::new("sh")
        .arg(&script)
        .env("MEGARA_VERSION", tag)
        .env("MEGARA_INSTALL_DIR", install_dir)
        .env("MEGARA_REPO", REPO)
        .status()
        .context("failed to run Megara installer")?;
    let _ = fs::remove_dir_all(&temp_dir);
    if !status.success() {
        bail!("Megara installer failed");
    }
    Ok(())
}

fn refresh_harnesses(args: UpdateArgs, megara_bin: &Path) -> Result<()> {
    let target: TargetRuntime = args.target.into();
    let scopes = selected_installed_scopes(args.scope, target)?;
    if scopes.is_empty() {
        println!("No installed Megara harness found; skipped harness update.");
        return Ok(());
    }

    for scope in scopes {
        println!("Refreshing harness: scope={scope}, target={target}");
        let mut command = Command::new(megara_bin);
        command
            .arg("install")
            .arg("--scope")
            .arg(scope.to_string())
            .arg("--target")
            .arg(target.to_string())
            .env(NO_UPDATE_CHECK_ENV, "1");
        if args.force {
            command.arg("--force");
        }
        let status = command
            .status()
            .with_context(|| format!("failed to refresh {scope} harness"))?;
        if !status.success() {
            bail!("failed to refresh {scope} harness");
        }
    }
    Ok(())
}

fn selected_installed_scopes(
    scope: UpdateScopeArg,
    target: TargetRuntime,
) -> Result<Vec<InstallScope>> {
    match scope {
        UpdateScopeArg::All => [InstallScope::Project, InstallScope::Global]
            .into_iter()
            .filter_map(|scope| installed(scope, target).transpose())
            .collect(),
        UpdateScopeArg::Project => require_installed(InstallScope::Project, target),
        UpdateScopeArg::Global => require_installed(InstallScope::Global, target),
    }
}

fn require_installed(scope: InstallScope, target: TargetRuntime) -> Result<Vec<InstallScope>> {
    if installed(scope, target)?.is_some() {
        Ok(vec![scope])
    } else {
        let paths = InstallPaths::resolve(scope, target)?;
        bail!(
            "{} harness is not installed at {}; run `megara install --scope {} --target {}` first",
            scope,
            paths.ssot_root.display(),
            scope,
            target
        );
    }
}

fn installed(scope: InstallScope, target: TargetRuntime) -> Result<Option<InstallScope>> {
    let paths = InstallPaths::resolve(scope, target)?;
    Ok(paths
        .ssot_root
        .join("megara.toml")
        .exists()
        .then_some(scope))
}

fn check_due(path: &Path) -> Result<bool> {
    let Some(state) = read_check_state(path) else {
        return Ok(true);
    };
    Ok(now_epoch_secs().saturating_sub(state.checked_at) >= CHECK_INTERVAL.as_secs())
}

fn read_check_state(path: &Path) -> Option<UpdateCheckState> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_check_state(path: &Path, state: &UpdateCheckState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(state)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn state_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".megara/state/update-check.json"))
}

fn current_install_dir() -> Result<PathBuf> {
    let exe = env::current_exe().context("failed to resolve current megara executable")?;
    exe.parent()
        .map(Path::to_path_buf)
        .context("current megara executable has no parent directory")
}

fn current_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn update_check_disabled() -> bool {
    env::var_os(NO_UPDATE_CHECK_ENV).is_some()
}

fn is_newer_tag(latest: &str, current: &str) -> bool {
    let Some(latest) = parse_version(latest) else {
        return false;
    };
    let Some(current) = parse_version(current) else {
        return false;
    };
    latest > current
}

fn parse_version(tag: &str) -> Option<[u64; 3]> {
    let version = tag.trim().trim_start_matches('v');
    let stable = version
        .split_once('-')
        .map_or(version, |(stable, _)| stable);
    let mut parts = stable.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some([major, minor, patch])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_release_tags() {
        assert!(is_newer_tag("v1.0.1", "v1.0.0"));
        assert!(is_newer_tag("v1.1.0", "v1.0.9"));
        assert!(!is_newer_tag("v1.0.0", "v1.0.0"));
        assert!(!is_newer_tag("v0.0.13", "v1.0.0"));
        assert!(!is_newer_tag("not-a-version", "v1.0.0"));
    }
}
