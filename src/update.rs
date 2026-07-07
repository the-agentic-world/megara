use std::{
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    cli::{Commands, UpdateArgs, UpdateScopeArg},
    paths::{home_dir, InstallPaths, InstallScope, TargetRuntime},
    ui::{self, Section},
};

const REPO: &str = "the-agentic-world/megara";
const LEGACY_INSTALLER_BIN: &str = "/usr/local/bin/megara";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const NO_UPDATE_CHECK_ENV: &str = "MEGARA_NO_UPDATE_CHECK";
const SKIP_LEGACY_CLEANUP_ENV: &str = "MEGARA_SKIP_LEGACY_CLEANUP";

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
    let current_bin = current_exe_path()?;
    let mut messages = Vec::new();

    let megara_bin = if is_newer_tag(&latest.tag_name, &current) || args.force {
        let (install_dir, install_messages) = selected_install_dir()?;
        messages.extend(install_messages);

        messages.push(format!(
            "Updating Megara binary: current={}, latest={}, install_dir={}",
            current,
            latest.tag_name,
            install_dir.display()
        ));
        install_release(&latest.tag_name, &install_dir)?;
        let installed_bin = install_dir.join("megara");
        messages.extend(cleanup_legacy_binary(&current_bin, &installed_bin));
        installed_bin
    } else {
        messages.push(format!("Megara binary is current: {current}"));
        current_bin
    };

    if let Ok(path) = state_path() {
        let _ = write_check_state(
            &path,
            &UpdateCheckState {
                checked_at: now_epoch_secs(),
                latest_tag: Some(latest.tag_name.clone()),
            },
        );
    }

    messages.extend(refresh_harnesses(args, &megara_bin)?);
    messages.push("Megara update complete.".to_string());
    ui::print_dashboard("Update", "complete", &[], &[Section::new("Run", messages)])?;
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

fn refresh_harnesses(args: UpdateArgs, megara_bin: &Path) -> Result<Vec<String>> {
    let mut messages = Vec::new();
    let target: TargetRuntime = args.target.into();
    let scopes = selected_installed_scopes(args.scope, target)?;
    if scopes.is_empty() {
        messages.push("No installed Megara harness found; skipped harness update.".to_string());
        return Ok(messages);
    }

    for scope in scopes {
        messages.push(format!(
            "Refreshing harness: scope={scope}, target={target}"
        ));
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
    Ok(messages)
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

fn selected_install_dir() -> Result<(PathBuf, Vec<String>)> {
    let current = current_install_dir()?;
    let explicit = env::var_os("MEGARA_INSTALL_DIR")
        .filter(|value| !value.as_os_str().is_empty())
        .map(PathBuf::from);
    let home = home_dir()?;
    let path_env = env::var_os("PATH");
    choose_install_dir(
        explicit,
        current.clone(),
        dir_is_writable(&current),
        writable_user_install_dir(&home),
        path_env.as_deref(),
    )
}

fn cleanup_legacy_binary(current_bin: &Path, installed_bin: &Path) -> Vec<String> {
    if env::var_os(SKIP_LEGACY_CLEANUP_ENV).is_some()
        || !should_cleanup_legacy_binary(current_bin, installed_bin)
    {
        return Vec::new();
    }

    let legacy = Path::new(LEGACY_INSTALLER_BIN);
    if !legacy.exists() {
        return Vec::new();
    }

    match fs::remove_file(legacy) {
        Ok(()) => vec![format!(
            "Removed legacy Megara binary: {}",
            legacy.display()
        )],
        Err(_) => vec![format!(
            "Legacy Megara binary remains at {}; remove it or place {} earlier in PATH.",
            legacy.display(),
            installed_bin
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .display()
        )],
    }
}

pub(crate) fn should_cleanup_legacy_binary(current_bin: &Path, installed_bin: &Path) -> bool {
    let legacy = Path::new(LEGACY_INSTALLER_BIN);
    paths_match(current_bin, legacy) && !paths_match(installed_bin, legacy)
}

fn paths_match(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    let Ok(left) = fs::canonicalize(left) else {
        return false;
    };
    let Ok(right) = fs::canonicalize(right) else {
        return false;
    };
    left == right
}

pub(crate) fn choose_install_dir(
    explicit: Option<PathBuf>,
    current: PathBuf,
    current_writable: bool,
    writable_fallback: Option<PathBuf>,
    path_env: Option<&OsStr>,
) -> Result<(PathBuf, Vec<String>)> {
    if let Some(path) = explicit {
        let message = format!("Using MEGARA_INSTALL_DIR={}", path.display());
        return Ok((path, vec![message]));
    }

    if current_writable {
        return Ok((current, Vec::new()));
    }

    let Some(fallback) = writable_fallback else {
        bail!(
            "no writable install directory found; set MEGARA_INSTALL_DIR to a writable directory"
        );
    };
    let mut messages = vec![format!(
        "Current install dir is not writable: {}; installing binary to {}",
        current.display(),
        fallback.display()
    )];
    if path_contains_dir(path_env, &fallback) {
        messages.push(format!(
            "Ensure {} appears before {} in PATH.",
            fallback.display(),
            current.display()
        ));
    } else {
        messages.push(format!(
            "Add {} to PATH before running megara again.",
            fallback.display()
        ));
    }
    Ok((fallback, messages))
}

fn writable_user_install_dir(home: &Path) -> Option<PathBuf> {
    user_install_candidates(home)
        .into_iter()
        .find(|path| ensure_writable_dir(path))
}

fn user_install_candidates(home: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("XDG_BIN_HOME")
        .filter(|value| !value.as_os_str().is_empty())
        .map(PathBuf::from)
    {
        candidates.push(path);
    }
    candidates.push(home.join(".local/bin"));
    candidates.push(home.join("bin"));
    candidates.push(home.join(".megara/bin"));
    dedup_paths(candidates)
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.iter().any(|existing| existing == &path) {
            deduped.push(path);
        }
    }
    deduped
}

fn ensure_writable_dir(path: &Path) -> bool {
    fs::create_dir_all(path).is_ok() && dir_is_writable(path)
}

fn path_contains_dir(path_env: Option<&OsStr>, dir: &Path) -> bool {
    path_env
        .into_iter()
        .flat_map(env::split_paths)
        .any(|entry| entry == dir)
}

fn dir_is_writable(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    let probe = path.join(format!(
        ".megara-write-test-{}-{}",
        std::process::id(),
        now_epoch_secs()
    ));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

fn current_exe_path() -> Result<PathBuf> {
    env::current_exe().context("failed to resolve current megara executable")
}

fn current_install_dir() -> Result<PathBuf> {
    let exe = current_exe_path()?;
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

pub(crate) fn is_newer_tag(latest: &str, current: &str) -> bool {
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
