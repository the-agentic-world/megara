use super::*;

#[test]
fn docs_init_and_check_default_root() {
    let dir = tempdir().unwrap();

    let init = megara()
        .arg("docs")
        .arg("init")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(dir.path().join("docs/index.md").exists());
    assert!(dir.path().join("docs/log.md").exists());

    let check = megara()
        .arg("docs")
        .arg("check")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn docs_init_respects_custom_root_and_force() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("custom-docs");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("index.md"), "# Existing\n").unwrap();

    let conflict = megara()
        .arg("docs")
        .arg("init")
        .arg("--root")
        .arg("custom-docs")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!conflict.status.success());

    let forced = megara()
        .arg("docs")
        .arg("init")
        .arg("--root")
        .arg("custom-docs")
        .arg("--force")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        forced.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(fs::read_to_string(root.join("index.md"))
        .unwrap()
        .contains("KnowledgeIndex"));
    assert!(root.join("log.md").exists());
}

#[test]
fn docs_check_fails_invalid_concept() {
    let dir = tempdir().unwrap();
    let init = megara()
        .arg("docs")
        .arg("init")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(init.status.success());
    fs::write(dir.path().join("docs/bad.md"), "# Bad Concept\n").unwrap();

    let check = megara()
        .arg("docs")
        .arg("check")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stdout).contains("missing YAML frontmatter"));
}

#[test]
fn docs_check_skips_runtime_artifacts_skills_and_harness_source() {
    let dir = tempdir().unwrap();
    let init = megara()
        .arg("docs")
        .arg("init")
        .arg("--root")
        .arg(".")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(init.status.success());
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

    let check = megara()
        .arg("docs")
        .arg("check")
        .arg("--root")
        .arg(".")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        check.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&check.stderr)
    );
}
