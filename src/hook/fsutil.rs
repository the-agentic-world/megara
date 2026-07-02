use std::{fs, io::Write, path::Path, process};

use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) fn append_jsonl(path: &Path, entry: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_vec(entry)?;
    line.push(b'\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(&line)?;
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn load_json(path: &Path) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .filter(Value::is_object)
}

pub(crate) fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("json"),
        process::id()
    ));
    let mut content = serde_json::to_string_pretty(value)?;
    content.push('\n');
    fs::write(&tmp, content)?;
    replace_file(&tmp, path)?;
    Ok(())
}

pub(crate) fn write_text_atomic(path: &Path, value: &str) -> Result<()> {
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
