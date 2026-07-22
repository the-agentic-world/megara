use std::fs;

use serde_json::Value;
use tempfile::tempdir;

use super::*;

#[test]
fn agents_configure_reprojects_codex_and_pi_roles() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(project.path(), codex_home.path());

    let output = megara_with_codex_home(codex_home.path())
        .args([
            "agents",
            "configure",
            "--scope",
            "project",
            "--target",
            "codex",
            "--role",
            "executor",
            "--model",
            "gpt-5.6-sol",
            "--reasoning-effort",
            "xhigh",
            "--json",
        ])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["action"], "configured");
    let executor = fs::read_to_string(project.path().join(".codex/agents/executor.toml")).unwrap();
    assert!(executor.contains("model = \"gpt-5.6-sol\""));
    assert!(executor.contains("model_reasoning_effort = \"xhigh\""));

    let output = megara_with_codex_home(codex_home.path())
        .args([
            "install",
            "--scope",
            "project",
            "--target",
            "codex",
            "--no-interactive",
        ])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let executor = fs::read_to_string(project.path().join(".codex/agents/executor.toml")).unwrap();
    assert!(executor.contains("model = \"gpt-5.6-sol\""));

    let output = megara_with_codex_home(codex_home.path())
        .args([
            "agents",
            "configure",
            "--scope",
            "project",
            "--target",
            "pi",
            "--role",
            "executor",
            "--model",
            "openai/gpt-5.5",
            "--thinking-level",
            "high",
            "--json",
        ])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let executor = fs::read_to_string(project.path().join(".pi/agents/executor.md")).unwrap();
    assert!(executor.contains("model: openai/gpt-5.5"));
    assert!(executor.contains("thinking_level: high"));
}

#[test]
fn project_policy_overrides_then_resets_to_global_default() {
    let project = tempdir().unwrap();
    let home = tempdir().unwrap();
    let codex_home = home.path().join(".codex");
    for scope in ["global", "project"] {
        let output = megara_with_codex_home(&codex_home)
            .args([
                "install",
                "--scope",
                scope,
                "--target",
                "codex",
                "--no-interactive",
            ])
            .env("HOME", home.path())
            .current_dir(project.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let configure = |scope: &str, model: &str| {
        megara_with_codex_home(&codex_home)
            .args([
                "agents",
                "configure",
                "--scope",
                scope,
                "--target",
                "codex",
                "--role",
                "executor",
                "--model",
                model,
                "--reasoning-effort",
                "high",
                "--json",
            ])
            .env("HOME", home.path())
            .current_dir(project.path())
            .output()
            .unwrap()
    };
    assert!(configure("global", "gpt-5.6-terra").status.success());
    assert!(configure("project", "gpt-5.6-sol").status.success());

    let show = |command: &[&str]| {
        megara_with_codex_home(&codex_home)
            .args(command)
            .env("HOME", home.path())
            .current_dir(project.path())
            .output()
            .unwrap()
    };
    let output = show(&[
        "agents", "show", "--scope", "project", "--target", "codex", "--role", "executor", "--json",
    ]);
    let policies: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(policies[0]["policy"]["model"], "gpt-5.6-sol");

    let output = show(&[
        "agents", "reset", "--scope", "project", "--target", "codex", "--role", "executor",
        "--json",
    ]);
    assert!(output.status.success());
    let output = show(&[
        "agents", "show", "--scope", "project", "--target", "codex", "--role", "executor", "--json",
    ]);
    let policies: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(policies[0]["policy"]["model"], "gpt-5.6-terra");
}

#[test]
fn agents_configuration_protects_unmanaged_ssot_without_force() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(project.path(), codex_home.path());
    let config = project.path().join(".agents/megara.toml");
    fs::write(&config, "name = \"user-owned\"\n").unwrap();

    let configure = |force: bool| {
        let mut command = megara_with_codex_home(codex_home.path());
        command
            .args([
                "agents",
                "configure",
                "--scope",
                "project",
                "--target",
                "codex",
                "--role",
                "executor",
                "--model",
                "gpt-5.6-sol",
                "--reasoning-effort",
                "high",
            ])
            .current_dir(project.path());
        if force {
            command.arg("--force");
        }
        command.output().unwrap()
    };

    let blocked = configure(false);
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("unmanaged Megara configuration"));
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "name = \"user-owned\"\n"
    );

    let forced = configure(true);
    assert!(
        forced.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(fs::read_to_string(&config)
        .unwrap()
        .contains("MEGARA:MANAGED"));
}

#[test]
fn agents_configuration_reports_missing_noninteractive_inputs() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(project.path(), codex_home.path());

    let output = megara_with_codex_home(codex_home.path())
        .args(["agents", "configure"])
        .current_dir(project.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("non-interactive mode"));
}
