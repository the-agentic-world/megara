use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::tempdir;

use super::planning_migration_support as support;

fn migration_root(project: &Path, migration_id: &str) -> PathBuf {
    project.join(".megara/migration-backups").join(migration_id)
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn seed_staging_temp(
    project: &Path,
    migration_id: &str,
    expected: &[u8],
    actual: &[u8],
    mode: u32,
) -> PathBuf {
    let staging_files = project
        .join(".megara/migration-backups/.staging")
        .join(migration_id)
        .join("files");
    fs::create_dir_all(&staging_files).unwrap();
    let temp = staging_files.join(format!(".legacy.json.megara-tmp-{}", uuid::Uuid::now_v7()));
    fs::write(&temp, actual).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(unix)]
    let stored_mode = {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(&temp).unwrap().permissions().mode()
    };
    #[cfg(not(unix))]
    let stored_mode = mode;
    support::write_owned_staging_marker(
        project,
        migration_id,
        &[serde_json::json!({
            "relative_path": "legacy.json",
            "sha256": sha256(expected),
            "size": expected.len(),
            "mode": stored_mode,
            "kind": "opaque"
        })],
    );
    temp
}

#[test]
fn final_orphan_is_preserved_and_blocks_apply_without_manual_cleanup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let orphan = project
        .path()
        .join(".megara/migration-backups/mig_orphan/files");
    fs::create_dir_all(&orphan).unwrap();
    let legacy = support::write_legacy_file(project.path(), b"orphan");
    let failed = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("mig_orphan"));
    assert!(legacy.exists());
    assert!(orphan.parent().unwrap().exists());
    let retry = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(!retry.status.success());
    assert!(String::from_utf8_lossy(&retry.stderr).contains("mig_orphan"));
}

#[test]
fn staging_recovery_and_prepared_projection_resume_are_idempotent() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let staging = project
        .path()
        .join(".megara/migration-backups/.staging/mig_staged/files");
    fs::create_dir_all(&staging).unwrap();
    support::write_owned_staging_marker(project.path(), "mig_staged", &[]);
    let legacy = support::write_legacy_file(project.path(), b"staging-recovery");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    assert_eq!(applied["phase"], "applied");
    assert!(!legacy.exists());
    assert!(!project
        .path()
        .join(".megara/migration-backups/.staging/mig_staged")
        .exists());

    let unknown = project
        .path()
        .join(".megara/migration-backups/not-a-migration/files");
    fs::create_dir_all(&unknown).unwrap();
    let second_legacy = support::write_legacy_file(project.path(), b"unknown-entry");
    let failed = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(!failed.status.success());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(stderr.contains("MIGRATION_INCOMPLETE"));
    assert!(stderr.contains("not-a-migration"));
    assert!(second_legacy.exists());

    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let import_command_id = {
        let manifest_path = migration_root(project.path(), &migration_id).join("manifest.json");
        serde_json::from_slice::<serde_json::Value>(&fs::read(manifest_path).unwrap()).unwrap()
            ["import_command_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    support::rewrite_manifest(project.path(), &migration_id, |manifest| {
        manifest["phase"] = serde_json::Value::String("planning_imported".to_string());
    });
    let blocked = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("MIGRATION_INCOMPLETE"));

    support::rewrite_manifest(project.path(), &migration_id, |manifest| {
        manifest["phase"] = serde_json::Value::String("prepared".to_string());
        manifest["session_id"] = serde_json::Value::Null;
        manifest["revision"] = serde_json::Value::Null;
    });
    let resumed = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--resume", &migration_id, "--json"],
    ));
    assert_eq!(resumed["phase"], "applied");
    assert!(resumed["session_id"].is_string());
    assert_eq!(resumed["revision"], 1);
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(resumed["session_id"], session_id);
    assert_eq!(store.event_count(&session_id).unwrap(), 1);
    assert_eq!(store.current(&session_id).unwrap().revision, 1);
    assert!(store
        .command_result_json(&import_command_id)
        .unwrap()
        .is_some());
    drop(store);

    support::rewrite_manifest(project.path(), &migration_id, |manifest| {
        manifest["phase"] = serde_json::Value::String("projection_removed".to_string());
    });
    let resumed_projection = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--resume", &migration_id, "--json"],
    ));
    assert_eq!(resumed_projection["phase"], "applied");
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), 1);
    assert_eq!(store.current(&session_id).unwrap().revision, 1);
    assert!(store
        .command_result_json(&import_command_id)
        .unwrap()
        .is_some());
}

#[test]
fn staging_temp_cleanup_requires_matching_bytes_size_and_mode() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::write_legacy_file(project.path(), b"real-legacy");
    let valid_temp = seed_staging_temp(
        project.path(),
        "mig_valid-temp",
        b"valid-temp",
        b"valid-temp",
        0o600,
    );
    assert!(valid_temp.parent().unwrap().is_dir());
    assert!(valid_temp
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .is_dir());
    let applied = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(!valid_temp.exists());
    assert!(!project
        .path()
        .join(".megara/migration-backups/.staging")
        .exists());

    for (migration_id, expected, actual) in [
        (
            "mig_wrong-temp-bytes",
            b"expected".as_slice(),
            b"wrong".as_slice(),
        ),
        (
            "mig_oversized-temp",
            b"short".as_slice(),
            b"too-large".as_slice(),
        ),
    ] {
        let project = tempdir().unwrap();
        let codex_home = tempdir().unwrap();
        support::write_legacy_file(project.path(), b"real-legacy");
        let temp = seed_staging_temp(project.path(), migration_id, expected, actual, 0o600);
        let failed = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
        assert!(!failed.status.success(), "{migration_id}");
        let stderr = String::from_utf8_lossy(&failed.stderr);
        assert!(stderr.contains("MIGRATION_INCOMPLETE"));
        if migration_id.contains("wrong-temp-bytes") {
            assert!(stderr.contains("staging backup temp digest mismatch"));
        } else {
            assert!(stderr.contains("exceeds bounded read limit"));
        }
        assert!(temp.exists(), "{migration_id}");
        assert!(project
            .path()
            .join(".megara/migration-backups/.staging")
            .join(migration_id)
            .exists());
    }
}

#[cfg(unix)]
#[test]
fn staging_temp_mode_mismatch_is_preserved() {
    use std::os::unix::fs::PermissionsExt;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::write_legacy_file(project.path(), b"mode-legacy");
    let temp = seed_staging_temp(
        project.path(),
        "mig_wrong-temp-mode",
        b"same-bytes",
        b"same-bytes",
        0o600,
    );
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o644)).unwrap();
    let failed = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(!failed.status.success());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(stderr.contains("MIGRATION_INCOMPLETE"));
    assert!(stderr.contains("staging backup temp mode mismatch"));
    assert_eq!(
        fs::metadata(&temp).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert!(temp.exists());
}

#[cfg(unix)]
#[test]
fn linked_backup_symlink_is_removed_without_following_outside() {
    use std::os::unix::fs::symlink;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_file = outside.path().join("outside.txt");
    fs::write(&outside_file, b"outside").unwrap();
    support::write_legacy_file(project.path(), b"linked-symlink");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let backup_link = migration_root(project.path(), &migration_id).join("files/link");
    symlink(&outside_file, &backup_link).unwrap();

    let mut store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let receipt = store
        .purge(
            &session_id,
            "cmd-purge-backup-symlink",
            "sha256:purge-backup-symlink",
            1,
            &session_id,
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "clean");
    assert_eq!(fs::read(&outside_file).unwrap(), b"outside");
    assert!(!backup_link.exists());
    assert!(!migration_root(project.path(), &migration_id).exists());
}

#[cfg(unix)]
#[test]
fn linked_backup_root_replacement_fails_closed_without_following_outside() {
    use std::os::unix::fs::symlink;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_file = outside.path().join("sentinel");
    fs::write(&outside_file, b"outside-root").unwrap();
    support::write_legacy_file(project.path(), b"root-replacement");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let backup_root = migration_root(project.path(), &migration_id);
    let held_root = project
        .path()
        .join(".megara/migration-backups/mig-held-root");
    fs::rename(&backup_root, &held_root).unwrap();
    symlink(outside.path(), &backup_root).unwrap();

    let mut store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let receipt = store
        .purge(
            &session_id,
            "cmd-purge-root-replacement",
            "sha256:purge-root-replacement",
            1,
            &session_id,
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "pending");
    assert_eq!(fs::read(&outside_file).unwrap(), b"outside-root");
    assert!(backup_root
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(held_root.exists());
    drop(store);

    fs::remove_file(&backup_root).unwrap();
    fs::rename(&held_root, &backup_root).unwrap();
    let mut store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(store.repair_pending_cleanup().unwrap(), 1);
    assert!(!backup_root.exists());
    assert_eq!(fs::read(&outside_file).unwrap(), b"outside-root");
}
