use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use tempfile::tempdir;

use super::planning_migration_support as support;

fn migration_root(project: &Path, migration_id: &str) -> std::path::PathBuf {
    project.join(".megara/migration-backups").join(migration_id)
}

fn apply_child(project: &Path, codex_home: &Path) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_megara"))
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .env("CODEX_HOME", codex_home)
        .args(["planning", "migrate", "--apply", "--json"])
        .current_dir(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn migration_lock_contention_returns_busy_without_filesystem_delta() {
    use std::os::fd::AsRawFd;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy = support::write_legacy_file(project.path(), b"concurrent");
    let before = support::tree_snapshot(project.path());
    let lock_directory = fs::File::open(project.path()).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock_directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let blocked = Command::new(env!("CARGO_BIN_EXE_megara"))
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .env("CODEX_HOME", codex_home.path())
        .args(["planning", "migrate", "--apply", "--json"])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("MIGRATION_BUSY")
            || String::from_utf8_lossy(&blocked.stdout).contains("MIGRATION_BUSY"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&blocked.stdout),
        String::from_utf8_lossy(&blocked.stderr)
    );
    assert_eq!(support::tree_snapshot(project.path()), before);
    assert!(legacy.exists());
    assert!(!project.path().join(".megara/planning/planning.db").exists());
    assert!(!project.path().join(".megara/migration-backups").exists());
    drop(lock_directory);

    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let session_id = applied["session_id"].as_str().unwrap();
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(store.event_count(session_id).unwrap(), 1);
    assert_eq!(store.current(session_id).unwrap().revision, 1);
    assert!(!legacy.exists());
    let migrations = fs::read_dir(project.path().join(".megara/migration-backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("mig_"))
        .count();
    assert_eq!(migrations, 1);
    assert!(!project
        .path()
        .join(".megara/migration-backups/.staging")
        .exists());
}

#[cfg(unix)]
#[test]
fn concurrent_apply_barrier_serializes_to_one_import_and_one_journal() {
    use std::os::fd::AsRawFd;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy = support::write_legacy_file(project.path(), b"barrier-legacy");
    let before = support::tree_snapshot(project.path());
    let lock_directory = fs::File::open(project.path()).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock_directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let mut children = vec![
        apply_child(project.path(), codex_home.path()),
        apply_child(project.path(), codex_home.path()),
    ];
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let both_alive = children
            .iter_mut()
            .all(|child| child.try_wait().unwrap().is_none());
        if both_alive {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "children did not reach lock barrier"
        );
        std::thread::yield_now();
    }
    assert_eq!(support::tree_snapshot(project.path()), before);
    drop(lock_directory);

    let outputs: Vec<Output> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    assert!(outputs.iter().all(|output| output.status.success()));
    let reports: Vec<serde_json::Value> = outputs.iter().map(support::report).collect();
    assert_eq!(
        reports
            .iter()
            .filter(|report| report["migration_id"].as_str().unwrap().starts_with("mig_"))
            .count(),
        1
    );
    assert_eq!(
        reports
            .iter()
            .filter(|report| report["migration_id"] == "noop")
            .count(),
        1
    );
    assert!(!legacy.exists());
    assert!(!project
        .path()
        .join(".megara/migration-backups/.staging")
        .exists());
    let store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let states = store.list(None).unwrap();
    assert_eq!(states.len(), 1);
    let state = &states[0];
    assert_eq!(state.revision, 1);
    assert_eq!(store.event_count(&state.session_id).unwrap(), 1);
    let migrations = fs::read_dir(project.path().join(".megara/migration-backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("mig_"))
        .count();
    assert_eq!(migrations, 1);
}

#[cfg(unix)]
#[test]
fn public_purge_lock_cannot_remove_backup_during_migration_lock() {
    use std::os::fd::AsRawFd;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::write_legacy_file(project.path(), b"purge-lock");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let before = support::tree_snapshot(project.path());
    let lock_directory = fs::File::open(project.path()).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock_directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );

    let blocked = Command::new(env!("CARGO_BIN_EXE_megara"))
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .env("CODEX_HOME", codex_home.path())
        .args([
            "planning",
            "purge",
            "--session",
            &session_id,
            "--expected-revision",
            "1",
            "--confirm",
            &session_id,
            "--command-id",
            "cmd-public-purge-locked",
            "--json",
        ])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("MIGRATION_BUSY")
            || String::from_utf8_lossy(&blocked.stdout).contains("MIGRATION_BUSY"),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&blocked.stdout),
        String::from_utf8_lossy(&blocked.stderr)
    );
    assert_eq!(support::tree_snapshot(project.path()), before);
    assert!(migration_root(project.path(), &migration_id).exists());
    drop(lock_directory);

    let mut store = super::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let receipt = store
        .purge(
            &session_id,
            "cmd-public-purge-after-lock",
            "sha256:public-purge-after-lock",
            1,
            &session_id,
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "clean");
    assert!(!migration_root(project.path(), &migration_id).exists());
}
