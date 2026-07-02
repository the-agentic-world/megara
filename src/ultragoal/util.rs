use std::{
    fs,
    path::Path,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::cli::ScopeArg;

pub(super) fn safe_part(value: impl AsRef<str>) -> String {
    let normalized = value
        .as_ref()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.trim().is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

pub(super) fn scope_label(scope: ScopeArg) -> &'static str {
    match scope {
        ScopeArg::Global => "global",
        ScopeArg::Project => "project",
    }
}

pub(super) fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(super) fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

pub(super) fn write_text_atomic(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt"),
        process::id()
    ));
    fs::write(&tmp, value)?;
    replace_file(&tmp, path)?;
    Ok(())
}

fn replace_file(tmp: &Path, path: &Path) -> Result<()> {
    match fs::rename(tmp, path) {
        Ok(()) => Ok(()),
        Err(_error) if path.exists() && tmp.exists() => {
            fs::remove_file(path)?;
            fs::rename(tmp, path).map_err(Into::into)
        }
        Err(error) => Err(error.into()),
    }
}
