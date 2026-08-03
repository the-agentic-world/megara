use std::{fs, path::Path};

use tempfile::tempdir;

use super::planning_migration_support as support;

fn migration_root(project: &Path, migration_id: &str) -> std::path::PathBuf {
    project.join(".megara/migration-backups").join(migration_id)
}

#[test]
fn rollback_restores_source_missing_even_when_manifest_removed_is_false() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy = support::write_legacy_file(project.path(), b"removed-before-journal");
    let original = fs::read(&legacy).unwrap();
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    assert!(!legacy.exists());
    support::rewrite_manifest(project.path(), &migration_id, |manifest| {
        manifest["phase"] = serde_json::Value::String("applied".to_string());
        manifest["files"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|file| file["relative_path"] == ".megara/state/workflows/legacy.json")
            .unwrap()["removed"] = serde_json::Value::Bool(false);
    });

    let rolled_back = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(rolled_back["phase"], "rolled_back");
    assert_eq!(fs::read(&legacy).unwrap(), original);
}

#[test]
fn rollback_binds_committed_import_when_prepared_manifest_lost_session_ids() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy = support::write_legacy_file(project.path(), b"import-commit-window");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let import_command_id = {
        let manifest = migration_root(project.path(), &migration_id).join("manifest.json");
        serde_json::from_slice::<serde_json::Value>(&fs::read(manifest).unwrap()).unwrap()
            ["import_command_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let manifest_path = migration_root(project.path(), &migration_id).join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let records = manifest["files"].as_array_mut().unwrap();
    for record in records.iter_mut() {
        let relative = record["relative_path"].as_str().unwrap();
        let source = project.path().join(relative);
        let backup = migration_root(project.path(), &migration_id)
            .join("files")
            .join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::copy(&backup, &source).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = record["mode"].as_u64().unwrap() as u32;
            fs::set_permissions(&source, fs::Permissions::from_mode(mode)).unwrap();
        }
        record["removed"] = serde_json::Value::Bool(false);
    }
    let source_before = fs::metadata(&legacy).unwrap();
    let source_bytes_before = fs::read(&legacy).unwrap();
    let store_before = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(store_before.event_count(&session_id).unwrap(), 1);
    drop(store_before);
    manifest["phase"] = serde_json::Value::String("prepared".to_string());
    manifest["session_id"] = serde_json::Value::Null;
    manifest["revision"] = serde_json::Value::Null;
    manifest["manifest_hash"] = serde_json::Value::String(String::new());
    manifest["manifest_hash"] =
        serde_json::Value::String(super::planning::canonical::canonical_hash(&manifest));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let rolled_back = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(rolled_back["phase"], "rolled_back");
    assert_eq!(rolled_back["session_id"], session_id);
    assert_eq!(fs::read(&legacy).unwrap(), source_bytes_before);
    let source_after = fs::metadata(&legacy).unwrap();
    assert_eq!(source_after.len(), source_before.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        assert_eq!(source_after.dev(), source_before.dev());
        assert_eq!(source_after.ino(), source_before.ino());
        assert_eq!(
            source_after.permissions().mode(),
            source_before.permissions().mode()
        );
        assert_eq!(
            source_after.modified().unwrap(),
            source_before.modified().unwrap()
        );
    }
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert!(matches!(
        store.current(&session_id),
        Err(super::planning::store::StoreError::SessionPurged(_))
    ));
    assert!(store.list(None).unwrap().is_empty());
    assert!(store
        .command_result_json(&import_command_id)
        .unwrap()
        .is_none());
    let retired: i64 = store
        .conn
        .query_row(
            "SELECT COUNT(*) FROM purged_command_ids WHERE command_id=?1",
            [&import_command_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retired, 1);
    drop(store);

    let retry = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(retry["phase"], "rolled_back");
    assert_eq!(retry["session_id"], session_id);
}

#[test]
fn rollback_prepared_without_import_receipt_does_not_create_database() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy = support::write_legacy_file(project.path(), b"pre-import-window");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let manifest_path = migration_root(project.path(), &migration_id).join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    for record in manifest["files"].as_array_mut().unwrap() {
        let relative = record["relative_path"].as_str().unwrap();
        let source = project.path().join(relative);
        let backup = migration_root(project.path(), &migration_id)
            .join("files")
            .join(relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::copy(&backup, &source).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &source,
                fs::Permissions::from_mode(record["mode"].as_u64().unwrap() as u32),
            )
            .unwrap();
        }
        record["removed"] = serde_json::Value::Bool(false);
    }
    manifest["phase"] = serde_json::Value::String("prepared".to_string());
    manifest["session_id"] = serde_json::Value::Null;
    manifest["revision"] = serde_json::Value::Null;
    manifest["manifest_hash"] = serde_json::Value::String(String::new());
    manifest["manifest_hash"] =
        serde_json::Value::String(super::planning::canonical::canonical_hash(&manifest));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let planning_dir = project.path().join(".megara/planning");
    fs::remove_dir_all(&planning_dir).unwrap();
    let db_paths = [
        planning_dir.join("planning.db"),
        planning_dir.join("planning.db-wal"),
        planning_dir.join("planning.db-shm"),
    ];
    assert!(db_paths.iter().all(|path| !path.exists()));

    let rolled_back = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(rolled_back["phase"], "rolled_back");
    assert_eq!(rolled_back["session_id"], serde_json::Value::Null);
    assert!(!planning_dir.exists());
    assert!(db_paths.iter().all(|path| !path.exists()));
    assert_eq!(fs::read(&legacy).unwrap(), b"pre-import-window");
}

#[cfg(unix)]
#[test]
fn rollback_preserves_empty_parent_directory_mode() {
    use std::os::unix::fs::PermissionsExt;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy = support::write_legacy_file(project.path(), b"mode-preserved");
    let parent = legacy.parent().unwrap().to_path_buf();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o751)).unwrap();

    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap();
    assert!(parent.is_dir());
    assert_eq!(
        fs::metadata(&parent).unwrap().permissions().mode() & 0o777,
        0o751
    );
    assert!(!legacy.exists());

    support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", migration_id, "--json"],
    ));
    assert_eq!(fs::read(&legacy).unwrap(), b"mode-preserved");
    assert_eq!(
        fs::metadata(parent).unwrap().permissions().mode() & 0o777,
        0o751
    );
}

#[test]
fn rollback_equal_file_is_noop_and_partial_restore_retry_is_safe() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let first = support::write_legacy_file(project.path(), b"first");
    let second = project.path().join(".megara/state/team/second.json");
    fs::create_dir_all(second.parent().unwrap()).unwrap();
    fs::write(&second, b"second").unwrap();
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap();
    fs::write(&first, b"first").unwrap();
    let first_before = fs::metadata(&first).unwrap();

    support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", migration_id, "--json"],
    ));
    assert_eq!(fs::read(&first).unwrap(), b"first");
    let first_after = fs::metadata(&first).unwrap();
    assert_eq!(first_after.len(), first_before.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        assert_eq!(first_after.dev(), first_before.dev());
        assert_eq!(first_after.ino(), first_before.ino());
        assert_eq!(
            first_after.permissions().mode(),
            first_before.permissions().mode()
        );
        assert_eq!(
            first_after.modified().unwrap(),
            first_before.modified().unwrap()
        );
    }
    assert_eq!(fs::read(&second).unwrap(), b"second");
}

#[cfg(unix)]
#[test]
fn rollback_rejects_parent_and_final_symlinks_without_mutation() {
    use std::os::unix::fs::symlink;

    for final_node in [false, true] {
        let project = tempdir().unwrap();
        let codex_home = tempdir().unwrap();
        let legacy = support::write_legacy_file(project.path(), b"symlink-protected");
        let applied = support::report(&support::run(
            project.path(),
            codex_home.path(),
            &["--apply", "--json"],
        ));
        let migration_id = applied["migration_id"].as_str().unwrap().to_string();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("outside.json");
        fs::write(&outside_file, b"outside").unwrap();
        if final_node {
            symlink(&outside_file, &legacy).unwrap();
        } else {
            let parent = legacy.parent().unwrap().to_path_buf();
            fs::remove_dir(&parent).unwrap();
            symlink(outside.path(), &parent).unwrap();
        }

        let failed = support::run(
            project.path(),
            codex_home.path(),
            &["--rollback", &migration_id, "--json"],
        );
        assert!(!failed.status.success(), "final_node={final_node}");
        assert_eq!(fs::read(&outside_file).unwrap(), b"outside");
        assert!(migration_root(project.path(), &migration_id)
            .join("manifest.json")
            .exists());
        assert!(migration_root(project.path(), &migration_id)
            .join("files")
            .exists());
    }
}

#[test]
fn migration_backup_root_keeps_terminal_manifest_after_files_cleanup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::write_legacy_file(project.path(), b"journal");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap();
    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", migration_id, "--json"],
    ));
    assert_eq!(report["phase"], "rolled_back");
    let root = migration_root(project.path(), migration_id);
    assert!(root.join("manifest.json").is_file());
    assert!(!root.join("files").exists());
}
