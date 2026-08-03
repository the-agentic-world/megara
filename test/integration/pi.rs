use std::{fs, process::Command};

use serde_json::Value;
use tempfile::tempdir;

use super::*;

fn install_pi(project: &std::path::Path, trust_project: bool) {
    let mut command = megara();
    command
        .args([
            "install",
            "--scope",
            "project",
            "--target",
            "pi",
            "--no-interactive",
        ])
        .current_dir(project);
    if trust_project {
        command.arg("--trust-project");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn pi_project_install_projects_planning_adapter_and_supports_explicit_trust() {
    let project = tempdir().unwrap();
    install_pi(project.path(), false);

    assert!(project
        .path()
        .join(".agents/pi/extensions/megara.ts")
        .exists());
    assert!(project
        .path()
        .join(".agents/pi/extensions/megara_process.ts")
        .exists());
    assert!(project.path().join(".pi/extensions/megara.ts").exists());
    assert!(project
        .path()
        .join(".pi/extensions/megara_process.ts")
        .exists());
    assert!(project.path().join(".pi/agents/executor.md").exists());
    assert!(project.path().join(".pi/settings.json").exists());

    let extension = fs::read_to_string(project.path().join(".pi/extensions/megara.ts")).unwrap();
    let process_helper =
        fs::read_to_string(project.path().join(".pi/extensions/megara_process.ts")).unwrap();
    assert!(extension.contains("./megara_process.js"));
    assert!(extension.contains("planning rpc"));
    assert!(extension.contains("planning_start"));
    assert!(extension.contains("planning_answer"));
    assert!(extension.contains("use the returned next_action and current work item"));
    assert!(extension.contains("never infer approval or invoke user-owned actions"));
    assert!(extension.contains("pi.registerCommand(\"megara-approve\""));
    assert!(extension.contains("pi.registerCommand(\"megara-revise\""));
    assert!(extension.contains("pi.registerCommand(\"megara-purge\""));
    assert!(extension.contains("pi.exec(megaraCommand()"));
    assert!(!extension.contains("name: \"planning_spec_approve\""));
    assert!(!extension.contains("name: \"planning_plan_approve\""));
    assert!(!extension.contains("name: \"planning_purge\""));
    assert!(!extension.contains("name: \"planning_spec_revise\""));
    assert!(!extension.contains("name: \"planning_plan_revise\""));
    assert!(!extension.contains("name: \"planning_export\""));
    assert!(!extension.contains("pi.on("));
    assert!(extension
        .contains("enumValue([\"interview\", \"specification\", \"planning\", \"complete\"])"));
    assert!(extension.contains("enumValue([\"delta\", \"full\"])"));
    assert!(extension.contains("enumValue([\"markdown\", \"json\"])"));
    assert!(extension.contains("additionalProperties: false"));
    assert!(process_helper.contains("MAX_RPC_OUTPUT_BYTES = 4 * 1024 * 1024"));
    assert!(process_helper.contains("TERMINATION_GRACE_MS = 2_000"));
    assert!(process_helper.contains("child.kill(\"SIGTERM\")"));
    assert!(process_helper.contains("child.kill(\"SIGKILL\")"));
    assert!(process_helper.contains("Megara planning rpc stderr exceeded 4 MiB"));

    install_pi(project.path(), true);
    assert!(project
        .path()
        .join(".megara/trust/pi-project.toml")
        .exists());
}

#[test]
fn pi_global_install_uses_pi_agent_directory_without_project_trust() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let output = megara()
        .arg("install")
        .arg("--scope")
        .arg("global")
        .arg("--target")
        .arg("pi")
        .arg("--no-interactive")
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.path().join(".megara/pi/extensions/megara.ts").exists());
    assert!(home.path().join(".pi/agent/extensions/megara.ts").exists());
    assert!(home.path().join(".pi/agent/agents/architect.md").exists());
}

#[test]
fn pi_projection_applies_explicit_role_model_override() {
    let project = tempdir().unwrap();
    install_pi(project.path(), true);
    let config_path = project.path().join(".agents/megara.toml");
    let config = fs::read_to_string(&config_path).unwrap().replace(
        "[target.pi]\nenabled = true",
        "[target.pi]\nenabled = true\n\n[target.pi.roles.executor]\nmodel = \"openai/gpt-5.6\"\nthinking_level = \"xhigh\"",
    );
    fs::write(&config_path, config).unwrap();

    let output = megara()
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("pi")
        .arg("--no-interactive")
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let executor = fs::read_to_string(project.path().join(".pi/agents/executor.md")).unwrap();
    assert!(executor.contains("model: openai/gpt-5.6"));
    assert!(executor.contains("thinking_level: xhigh"));
}

#[cfg(unix)]
#[test]
fn pi_doctor_accepts_supported_runtime_version() {
    use std::os::unix::fs::PermissionsExt;

    let project = tempdir().unwrap();
    let bin = tempdir().unwrap();
    let executable = bin.path().join("pi");
    fs::write(&executable, "#!/bin/sh\nprintf '0.80.10\\n'\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    install_pi(project.path(), true);

    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_megara"))
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .env("PATH", path)
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("pi")
        .arg("--json")
        .arg("--no-interactive")
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["warnings"].as_array().unwrap().is_empty());

    let agent_path = project.path().join(".agents/agents/executor.toml");
    fs::write(
        &agent_path,
        format!("{}\n# changed\n", fs::read_to_string(&agent_path).unwrap()),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_megara"))
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.path().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("pi")
        .arg("--json")
        .arg("--no-interactive")
        .current_dir(project.path())
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning
            .as_str()
            .is_some_and(|warning| warning.contains("trust no longer matches"))));
}
