use super::*;

#[test]
fn uninstall_removes_managed_project_harness_and_preserves_runtime_data() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let runtime_state = dir.path().join(".megara/state/recovery.json");
    fs::create_dir_all(runtime_state.parent().unwrap()).unwrap();
    fs::write(&runtime_state, "{}\n").unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .args(["uninstall", "--scope", "project", "--target", "codex"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join(".codex/AGENTS.md").exists());
    assert!(!dir.path().join(".codex/hooks.json").exists());
    assert!(!dir.path().join(".agents/megara.toml").exists());
    assert!(!dir.path().join(".agents/bin/megara").exists());
    assert!(runtime_state.exists());
    let codex_config = codex_home.path().join("config.toml");
    if codex_config.exists() {
        assert!(!fs::read_to_string(codex_config)
            .unwrap()
            .contains("megara_planning"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Runtime data remains"), "stdout={stdout}");
}

#[test]
fn uninstall_removes_pi_process_helper_and_preserves_planning_runtime() {
    let dir = tempdir().unwrap();
    let install = megara()
        .args([
            "install",
            "--scope",
            "project",
            "--target",
            "pi",
            "--no-interactive",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );
    let helper = dir.path().join(".pi/extensions/megara_process.ts");
    assert!(helper.exists());
    let planning_runtime = dir.path().join(".megara/planning/sentinel");
    fs::create_dir_all(planning_runtime.parent().unwrap()).unwrap();
    fs::write(&planning_runtime, "planning data").unwrap();

    let uninstall = megara()
        .args(["uninstall", "--scope", "project", "--target", "pi"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        uninstall.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&uninstall.stderr)
    );
    assert!(!dir.path().join(".pi/extensions/megara.ts").exists());
    assert!(!helper.exists());
    assert!(planning_runtime.exists());
}

#[test]
fn uninstall_keeps_unmanaged_projection_files() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let agents = dir.path().join(".codex/AGENTS.md");
    fs::write(&agents, "# User-owned instructions\n").unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .args(["uninstall", "--scope", "project", "--target", "codex"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(agents).unwrap(),
        "# User-owned instructions\n"
    );
}

#[test]
fn uninstall_dry_run_keeps_managed_files() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let output = megara_with_codex_home(codex_home.path())
        .args([
            "uninstall",
            "--scope",
            "project",
            "--target",
            "codex",
            "--dry-run",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(dir.path().join(".codex/AGENTS.md").exists());
    assert!(dir.path().join(".agents/megara.toml").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("uninstall planned"));
}

#[test]
fn uninstall_keeps_shared_files_when_pi_remains_installed() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let pi = megara_with_codex_home(codex_home.path())
        .args([
            "install",
            "--scope",
            "project",
            "--target",
            "pi",
            "--no-interactive",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        pi.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&pi.stderr)
    );

    let output = megara_with_codex_home(codex_home.path())
        .args(["uninstall", "--scope", "project", "--target", "codex"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!dir.path().join(".codex/AGENTS.md").exists());
    assert!(dir.path().join(".pi/extensions/megara.ts").exists());
    assert!(dir.path().join(".agents/megara.toml").exists());
}

#[test]
fn uninstall_keeps_shared_files_when_other_projection_is_partially_missing() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let pi = megara_with_codex_home(codex_home.path())
        .args([
            "install",
            "--scope",
            "project",
            "--target",
            "pi",
            "--no-interactive",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(pi.status.success());
    fs::remove_file(dir.path().join(".pi/extensions/megara.ts")).unwrap();
    fs::remove_file(dir.path().join(".pi/agents/executor.md")).unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .args(["uninstall", "--scope", "project", "--target", "codex"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(dir.path().join(".agents/megara.toml").exists());
    assert!(dir.path().join(".pi/settings.json").exists());
}
