use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::domain::{EvidenceRange, EvidenceRecord, RepoEvidenceSnapshot};

pub const EVIDENCE_CITATIONS_SCHEMA: &str = "megara.evidence-citations/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCitation {
    pub temp_ref: String,
    pub path: String,
    pub ranges: Vec<EvidenceRange>,
    pub claim: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCitationRequest {
    pub schema: String,
    pub base_revision: u64,
    pub citations: Vec<EvidenceCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceError {
    InvalidRequest(String),
    PathOutsideRoot(String),
    SensitivePath(String),
    IgnoredPath(String),
    MissingFile(String),
    InvalidRange(String),
    Git(String),
    Io(String),
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "invalid evidence request: {message}"),
            Self::PathOutsideRoot(path) => {
                write!(f, "evidence path is outside project root: {path}")
            }
            Self::SensitivePath(path) => write!(f, "sensitive evidence path is forbidden: {path}"),
            Self::IgnoredPath(path) => write!(f, "ignored evidence path is forbidden: {path}"),
            Self::MissingFile(path) => write!(f, "cited evidence file is missing: {path}"),
            Self::InvalidRange(message) => write!(f, "invalid evidence range: {message}"),
            Self::Git(message) => write!(f, "git evidence inspection failed: {message}"),
            Self::Io(message) => write!(f, "evidence I/O failed: {message}"),
        }
    }
}

impl std::error::Error for EvidenceError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitRepoIdentity {
    pub head_oid: Option<String>,
    pub head_ref: Option<String>,
    pub dirty: bool,
    pub status_hash: String,
    pub cited_files_hash: String,
}

pub fn capture_snapshot(
    root: impl AsRef<Path>,
    citations: &[EvidenceCitation],
) -> Result<RepoEvidenceSnapshot, EvidenceError> {
    capture_snapshot_with_previous(root, citations, None)
}

pub fn capture_snapshot_with_previous(
    root: impl AsRef<Path>,
    citations: &[EvidenceCitation],
    previous: Option<&RepoEvidenceSnapshot>,
) -> Result<RepoEvidenceSnapshot, EvidenceError> {
    let root = root
        .as_ref()
        .canonicalize()
        .map_err(|error| EvidenceError::Io(error.to_string()))?;
    let git = inspect_git(&root)?;
    let mut temp_refs = BTreeSet::new();
    let mut semantic_keys = BTreeSet::new();
    let mut next_id = previous.map(next_evidence_id).transpose()?.unwrap_or(1);
    let mut records = Vec::with_capacity(citations.len());
    for citation in citations {
        if citation.temp_ref.trim().is_empty() || !temp_refs.insert(&citation.temp_ref) {
            return Err(EvidenceError::InvalidRequest(
                "citation temp_ref must be unique and non-empty".to_string(),
            ));
        }
        if citation.claim.trim().is_empty() {
            return Err(EvidenceError::InvalidRequest(
                "citation claim must not be blank".to_string(),
            ));
        }
        let (relative, absolute, target_relative) = safe_citation_path(&root, &citation.path)?;
        let tracked = git
            .as_ref()
            .map(|_| git_is_tracked(&root, &relative))
            .transpose()?
            .unwrap_or(false);
        if relative
            .rsplit('/')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(".env.example"))
            && !tracked
        {
            return Err(EvidenceError::SensitivePath(relative));
        }
        if git.is_some()
            && (git_is_ignored(&root, &relative)? || git_is_ignored(&root, &target_relative)?)
        {
            return Err(EvidenceError::IgnoredPath(relative));
        }
        let bytes = std::fs::read(&absolute)
            .map_err(|error| EvidenceError::MissingFile(format!("{}: {error}", relative)))?;
        let line_count = count_lines(&bytes)?;
        validate_ranges(&citation.ranges, line_count)?;
        let key = (relative.clone(), citation.ranges.clone());
        if !semantic_keys.insert(key.clone()) {
            return Err(EvidenceError::InvalidRequest(
                "duplicate citation path and ranges are not allowed".to_string(),
            ));
        }
        let evidence_id = previous
            .and_then(|snapshot| {
                snapshot
                    .evidence
                    .iter()
                    .find(|record| record.path == key.0 && record.ranges == key.1)
                    .map(|record| record.evidence_id.clone())
            })
            .unwrap_or_else(|| {
                let id = format!("EVID-{next_id:03}");
                next_id += 1;
                id
            });
        records.push(EvidenceRecord {
            evidence_id,
            path: relative,
            ranges: citation.ranges.clone(),
            size: bytes.len() as u64,
            sha256: digest(&bytes),
            tracked,
            captured_at: timestamp(),
        });
    }
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let cited_files_hash = digest_json(&records);
    let (head_oid, head_ref, status_hash, dirty) = git.map_or_else(
        || (None, None, digest(b"non-git"), false),
        |identity| {
            (
                identity.head_oid,
                identity.head_ref,
                identity.status_hash,
                identity.dirty,
            )
        },
    );
    let evidence_hash = digest_json(&(
        root.to_string_lossy().to_string(),
        &head_oid,
        &head_ref,
        dirty,
        &status_hash,
        &cited_files_hash,
        &records,
    ));
    Ok(RepoEvidenceSnapshot {
        evidence_hash,
        head_oid,
        head_ref,
        dirty,
        status_hash,
        cited_files_hash,
        evidence: records,
    })
}

pub fn snapshot_is_current(
    root: impl AsRef<Path>,
    stored: &RepoEvidenceSnapshot,
) -> Result<bool, EvidenceError> {
    let citations = stored
        .evidence
        .iter()
        .map(|record| EvidenceCitation {
            temp_ref: record.evidence_id.clone(),
            path: record.path.clone(),
            ranges: record.ranges.clone(),
            claim: "stored evidence citation".to_string(),
        })
        .collect::<Vec<_>>();
    match capture_snapshot_with_previous(root, &citations, Some(stored)) {
        Ok(current) => Ok(current.semantic_eq(stored)),
        Err(EvidenceError::MissingFile(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

fn next_evidence_id(snapshot: &RepoEvidenceSnapshot) -> Result<u64, EvidenceError> {
    snapshot
        .evidence
        .iter()
        .map(|record| {
            record
                .evidence_id
                .strip_prefix("EVID-")
                .and_then(|number| number.parse::<u64>().ok())
                .ok_or_else(|| {
                    EvidenceError::InvalidRequest(
                        "stored evidence IDs must use EVID-<number>".to_string(),
                    )
                })
        })
        .try_fold(1, |next, value| Ok(next.max(value? + 1)))
}

fn safe_citation_path(root: &Path, raw: &str) -> Result<(String, PathBuf, String), EvidenceError> {
    if raw.trim().is_empty() || Path::new(raw).is_absolute() {
        return Err(EvidenceError::PathOutsideRoot(raw.to_string()));
    }
    let path = Path::new(raw);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(EvidenceError::PathOutsideRoot(raw.to_string()));
    }
    let lexical_relative = lexical_path(path);
    let lexical_folded = lexical_relative.to_ascii_lowercase();
    let lexical_basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase);
    if lexical_folded == ".git"
        || lexical_folded.starts_with(".git/")
        || lexical_folded == ".megara"
        || lexical_folded.starts_with(".megara/")
        || lexical_basename.is_some_and(|name| is_sensitive_basename(&name))
    {
        return Err(EvidenceError::SensitivePath(lexical_relative));
    }
    let absolute = root.join(path);
    let canonical = absolute
        .canonicalize()
        .map_err(|_| EvidenceError::MissingFile(raw.to_string()))?;
    if !canonical.starts_with(root) {
        return Err(EvidenceError::PathOutsideRoot(raw.to_string()));
    }
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| EvidenceError::PathOutsideRoot(raw.to_string()))?
        .to_string_lossy()
        .replace('\\', "/");
    let folded = relative.to_ascii_lowercase();
    if folded == ".git"
        || folded.starts_with(".git/")
        || folded == ".megara"
        || folded.starts_with(".megara/")
    {
        return Err(EvidenceError::SensitivePath(relative));
    }
    let basename = Path::new(&relative)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if is_sensitive_basename(&basename) {
        return Err(EvidenceError::SensitivePath(relative));
    }
    if !canonical.is_file() {
        return Err(EvidenceError::MissingFile(relative));
    }
    Ok((lexical_relative, canonical, relative))
}

fn lexical_path(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        if let Component::Normal(part) = component {
            normalized.push(part);
        }
    }
    normalized.to_string_lossy().replace('\\', "/")
}

fn is_sensitive_basename(basename: &str) -> bool {
    if basename == ".env" || basename == ".env.sample" || basename == ".env.template" {
        return true;
    }
    [
        "secret",
        "credential",
        "password",
        "passwd",
        "token",
        "api_key",
        "apikey",
    ]
    .iter()
    .any(|needle| basename.contains(needle))
        || Path::new(basename)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "pem" | "key" | "p12" | "pfx" | "der"))
}

fn validate_ranges(ranges: &[EvidenceRange], line_count: u64) -> Result<(), EvidenceError> {
    let mut seen = BTreeSet::new();
    for range in ranges {
        if range.start_line == 0 || range.end_line < range.start_line {
            return Err(EvidenceError::InvalidRange(
                "ranges are 1-based inclusive".to_string(),
            ));
        }
        if range.end_line > line_count {
            return Err(EvidenceError::InvalidRange(format!(
                "line {} exceeds EOF {}",
                range.end_line, line_count
            )));
        }
        if !seen.insert((range.start_line, range.end_line)) {
            return Err(EvidenceError::InvalidRange(
                "duplicate ranges are not allowed".to_string(),
            ));
        }
    }
    Ok(())
}

fn count_lines(bytes: &[u8]) -> Result<u64, EvidenceError> {
    if std::str::from_utf8(bytes).is_err() {
        return Err(EvidenceError::InvalidRequest(
            "cited file must be UTF-8 text".to_string(),
        ));
    }
    if bytes.is_empty() {
        return Ok(0);
    }
    let newline_count = bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
    Ok(newline_count + u64::from(bytes.last() != Some(&b'\n')))
}

fn inspect_git(root: &Path) -> Result<Option<GitRepoIdentity>, EvidenceError> {
    let top = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .map_err(|error| EvidenceError::Git(error.to_string()))?;
    if !top.status.success() {
        if top.status.code() == Some(128) {
            return Ok(None);
        }
        return Err(EvidenceError::Git(
            String::from_utf8_lossy(&top.stderr).trim().to_string(),
        ));
    }
    let head_oid = git_value(root, &["rev-parse", "--verify", "HEAD"]);
    let head_ref = git_value(root, &["symbolic-ref", "--short", "-q", "HEAD"]);
    let status = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).megara/**",
            ":(exclude).git/**",
        ])
        .output()
        .map_err(|error| EvidenceError::Git(error.to_string()))?;
    if !status.status.success() {
        return Err(EvidenceError::Git(
            String::from_utf8_lossy(&status.stderr).trim().to_string(),
        ));
    }
    Ok(Some(GitRepoIdentity {
        head_oid,
        head_ref,
        dirty: !status.stdout.is_empty(),
        status_hash: digest(&status.stdout),
        cited_files_hash: digest(b"no-citations"),
    }))
}

fn git_value(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_is_tracked(root: &Path, path: &str) -> Result<bool, EvidenceError> {
    let output = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "ls-files",
            "--error-unmatch",
            "--",
            path,
        ])
        .output()
        .map_err(|error| EvidenceError::Git(error.to_string()))?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    Err(EvidenceError::Git(
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

fn git_is_ignored(root: &Path, path: &str) -> Result<bool, EvidenceError> {
    let output = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "check-ignore",
            "-q",
            "--",
            path,
        ])
        .output()
        .map_err(|error| EvidenceError::Git(error.to_string()))?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    Err(EvidenceError::Git(
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn digest_json<T: Serialize>(value: &T) -> String {
    super::canonical::canonical_hash(value)
}

fn timestamp() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("unix-nanos:{nanos}")
}
