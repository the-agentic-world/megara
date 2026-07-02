use super::*;
use tempfile::tempdir;

#[test]
fn protects_unmanaged_files_without_force() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    fs::write(&path, "manual").unwrap();

    let file = PlannedFile::new(path.clone(), "generated");
    let summary = write_files(&[file], true, false).unwrap();

    assert_eq!(summary.conflicts, vec![path]);
}

#[test]
fn conflicts_do_not_partially_write() {
    let dir = tempdir().unwrap();
    let conflict = dir.path().join("AGENTS.md");
    let created = dir.path().join("new.md");
    fs::write(&conflict, "manual").unwrap();

    let files = vec![
        PlannedFile::new(conflict, "generated"),
        PlannedFile::new(created.clone(), "generated"),
    ];
    let err = write_files(&files, false, false).unwrap_err();

    assert!(err.to_string().contains("refusing to overwrite"));
    assert!(!created.exists());
}

#[test]
fn force_updates_unmanaged_files() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    fs::write(&path, "manual").unwrap();

    let file = PlannedFile::new(path.clone(), "generated");
    let summary = write_files(&[file], false, true).unwrap();

    assert_eq!(summary.updated, vec![path.clone()]);
    assert!(fs::read_to_string(path).unwrap().contains(MANAGED_MARKER));
}

#[cfg(unix)]
#[test]
fn writes_executable_shell_files() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("bin/megara");
    let file = PlannedFile::new_executable_shell(path.clone(), "#!/bin/sh\nexit 0\n");

    let summary = write_files(&[file], false, false).unwrap();

    assert_eq!(summary.created, vec![path.clone()]);
    let mode = fs::metadata(path).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0);
}
