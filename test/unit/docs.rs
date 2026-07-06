use super::*;

use tempfile::tempdir;

#[test]
fn okf_check_accepts_valid_concept() {
    let dir = tempdir().unwrap();
    docs::init_bundle(dir.path(), false, false).unwrap();
    fs::write(
        dir.path().join("concept.md"),
        "---\ntype: Concept\ntitle: Test Concept\ndescription: A valid concept.\ntimestamp: 1\ntags: [test]\n---\n\n# Test Concept\n",
    )
    .unwrap();

    let report = docs::check_bundle(dir.path(), false).unwrap();

    assert!(report.ok, "{report:?}");
    assert!(report.errors.is_empty());
}

#[test]
fn okf_check_rejects_missing_frontmatter() {
    let dir = tempdir().unwrap();
    docs::init_bundle(dir.path(), false, false).unwrap();
    fs::write(dir.path().join("concept.md"), "# Test Concept\n").unwrap();

    let report = docs::check_bundle(dir.path(), false).unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("missing YAML frontmatter")));
}

#[test]
fn okf_check_rejects_missing_type() {
    let dir = tempdir().unwrap();
    docs::init_bundle(dir.path(), false, false).unwrap();
    fs::write(
        dir.path().join("concept.md"),
        "---\ntitle: Test Concept\n---\n\n# Test Concept\n",
    )
    .unwrap();

    let report = docs::check_bundle(dir.path(), false).unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("missing required OKF type")));
}

#[test]
fn okf_check_skips_runtime_artifacts_skills_and_harness_source() {
    let dir = tempdir().unwrap();
    docs::init_bundle(dir.path(), false, false).unwrap();
    fs::create_dir_all(dir.path().join(".megara/state")).unwrap();
    fs::create_dir_all(dir.path().join(".agents/skills/example")).unwrap();
    fs::create_dir_all(dir.path().join("harness/agents")).unwrap();
    fs::create_dir_all(dir.path().join("harness/skills/example")).unwrap();
    fs::write(dir.path().join(".megara/state/bad.md"), "# Runtime State\n").unwrap();
    fs::write(
        dir.path().join(".agents/skills/example/SKILL.md"),
        "# Skill\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("harness/skills/example/SKILL.md"),
        "# Product Skill Source\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("harness/megara.toml"),
        "name = \"megara\"\n",
    )
    .unwrap();

    let report = docs::check_bundle(dir.path(), false).unwrap();

    assert!(report.ok, "{report:?}");
    assert!(report.errors.is_empty());
}

#[test]
fn okf_check_does_not_skip_unrelated_harness_directory() {
    let dir = tempdir().unwrap();
    docs::init_bundle(dir.path(), false, false).unwrap();
    fs::create_dir_all(dir.path().join("harness")).unwrap();
    fs::write(dir.path().join("harness/bad.md"), "# User Harness Notes\n").unwrap();

    let report = docs::check_bundle(dir.path(), false).unwrap();

    assert!(!report.ok);
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("harness/bad.md")));
}
