use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde_json::Value;

pub(super) fn validate(value: &Value, key: &str, artifact_root: &Path) -> Result<()> {
    for raw in require_string_array(value, key)? {
        let path = if Path::new(&raw).is_absolute() {
            PathBuf::from(&raw)
        } else {
            artifact_root.join(&raw)
        };
        require_file_artifact(key, &path)?;
    }
    Ok(())
}

pub(super) fn require_string_array(value: &Value, key: &str) -> Result<Vec<String>> {
    let items = value
        .get(key)
        .and_then(Value::as_array)
        .with_context(|| format!("quality gate missing array field {key}"))?;
    if items.is_empty() {
        bail!("quality gate {key} must not be empty");
    }
    items
        .iter()
        .map(|item| {
            let value = item
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .with_context(|| format!("quality gate {key} entries must be non-empty strings"))?;
            Ok(value.to_string())
        })
        .collect()
}

fn require_file_artifact(key: &str, path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("quality gate {key} artifact is missing: {}", path.display()))?;
    if !metadata.is_file() {
        bail!(
            "quality gate {key} artifact must be a file: {}",
            path.display()
        );
    }
    if metadata.len() == 0 {
        bail!(
            "quality gate {key} artifact must not be empty: {}",
            path.display()
        );
    }
    Ok(())
}
