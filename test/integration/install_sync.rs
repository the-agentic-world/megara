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
        .arg("--force")
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
fn sync_preserves_edited_managed_projection_without_force() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let planner = dir.path().join(".codex/agents/planner.toml");
    let mut edited = fs::read_to_string(&planner).unwrap();
    edited.push_str("\n# UAT unmanaged drift sentinel\n");
    fs::write(&planner, &edited).unwrap();

    let dry_run = megara_with_codex_home(codex_home.path())
        .args([
            "sync",
            "--scope",
            "project",
            "--target",
            "codex",
            "--dry-run",
            "--json",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(dry_run.status.success());
    assert!(String::from_utf8_lossy(&dry_run.stdout).contains("conflicts"));

    let sync = megara_with_codex_home(codex_home.path())
        .args(["sync", "--scope", "project", "--target", "codex"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!sync.status.success());
    assert_eq!(fs::read_to_string(&planner).unwrap(), edited);

    let forced = megara_with_codex_home(codex_home.path())
        .args(["sync", "--scope", "project", "--target", "codex", "--force"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(forced.status.success());
    assert!(!fs::read_to_string(&planner)
        .unwrap()
        .contains("UAT unmanaged drift sentinel"));
}

#[test]
fn project_insane_search_wrapper_uses_runtime_state_and_python_preflight() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    let wrapper = fs::read_to_string(dir.path().join(".agents/bin/insane-search")).unwrap();
    assert!(wrapper.contains("root_dir=$(CDPATH= cd \"$bin_dir/..\" && pwd -P)"));
    assert!(wrapper.contains("runtime_root=\"$root_dir/../.megara\""));
    assert!(wrapper.contains("state/tools/insane-search"));
    assert!(wrapper.contains("Python 3.10 or newer"));
}

#[cfg(unix)]
#[test]
fn project_insane_search_wrapper_creates_runtime_state_then_reports_python_recovery() {
    use std::{os::unix::fs::PermissionsExt, process::Command};

    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let fake_bin = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());
    for candidate in [
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "python3",
    ] {
        let executable = fake_bin.path().join(candidate);
        fs::write(&executable, "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        fake_bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(dir.path().join(".agents/bin/insane-search"))
        .arg("https://example.com")
        .env("PATH", path)
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .last()
            .unwrap(),
        "insane-search requires Python 3.10 or newer; install one and rerun."
    );
    assert!(dir
        .path()
        .join(".megara/state/tools/insane-search")
        .is_dir());
    assert!(!dir.path().join(".agents/state").exists());
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
    let process_helper = dir.path().join(".pi/megara_process.ts");
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

    let ssot_helper = dir.path().join(".agents/pi/megara_process.ts");
    let projected_helper = dir.path().join(".pi/megara_process.ts");
    let mut ssot = fs::read_to_string(&ssot_helper).unwrap();
    ssot.push_str("\n// PI HELPER SSOT UPDATE\n");
    fs::write(&ssot_helper, &ssot).unwrap();

    let sync = megara()
        .args(["sync", "--scope", "project", "--target", "pi"])
        .arg("--force")
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
        .arg("--force")
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
