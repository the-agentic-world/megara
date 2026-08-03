use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;
use crate::planning::store::normalized_state_hash;
use crate::planning_service_support::{question, start_request};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn pi_and_codex_use_identical_question_projection_and_normalized_state() {
    let directory = tempdir().unwrap();
    let (pi_response, codex_response) = prepare_pending_pair(directory.path());

    assert_eq!(
        pi_response["result"]["next_action"]["projection"],
        codex_response["result"]["next_action"]["projection"]
    );
    assert_eq!(
        pi_response["result"]["next_action"]["projection"]["provenance"]["question_source_refs"],
        json!([{"kind":"initial_request","id":"request"}])
    );

    let pi_state = serde_json::from_value(pi_response["result"]["state"].clone()).unwrap();
    let codex_state = serde_json::from_value(codex_response["result"]["state"].clone()).unwrap();
    assert_eq!(
        normalized_state_hash(&pi_state),
        normalized_state_hash(&codex_state)
    );
}

fn prepare_pending_pair(project: &std::path::Path) -> (serde_json::Value, serde_json::Value) {
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    let mut service = PlanningService::open_project(project).unwrap();
    let started = service.handle_request(start_request("cmd-start", "req-start", Some("title")));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let evidence = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-evidence".to_string(),
        operation: "planning.evidence.refresh".to_string(),
        command_id: Some("cmd-evidence".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(1),
        params: Some(
            json!({"citations":[{"temp_ref":"main","path":"src/main.rs","ranges":[],"claim":"entry point"}]}),
        ),
    };
    let evidence = service.handle_request(evidence);
    let state = evidence["result"]["state"].clone();
    let work = state["required_model_action"].clone();
    let audit = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-audit".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-audit".to_string()),
        session_id: Some(session_id),
        expected_revision: Some(evidence["revision"].as_u64().unwrap()),
        params: Some(json!({"mode":"delta","proposal":{
            "schema":"megara.audit-proposal/v1","mode":"delta",
            "work_item_id":work["work_item_id"],"base_revision":work["base_revision"],
            "base_domain_revision":work["base_domain_revision"],"input_hash":work["input_hash"],
            "readiness":"continue","next_question":serde_json::to_value(question()).unwrap(),
            "entity_ops":[],"edge_ops":[],"blocker_ops":[],"counterexample_review":null
        }})),
    };
    let pi_response = service.handle_request(audit.clone());
    let mut codex_request = audit;
    codex_request.request_id = "req-audit-codex".to_string();
    let codex_response = service.handle_codex_request(codex_request);
    (pi_response, codex_response)
}
