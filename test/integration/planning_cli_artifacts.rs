use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

use super::planning_cli_artifact_support::{
    plan_proposal, prepare_at_specification, prepare_complete, spec_proposal,
};

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

fn run_with_input(project: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = megara(project)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn one_line(output: &Output) -> Value {
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(text.lines().count(), 1, "stdout={text:?}");
    serde_json::from_str(text.trim()).unwrap()
}

#[test]
fn nested_artifact_commands_aliases_and_default_bundle_are_real_binary_paths() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    let started = run(
        project,
        &[
            "planning",
            "start",
            "--project",
            project.to_str().unwrap(),
            "--request",
            "CLI artifact request",
            "--json",
        ],
    );
    assert!(started.status.success(), "stderr={:?}", started.stderr);
    let started_json = one_line(&started);
    let session = started_json["session_id"].as_str().unwrap().to_string();
    prepare_complete(project, &session);

    let wrong_candidate = run(
        project,
        &[
            "planning",
            "spec",
            "show",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--candidate",
            "cand_missing",
            "--json",
        ],
    );
    assert_eq!(wrong_candidate.status.code(), Some(3));
    assert_eq!(
        one_line(&wrong_candidate)["error"]["code"],
        "CANDIDATE_NOT_FOUND"
    );

    let shown = run(
        project,
        &[
            "planning",
            "spec",
            "show",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--format",
            "markdown",
        ],
    );
    assert!(shown.status.success(), "stderr={:?}", shown.stderr);
    let markdown = String::from_utf8(shown.stdout).unwrap();
    assert!(
        markdown.starts_with("# A traceable canonical specification"),
        "markdown={markdown:?}"
    );
    assert!(markdown.contains("- summary: A traceable canonical specification"));
    assert!(!markdown.contains("<script"));
    let projected = fs::read_to_string(
        project
            .join(".megara/planning/artifacts")
            .join(&session)
            .join("spec.md"),
    )
    .unwrap();
    let projected_body = projected.split_once("-->\n").unwrap().1.trim();
    assert_eq!(projected_body, markdown.trim());

    let shown_json = run(
        project,
        &[
            "planning",
            "plan",
            "show",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--json",
        ],
    );
    assert!(shown_json.status.success());
    assert!(one_line(&shown_json)["result"]["candidate"].is_object());

    let output = project.join("cli-bundle");
    let exported = run(
        project,
        &[
            "planning",
            "export",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--out",
            output.to_str().unwrap(),
            "--json",
        ],
    );
    assert!(exported.status.success(), "stderr={:?}", exported.stderr);
    let exported_json = one_line(&exported);
    assert_eq!(exported_json["result"]["format"], "bundle");
    assert_eq!(
        exported_json["result"]["path"],
        output.to_string_lossy().to_string()
    );
    assert!(output.join("manifest.json").is_file());
    assert!(output.join("spec.md").is_file());
    assert!(output.join("plan.md").is_file());

    let conflict = run(
        project,
        &[
            "planning",
            "export",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--out",
            output.to_str().unwrap(),
            "--json",
        ],
    );
    assert_eq!(conflict.status.code(), Some(5));
    assert_eq!(one_line(&conflict)["error"]["code"], "PROJECTION_DIVERGED");

    let define = run(
        project,
        &[
            "define",
            "an alias request",
            "--project",
            project.to_str().unwrap(),
            "--json",
        ],
    );
    assert!(define.status.success());
    assert_eq!(one_line(&define)["ok"], true);

    let plan_alias = run(
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
    assert!(plan_alias.status.success());
    assert_eq!(
        one_line(&plan_alias)["result"]["state"]["phase"],
        "complete"
    );
}

#[test]
fn show_without_candidate_and_wrong_candidate_are_typed_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path();
    let started = run(
        project,
        &[
            "planning",
            "start",
            "--project",
            project.to_str().unwrap(),
            "--request",
            "show candidate request",
            "--json",
        ],
    );
    assert!(started.status.success());
    let session = one_line(&started)["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    let missing = run(
        project,
        &[
            "planning",
            "spec",
            "show",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--json",
        ],
    );
    assert_eq!(missing.status.code(), Some(3));
    assert_eq!(one_line(&missing)["error"]["code"], "CANDIDATE_NOT_FOUND");
}

#[test]
fn cli_generate_and_approve_use_file_and_stdin_proposal_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    let started = run(
        project,
        &[
            "planning",
            "start",
            "--project",
            project.to_str().unwrap(),
            "--request",
            "CLI proposal boundary request",
            "--json",
        ],
    );
    assert!(started.status.success());
    let started_json = one_line(&started);
    let session = started_json["session_id"].as_str().unwrap().to_string();
    let (service, full) = prepare_at_specification(project, &session);
    let full_state = full["result"]["state"].clone();
    let spec_file = project.join("spec-proposal.json");
    fs::write(
        &spec_file,
        serde_json::to_vec_pretty(&spec_proposal(
            &full_state,
            &full_state["required_model_action"],
        ))
        .unwrap(),
    )
    .unwrap();
    drop(service);

    let generated_spec = run(
        project,
        &[
            "planning",
            "spec",
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--expected-revision",
            &full["revision"].as_u64().unwrap().to_string(),
            "--proposal",
            spec_file.to_str().unwrap(),
            "--command-id",
            "cmd-cli-boundary-spec",
            "--json",
        ],
    );
    assert!(
        generated_spec.status.success(),
        "stderr={:?}",
        generated_spec.stderr
    );
    let generated_spec_json = one_line(&generated_spec);
    let candidate = generated_spec_json["result"]["candidate"].clone();
    let approved_spec = run(
        project,
        &[
            "planning",
            "spec",
            "approve",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--expected-revision",
            &generated_spec_json["revision"]
                .as_u64()
                .unwrap()
                .to_string(),
            "--candidate",
            candidate["candidate_id"].as_str().unwrap(),
            "--semantic-hash",
            candidate["semantic_hash"].as_str().unwrap(),
            "--base-domain-revision",
            &candidate["base_domain_revision"]
                .as_u64()
                .unwrap()
                .to_string(),
            "--command-id",
            "cmd-cli-boundary-spec-approve",
            "--json",
        ],
    );
    assert!(
        approved_spec.status.success(),
        "stderr={:?}",
        approved_spec.stderr
    );
    let approved_spec_json = one_line(&approved_spec);
    let store = crate::planning::store::PlanningStore::open_project(project).unwrap();
    let state = store.current(&session).unwrap();
    let state_value = serde_json::to_value(&state).unwrap();
    let plan = plan_proposal(
        &state_value,
        &serde_json::to_value(state.required_model_action.as_ref().unwrap()).unwrap(),
        &serde_json::to_value(state.spec.approval.as_ref().unwrap()).unwrap(),
    );
    let plan_bytes = serde_json::to_vec(&plan).unwrap();
    let generated_plan = run_with_input(
        project,
        &[
            "planning",
            "plan",
            "generate",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--expected-revision",
            &approved_spec_json["revision"].as_u64().unwrap().to_string(),
            "--proposal",
            "-",
            "--command-id",
            "cmd-cli-boundary-plan",
            "--json",
        ],
        &plan_bytes,
    );
    assert!(
        generated_plan.status.success(),
        "stderr={:?}",
        generated_plan.stderr
    );
    let generated_plan_json = one_line(&generated_plan);
    let plan_candidate = generated_plan_json["result"]["candidate"].clone();
    let approved_plan = run(
        project,
        &[
            "planning",
            "plan",
            "approve",
            "--project",
            project.to_str().unwrap(),
            "--session",
            &session,
            "--expected-revision",
            &generated_plan_json["revision"]
                .as_u64()
                .unwrap()
                .to_string(),
            "--candidate",
            plan_candidate["candidate_id"].as_str().unwrap(),
            "--semantic-hash",
            plan_candidate["semantic_hash"].as_str().unwrap(),
            "--base-plan-revision",
            &plan_candidate["base_plan_revision"]
                .as_u64()
                .unwrap()
                .to_string(),
            "--command-id",
            "cmd-cli-boundary-plan-approve",
            "--json",
        ],
    );
    assert!(
        approved_plan.status.success(),
        "stderr={:?}",
        approved_plan.stderr
    );
    assert_eq!(
        one_line(&approved_plan)["result"]["state"]["phase"],
        "complete"
    );
}
