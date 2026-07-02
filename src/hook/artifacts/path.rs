use std::path::{Path, PathBuf};

use crate::hook::state_paths::safe_part;

pub(super) fn yaml_string(value: impl std::fmt::Display) -> String {
    serde_json::to_string(&value.to_string()).unwrap_or_else(|_| "\"\"".to_string())
}

pub(super) fn unique_spec_path(workflow_dir: &Path, session_id: &str, timestamp: &str) -> PathBuf {
    let specs_dir = workflow_dir.join("specs");
    let base = format!(
        "deep-interview-{}-{}",
        safe_part(session_id),
        safe_part(timestamp)
    );
    unique_path(&specs_dir, &base)
}

pub(super) fn unique_plan_path(
    workflow_dir: &Path,
    session_id: &str,
    plan_id: &str,
    timestamp: &str,
) -> PathBuf {
    let plans_dir = workflow_dir.join("plans");
    let base = format!(
        "ralplan-{}-{}-{}",
        safe_part(session_id),
        safe_part(plan_id),
        safe_part(timestamp)
    );
    unique_path(&plans_dir, &base)
}

pub(super) fn unique_review_path(
    workflow_dir: &Path,
    session_id: &str,
    role: &str,
    round: i64,
    timestamp: &str,
) -> PathBuf {
    let reviews_dir = workflow_dir.join("reviews");
    let base = format!(
        "ralplan-review-{}-{}-r{}-{}",
        safe_part(session_id),
        safe_part(role),
        round,
        safe_part(timestamp)
    );
    unique_path(&reviews_dir, &base)
}

fn unique_path(dir: &Path, base: &str) -> PathBuf {
    let mut path = dir.join(format!("{base}.md"));
    let mut suffix = 0;
    while path.exists() {
        suffix += 1;
        path = dir.join(format!("{base}-{suffix}.md"));
    }
    path
}
