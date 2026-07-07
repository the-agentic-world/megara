use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Result;
use serde_json::{json, Value};

use super::{
    append_jsonl, canonical_session_id, load_json, safe_part, write_json_atomic, HookOptions,
};
use crate::hook::fsutil::sha256_hex;

const ALLOWED_TYPES: &[&str] = &[
    "feat", "fix", "refactor", "docs", "test", "chore", "style", "perf",
];

pub(super) fn capture_baseline_if_absent(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
    reason: &str,
) -> Result<()> {
    if is_subagent_payload(payload) {
        return Ok(());
    }
    let path = guard_path(state_dir, payload);
    if path.exists() {
        return Ok(());
    }
    let Some(snapshot) = GitSnapshot::capture(payload)? else {
        return Ok(());
    };
    let state = json!({
        "version": 1,
        "session_id": canonical_session_id(payload),
        "captured_at": timestamp,
        "capture_reason": reason,
        "repo_root": snapshot.repo_root,
        "baseline": snapshot.to_json(),
        "payload": payload_file,
    });
    write_json_atomic(&path, &state)?;
    append_jsonl(
        &state_dir.join("git-guard/events.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "git_baseline_captured",
            "session_id": canonical_session_id(payload),
            "reason": reason,
            "payload": payload_file,
        }),
    )
}

pub(super) fn block_unsafe_staging_if_needed(
    timestamp: &str,
    state_dir: &Path,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Result<Option<String>> {
    if options.event != "PreToolUse" {
        return Ok(None);
    }
    let Some(command) = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|input| input.get("command").or_else(|| input.get("cmd")))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    if !uses_unsafe_git_add(command) {
        return Ok(None);
    }
    capture_baseline_if_absent(timestamp, state_dir, payload, payload_file, "pre_tool")?;
    append_jsonl(
        &state_dir.join("git-guard/events.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "unsafe_git_add_blocked",
            "session_id": canonical_session_id(payload),
            "command": command,
            "payload": payload_file,
        }),
    )?;
    Ok(Some(
        "MEGARA git guard: do not use `git add .` or `git add -A`. Stage explicit files, then create a focused Conventional Commit."
            .to_string(),
    ))
}

pub(super) fn block_completion_if_needed(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
    assistant_message: &str,
) -> Result<Option<String>> {
    let Some(state) = load_json(&guard_path(state_dir, payload)) else {
        return Ok(None);
    };
    if !looks_like_completion(assistant_message) {
        return Ok(None);
    }

    let Some(baseline) = GitSnapshot::from_state(&state) else {
        return Ok(None);
    };
    let Some(current) = GitSnapshot::capture_at(&baseline.repo_root)? else {
        return Ok(None);
    };
    if current.repo_root != baseline.repo_root {
        return Ok(None);
    }

    let evaluation = evaluate_repo(&baseline, &current)?;
    if evaluation.is_clear() {
        return Ok(None);
    }

    append_jsonl(
        &state_dir.join("git-guard/events.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "completion_blocked",
            "session_id": canonical_session_id(payload),
            "reasons": evaluation.reasons,
            "payload": payload_file,
        }),
    )?;
    Ok(Some(evaluation.message()))
}

fn evaluate_repo(baseline: &GitSnapshot, current: &GitSnapshot) -> Result<GitEvaluation> {
    let mut result = GitEvaluation::default();

    for (path, fingerprint) in &current.path_fingerprints {
        match baseline.path_fingerprints.get(path) {
            Some(previous) if previous == fingerprint => {}
            Some(_) => {
                result.overlap_paths.insert(path.to_string());
            }
            None => {
                if is_forbidden_path(path) {
                    result.forbidden_paths.insert(path.to_string());
                }
                result.uncommitted_paths.insert(path.to_string());
            }
        }
    }
    for path in baseline.path_fingerprints.keys() {
        if !current.path_fingerprints.contains_key(path) {
            result.overlap_paths.insert(path.to_string());
        }
    }

    for commit in post_baseline_commits(baseline)? {
        let files = commit_files(&baseline.repo_root, &commit)?;
        let files = files
            .into_iter()
            .filter(|path| !is_runtime_path(path))
            .collect::<Vec<_>>();
        if files.is_empty() {
            continue;
        }
        let subject = commit_subject(&baseline.repo_root, &commit)?;
        if let Some(reason) = validate_commit_subject(&subject) {
            result.commit_issues.push(format!("{commit}: {reason}"));
        }
        if let Some(reason) = validate_single_intent(&files) {
            result.commit_issues.push(format!("{commit}: {reason}"));
        }
        for file in files {
            if baseline.path_fingerprints.contains_key(&file) {
                result.overlap_paths.insert(file.clone());
            }
            if is_forbidden_path(&file) {
                result.forbidden_paths.insert(file);
            }
        }
    }

    if let Some(reason) =
        validate_single_intent(&result.uncommitted_paths.iter().cloned().collect::<Vec<_>>())
    {
        result.intent_issues.push(reason);
    }

    Ok(result)
}

fn post_baseline_commits(baseline: &GitSnapshot) -> Result<Vec<String>> {
    if let Some(head) = baseline.head.as_deref() {
        return Ok(git_lines(
            &baseline.repo_root,
            &["rev-list", "--reverse", &format!("{head}..HEAD")],
        )
        .unwrap_or_default());
    }
    Ok(git_lines(&baseline.repo_root, &["rev-list", "--reverse", "HEAD"]).unwrap_or_default())
}

fn commit_files(repo_root: &Path, commit: &str) -> Result<Vec<String>> {
    Ok(git_lines(
        repo_root,
        &[
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-only",
            "-r",
            commit,
        ],
    )
    .unwrap_or_default())
}

fn commit_subject(repo_root: &Path, commit: &str) -> Result<String> {
    Ok(
        git_output(repo_root, &["show", "-s", "--format=%s", commit])
            .unwrap_or_default()
            .trim()
            .to_string(),
    )
}

fn validate_commit_subject(subject: &str) -> Option<String> {
    if subject.len() > 72 {
        return Some("commit subject must be 72 characters or less".to_string());
    }
    let Some((prefix, description)) = subject.split_once(": ") else {
        return Some("commit subject must use `<type>(<scope>): <description>`".to_string());
    };
    let commit_type = prefix.split('(').next().unwrap_or(prefix);
    if !ALLOWED_TYPES.contains(&commit_type) {
        return Some("commit type must follow OMA /scm allowed types".to_string());
    }
    if prefix.contains('(') && !prefix.ends_with(')') {
        return Some("commit scope must use `<type>(<scope>)`".to_string());
    }
    if description.trim().is_empty() {
        return Some("commit description is required".to_string());
    }
    if description.ends_with('.') {
        return Some("commit description must not end with a period".to_string());
    }
    if description
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() && !ch.is_ascii_lowercase())
    {
        return Some("commit description must start lowercase".to_string());
    }
    None
}

fn validate_single_intent(files: &[String]) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    let groups = files
        .iter()
        .map(|path| path_group(path))
        .collect::<BTreeSet<_>>();
    let primary_groups = groups
        .iter()
        .filter(|group| !is_support_group(group))
        .collect::<BTreeSet<_>>();
    if files.len() > 5 && primary_groups.len() > 1 {
        return Some("split mixed path groups into focused commits".to_string());
    }
    None
}

fn path_group(path: &str) -> String {
    let mut parts = path.split('/');
    match (parts.next(), parts.next()) {
        (Some("src"), Some("targets")) => "projection".to_string(),
        (Some("src"), Some(next)) => format!("src/{next}"),
        (Some("test" | "tests"), _) => "test".to_string(),
        (Some("docs" | "doc"), _) => "docs".to_string(),
        (Some("harness"), Some(next)) => format!("harness/{next}"),
        (Some("Cargo.toml" | "Cargo.lock"), _) => "build".to_string(),
        (Some(first), _) => first.to_string(),
        _ => "root".to_string(),
    }
}

fn is_support_group(group: &str) -> bool {
    matches!(group, "test" | "docs" | "harness" | "build" | "projection")
        || group.starts_with("harness/")
}

#[derive(Default)]
struct GitEvaluation {
    uncommitted_paths: BTreeSet<String>,
    overlap_paths: BTreeSet<String>,
    forbidden_paths: BTreeSet<String>,
    commit_issues: Vec<String>,
    intent_issues: Vec<String>,
    reasons: Vec<String>,
}

impl GitEvaluation {
    fn is_clear(&self) -> bool {
        self.uncommitted_paths.is_empty()
            && self.overlap_paths.is_empty()
            && self.forbidden_paths.is_empty()
            && self.commit_issues.is_empty()
            && self.intent_issues.is_empty()
    }

    fn message(mut self) -> String {
        if !self.uncommitted_paths.is_empty() {
            self.reasons.push(format!(
                "uncommitted agent changes: {}",
                preview_set(&self.uncommitted_paths)
            ));
        }
        if !self.overlap_paths.is_empty() {
            self.reasons.push(format!(
                "agent changes overlap pre-existing dirty paths: {}",
                preview_set(&self.overlap_paths)
            ));
        }
        if !self.forbidden_paths.is_empty() {
            self.reasons.push(format!(
                "secret-like paths must not be committed: {}",
                preview_set(&self.forbidden_paths)
            ));
        }
        self.reasons.extend(self.commit_issues);
        self.reasons.extend(self.intent_issues);
        format!(
            "MEGARA git guard: commit required before completion. {}. Stage explicit files only (`git add <file>`), create focused OMA /scm-style Conventional Commits, rerun verification, then answer again.",
            self.reasons.join("; ")
        )
    }
}

#[derive(Clone)]
struct GitSnapshot {
    repo_root: PathBuf,
    head: Option<String>,
    branch: Option<String>,
    status: Vec<String>,
    tracked_diff: String,
    staged_diff: String,
    path_fingerprints: BTreeMap<String, String>,
}

impl GitSnapshot {
    fn capture(payload: &Value) -> Result<Option<Self>> {
        Self::capture_at(&work_dir(payload))
    }

    fn capture_at(work_dir: &Path) -> Result<Option<Self>> {
        let Some(repo_root_text) = git_output(work_dir, &["rev-parse", "--show-toplevel"]) else {
            return Ok(None);
        };
        let repo_root = PathBuf::from(repo_root_text.trim());
        let head = git_output(&repo_root, &["rev-parse", "--verify", "HEAD"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let branch = git_output(&repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let status = git_lines(
            &repo_root,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !status_path(line).is_some_and(is_runtime_path))
        .collect::<Vec<_>>();
        let tracked_diff = diff_fingerprint(&repo_root, false, None);
        let staged_diff = diff_fingerprint(&repo_root, true, None);
        let path_fingerprints = path_fingerprints(&repo_root, &status);
        Ok(Some(Self {
            repo_root,
            head,
            branch,
            status,
            tracked_diff,
            staged_diff,
            path_fingerprints,
        }))
    }

    fn from_state(state: &Value) -> Option<Self> {
        let baseline = state.get("baseline")?;
        let repo_root = PathBuf::from(baseline.get("repo_root")?.as_str()?);
        let head = baseline
            .get("head")
            .and_then(Value::as_str)
            .map(str::to_string);
        let branch = baseline
            .get("branch")
            .and_then(Value::as_str)
            .map(str::to_string);
        let status = baseline
            .get("status")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let tracked_diff = baseline
            .get("tracked_diff")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let staged_diff = baseline
            .get("staged_diff")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let path_fingerprints = baseline
            .get("path_fingerprints")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(path, value)| {
                value
                    .as_str()
                    .map(|hash| (path.to_string(), hash.to_string()))
            })
            .collect::<BTreeMap<_, _>>();
        Some(Self {
            repo_root,
            head,
            branch,
            status,
            tracked_diff,
            staged_diff,
            path_fingerprints,
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "repo_root": self.repo_root,
            "head": self.head,
            "branch": self.branch,
            "status": self.status,
            "tracked_diff": self.tracked_diff,
            "staged_diff": self.staged_diff,
            "path_fingerprints": self.path_fingerprints,
        })
    }
}

fn path_fingerprints(repo_root: &Path, status: &[String]) -> BTreeMap<String, String> {
    let mut paths = BTreeSet::new();
    for path in git_lines(repo_root, &["diff", "--name-only"]).unwrap_or_default() {
        if !is_runtime_path(&path) {
            paths.insert(path);
        }
    }
    for path in git_lines(repo_root, &["diff", "--cached", "--name-only"]).unwrap_or_default() {
        if !is_runtime_path(&path) {
            paths.insert(path);
        }
    }
    for line in status {
        if let Some(path) = status_path(line) {
            if !is_runtime_path(path) {
                paths.insert(path.to_string());
            }
        }
    }

    paths
        .into_iter()
        .map(|path| {
            let fingerprint = if repo_root.join(&path).is_file() && is_untracked(status, &path) {
                fs::read(repo_root.join(&path))
                    .map(|bytes| sha256_hex(&bytes))
                    .unwrap_or_else(|_| "unreadable".to_string())
            } else {
                let mut content = String::new();
                content.push_str(&diff_fingerprint(repo_root, false, Some(&path)));
                content.push_str(&diff_fingerprint(repo_root, true, Some(&path)));
                sha256_hex(content.as_bytes())
            };
            (path, fingerprint)
        })
        .collect()
}

fn is_untracked(status: &[String], path: &str) -> bool {
    status
        .iter()
        .any(|line| line.starts_with("?? ") && status_path(line) == Some(path))
}

fn diff_fingerprint(repo_root: &Path, cached: bool, path: Option<&str>) -> String {
    let mut args = vec!["diff", "--binary"];
    if cached {
        args.insert(1, "--cached");
    }
    if let Some(path) = path {
        args.push("--");
        args.push(path);
    }
    git_output(repo_root, &args)
        .map(|diff| sha256_hex(diff.as_bytes()))
        .unwrap_or_default()
}

fn status_path(line: &str) -> Option<&str> {
    let path = line.get(3..)?.trim();
    let path = path.rsplit(" -> ").next().unwrap_or(path);
    (!path.is_empty()).then_some(path.trim_matches('"'))
}

fn work_dir(payload: &Value) -> PathBuf {
    payload
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn guard_path(state_dir: &Path, payload: &Value) -> PathBuf {
    state_dir
        .join("git-guard")
        .join(format!("{}.json", safe_part(canonical_session_id(payload))))
}

fn git_lines(repo_root: &Path, args: &[&str]) -> Option<Vec<String>> {
    git_output(repo_root, args).map(|output| {
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn git_output(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn uses_unsafe_git_add(command: &str) -> bool {
    command
        .split([';', '&', '|'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            let tokens = segment.split_whitespace().collect::<Vec<_>>();
            tokens.first() == Some(&"git")
                && tokens.get(1) == Some(&"add")
                && tokens
                    .iter()
                    .skip(2)
                    .any(|token| matches!(*token, "." | "-A" | "--all"))
        })
}

fn is_subagent_payload(payload: &Value) -> bool {
    ["agent_id", "subagent_id", "agent_type", "subagent_name"]
        .into_iter()
        .any(|key| payload.get(key).is_some())
}

fn is_runtime_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized == ".megara" || normalized.starts_with(".megara/")
}

fn is_forbidden_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let file = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    file == ".env"
        || file == ".env.local"
        || file == "credentials.json"
        || file == "secrets.yaml"
        || file.ends_with(".env")
        || file.contains(".env.")
        || file.ends_with(".pem")
        || file.ends_with(".key")
}

fn preview_set(values: &BTreeSet<String>) -> String {
    let mut items = values.iter().take(5).cloned().collect::<Vec<_>>();
    if values.len() > items.len() {
        items.push(format!("+{} more", values.len() - items.len()));
    }
    items.join(", ")
}

fn looks_like_completion(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    [
        "complete",
        "completed",
        "done",
        "implemented",
        "fixed",
        "resolved",
        "pass",
        "완료",
        "구현",
        "수정",
        "해결",
        "통과",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}
