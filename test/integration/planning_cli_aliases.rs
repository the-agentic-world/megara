use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

use super::planning_cli_artifact_support::prepare_at_specification;

fn megara(project: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_megara"));
    command
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .current_dir(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run(project: &Path, args: &[&str]) -> Output {
    megara(project).args(args).output().unwrap()
}

fn one_line(output: &Output) -> Value {
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(text.lines().count(), 1, "stdout={text:?}");
    serde_json::from_str(text.trim()).unwrap()
}

#[test]
fn plain_plan_and_define_print_next_action_without_model_calls() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    let started = run(
        project,
        &[
            "define",
            "plain plan request",
            "--project",
            project.to_str().unwrap(),
            "--json",
        ],
    );
    assert!(started.status.success());
    let session = one_line(&started)["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, full) = prepare_at_specification(project, &session);
    assert_eq!(full["result"]["state"]["phase"], "specification");

    let plan = run(
        project,
        &[
            "plan",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
        ],
    );
    assert!(plan.status.success(), "stderr={:?}", plan.stderr);
    let stdout = String::from_utf8(plan.stdout).unwrap();
    assert!(
        stdout.contains("next action: generate_spec"),
        "stdout={stdout:?}"
    );
    assert!(stdout.contains("work item: wrk_"), "stdout={stdout:?}");

    let plan_json = run(
        project,
        &[
            "plan",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--json",
        ],
    );
    let plan_json = one_line(&plan_json);
    assert!(plan_json["result"].get("next_action").is_none());
    assert!(plan_json["result"]["state"]["required_model_action"].is_object());

    let define = run(
        project,
        &[
            "define",
            "second plain request",
            "--project",
            project.to_str().unwrap(),
        ],
    );
    assert!(define.status.success(), "stderr={:?}", define.stderr);
    let stdout = String::from_utf8(define.stdout).unwrap();
    assert!(
        stdout.contains("next action: delta_audit"),
        "stdout={stdout:?}"
    );
    assert!(stdout.contains("work item: wrk_"), "stdout={stdout:?}");
}
