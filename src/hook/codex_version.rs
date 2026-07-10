use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::{
    runtime_input::RuntimeSurface,
    state_paths::{canonical_session_id, safe_part},
};

pub(crate) const MIN_CODEX_CLI_VERSION: &str = "0.144.0";
pub(crate) const MIN_CODEX_APP_VERSION: &str = "26.707.30751";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NumericVersion(Vec<u64>);

impl NumericVersion {
    fn display(&self) -> String {
        self.0
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }
}

pub(crate) fn parse_numeric_version(text: &str) -> Option<NumericVersion> {
    text.split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .filter(|candidate| candidate.matches('.').count() >= 2)
        .find_map(|candidate| {
            candidate
                .split('.')
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()
                .filter(|parts| parts.len() >= 3)
                .map(NumericVersion)
        })
}

pub(crate) fn is_outdated(detected: &str, minimum: &str) -> bool {
    match (
        parse_numeric_version(detected),
        parse_numeric_version(minimum),
    ) {
        (Some(detected), Some(minimum)) => detected < minimum,
        _ => false,
    }
}

pub(crate) fn outdated_notice_once(
    state_dir: &Path,
    payload: &Value,
    surface: RuntimeSurface,
) -> Option<String> {
    let (product, detected, minimum) = match surface {
        RuntimeSurface::Cli => (
            "Codex CLI",
            detect_cli_version(payload)?,
            MIN_CODEX_CLI_VERSION,
        ),
        RuntimeSurface::App => (
            "ChatGPT desktop app",
            detect_app_version(payload)?,
            MIN_CODEX_APP_VERSION,
        ),
        RuntimeSurface::Unknown => return None,
    };
    if !is_outdated(&detected.display(), minimum) {
        return None;
    }

    let session = safe_part(canonical_session_id(payload));
    let marker_dir = state_dir.join("version-notices");
    fs::create_dir_all(&marker_dir).ok()?;
    let marker = marker_dir.join(format!(
        "{}-{}-{}.notice",
        session,
        surface.as_str(),
        safe_part(detected.display())
    ));
    let mut file = match OpenOptions::new().write(true).create_new(true).open(marker) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return None,
        Err(_) => return None,
    };
    let _ = writeln!(file, "minimum={minimum}");

    Some(update_notice(
        &configured_locale(state_dir),
        product,
        &detected.display(),
        minimum,
    ))
}

fn detect_cli_version(payload: &Value) -> Option<NumericVersion> {
    version_from_payload(payload, &["cli_version", "codex_cli_version"]).or_else(|| {
        let bin = env::var("MEGARA_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
        command_text(&bin, &["--version"]).and_then(|text| parse_numeric_version(&text))
    })
}

fn detect_app_version(payload: &Value) -> Option<NumericVersion> {
    version_from_payload(
        payload,
        &["app_version", "client_version", "codex_app_version"],
    )
    .or_else(|| {
        env::var("MEGARA_CODEX_APP_VERSION")
            .ok()
            .and_then(|value| parse_numeric_version(&value))
    })
    .or_else(platform_app_version)
}

fn version_from_payload(payload: &Value, keys: &[&str]) -> Option<NumericVersion> {
    keys.iter().find_map(|key| {
        payload
            .get(*key)
            .and_then(Value::as_str)
            .and_then(parse_numeric_version)
    })
}

#[cfg(target_os = "macos")]
fn platform_app_version() -> Option<NumericVersion> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("MEGARA_CODEX_APP_PATH") {
        candidates.push(std::path::PathBuf::from(path));
    }
    candidates.push(std::path::PathBuf::from("/Applications/ChatGPT.app"));
    if let Some(home) = env::var_os("HOME") {
        candidates.push(std::path::PathBuf::from(home).join("Applications/ChatGPT.app"));
    }
    candidates.into_iter().find_map(|app| {
        let plist = app.join("Contents/Info.plist");
        let plist = plist.to_string_lossy().to_string();
        command_text(
            "/usr/bin/plutil",
            &["-extract", "CFBundleShortVersionString", "raw", &plist],
        )
        .and_then(|text| parse_numeric_version(&text))
    })
}

#[cfg(target_os = "windows")]
fn platform_app_version() -> Option<NumericVersion> {
    command_text(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-AppxPackage | Where-Object { $_.Name -match 'ChatGPT' } | Sort-Object Version -Descending | Select-Object -First 1 -ExpandProperty Version",
        ],
    )
    .and_then(|text| parse_numeric_version(&text))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_app_version() -> Option<NumericVersion> {
    None
}

fn command_text(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().ok()?;
                return status
                    .success()
                    .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(None) if started.elapsed() < Duration::from_millis(750) => {
                thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn configured_locale(state_dir: &Path) -> String {
    let runtime_root = state_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(state_dir);
    let mut candidates = vec![runtime_root.join("megara.toml")];
    if let Some(project_root) = runtime_root.parent() {
        candidates.push(project_root.join(".agents/megara.toml"));
    }
    candidates
        .into_iter()
        .find_map(|path| {
            fs::read_to_string(path)
                .ok()
                .and_then(|text| text.parse::<toml::Value>().ok())
                .and_then(|config| config.get("locale")?.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| "en-US".to_string())
}

fn update_notice(locale: &str, product: &str, detected: &str, minimum: &str) -> String {
    if locale.to_ascii_lowercase().starts_with("ko") {
        format!(
            "{product} {detected}은(는) GPT-5.6 최소 버전 {minimum}보다 낮습니다. 설치 방식에 맞게 업데이트해 주세요."
        )
    } else {
        format!(
            "{product} {detected} is below the GPT-5.6 minimum version {minimum}. Update it using the method appropriate for your installation."
        )
    }
}
