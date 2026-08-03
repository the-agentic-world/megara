use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::super::protocol::LogicalRequest;
use super::super::store::StoredOutcome;
use super::response::mutation_response;

#[derive(Clone, Copy)]
pub(crate) enum ArtifactKind {
    Spec,
    Plan,
}

impl ArtifactKind {
    fn name(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::Plan => "plan",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "spec" => Some(Self::Spec),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionStatus {
    Written,
    Unchanged,
    Missing,
    Stale,
    Conflict,
    IoError,
}

impl ProjectionStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Written => "written",
            Self::Unchanged => "unchanged",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Conflict => "conflict",
            Self::IoError => "io_error",
        }
    }
}

pub(crate) fn inspect_candidate_projection(
    root: &Path,
    session_id: &str,
    kind: &str,
    candidate: &Value,
) -> ProjectionStatus {
    let Some(kind) = ArtifactKind::from_name(kind) else {
        return ProjectionStatus::IoError;
    };
    let directory = root
        .join(".megara")
        .join("planning")
        .join("artifacts")
        .join(session_id);
    let path = directory.join(format!("{}.md", kind.name()));
    let manifest_path = directory.join("projection-manifest.json");
    let manifest = match read_manifest(&manifest_path, session_id) {
        Ok(manifest) => manifest,
        Err(status) => return status,
    };
    let managed_digest = manifest.as_ref().and_then(|manifest| {
        manifest["files"][kind.name()]["digest"]
            .as_str()
            .map(str::to_owned)
    });
    let expected = render_markdown(session_id, kind, candidate);
    let expected_digest = digest_bytes(expected.as_bytes());
    match fs::read(&path) {
        Ok(existing)
            if existing == expected.as_bytes()
                && managed_digest.as_deref() == Some(&expected_digest) =>
        {
            ProjectionStatus::Unchanged
        }
        Ok(existing) if managed_digest.as_deref() == Some(&digest_bytes(&existing)) => {
            ProjectionStatus::Stale
        }
        Ok(_) => ProjectionStatus::Conflict,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ProjectionStatus::Missing,
        Err(_) => ProjectionStatus::IoError,
    }
}

pub(crate) fn repair_candidate_projection(
    root: &Path,
    session_id: &str,
    kind: &str,
    candidate: &Value,
) -> ProjectionStatus {
    let Some(kind) = ArtifactKind::from_name(kind) else {
        return ProjectionStatus::IoError;
    };
    write_projection(root, session_id, kind, candidate, false)
}

pub(super) fn project_generated_candidate(
    request: &LogicalRequest,
    outcome: StoredOutcome,
    force: bool,
    root: &Path,
    kind: ArtifactKind,
) -> Value {
    let session_id = outcome.state.session_id.clone();
    let candidate = match kind {
        ArtifactKind::Spec => outcome
            .state
            .spec
            .current_candidate
            .as_ref()
            .and_then(|candidate| serde_json::to_value(candidate).ok()),
        ArtifactKind::Plan => outcome
            .state
            .plan
            .current_candidate
            .as_ref()
            .and_then(|candidate| serde_json::to_value(candidate).ok()),
    };
    let mut response = mutation_response(request, outcome, json!({"candidate": candidate}));
    let status = candidate
        .as_ref()
        .map(|candidate| write_projection(root, &session_id, kind, candidate, force))
        .unwrap_or(ProjectionStatus::IoError);
    response["observed"]["projection_status"] = json!(status.as_str());
    if status == ProjectionStatus::Conflict {
        response["observed"]["warnings"] = json!(["PROJECTION_CONFLICT"]);
    } else if status == ProjectionStatus::IoError {
        response["observed"]["warnings"] = json!(["PROJECTION_IO"]);
    }
    response
}

fn write_projection(
    root: &Path,
    session_id: &str,
    kind: ArtifactKind,
    candidate: &Value,
    force: bool,
) -> ProjectionStatus {
    let directory = root
        .join(".megara")
        .join("planning")
        .join("artifacts")
        .join(session_id);
    let path = directory.join(format!("{}.md", kind.name()));
    let manifest_path = directory.join("projection-manifest.json");
    let content = render_markdown(session_id, kind, candidate);
    let expected_digest = digest_bytes(content.as_bytes());
    let manifest = match read_manifest(&manifest_path, session_id) {
        Ok(manifest) => manifest,
        Err(status) => return status,
    };
    let managed_digest = manifest.as_ref().and_then(|manifest| {
        manifest["files"][kind.name()]["digest"]
            .as_str()
            .map(str::to_owned)
    });
    match fs::read(&path) {
        Ok(existing)
            if !force
                && managed_digest
                    .as_deref()
                    .is_some_and(|digest| digest != digest_bytes(&existing)) =>
        {
            ProjectionStatus::Conflict
        }
        Ok(existing) if !force && managed_digest.is_some() && existing == content.as_bytes() => {
            ProjectionStatus::Unchanged
        }
        Ok(existing) if !force => {
            let managed = managed_digest
                .as_deref()
                .is_some_and(|digest| digest == digest_bytes(&existing));
            if !managed {
                return ProjectionStatus::Conflict;
            }
            atomic_write_projection(
                &path,
                content.as_bytes(),
                Some(&digest_bytes(&existing)),
                false,
            )
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => ProjectionStatus::IoError,
        _ => atomic_write_projection(&path, content.as_bytes(), None, force),
    }
    .and_then_manifest(
        &manifest_path,
        session_id,
        kind,
        candidate,
        expected_digest,
        manifest,
    )
}

fn read_manifest(
    manifest_path: &Path,
    session_id: &str,
) -> Result<Option<Value>, ProjectionStatus> {
    match fs::read(manifest_path) {
        Ok(bytes) => {
            let Ok(manifest) = serde_json::from_slice::<Value>(&bytes) else {
                return Err(ProjectionStatus::IoError);
            };
            if manifest["schema"] != "megara.projection-manifest/v1"
                || manifest["session_id"] != session_id
                || !manifest["files"].is_object()
            {
                return Err(ProjectionStatus::IoError);
            }
            Ok(Some(manifest))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ProjectionStatus::IoError),
    }
}

trait ProjectionStatusExt {
    fn and_then_manifest(
        self,
        manifest_path: &Path,
        session_id: &str,
        kind: ArtifactKind,
        candidate: &Value,
        expected_digest: String,
        manifest: Option<Value>,
    ) -> ProjectionStatus;
}

impl ProjectionStatusExt for ProjectionStatus {
    fn and_then_manifest(
        self,
        manifest_path: &Path,
        session_id: &str,
        kind: ArtifactKind,
        candidate: &Value,
        expected_digest: String,
        manifest: Option<Value>,
    ) -> ProjectionStatus {
        if self != ProjectionStatus::Written {
            return self;
        }
        let mut manifest = manifest.unwrap_or_else(
            || json!({"schema":"megara.projection-manifest/v1","session_id":session_id,"files":{}}),
        );
        manifest["schema"] = json!("megara.projection-manifest/v1");
        manifest["session_id"] = json!(session_id);
        manifest["files"][kind.name()] = json!({
            "path": format!("{}.md", kind.name()),
            "digest": expected_digest,
            "candidate_id": candidate["candidate_id"],
            "semantic_hash": candidate["semantic_hash"],
            "base_revision": candidate["base_domain_revision"]
                .as_u64()
                .or_else(|| candidate["base_plan_revision"].as_u64())
                .unwrap_or_default()
        });
        let Ok(bytes) = serde_json::to_vec_pretty(&manifest) else {
            return ProjectionStatus::IoError;
        };
        if atomic_write(manifest_path, &bytes).is_err() {
            ProjectionStatus::IoError
        } else {
            ProjectionStatus::Written
        }
    }
}

fn render_markdown(session_id: &str, kind: ArtifactKind, candidate: &Value) -> String {
    let candidate_id = candidate["candidate_id"].as_str().unwrap_or("unknown");
    let semantic_hash = candidate["semantic_hash"].as_str().unwrap_or("unknown");
    let base = candidate
        .get("base_domain_revision")
        .or_else(|| candidate.get("base_plan_revision"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    format!(
        "<!--\nGenerated by Megara Planning Core.\nDo not edit directly.\nsession_id: {session_id}\ncandidate_id: {candidate_id}\nsemantic_hash: {semantic_hash}\nbase_revision: {base}\n-->\n{body}\n",
        body = render_candidate_markdown(candidate, kind.name())
    )
}

pub(crate) fn render_candidate_markdown(candidate: &Value, kind: &str) -> String {
    let default_title = if kind == "plan" {
        "Planning plan"
    } else {
        "Planning specification"
    };
    let title = candidate["content"]["title"]
        .as_str()
        .unwrap_or(default_title);
    let order = if kind == "plan" {
        &["baseline", "steps", "verifications", "plan_risks"][..]
    } else {
        &[
            "schema",
            "title",
            "summary",
            "problem_ref",
            "outcome_refs",
            "decision_refs",
            "decision_boundary_refs",
            "requirement_refs",
            "acceptance_criterion_refs",
            "constraint_refs",
            "non_goal_refs",
            "assumption_refs",
            "risk_refs",
            "advisories",
            "entities",
        ][..]
    };
    format!(
        "# {title}\n\n{}",
        render_ordered_body(&candidate["content"], order)
    )
}

fn render_ordered_body(content: &Value, order: &[&str]) -> String {
    let mut output = String::new();
    for key in order {
        if let Some(value) = content.get(*key) {
            render_field(&mut output, key, value, 0);
        }
    }
    output.trim_end().to_string()
}

fn render_field(output: &mut String, key: &str, value: &Value, indent: usize) {
    let prefix = "  ".repeat(indent);
    match value {
        Value::Object(object) => {
            output.push_str(&format!("{prefix}- {key}:\n"));
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            for child_key in keys {
                render_field(output, child_key, &object[child_key], indent + 1);
            }
        }
        Value::Array(values) => {
            output.push_str(&format!("{prefix}- {key}:\n"));
            for value in values {
                match value {
                    Value::Object(object) => {
                        output.push_str(&format!("{prefix}  -\n"));
                        let mut keys = object.keys().collect::<Vec<_>>();
                        keys.sort();
                        for child_key in keys {
                            render_field(output, child_key, &object[child_key], indent + 2);
                        }
                    }
                    _ => output.push_str(&format!("{prefix}  - {}\n", markdown_scalar(value))),
                }
            }
        }
        _ => output.push_str(&format!("{prefix}- {key}: {}\n", markdown_scalar(value))),
    }
}

fn markdown_scalar(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

enum AtomicWriteError {
    Conflict,
    Io(std::io::Error),
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    match atomic_write_inner(path, bytes, None, true) {
        Ok(()) => Ok(()),
        Err(AtomicWriteError::Io(error)) => Err(error),
        Err(AtomicWriteError::Conflict) => {
            Err(std::io::Error::other("unexpected atomic write conflict"))
        }
    }
}

fn atomic_write_projection(
    path: &Path,
    bytes: &[u8],
    expected_digest: Option<&str>,
    force: bool,
) -> ProjectionStatus {
    match atomic_write_inner(path, bytes, expected_digest, force) {
        Ok(()) => ProjectionStatus::Written,
        Err(AtomicWriteError::Conflict) => ProjectionStatus::Conflict,
        Err(AtomicWriteError::Io(_)) => ProjectionStatus::IoError,
    }
}

fn atomic_write_inner(
    path: &Path,
    bytes: &[u8],
    expected_digest: Option<&str>,
    force: bool,
) -> Result<(), AtomicWriteError> {
    let parent = path
        .parent()
        .ok_or_else(|| AtomicWriteError::Io(std::io::Error::other("projection has no parent")))?;
    fs::create_dir_all(parent).map_err(AtomicWriteError::Io)?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        Uuid::now_v7()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(AtomicWriteError::Io)?;
        file.write_all(bytes).map_err(AtomicWriteError::Io)?;
        file.sync_all().map_err(AtomicWriteError::Io)?;
        drop(file);
        let written = fs::read(&temp).map_err(AtomicWriteError::Io)?;
        if digest_bytes(&written) != digest_bytes(bytes) {
            return Err(AtomicWriteError::Io(std::io::Error::other(
                "temporary digest mismatch",
            )));
        }
        if !force {
            let current_digest = match fs::read(path) {
                Ok(current) => Some(digest_bytes(&current)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(AtomicWriteError::Io(error)),
            };
            if current_digest.as_deref() != expected_digest {
                return Err(AtomicWriteError::Conflict);
            }
        }
        fs::rename(&temp, path).map_err(AtomicWriteError::Io)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(super) fn atomic_export_write(
    path: &Path,
    bytes: &[u8],
    force: bool,
) -> Result<(), super::error::ServiceError> {
    if path.exists() && !force {
        return Err(super::error::ServiceError::with_code(
            "PROJECTION_DIVERGED",
            format!("export output already exists: {}", path.display()),
        ));
    }
    match atomic_write_inner(path, bytes, None, force) {
        Ok(()) => Ok(()),
        Err(AtomicWriteError::Conflict) => Err(super::error::ServiceError::with_code(
            "PROJECTION_DIVERGED",
            format!("export output appeared during write: {}", path.display()),
        )),
        Err(AtomicWriteError::Io(error)) => Err(super::error::ServiceError::with_code(
            "IO_ERROR",
            error.to_string(),
        )),
    }
}

pub(super) fn render_export_markdown(session_id: &str, kind: &str, candidate: &Value) -> String {
    render_markdown(
        session_id,
        if kind == "plan" {
            ArtifactKind::Plan
        } else {
            ArtifactKind::Spec
        },
        candidate,
    )
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}
