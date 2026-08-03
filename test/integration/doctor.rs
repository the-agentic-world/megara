use super::*;

#[test]
fn doctor_reports_missing_then_ok() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let missing = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(missing.status.success());
    let missing_stdout = String::from_utf8_lossy(&missing.stdout);
    assert!(missing_stdout.contains("\"ok\": false"));

    let install = megara_with_codex_home(codex_home.path())
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let agents_md = dir.path().join(".codex/AGENTS.md");
    fs::write(&agents_md, "# MEGARA:MANAGED\nstale").unwrap();

    let stale = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(stale.status.success());
    let stale_stdout = String::from_utf8_lossy(&stale.stdout);
    assert!(stale_stdout.contains("\"ok\": false"));
    assert!(stale_stdout.contains(".codex/AGENTS.md"));

    let sync = megara_with_codex_home(codex_home.path())
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(sync.status.success());

    let ok = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(ok.status.success());
    let ok_stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(ok_stdout.contains("\"ok\": true"));
    assert!(ok_stdout.contains("\"warnings\": []"));

    let human = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(human.status.success());
    let human_stdout = String::from_utf8_lossy(&human.stdout);
    assert!(human_stdout.contains("Megara / Doctor"));
    assert!(human_stdout.contains("megara doctor: scope=project, target=codex, ok=true"));
}

#[test]
fn doctor_reports_broken_project_wrapper() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    fs::write(dir.path().join(".agents/bin/megara"), "not executable").unwrap();

    let output = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": false"));
    assert!(stdout.contains(".agents/bin/megara"));
}

#[cfg(unix)]
#[test]
fn doctor_repair_retries_pending_planning_purge_cleanup() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    super::planning_migration_support::install(dir.path(), codex_home.path());
    super::planning_migration_support::write_legacy_file(dir.path(), b"pending-cleanup");
    let applied =
        super::planning_migration_support::report(&super::planning_migration_support::run(
            dir.path(),
            codex_home.path(),
            &["--apply", "--json"],
        ));
    let migration_id = applied["migration_id"].as_str().unwrap();
    let session_id = applied["session_id"].as_str().unwrap();
    let backup_root = dir
        .path()
        .join(format!(".megara/migration-backups/{migration_id}"));
    let held_root = dir
        .path()
        .join(format!(".megara/migration-backups/{migration_id}-held"));
    fs::rename(&backup_root, &held_root).unwrap();
    symlink(&held_root, &backup_root).unwrap();

    let mut store = super::planning::store::PlanningStore::open_project(dir.path()).unwrap();
    let receipt = store
        .purge(
            session_id,
            "cmd-doctor-pending-cleanup",
            "sha256:doctor-pending-cleanup",
            1,
            session_id,
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "pending");
    drop(store);

    let read_only = megara()
        .args([
            "doctor", "--scope", "project", "--target", "codex", "--json",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(read_only.status.success());
    let read_only_stdout = String::from_utf8_lossy(&read_only.stdout);
    assert!(read_only_stdout.contains("\"ok\": false"));
    assert!(read_only_stdout.contains("pending Planning purge cleanup"));
    assert!(backup_root
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());

    fs::remove_file(&backup_root).unwrap();
    fs::rename(&held_root, &backup_root).unwrap();

    let repaired = megara()
        .args([
            "doctor", "--scope", "project", "--target", "codex", "--repair", "--json",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        repaired.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&repaired.stdout),
        String::from_utf8_lossy(&repaired.stderr)
    );
    let repaired_stdout = String::from_utf8_lossy(&repaired.stdout);
    assert!(repaired_stdout.contains("\"warnings\": []"));
    assert!(!repaired_stdout.contains("pending Planning purge cleanup"));
    assert!(repaired_stdout.contains("repaired=1, pending=0"));
    assert!(!backup_root.exists());
    let store = super::planning::store::PlanningStore::open_project(dir.path()).unwrap();
    assert_eq!(store.pending_cleanup_count().unwrap(), 0);
}
