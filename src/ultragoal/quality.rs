use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::quality_refs;

pub(crate) fn read_quality_gate(value_or_path: &str) -> Result<Value> {
    let trimmed = value_or_path.trim();
    let raw = if trimmed.starts_with('{') {
        trimmed.to_string()
    } else {
        fs::read_to_string(value_or_path)
            .with_context(|| format!("failed to read quality gate json: {value_or_path}"))?
    };
    serde_json::from_str(&raw).context("failed to parse quality gate json")
}

pub(crate) fn validate_quality_gate(value: &Value, artifact_root: &Path) -> Result<()> {
    require_object(value, "quality gate")?;
    let architect = section(value, "architectReview")?;
    require_str_eq(architect, "recommendation", "APPROVE")?;
    require_str_eq(architect, "architectureStatus", "CLEAR")?;
    require_str_eq(architect, "productStatus", "CLEAR")?;
    require_str_eq(architect, "codeStatus", "CLEAR")?;
    require_substantive_str(architect, "evidence")?;
    quality_refs::validate(architect, "reviewedFiles", artifact_root)?;
    require_empty_array(architect, "blockers")?;

    let qa = section(value, "executorQa")?;
    require_str_eq(qa, "status", "passed")?;
    require_str_eq(qa, "e2eStatus", "passed")?;
    require_str_eq(qa, "redTeamStatus", "passed")?;
    require_substantive_str(qa, "evidence")?;
    quality_refs::require_string_array(qa, "commands")?;
    quality_refs::validate(qa, "artifactRefs", artifact_root)?;
    require_empty_array(qa, "blockers")?;

    let iteration = section(value, "iteration")?;
    require_str_eq(iteration, "status", "passed")?;
    if iteration.get("fullRerun").and_then(Value::as_bool) != Some(true) {
        bail!("quality gate iteration.fullRerun must be true");
    }
    require_substantive_str(iteration, "evidence")?;
    quality_refs::require_string_array(iteration, "commands")?;
    quality_refs::validate(iteration, "artifactRefs", artifact_root)?;
    require_empty_array(iteration, "blockers")?;
    Ok(())
}

fn section<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    let section = value
        .get(key)
        .with_context(|| format!("quality gate missing {key}"))?;
    require_object(section, key)?;
    Ok(section)
}

fn require_object(value: &Value, label: &str) -> Result<()> {
    if value.as_object().is_none() {
        bail!("quality gate {label} must be an object");
    }
    Ok(())
}

fn require_str_eq(value: &Value, key: &str, expected: &str) -> Result<()> {
    let actual = value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("quality gate missing string field {key}"))?;
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("quality gate {key} must be {expected}, got {actual}");
    }
    Ok(())
}

fn require_substantive_str(value: &Value, key: &str) -> Result<()> {
    let actual = value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("quality gate missing string field {key}"))?;
    let trimmed = actual.trim();
    if trimmed.is_empty() {
        bail!("quality gate {key} must not be empty");
    }
    let normalized = trimmed.to_ascii_lowercase();
    if trimmed.len() < 16
        || matches!(
            normalized.as_str(),
            "todo" | "tbd" | "n/a" | "na" | "none" | "later" | "done" | "passed"
        )
    {
        bail!("quality gate {key} must contain substantive evidence");
    }
    Ok(())
}

fn require_empty_array(value: &Value, key: &str) -> Result<()> {
    match value.get(key).and_then(Value::as_array) {
        Some(items) if items.is_empty() => Ok(()),
        Some(_) => bail!("quality gate {key} must be empty"),
        None => bail!("quality gate missing array field {key}"),
    }
}
