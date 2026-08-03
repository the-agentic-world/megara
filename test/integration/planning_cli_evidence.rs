use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::tempdir;

use crate::planning::store::{EventActor, EventAdapter, PlanningStore};

fn run(project: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_megara"));
    command
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .args(args)
        .current_dir(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.output().unwrap()
}

fn json_output(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "status={:?} stdout={:?} stderr={:?}",
        output.status,
        output.stdout,
        output.stderr
    );
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn evidence_and_audit_cli_use_exact_files_typed_proposals_and_idempotency() {
    let directory = tempdir().unwrap();
    let inputs = tempdir().unwrap();
    let project = directory.path();
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    let project_arg = project.to_string_lossy().to_string();

    let started = json_output(&run(
        project,
        &[
            "planning",
            "start",
            "--project",
            &project_arg,
            "--request",
            "capture evidence",
            "--command-id",
            "cmd-cli-s3-start",
            "--json",
        ],
    ));
    let session = started["session_id"].as_str().unwrap().to_string();
    assert_eq!(started["revision"], 1);

    let citations_path = inputs.path().join("citations.json");
    std::fs::write(
        &citations_path,
        serde_json::to_vec(&json!({
            "schema": "megara.evidence-citations/v1",
            "base_revision": 1,
            "citations": [{
                "temp_ref": "main",
                "path": "src/main.rs",
                "ranges": [{"start_line": 1, "end_line": 1}],
                "claim": "main is the executable entry point"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let citations_arg = citations_path.to_string_lossy().to_string();
    let evidence_output = run(
        project,
        &[
            "planning",
            "evidence",
            "refresh",
            "--project",
            &project_arg,
            "--session",
            &session,
            "--expected-revision",
            "1",
            "--citations",
            &citations_arg,
            "--command-id",
            "cmd-cli-s3-evidence",
            "--json",
        ],
    );
    let evidence = json_output(&evidence_output);
    assert_eq!(evidence["revision"], 2);
    assert_eq!(evidence["observed"]["evidence_current"], true);

    let status = json_output(&run(
        project,
        &[
            "planning",
            "status",
            "--project",
            &project_arg,
            "--session",
            &session,
            "--json",
        ],
    ));
    let work_item = status["result"]["state"]["required_model_action"].clone();
    let proposal_path = inputs.path().join("audit.json");
    let proposal_value = json!({
        "schema": "megara.audit-proposal/v1",
        "mode": "delta",
        "work_item_id": work_item["work_item_id"],
        "base_revision": work_item["base_revision"],
        "base_domain_revision": work_item["base_domain_revision"],
        "input_hash": work_item["input_hash"],
        "readiness": "continue",
        "next_question": {
            "context": "현재 상태를 확인합니다.",
            "question": "어떤 결과가 필요할까요?",
            "why_it_matters": "답에 따라 다음 검토가 달라집니다.",
            "technical_terms": [],
            "source_refs": [{"kind":"initial_request","id":"request"}],
            "answer": {"mode":"freeform","freeform_hint":"원하는 결과를 적어 주세요."}
        },
        "entity_ops": [],
        "edge_ops": [],
        "blocker_ops": [],
        "counterexample_review": null
    });
    std::fs::write(&proposal_path, serde_json::to_vec(&proposal_value).unwrap()).unwrap();
    let proposal_arg = proposal_path.to_string_lossy().to_string();
    let audit_args = [
        "planning",
        "audit",
        "apply",
        "--project",
        &project_arg,
        "--session",
        &session,
        "--expected-revision",
        "2",
        "--mode",
        "delta",
        "--proposal",
        &proposal_arg,
        "--command-id",
        "cmd-cli-s3-audit",
        "--json",
    ];
    let audit = json_output(&run(project, &audit_args));
    assert_eq!(audit["revision"], 3);
    assert_eq!(audit["result"]["next_action"]["kind"], "question");
    let projection = &audit["result"]["next_action"]["projection"];
    assert_eq!(
        projection["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|block| block["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["context", "question", "why_it_matters", "freeform_hint"]
    );
    assert_eq!(
        projection["provenance"]["question_source_refs"],
        json!([{"kind":"initial_request","id":"request"}])
    );
    let blocks = projection["blocks"].as_array().unwrap();
    assert_eq!(blocks[0]["text"], "현재 상태를 확인합니다.");
    assert_eq!(blocks[1]["text"], "어떤 결과가 필요할까요?");
    assert_eq!(blocks[2]["text"], "답에 따라 다음 검토가 달라집니다.");
    assert_eq!(blocks[3]["text"], "원하는 결과를 적어 주세요.");
    let encoded_projection = projection.to_string();
    for sentinel in [
        "현재 상태를 확인합니다.",
        "어떤 결과가 필요할까요?",
        "답에 따라 다음 검토가 달라집니다.",
        "원하는 결과를 적어 주세요.",
    ] {
        assert_eq!(
            encoded_projection.matches(sentinel).count(),
            1,
            "{sentinel}"
        );
    }
    let replay = json_output(&run(project, &audit_args));
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["revision"], 3);

    let mut unchanged_citations =
        serde_json::from_slice::<Value>(&std::fs::read(&citations_path).unwrap()).unwrap();
    unchanged_citations["base_revision"] = json!(3);
    let unchanged_citations_path = inputs.path().join("citations-noop.json");
    std::fs::write(
        &unchanged_citations_path,
        serde_json::to_vec(&unchanged_citations).unwrap(),
    )
    .unwrap();
    let unchanged_citations_arg = unchanged_citations_path.to_string_lossy().to_string();
    let unchanged = json_output(&run(
        project,
        &[
            "planning",
            "evidence",
            "refresh",
            "--project",
            &project_arg,
            "--session",
            &session,
            "--expected-revision",
            "3",
            "--citations",
            &unchanged_citations_arg,
            "--command-id",
            "cmd-cli-s3-evidence-noop",
            "--json",
        ],
    ));
    assert_eq!(unchanged["revision"], 3);
    assert_eq!(unchanged["replayed"], false);

    let mut invalid =
        serde_json::from_slice::<Value>(&std::fs::read(&proposal_path).unwrap()).unwrap();
    invalid.as_object_mut().unwrap().remove("next_question");
    std::fs::write(&proposal_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let invalid_args = [
        "planning",
        "audit",
        "apply",
        "--project",
        &project_arg,
        "--session",
        &session,
        "--expected-revision",
        "3",
        "--mode",
        "delta",
        "--proposal",
        &proposal_arg,
        "--command-id",
        "cmd-cli-s3-audit-invalid",
        "--json",
    ];
    let before_invalid = PlanningStore::open_project(project).unwrap();
    let event_count_before_invalid = before_invalid.event_count(&session).unwrap();
    let state_before_invalid = before_invalid.current(&session).unwrap();
    let state_hash_before_invalid =
        crate::planning::store::normalized_state_hash(&state_before_invalid);
    let invalid_output = run(project, &invalid_args);
    assert_eq!(invalid_output.status.code(), Some(2));
    assert!(invalid_output.stderr.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&invalid_output.stdout)
            .lines()
            .count(),
        1
    );
    let invalid_response: Value = serde_json::from_slice(&invalid_output.stdout).unwrap();
    assert_eq!(invalid_response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    let after_invalid = PlanningStore::open_project(project).unwrap();
    assert_eq!(
        after_invalid.event_count(&session).unwrap(),
        event_count_before_invalid
    );
    assert_eq!(
        crate::planning::store::normalized_state_hash(&after_invalid.current(&session).unwrap()),
        state_hash_before_invalid
    );

    let store = PlanningStore::open_project(project).unwrap();
    let events = store.event_envelopes(&session).unwrap();
    assert!(events.iter().any(|event| {
        event.metadata.actor == EventActor::User && event.metadata.adapter == EventAdapter::Cli
    }));
}
