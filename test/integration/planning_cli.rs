use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::tempdir;
use uuid::Uuid;

use crate::planning::domain::{AnswerMode, QuestionProposal, RepoEvidenceSnapshot, SourceRef};
use crate::planning::engine::{AuditCommand, AuditMode, AuditReadiness, EvidenceRefreshCommand};
use crate::planning::store::PlanningStore;

fn megara() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_megara"));
    command.env_remove("MEGARA_NO_UPDATE_CHECK");
    command
}

fn run(project: &Path, args: &[&str], input: Option<&[u8]>) -> Output {
    let mut command = megara();
    command
        .args(args)
        .current_dir(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child.stdin.as_mut().unwrap().write_all(input).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn one_line(output: &Output) -> Value {
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().count(), 1, "stdout={stdout:?}");
    serde_json::from_str(stdout.trim()).unwrap()
}

fn snapshot(hash: &str) -> RepoEvidenceSnapshot {
    RepoEvidenceSnapshot {
        evidence_hash: hash.to_string(),
        head_oid: None,
        status_hash: format!("{hash}-status"),
        cited_files_hash: format!("{hash}-files"),
    }
}

fn prepare_pending(project: &Path) -> (String, String, u64) {
    let mut store = PlanningStore::open_project(project).unwrap();
    let project_id = store.project_id().to_string();
    let started = store
        .start(
            "cmd-integration-start",
            "sha256:integration-start",
            crate::planning::engine::StartCommand {
                session_id: None,
                project_id,
                request: "integration answer request".to_string(),
                title: None,
            },
        )
        .unwrap();
    let session_id = started.state.session_id.clone();
    store
        .refresh_evidence(
            "cmd-integration-evidence",
            "sha256:integration-evidence",
            EvidenceRefreshCommand {
                session_id: session_id.clone(),
                expected_revision: started.state.revision,
                snapshot: snapshot("sha256:integration-evidence"),
            },
        )
        .unwrap();
    let state = store.current(&session_id).unwrap();
    let work_item = state.required_model_action.clone().unwrap();
    store
        .apply_audit(
            "cmd-integration-audit",
            "sha256:integration-audit",
            AuditCommand {
                session_id: session_id.clone(),
                expected_revision: state.revision,
                work_item_id: work_item.work_item_id,
                mode: AuditMode::Delta,
                base_revision: work_item.base_revision,
                base_domain_revision: work_item.base_domain_revision,
                input_hash: work_item.input_hash,
                readiness: AuditReadiness::Continue,
                next_question: Some(QuestionProposal {
                    context: "배경".to_string(),
                    question: "무엇을 결정할까요?".to_string(),
                    why_it_matters: "답에 따라 계획이 달라집니다.".to_string(),
                    technical_terms: Vec::new(),
                    source_refs: vec![SourceRef::InitialRequest {
                        id: "request".to_string(),
                    }],
                    answer: AnswerMode::Freeform {
                        freeform_hint: "답을 적어 주세요.".to_string(),
                    },
                }),
                entity_ops: Vec::new(),
                edge_ops: Vec::new(),
                blocker_ops: Vec::new(),
                counterexample_review_performed: false,
            },
        )
        .unwrap();
    let pending = store
        .current(&session_id)
        .unwrap()
        .pending_question
        .unwrap();
    (session_id, pending.question_id, pending.based_on_revision)
}

#[test]
fn planning_cli_json_start_answer_status_current_list_purge_is_deterministic() {
    let directory = tempdir().unwrap();
    let project = directory.path();
    let started = run(
        project,
        &[
            "planning",
            "start",
            "--project",
            project.to_str().unwrap(),
            "--request",
            "CLI planning request",
            "--title",
            "CLI title",
            "--json",
        ],
        None,
    );
    assert!(started.status.success());
    let started_json = one_line(&started);
    assert_eq!(started_json["ok"], true);
    let generated_command_id = started_json["command_id"].as_str().unwrap();
    let command_uuid = generated_command_id.strip_prefix("cmd_").unwrap();
    assert_eq!(Uuid::parse_str(command_uuid).unwrap().get_version_num(), 7);
    let session = started_json["session_id"].as_str().unwrap();
    let event_store = PlanningStore::open_project(project).unwrap();
    let event_id = event_store
        .event_envelopes(session)
        .unwrap()
        .remove(0)
        .event_id;
    assert_eq!(Uuid::parse_str(&event_id).unwrap().get_version_num(), 7);

    for operation in ["status", "current"] {
        let output = run(
            project,
            &[
                "planning",
                operation,
                "--project",
                project.to_str().unwrap(),
                "--session",
                session,
                "--json",
            ],
            None,
        );
        assert!(output.status.success());
        let response = one_line(&output);
        assert_eq!(response["ok"], true);
        assert_eq!(response["revision"], 1);
    }

    let listed = run(
        project,
        &[
            "planning",
            "list",
            "--project",
            project.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    assert!(listed.status.success());
    assert_eq!(
        one_line(&listed)["result"]["sessions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let answer = run(
        project,
        &[
            "planning",
            "answer",
            "--project",
            project.to_str().unwrap(),
            "--session",
            session,
            "--question",
            "qst-missing",
            "--expected-revision",
            "1",
            "--text",
            "answer",
            "--command-id",
            "cmd-cli-answer",
            "--json",
        ],
        None,
    );
    assert_eq!(answer.status.code(), Some(3));
    assert_eq!(one_line(&answer)["error"]["code"], "QUESTION_MISMATCH");

    let purged = run(
        project,
        &[
            "planning",
            "purge",
            "--project",
            project.to_str().unwrap(),
            "--session",
            session,
            "--expected-revision",
            "1",
            "--confirm",
            session,
            "--command-id",
            "cmd-cli-purge",
            "--json",
        ],
        None,
    );
    assert!(purged.status.success());
    assert_eq!(one_line(&purged)["result"]["purged"], true);

    let listed_after = run(
        project,
        &[
            "planning",
            "list",
            "--project",
            project.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    assert!(listed_after.status.success());
    assert!(one_line(&listed_after)["result"]["sessions"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn planning_rpc_is_one_shot_jsonl_model_authority_and_has_no_update_noise() {
    let directory = tempdir().unwrap();
    let project = directory.path();
    let request = br#"{"protocol_version":1,"request_id":"req-rpc-start","operation":"planning.start","command_id":"cmd-rpc-start","params":{"request":"RPC request"}}
"#;
    let started = run(
        project,
        &["planning", "rpc", "--project", project.to_str().unwrap()],
        Some(request),
    );
    assert!(started.status.success());
    let started_json = one_line(&started);
    assert_eq!(started_json["ok"], true);
    let session = started_json["session_id"].as_str().unwrap();

    let purge = format!(
        "{{\"protocol_version\":1,\"request_id\":\"req-rpc-purge\",\"operation\":\"planning.purge\",\"command_id\":\"cmd-rpc-purge\",\"session_id\":\"{session}\",\"expected_revision\":1,\"params\":{{\"confirm\":\"{session}\"}}}}\n"
    );
    let rejected = run(
        project,
        &["planning", "rpc", "--project", project.to_str().unwrap()],
        Some(purge.as_bytes()),
    );
    assert_eq!(rejected.status.code(), Some(2));
    assert_eq!(
        one_line(&rejected)["error"]["code"],
        "USER_ENTRYPOINT_REQUIRED"
    );

    let missing_session = br#"{"protocol_version":1,"request_id":"req-rpc-answer","operation":"planning.answer","command_id":"cmd-rpc-answer","expected_revision":1,"params":{"question_id":"qst","text":"answer"}}
"#;
    let missing = run(
        project,
        &["planning", "rpc", "--project", project.to_str().unwrap()],
        Some(missing_session),
    );
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(one_line(&missing)["error"]["code"], "SESSION_REQUIRED");
}

#[test]
fn planning_rpc_malformed_and_oversize_frames_are_bounded_single_line_errors() {
    let directory = tempdir().unwrap();
    let malformed = run(
        directory.path(),
        &[
            "planning",
            "rpc",
            "--project",
            directory.path().to_str().unwrap(),
        ],
        Some(b"{"),
    );
    assert_eq!(malformed.status.code(), Some(2));
    assert_eq!(one_line(&malformed)["ok"], false);

    let oversized = vec![b'x'; 4 * 1024 * 1024 + 1];
    let output = run(
        directory.path(),
        &[
            "planning",
            "rpc",
            "--project",
            directory.path().to_str().unwrap(),
        ],
        Some(&oversized),
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(one_line(&output)["error"]["code"], "INVALID_REQUEST");
}

#[test]
fn planning_answer_requires_exactly_one_input_flag() {
    let directory = tempdir().unwrap();
    let output = run(
        directory.path(),
        &[
            "planning",
            "answer",
            "--project",
            directory.path().to_str().unwrap(),
            "--session",
            "pln",
            "--question",
            "qst",
            "--expected-revision",
            "1",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn planning_cli_schema_errors_use_exit_code_five() {
    let directory = tempdir().unwrap();
    let database_dir = directory.path().join(".megara/planning");
    std::fs::create_dir_all(&database_dir).unwrap();
    let database = database_dir.join("planning.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
    drop(connection);
    let output = run(
        directory.path(),
        &[
            "planning",
            "list",
            "--project",
            directory.path().to_str().unwrap(),
            "--json",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(
        one_line(&output)["error"]["code"],
        "SCHEMA_VERSION_UNSUPPORTED"
    );
}

#[test]
fn planning_answer_oversize_stdin_is_a_flushed_json_error() {
    let directory = tempdir().unwrap();
    let oversized = vec![b'a'; 64 * 1024 + 1];
    let output = run(
        directory.path(),
        &[
            "planning",
            "answer",
            "--project",
            directory.path().to_str().unwrap(),
            "--session",
            "pln",
            "--question",
            "qst",
            "--expected-revision",
            "1",
            "--stdin",
            "--json",
        ],
        Some(&oversized),
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(one_line(&output)["error"]["code"], "INVALID_REQUEST");
}

#[test]
fn planning_cli_answer_success_preserves_raw_binding_and_replays_after_restart() {
    let directory = tempdir().unwrap();
    let project = directory.path();
    let (session, question, revision) = prepare_pending(project);
    let revision_text = revision.to_string();
    let args = [
        "planning",
        "answer",
        "--project",
        project.to_str().unwrap(),
        "--session",
        session.as_str(),
        "--question",
        question.as_str(),
        "--expected-revision",
        &revision_text,
        "--text",
        "raw CLI answer",
        "--command-id",
        "cmd-integration-answer",
        "--json",
    ];
    let first = run(project, &args, None);
    assert!(first.status.success());
    let first_json = one_line(&first);
    assert_eq!(first_json["replayed"], false);
    assert_eq!(
        first_json["result"]["state"]["transcript"]["answers"][0]["text"],
        "raw CLI answer"
    );
    assert_eq!(
        first_json["result"]["state"]["transcript"]["answers"][0]["question_id"],
        question
    );
    let first_revision = first_json["revision"].as_u64().unwrap();
    let store = PlanningStore::open_project(project).unwrap();
    let event_count = store.event_count(&session).unwrap();
    assert_eq!(store.current(&session).unwrap().revision, first_revision);
    drop(store);

    let retry = run(project, &args, None);
    assert!(retry.status.success());
    let retry_json = one_line(&retry);
    assert_eq!(retry_json["replayed"], true);
    assert_eq!(retry_json["revision"], first_revision);
    let store = PlanningStore::open_project(project).unwrap();
    assert_eq!(store.event_count(&session).unwrap(), event_count);
    assert_eq!(store.current(&session).unwrap().revision, first_revision);
}
