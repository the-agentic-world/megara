use std::fs;
use std::path::Path;
use std::process::Command;

use crate::planning::domain::EvidenceRange;
use crate::planning::evidence::{
    capture_snapshot, snapshot_is_current, EvidenceCitation, EvidenceError,
};
use tempfile::TempDir;

fn citation(path: &str, ranges: Vec<EvidenceRange>) -> EvidenceCitation {
    EvidenceCitation {
        temp_ref: path.replace('/', "_"),
        path: path.to_string(),
        ranges,
        claim: "구조를 확인한다.".to_string(),
    }
}

fn git_project() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    Command::new("git")
        .args(["init", "-q", "--initial-branch", "main"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.invalid"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Evidence Test"])
        .current_dir(root)
        .status()
        .unwrap();
    directory
}

fn commit(root: &Path) {
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-q", "-m", "fixture"])
        .current_dir(root)
        .status()
        .unwrap();
}

#[test]
fn path_protection_covers_traversal_symlink_casefold_and_secret_extensions() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("safe.rs"), "safe").unwrap();
    for path in [
        "../outside.rs",
        ".env",
        ".env.sample",
        "nested/SECRET.txt",
        "private.KEY",
        "certificate.PEM",
    ] {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if !path.starts_with("..") {
            let _ = fs::write(&target, "secret");
        }
        assert!(matches!(
            capture_snapshot(root, &[citation(path, Vec::new())]),
            Err(EvidenceError::PathOutsideRoot(_))
                | Err(EvidenceError::SensitivePath(_))
                | Err(EvidenceError::MissingFile(_))
        ));
    }
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("escape.rs"), "escape").unwrap();
        std::os::unix::fs::symlink(outside.path().join("escape.rs"), root.join("link.rs")).unwrap();
        assert!(matches!(
            capture_snapshot(root, &[citation("link.rs", Vec::new())]),
            Err(EvidenceError::PathOutsideRoot(_))
        ));

        fs::write(root.join("safe-target.rs"), "safe").unwrap();
        let _ = fs::remove_file(root.join(".env"));
        std::os::unix::fs::symlink(root.join("safe-target.rs"), root.join(".env")).unwrap();
        assert!(matches!(
            capture_snapshot(root, &[citation(".env", Vec::new())]),
            Err(EvidenceError::SensitivePath(_))
        ));

        fs::write(root.join("secret-target.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(
            root.join("secret-target.txt"),
            root.join("safe-target-link.rs"),
        )
        .unwrap();
        assert!(matches!(
            capture_snapshot(root, &[citation("safe-target-link.rs", Vec::new())]),
            Err(EvidenceError::SensitivePath(_))
        ));

        std::os::unix::fs::symlink(root.join("safe-target.rs"), root.join("safe-link.rs")).unwrap();
        assert!(capture_snapshot(root, &[citation("safe-link.rs", Vec::new())]).is_ok());
    }
}

#[test]
fn path_matrix_rejects_ignored_git_and_secret_variants_but_allows_tracked_env_example() {
    let directory = git_project();
    let root = directory.path();
    fs::write(root.join(".env.example"), "EXAMPLE=1\n").unwrap();
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join("ignored.txt"), "ignored\n").unwrap();
    fs::create_dir_all(root.join(".megara")).unwrap();
    fs::write(root.join(".megara/state"), "internal\n").unwrap();
    commit(root);

    assert!(capture_snapshot(root, &[citation(".env.example", Vec::new())]).is_ok());
    assert!(matches!(
        capture_snapshot(root, &[citation(".megara/state", Vec::new())]),
        Err(EvidenceError::SensitivePath(_))
    ));
    assert!(matches!(
        capture_snapshot(root, &[citation(".git/config", Vec::new())]),
        Err(EvidenceError::SensitivePath(_))
    ));
    assert!(matches!(
        capture_snapshot(root, &[citation("ignored.txt", Vec::new())]),
        Err(EvidenceError::IgnoredPath(_))
    ));

    for name in [
        ".ENV",
        ".ENV.SAMPLE",
        ".ENV.TEMPLATE",
        "mySECRET.txt",
        "myCredential.txt",
        "myPASSWORD.txt",
        "myPASSWD.txt",
        "myTOKEN.txt",
        "myAPI_KEY.txt",
        "myAPIKEY.txt",
        "certificate.PEM",
        "private.KEY",
        "bundle.P12",
        "bundle.PFX",
        "certificate.DER",
    ] {
        fs::write(root.join(name), "secret\n").unwrap();
        assert!(matches!(
            capture_snapshot(root, &[citation(name, Vec::new())]),
            Err(EvidenceError::SensitivePath(_))
        ));
    }

    let non_git = tempfile::tempdir().unwrap();
    fs::write(non_git.path().join(".env.example"), "EXAMPLE=1\n").unwrap();
    assert!(matches!(
        capture_snapshot(non_git.path(), &[citation(".env.example", Vec::new())]),
        Err(EvidenceError::SensitivePath(_))
    ));
    let untracked = git_project();
    fs::write(untracked.path().join(".env.example"), "EXAMPLE=1\n").unwrap();
    assert!(matches!(
        capture_snapshot(untracked.path(), &[citation(".env.example", Vec::new())]),
        Err(EvidenceError::SensitivePath(_))
    ));

    #[cfg(unix)]
    {
        let symlink_project = git_project();
        fs::write(symlink_project.path().join("safe.txt"), "safe\n").unwrap();
        commit(symlink_project.path());
        std::os::unix::fs::symlink(
            symlink_project.path().join("safe.txt"),
            symlink_project.path().join(".env.example"),
        )
        .unwrap();
        assert!(matches!(
            capture_snapshot(
                symlink_project.path(),
                &[citation(".env.example", Vec::new())]
            ),
            Err(EvidenceError::SensitivePath(_))
        ));
    }
}

#[cfg(unix)]
#[test]
fn safe_symlink_into_ignored_target_is_rejected() {
    let directory = git_project();
    let root = directory.path();
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join("safe.rs"), "safe\n").unwrap();
    commit(root);
    fs::write(root.join("ignored.txt"), "ignored\n").unwrap();
    std::os::unix::fs::symlink(root.join("ignored.txt"), root.join("safe-link.rs")).unwrap();
    assert!(matches!(
        capture_snapshot(root, &[citation("safe-link.rs", Vec::new())]),
        Err(EvidenceError::IgnoredPath(_))
    ));
}

#[test]
fn safe_symlink_retarget_and_removal_are_observable_by_freshness() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("target-a.txt"), "same\n").unwrap();
    fs::write(root.join("target-b.txt"), "different\n").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("target-a.txt"), root.join("link.rs")).unwrap();
        let first = capture_snapshot(root, &[citation("link.rs", Vec::new())]).unwrap();
        assert_eq!(first.evidence[0].path, "link.rs");
        assert!(snapshot_is_current(root, &first).unwrap());

        fs::remove_file(root.join("link.rs")).unwrap();
        std::os::unix::fs::symlink(root.join("target-b.txt"), root.join("link.rs")).unwrap();
        assert!(!snapshot_is_current(root, &first).unwrap());

        fs::remove_file(root.join("link.rs")).unwrap();
        std::os::unix::fs::symlink(root.join("target-a.txt"), root.join("link.rs")).unwrap();
        assert!(snapshot_is_current(root, &first).unwrap());

        fs::remove_file(root.join("link.rs")).unwrap();
        assert!(!snapshot_is_current(root, &first).unwrap());
    }
}
