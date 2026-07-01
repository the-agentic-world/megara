use std::{fs, process::Command};

use tempfile::tempdir;

fn megara() -> Command {
    Command::new(env!("CARGO_BIN_EXE_megara"))
}

#[test]
fn installs_project_scope_codex_harness() {
    let dir = tempdir().unwrap();

    let output = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join(".agents/megara.toml").exists());
    assert!(dir.path().join(".codex/AGENTS.md").exists());
    assert!(dir
        .path()
        .join(".codex/skills/deep-interview/SKILL.md")
        .exists());
    assert!(dir
        .path()
        .join(".agents/skill-fragments/deep-interview/auto-research-greenfield.md")
        .exists());
    assert!(dir
        .path()
        .join(".codex/skill-fragments/deep-interview/auto-research-greenfield.md")
        .exists());
    assert!(dir.path().join(".codex/agents/executor.toml").exists());
    let skill =
        fs::read_to_string(dir.path().join(".codex/skills/deep-interview/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\n"));
    assert!(skill.contains("MEGARA:MANAGED"));
    toml::from_str::<toml::Value>(
        &fs::read_to_string(dir.path().join(".codex/agents/executor.toml")).unwrap(),
    )
    .unwrap();
    toml::from_str::<toml::Value>(
        &fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap(),
    )
    .unwrap();
}

#[test]
fn installs_global_scope_codex_harness() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    let output = megara()
        .arg("install")
        .arg("--scope")
        .arg("global")
        .arg("--target")
        .arg("codex")
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.path().join(".megara/megara.toml").exists());
    assert!(home.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn sync_refreshes_managed_projection() {
    let dir = tempdir().unwrap();
    let agents = dir.path().join(".codex/AGENTS.md");
    let ssot_skill = dir.path().join(".agents/skills/deep-interview/SKILL.md");
    let projected_skill = dir.path().join(".codex/skills/deep-interview/SKILL.md");

    let install = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    fs::write(&agents, "# MEGARA:MANAGED\nstale").unwrap();
    let mut ssot_content = fs::read_to_string(&ssot_skill).unwrap();
    ssot_content.push_str("\nSSOT EDIT TOKEN\n");
    fs::write(&ssot_skill, ssot_content).unwrap();

    let sync = megara()
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
    let content = fs::read_to_string(agents).unwrap();
    assert!(content.contains("Megara Codex Harness"));
    let skill_content = fs::read_to_string(projected_skill).unwrap();
    assert!(skill_content.contains("SSOT EDIT TOKEN"));
}

#[test]
fn doctor_reports_missing_then_ok() {
    let dir = tempdir().unwrap();

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

    let install = megara()
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let ssot_skill = dir.path().join(".agents/skills/deep-interview/SKILL.md");
    let mut ssot_content = fs::read_to_string(&ssot_skill).unwrap();
    ssot_content.push_str("\nDOCTOR DRIFT TOKEN\n");
    fs::write(&ssot_skill, ssot_content).unwrap();

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
    assert!(stale_stdout.contains(".codex/skills/deep-interview/SKILL.md"));

    let sync = megara()
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
}

#[test]
fn lists_targets_and_templates() {
    let targets = megara().arg("targets").arg("list").output().unwrap();
    assert!(targets.status.success());
    assert!(String::from_utf8_lossy(&targets.stdout).contains("codex"));

    let templates = megara().arg("templates").arg("list").output().unwrap();
    assert!(templates.status.success());
    let stdout = String::from_utf8_lossy(&templates.stdout);
    assert!(stdout.contains("deep-interview"));
    assert!(stdout.contains("deep-interview/auto-research-greenfield"));
}
