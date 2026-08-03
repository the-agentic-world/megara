use super::*;

#[test]
fn sync_refreshes_managed_projection() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let agents = dir.path().join(".codex/AGENTS.md");
    let ssot_skill = dir.path().join(".agents/skills/caveman/SKILL.md");
    let ssot_agent = dir.path().join(".agents/agents/executor.toml");
    let projected_agent = dir.path().join(".codex/agents/executor.toml");
    let ssot_config = dir.path().join(".agents/megara.toml");

    install_project_harness(dir.path(), codex_home.path());
    assert!(!dir.path().join(".codex/skills").exists());
    fs::write(&agents, "# MEGARA:MANAGED\nstale").unwrap();
    let mut ssot_content = fs::read_to_string(&ssot_skill).unwrap();
    ssot_content.push_str("\nSSOT EDIT TOKEN\n");
    fs::write(&ssot_skill, ssot_content).unwrap();
    update_executor_ssot(&ssot_agent);
    let config_content = fs::read_to_string(&ssot_config).unwrap();
    fs::write(&ssot_config, config_content.replace("ko-KR", "en-US")).unwrap();

    let sync = megara_with_codex_home(codex_home.path())
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        sync.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&sync.stderr)
    );
    assert!(fs::read_to_string(agents)
        .unwrap()
        .contains("Locale: `en-US`"));
    assert!(!dir.path().join(".codex/skills").exists());
    assert!(fs::read_to_string(ssot_skill)
        .unwrap()
        .contains("SSOT EDIT TOKEN"));
    assert!(fs::read_to_string(projected_agent)
        .unwrap()
        .contains("SSOT AGENT TOKEN"));
}

#[test]
fn sync_without_target_detects_only_installed_runtime() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let agents = dir.path().join(".codex/AGENTS.md");
    fs::remove_file(&agents).unwrap();

    let sync = megara_with_codex_home(codex_home.path())
        .arg("sync")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        sync.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&sync.stderr)
    );
    let stdout = String::from_utf8_lossy(&sync.stdout);
    assert!(stdout.contains("target=codex"));
    assert!(!stdout.contains("target=pi"));
    assert!(agents.exists());
    assert!(!dir.path().join(".pi/extensions/megara.ts").exists());
}

#[test]
fn sync_without_target_detects_pi_when_it_is_the_only_runtime() {
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
    let extension = dir.path().join(".pi/extensions/megara.ts");
    let process_helper = dir.path().join(".pi/extensions/megara_process.ts");
    fs::remove_file(&extension).unwrap();
    fs::remove_file(&process_helper).unwrap();

    let sync = megara()
        .arg("sync")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        sync.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&sync.stderr)
    );
    let stdout = String::from_utf8_lossy(&sync.stdout);
    assert!(stdout.contains("target=pi"));
    assert!(!stdout.contains("target=codex"));
    assert!(extension.exists());
    assert!(process_helper.exists());
    assert!(!dir.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn sync_repairs_drifted_pi_process_helper_from_ssot() {
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
    assert!(install.status.success());

    let ssot_helper = dir.path().join(".agents/pi/extensions/megara_process.ts");
    let projected_helper = dir.path().join(".pi/extensions/megara_process.ts");
    let mut ssot = fs::read_to_string(&ssot_helper).unwrap();
    ssot.push_str("\n// PI HELPER SSOT UPDATE\n");
    fs::write(&ssot_helper, &ssot).unwrap();

    let sync = megara()
        .args(["sync", "--scope", "project", "--target", "pi"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        sync.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&sync.stderr)
    );
    assert!(fs::read_to_string(&projected_helper)
        .unwrap()
        .contains("PI HELPER SSOT UPDATE"));

    let mut drifted = fs::read_to_string(&projected_helper).unwrap();
    drifted.push_str("// PI HELPER DRIFT\n");
    fs::write(&projected_helper, drifted).unwrap();
    let doctor = megara()
        .args([
            "doctor",
            "--scope",
            "project",
            "--target",
            "pi",
            "--json",
            "--no-interactive",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert!(report["stale"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path.as_str().unwrap().ends_with("megara_process.ts")));

    let repair = megara()
        .args(["sync", "--scope", "project", "--target", "pi"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(repair.status.success());
    let repaired = fs::read_to_string(projected_helper).unwrap();
    assert!(repaired.contains("PI HELPER SSOT UPDATE"));
    assert!(!repaired.contains("PI HELPER DRIFT"));
}

fn update_executor_ssot(ssot_agent: &Path) {
    let ssot_agent_content = fs::read_to_string(ssot_agent).unwrap();
    fs::write(
        ssot_agent,
        ssot_agent_content.replace(
            "Report changed files, decisions, verification performed, and remaining blockers.",
            "Report changed files, decisions, verification performed, and remaining blockers.\nSSOT AGENT TOKEN",
        ),
    )
    .unwrap();
}
