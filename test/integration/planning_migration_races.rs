use std::fs;

use tempfile::tempdir;

use super::planning_migration_support as support;

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(unix)]
#[test]
fn complete_prepared_staging_publishes_exact_id_before_resume() {
    use std::os::unix::fs::PermissionsExt;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let migration_id = "mig_prepared-crash";
    let relative = ".megara/state/workflows/prepared.json";
    let bytes = b"prepared legacy context";
    let source = project.path().join(relative);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, bytes).unwrap();
    let mode = fs::metadata(&source).unwrap().permissions().mode();
    let digest = sha256(bytes);
    let record = serde_json::json!({
        "relative_path": relative,
        "sha256": digest,
        "size": bytes.len(),
        "mode": mode,
        "kind": "opaque"
    });
    let staging_files = project
        .path()
        .join(".megara/migration-backups/.staging")
        .join(migration_id)
        .join("files")
        .join(".megara/state/workflows");
    fs::create_dir_all(&staging_files).unwrap();
    let backup = staging_files.join("prepared.json");
    fs::write(&backup, bytes).unwrap();
    fs::set_permissions(&backup, fs::Permissions::from_mode(mode)).unwrap();
    support::write_owned_staging_marker(
        project.path(),
        migration_id,
        std::slice::from_ref(&record),
    );

    let source_hash = super::planning::canonical::canonical_hash(&vec![serde_json::json!({
        "path": relative,
        "sha256": sha256(bytes),
        "size": bytes.len()
    })]);
    let project_id = super::planning::store::canonical_project_identity(project.path())
        .unwrap()
        .project_id;
    let import_command_id = format!(
        "cmd_mig_{}",
        super::planning::canonical::canonical_hash(&serde_json::json!([
            project_id,
            migration_id,
            "legacy-import",
            source_hash
        ]))
        .trim_start_matches("sha256:")
    );
    let mut manifest = serde_json::json!({
        "schema": "megara.planning-migration/v1",
        "manifest_hash": "",
        "migration_id": migration_id,
        "project_id": project_id,
        "source_bundle_hash": source_hash,
        "backup_bundle_hash": source_hash,
        "phase": "prepared",
        "files": [{
            "relative_path": relative,
            "sha256": sha256(bytes),
            "size": bytes.len(),
            "mode": mode,
            "kind": "opaque",
            "removable": true,
            "removed": false
        }],
        "session_id": null,
        "revision": null,
        "rollback_export_sha256": null,
        "import_command_id": import_command_id,
        "warnings": []
    });
    manifest["manifest_hash"] =
        serde_json::Value::String(super::planning::canonical::canonical_hash(&manifest));
    let staging_root = project
        .path()
        .join(".megara/migration-backups/.staging")
        .join(migration_id);
    let manifest_path = staging_root.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o600)).unwrap();

    let first = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(!first.status.success());
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    assert!(first_stderr.contains("MIGRATION_INCOMPLETE"));
    assert!(first_stderr.contains(migration_id));
    assert!(first_stderr.contains("ready; resume"));
    assert!(source.exists());
    assert!(!project.path().join(".megara/planning").exists());
    assert!(project
        .path()
        .join(".megara/migration-backups")
        .join(migration_id)
        .join("manifest.json")
        .exists());
    let backup_entries = fs::read_dir(project.path().join(".megara/migration-backups"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(backup_entries, vec![migration_id]);
    assert!(!project
        .path()
        .join(".megara/migration-backups/.staging")
        .exists());

    let resumed = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--resume", migration_id, "--json"],
    ));
    assert_eq!(resumed["phase"], "applied");
    assert_eq!(resumed["revision"], 1);
    assert!(!source.exists());
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let session_id = resumed["session_id"].as_str().unwrap();
    assert_eq!(store.list(None).unwrap().len(), 1);
    assert_eq!(store.event_count(session_id).unwrap(), 1);
    assert_eq!(store.current(session_id).unwrap().revision, 1);
    assert!(store
        .command_result_json(&import_command_id)
        .unwrap()
        .is_some());
    drop(store);
    let replay = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--resume", migration_id, "--json"],
    ));
    assert_eq!(replay["phase"], "applied");
    assert_eq!(replay["session_id"], resumed["session_id"]);
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(store.list(None).unwrap().len(), 1);
    assert_eq!(store.event_count(session_id).unwrap(), 1);
    assert!(store
        .command_result_json(&import_command_id)
        .unwrap()
        .is_some());
}

#[cfg(unix)]
#[test]
fn post_quarantine_validation_failure_restores_owned_staging() {
    use std::{
        io::{self, Write},
        os::fd::{AsRawFd, FromRawFd},
        os::unix::fs::MetadataExt,
    };

    let project = tempdir().unwrap();
    let migration_id = "mig_post-validate";
    let staging_path = project
        .path()
        .join(".megara/migration-backups/.staging")
        .join(migration_id);
    fs::create_dir_all(staging_path.join("files")).unwrap();
    support::write_owned_staging_marker(project.path(), migration_id, &[]);
    let project_id = super::planning::store::canonical_project_identity(project.path())
        .unwrap()
        .project_id;
    let marker = staging_path.join("staging.json");
    let before = fs::metadata(&marker).unwrap();
    let before_identity = (before.dev(), before.ino(), before.mode());
    let outside = project.path().join("outside-sentinel");
    fs::write(&outside, b"outside").unwrap();
    let namespace = fs::File::open(staging_path.parent().unwrap()).unwrap();
    let entry = fs::File::open(&staging_path).unwrap();
    let entry_name = std::ffi::CString::new(migration_id).unwrap();

    let error = super::planning::migration::remove_tree_at_validated(
        &namespace,
        &entry_name,
        &entry,
        |held| {
            let sentinel = std::ffi::CString::new("inserted-after-validation").unwrap();
            let fd = unsafe {
                libc::openat(
                    held.as_raw_fd(),
                    sentinel.as_ptr(),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let mut file = unsafe { fs::File::from_raw_fd(fd) };
            file.write_all(b"sentinel")?;
            file.sync_all()?;
            super::planning::migration::validate_staging_held(
                project.path(),
                &staging_path,
                migration_id,
                &project_id,
                held,
            )
            .map(|_| ())
            .map_err(|error| io::Error::other(format!("MIGRATION_INCOMPLETE: {error}")))
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("unexpected staging entry"));
    let after = fs::metadata(&marker).unwrap();
    assert_eq!((after.dev(), after.ino(), after.mode()), before_identity);
    assert_eq!(fs::read(&outside).unwrap(), b"outside");
    assert_eq!(
        fs::read(staging_path.join("inserted-after-validation")).unwrap(),
        b"sentinel"
    );
    assert!(fs::read_dir(staging_path.parent().unwrap())
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("megara-tmp-")));
}

#[cfg(unix)]
#[test]
fn held_root_replacement_fails_closed_without_deleting_replacement() {
    use std::os::unix::fs::MetadataExt;

    let project = tempdir().unwrap();
    let root_path = project.path().join("owned");
    let saved_path = project.path().join("owned-original");
    fs::create_dir(&root_path).unwrap();
    fs::write(root_path.join("original"), b"original").unwrap();
    let parent = fs::File::open(project.path()).unwrap();
    let held = fs::File::open(&root_path).unwrap();
    fs::rename(&root_path, &saved_path).unwrap();
    fs::create_dir(&root_path).unwrap();
    fs::write(root_path.join("replacement"), b"replacement").unwrap();
    let replacement = fs::metadata(root_path.join("replacement")).unwrap();
    let name = std::ffi::CString::new("owned").unwrap();

    let error =
        super::planning::migration::remove_tree_at_validated(&parent, &name, &held, |_| Ok(()))
            .unwrap_err();
    assert!(error.to_string().contains("changed during preflight"));
    let replacement_after = fs::metadata(root_path.join("replacement")).unwrap();
    assert_eq!(
        (
            replacement_after.dev(),
            replacement_after.ino(),
            replacement_after.mode()
        ),
        (replacement.dev(), replacement.ino(), replacement.mode())
    );
    assert_eq!(
        fs::read(root_path.join("replacement")).unwrap(),
        b"replacement"
    );
    assert_eq!(fs::read(saved_path.join("original")).unwrap(), b"original");
    assert!(fs::read_dir(project.path()).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("megara-tmp-")));
}
