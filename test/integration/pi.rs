use std::{fs, process::Command, thread};

use serde_json::{json, Value};
use tempfile::tempdir;

use super::*;

fn install_pi(project: &std::path::Path, trust_project: bool) {
    let mut command = megara();
    command
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("pi")
        .arg("--no-interactive")
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

fn event(project: &std::path::Path, value: Value) -> Value {
    let output = run_pi_event(project, serde_json::to_vec(&value).unwrap().as_slice());
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn pi_project_install_projects_agents_and_requires_explicit_trust() {
    let project = tempdir().unwrap();
    install_pi(project.path(), false);

    assert!(project
        .path()
        .join(".agents/pi/extensions/megara.ts")
        .exists());
    assert!(project.path().join(".pi/extensions/megara.ts").exists());
    assert!(project.path().join(".pi/agents/executor.md").exists());
    assert!(project.path().join(".pi/settings.json").exists());
    let extension = fs::read_to_string(project.path().join(".pi/extensions/megara.ts")).unwrap();
    assert!(extension.contains("--append-system-prompt"));
    assert!(extension.contains("\"--approve\""));
    assert!(!extension.contains("--agent"));
    assert!(extension.contains("event.text.match(WORKFLOW_PATTERN)"));
    assert!(!extension.contains("event.input.match(WORKFLOW_PATTERN)"));
    assert!(extension.contains("${event.systemPrompt}"));
    assert!(extension.contains("Follow the loaded workflow skill"));
    let executor = fs::read_to_string(project.path().join(".pi/agents/executor.md")).unwrap();
    assert!(executor.contains("name: executor"));
    assert!(executor.contains("# Executor"));

    let blocked = event(
        project.path(),
        json!({"protocol_version": 1, "action": "activate", "event_id": "blocked", "workflow": "ralplan"}),
    );
    assert_eq!(blocked["status"], "blocked");

    install_pi(project.path(), true);
    assert!(project
        .path()
        .join(".megara/trust/pi-project.toml")
        .exists());
    let active = event(
        project.path(),
        json!({"protocol_version": 1, "action": "activate", "event_id": "trusted", "workflow": "ralplan"}),
    );
    assert_eq!(active["status"], "active");
    let roles = event(
        project.path(),
        json!({"protocol_version": 1, "action": "next-action", "event_id": "trusted", "workflow": "ralplan"}),
    );
    assert_eq!(
        roles["required_roles"],
        json!(["planner", "architect", "critic"])
    );
}

#[test]
fn pi_event_protocol_recovers_completed_output_and_bounds_retries() {
    let project = tempdir().unwrap();
    install_pi(project.path(), true);
    let activate = json!({"protocol_version": 1, "action": "activate", "event_id": "run", "workflow": "ultragoal"});
    assert_eq!(event(project.path(), activate.clone())["status"], "active");
    assert_eq!(event(project.path(), activate)["status"], "active");

    let started = event(
        project.path(),
        json!({"protocol_version": 1, "action": "prepare-attempt", "event_id": "run", "workflow": "ultragoal", "role": "executor"}),
    );
    let attempt_id = started["attempt_id"].as_str().unwrap();
    let completed = event(
        project.path(),
        json!({"protocol_version": 1, "action": "attempt-finished", "event_id": "run", "workflow": "ultragoal", "attempt_id": attempt_id, "status": "completed", "output": "verified"}),
    );
    assert_eq!(completed["status"], "completed");
    let replay = event(
        project.path(),
        json!({"protocol_version": 1, "action": "prepare-attempt", "event_id": "run", "workflow": "ultragoal", "role": "executor"}),
    );
    assert_eq!(replay["status"], "completed");
    assert_eq!(replay["output"], "verified");

    assert_eq!(
        event(
            project.path(),
            json!({"protocol_version": 1, "action": "activate", "event_id": "retry", "workflow": "team"})
        )["status"],
        "active"
    );
    for expected in ["retry", "retry", "fallback", "blocked"] {
        let started = event(
            project.path(),
            json!({"protocol_version": 1, "action": "prepare-attempt", "event_id": "retry", "workflow": "team", "role": "critic", "model": "primary"}),
        );
        let response = event(
            project.path(),
            json!({"protocol_version": 1, "action": "attempt-finished", "event_id": "retry", "workflow": "team", "attempt_id": started["attempt_id"], "status": "failed", "error": "Selected model is at capacity"}),
        );
        assert_eq!(response["status"], expected);
    }
}

#[test]
fn pi_role_receipts_keep_each_completed_output() {
    let project = tempdir().unwrap();
    install_pi(project.path(), true);
    let activate = json!({"protocol_version": 1, "action": "activate", "event_id": "role-output", "workflow": "deep-interview"});
    assert_eq!(event(project.path(), activate)["status"], "active");

    for (role, output) in [
        ("researcher", "research finding"),
        ("contrarian", "risk finding"),
    ] {
        let started = event(
            project.path(),
            json!({"protocol_version": 1, "action": "prepare-attempt", "event_id": "role-output", "workflow": "deep-interview", "role": role}),
        );
        assert_eq!(started["status"], "started");
        let completed = event(
            project.path(),
            json!({"protocol_version": 1, "action": "attempt-finished", "event_id": "role-output", "workflow": "deep-interview", "attempt_id": started["attempt_id"], "status": "completed", "output": output}),
        );
        assert_eq!(completed["output"], output);
    }

    for (role, output) in [
        ("researcher", "research finding"),
        ("contrarian", "risk finding"),
    ] {
        let replay = event(
            project.path(),
            json!({"protocol_version": 1, "action": "prepare-attempt", "event_id": "role-output", "workflow": "deep-interview", "role": role}),
        );
        assert_eq!(replay["status"], "completed");
        assert_eq!(replay["output"], output);
    }
}

#[test]
fn pi_parallel_role_attempts_preserve_every_receipt() {
    let project = tempdir().unwrap();
    install_pi(project.path(), true);
    assert_eq!(
        event(
            project.path(),
            json!({"protocol_version": 1, "action": "activate", "event_id": "parallel", "workflow": "deep-interview"})
        )["status"],
        "active"
    );

    let project_path = project.path().to_path_buf();
    let roles = ["researcher", "contrarian", "simplifier", "architect"];
    let responses = thread::scope(|scope| {
        let handles = roles.map(|role| {
            let project_path = project_path.clone();
            scope.spawn(move || {
                event(
                    &project_path,
                    json!({"protocol_version": 1, "action": "prepare-attempt", "event_id": "parallel", "workflow": "deep-interview", "role": role}),
                )
            })
        });
        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel Pi event should finish"))
            .collect::<Vec<_>>()
    });
    assert!(responses
        .iter()
        .all(|response| response["status"] == "started"));
    let mut attempt_ids = responses
        .iter()
        .map(|response| response["attempt_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    attempt_ids.sort_unstable();
    attempt_ids.dedup();
    assert_eq!(attempt_ids.len(), roles.len());

    let receipt: Value = serde_json::from_slice(
        &fs::read(
            project
                .path()
                .join(".megara/state/workflows/pi/events/parallel.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["attempts"].as_array().unwrap().len(), roles.len());
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
