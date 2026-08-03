use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

pub(super) fn install(project: &Path, codex_home: &Path) {
    super::install_project_harness(project, codex_home);
}

pub(super) fn run(project: &Path, codex_home: &Path, args: &[&str]) -> Output {
    let mut command = super::megara_with_codex_home(codex_home);
    command.arg("planning").arg("migrate");
    command.args(args).current_dir(project);
    command.output().unwrap()
}

pub(super) fn write_legacy_file(project: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = project.join(".megara/state/workflows/legacy.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

pub(super) fn seed_codex_legacy_projections(project: &Path) {
    let fixture_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/fixtures/planning/legacy");
    for relative in [
        "skills/deep-interview/SKILL.md",
        "skills/ralplan/SKILL.md",
        "skills/team/SKILL.md",
        "skills/ultragoal/SKILL.md",
        "skill-fragments/deep-interview/auto-answer-uncertain.md",
        "skill-fragments/deep-interview/auto-research-greenfield.md",
        "skill-fragments/deep-interview/lateral-review-panel.md",
        "skill-fragments/ultragoal/ai-slop-cleaner.md",
    ] {
        let source = fixture_root.join(relative);
        for root in [".agents", ".codex"] {
            let target = project.join(root).join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(&source, target).unwrap();
        }
    }
    fs::copy(
        fixture_root.join("hooks.json"),
        project.join(".codex/hooks.json"),
    )
    .unwrap();
}

pub(super) fn report(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub(super) fn entry_action(
    report: &serde_json::Value,
    path: &str,
    action: &str,
) -> serde_json::Value {
    report["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["relative_path"] == path && entry["action"] == action)
        .cloned()
        .unwrap_or_else(|| {
            panic!("missing migration report entry for {path} action {action}: {report}")
        })
}

pub(super) fn rewrite_manifest<F>(project: &Path, migration_id: &str, edit: F)
where
    F: FnOnce(&mut serde_json::Value),
{
    let path = project
        .join(".megara/migration-backups")
        .join(migration_id)
        .join("manifest.json");
    let bytes = fs::read(&path).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    edit(&mut manifest);
    manifest["manifest_hash"] = serde_json::Value::String(String::new());
    manifest["manifest_hash"] =
        serde_json::Value::String(super::planning::canonical::canonical_hash(&manifest));
    fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

pub(super) fn write_owned_staging_marker(
    project: &Path,
    migration_id: &str,
    files: &[serde_json::Value],
) {
    let project_id = super::planning::store::canonical_project_identity(project)
        .unwrap()
        .project_id;
    let mut files = files.to_vec();
    files.sort_by(|left, right| {
        left["relative_path"]
            .as_str()
            .cmp(&right["relative_path"].as_str())
    });
    let marker = serde_json::json!({
        "schema": "megara.planning-migration-staging/v1",
        "migration_id": migration_id,
        "project_id": project_id,
        "nonce": uuid::Uuid::now_v7().to_string(),
        "inventory_hash": super::planning::canonical::canonical_hash(&files),
        "files": files,
    });
    let path = project
        .join(".megara/migration-backups/.staging")
        .join(migration_id)
        .join("staging.json");
    fs::write(path, serde_json::to_vec_pretty(&marker).unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let staging = project
            .join(".megara/migration-backups/.staging")
            .join(migration_id);
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(staging.join("files"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(
            staging.join("staging.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct TreeSnapshotEntry {
    pub relative_path: String,
    pub node_type: &'static str,
    pub bytes: Option<Vec<u8>>,
    pub symlink_target: Option<PathBuf>,
    pub mode: u32,
}

pub(super) fn tree_snapshot(root: &Path) -> Vec<TreeSnapshotEntry> {
    let mut entries = Vec::new();
    collect_tree_snapshot(root, root, &mut entries);
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    entries
}

fn collect_tree_snapshot(root: &Path, directory: &Path, entries: &mut Vec<TreeSnapshotEntry>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let relative_path = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let metadata = fs::symlink_metadata(&path).unwrap();
        let file_type = metadata.file_type();
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };
        #[cfg(not(unix))]
        let mode = 0;
        if file_type.is_symlink() {
            entries.push(TreeSnapshotEntry {
                relative_path,
                node_type: "symlink",
                bytes: None,
                symlink_target: Some(fs::read_link(&path).unwrap()),
                mode,
            });
        } else if file_type.is_dir() {
            entries.push(TreeSnapshotEntry {
                relative_path,
                node_type: "dir",
                bytes: None,
                symlink_target: None,
                mode,
            });
            collect_tree_snapshot(root, &path, entries);
        } else if file_type.is_file() {
            entries.push(TreeSnapshotEntry {
                relative_path,
                node_type: "file",
                bytes: Some(fs::read(&path).unwrap()),
                symlink_target: None,
                mode,
            });
        } else {
            entries.push(TreeSnapshotEntry {
                relative_path,
                node_type: "other",
                bytes: None,
                symlink_target: None,
                mode,
            });
        }
    }
}
