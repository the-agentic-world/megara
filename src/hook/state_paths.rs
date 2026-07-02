use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Result};
use serde_json::Value;

use crate::cli::ScopeArg;

pub(crate) struct WorkflowPaths {
    pub(crate) session_id: String,
    pub(crate) workflow_dir: PathBuf,
    pub(crate) session_file: PathBuf,
    pub(crate) events_file: PathBuf,
}

pub(crate) fn scoped_state_dir(scope: ScopeArg, project_root: Option<&Path>) -> Result<PathBuf> {
    match scope {
        ScopeArg::Project => {
            let Some(project_root) = project_root else {
                bail!("project scope hook requires --project-root");
            };
            let project_root = fs::canonicalize(project_root)?;
            let cwd = fs::canonicalize(env::current_dir()?)?;
            if !cwd.starts_with(&project_root) {
                bail!(
                    "project scope hook cwd {} is outside project root {}",
                    cwd.display(),
                    project_root.display()
                );
            }
            Ok(project_root.join(".agents").join("state").join("hooks"))
        }
        ScopeArg::Global => Ok(home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".megara")
            .join("state")
            .join("hooks")),
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| env::var_os("USERPROFILE").filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

pub(crate) fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(crate) fn unique_payload_path(payload_dir: &Path) -> PathBuf {
    let base = format!("{}-{}", timestamp_millis(), process::id());
    let mut path = payload_dir.join(format!("{base}.json"));
    let mut suffix = 0;
    while path.exists() {
        suffix += 1;
        path = payload_dir.join(format!("{base}-{suffix}.json"));
    }
    path
}

pub(crate) fn safe_part(value: impl AsRef<str>) -> String {
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

pub(crate) fn workflow_paths(state_dir: &Path, payload: &Value, skill: &str) -> WorkflowPaths {
    let session_id = canonical_session_id(payload);
    let workflow_dir = workflow_base_dir(state_dir).join(skill);
    let session_file = workflow_dir.join(format!("{}.json", safe_part(&session_id)));
    let events_file = workflow_dir.join("events.jsonl");
    WorkflowPaths {
        session_id,
        workflow_dir,
        session_file,
        events_file,
    }
}

pub(crate) fn canonical_session_id(payload: &Value) -> String {
    payload
        .get("thread_id")
        .map(value_to_string)
        .or_else(|| transcript_session_id(payload))
        .or_else(|| payload.get("session_id").map(value_to_string))
        .or_else(|| payload.get("turn_id").map(value_to_string))
        .unwrap_or_else(|| "unknown-session".to_string())
}

pub(crate) fn session_alias_ids(payload: &Value) -> Vec<String> {
    let canonical = canonical_session_id(payload);
    let mut aliases = Vec::new();
    push_alias(
        &mut aliases,
        payload.get("thread_id").map(value_to_string),
        &canonical,
    );
    push_alias(&mut aliases, transcript_session_id(payload), &canonical);
    push_alias(
        &mut aliases,
        payload.get("session_id").map(value_to_string),
        &canonical,
    );
    aliases
}

fn push_alias(aliases: &mut Vec<String>, candidate: Option<String>, canonical: &str) {
    let Some(candidate) = candidate else {
        return;
    };
    if candidate.trim().is_empty() || candidate == canonical || aliases.contains(&candidate) {
        return;
    }
    aliases.push(candidate);
}

fn transcript_session_id(payload: &Value) -> Option<String> {
    let transcript = payload.get("transcript_path").and_then(Value::as_str)?;
    let file_name = Path::new(transcript).file_name()?.to_string_lossy();
    let session = file_name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let session = session
        .len()
        .checked_sub(36)
        .and_then(|start| session.get(start..))?;
    (!session.trim().is_empty()).then(|| session.to_string())
}

pub(crate) fn workflow_base_dir(state_dir: &Path) -> PathBuf {
    if state_dir.file_name().is_some_and(|name| name == "hooks") {
        state_dir.parent().unwrap_or(state_dir).join("workflows")
    } else {
        state_dir.join("workflows")
    }
}

pub(crate) fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
