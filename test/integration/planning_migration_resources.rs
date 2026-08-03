use std::fs;

use tempfile::tempdir;

use super::planning_migration_support as support;

fn legacy_root(project: &std::path::Path) -> std::path::PathBuf {
    project.join(".megara/state/workflows")
}

fn assert_rejected_without_delta(
    project: &std::path::Path,
    codex_home: &std::path::Path,
    expected_error: &str,
) {
    let before = support::tree_snapshot(project);
    let output = support::run(project, codex_home, &["--apply", "--json"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_error),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(support::tree_snapshot(project), before);
    assert!(!project.join(".megara/planning").exists());
    assert!(!project.join(".megara/migration-backups").exists());
}

#[cfg(unix)]
#[test]
fn exact_legacy_context_byte_limit_is_accepted() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let bytes = vec![0xff; super::planning::engine::LEGACY_CONTEXT_MAX_BYTES];
    let path = legacy_root(project.path()).join("exact.bin");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, &bytes).unwrap();

    let output = support::run(project.path(), codex_home.path(), &["--apply", "--json"]);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = support::report(&output);
    assert_eq!(report["phase"], "applied");
    assert!(!path.exists());
}

#[test]
fn one_byte_over_legacy_context_limit_is_zero_delta() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let bytes = vec![0x41; super::planning::engine::LEGACY_CONTEXT_MAX_BYTES + 1];
    let path = legacy_root(project.path()).join("too-large.bin");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    assert_rejected_without_delta(
        project.path(),
        codex_home.path(),
        "legacy inventory file exceeds decoded byte limit",
    );
}

#[test]
fn exact_legacy_inventory_count_is_accepted() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let root = legacy_root(project.path());
    fs::create_dir_all(&root).unwrap();
    for index in 0..super::planning::engine::LEGACY_MAX_FILES {
        fs::write(root.join(format!("file-{index}.json")), b"x").unwrap();
    }
    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    assert_eq!(report["phase"], "applied");
}

#[test]
fn one_file_over_legacy_inventory_count_is_zero_delta() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let root = legacy_root(project.path());
    fs::create_dir_all(&root).unwrap();
    for index in 0..=super::planning::engine::LEGACY_MAX_FILES {
        fs::write(root.join(format!("file-{index}.json")), b"x").unwrap();
    }
    assert_rejected_without_delta(
        project.path(),
        codex_home.path(),
        "legacy inventory contains too many files",
    );
}

#[test]
fn exact_path_byte_limit_is_accepted_and_plus_one_is_rejected() {
    let exact = "a".repeat(super::planning::engine::LEGACY_MAX_PATH_BYTES);
    assert!(super::planning::migration::validate_candidate_path(&exact).is_ok());
    let plus_one = format!("{}b", exact);
    assert!(super::planning::migration::validate_candidate_path(&plus_one).is_err());
}

#[test]
fn exact_legacy_path_depth_is_accepted() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let mut current = legacy_root(project.path());
    for index in 0..60 {
        current = current.join(format!("d{index}"));
    }
    let path = current.join("legacy.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"path-at-depth-limit").unwrap();
    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    assert_eq!(report["phase"], "applied");
}

#[test]
fn exact_visited_entry_budget_is_accepted() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let root = legacy_root(project.path());
    fs::create_dir_all(&root).unwrap();
    for index in 0..super::planning::migration::inventory::MAX_VISITED_ENTRIES {
        fs::create_dir(root.join(format!("directory-{index}"))).unwrap();
    }
    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    assert_eq!(report["migration_id"], "noop");
}

#[test]
fn one_visited_entry_over_budget_is_zero_delta() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let root = legacy_root(project.path());
    fs::create_dir_all(&root).unwrap();
    for index in 0..=super::planning::migration::inventory::MAX_VISITED_ENTRIES {
        fs::create_dir(root.join(format!("directory-{index}"))).unwrap();
    }
    assert_rejected_without_delta(
        project.path(),
        codex_home.path(),
        "legacy inventory visited-entry budget exceeded",
    );
}

#[cfg(unix)]
#[test]
fn exact_warning_budget_is_accepted() {
    use std::os::unix::fs::symlink;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let root = legacy_root(project.path());
    let target = project.path().join("outside-target");
    fs::create_dir_all(&root).unwrap();
    fs::write(&target, b"outside").unwrap();
    for index in 0..super::planning::migration::inventory::MAX_WARNINGS {
        symlink(&target, root.join(format!("link-{index}"))).unwrap();
    }
    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    assert_eq!(report["migration_id"], "noop");
    assert_eq!(fs::read(&target).unwrap(), b"outside");
}

#[cfg(unix)]
#[test]
fn one_warning_over_budget_is_zero_delta() {
    use std::os::unix::fs::symlink;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let root = legacy_root(project.path());
    let target = project.path().join("outside-target");
    fs::create_dir_all(&root).unwrap();
    fs::write(&target, b"outside").unwrap();
    for index in 0..=super::planning::migration::inventory::MAX_WARNINGS {
        symlink(&target, root.join(format!("link-{index}"))).unwrap();
    }
    assert_rejected_without_delta(
        project.path(),
        codex_home.path(),
        "legacy inventory warning budget exceeded",
    );
    assert_eq!(fs::read(&target).unwrap(), b"outside");
}

#[test]
fn overdeep_legacy_path_is_zero_delta_before_read_or_stage() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let mut current = legacy_root(project.path());
    for index in 0..61 {
        current = current.join(format!("d{index}"));
    }
    let path = current.join("legacy.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"path-too-deep").unwrap();
    assert_rejected_without_delta(
        project.path(),
        codex_home.path(),
        "legacy inventory path is too large or deep",
    );
}
