use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;
use crate::planning::store::PlanningStore;
use crate::planning_service_support::{question, start_request};
use serde_json::{json, Value};
use tempfile::tempdir;

fn evidence_request(session_id: &str, command_id: &str, request_id: &str) -> LogicalRequest {
    LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.to_string(),
        operation: "planning.evidence.refresh".to_string(),
        command_id: Some(command_id.to_string()),
        session_id: Some(session_id.to_string()),
        expected_revision: Some(1),
        params: Some(json!({"citations":[{
            "temp_ref":"source", "path":"source.txt", "ranges":[], "claim":"health marker claim"
        }]})),
    }
}

fn audit_proposal(work: &Value, mode: &str, readiness: &str, next_question: Value) -> Value {
    json!({
        "schema":"megara.audit-proposal/v1", "mode":mode,
        "work_item_id":work["work_item_id"], "base_revision":work["base_revision"],
        "base_domain_revision":work["base_domain_revision"], "input_hash":work["input_hash"],
        "readiness":readiness, "next_question":next_question,
        "entity_ops":[], "edge_ops":[], "blocker_ops":[], "counterexample_review":null
    })
}

fn required_entity_ops() -> Value {
    let source = json!([{"kind":"initial_request","id":"request"}]);
    json!([
        {"op":"create","temp_ref":"problem","kind":"problem","body":{"statement":"문제"},"source_refs":source.clone()},
        {"op":"create","temp_ref":"outcome","kind":"outcome","body":{"statement":"결과","observable_result":"관찰 결과"},"source_refs":source.clone()},
        {"op":"create","temp_ref":"requirement","kind":"requirement","body":{"statement":"요구사항","priority":"must"},"source_refs":source.clone()},
        {"op":"create","temp_ref":"non_goal","kind":"non_goal","body":{"statement":"비목표"},"source_refs":source.clone()},
        {"op":"create","temp_ref":"boundary","kind":"decision_boundary","body":{"autonomous_scope":["검증"],"requires_user_approval":["승인"]},"source_refs":source.clone()},
        {"op":"create","temp_ref":"criterion","kind":"acceptance_criterion","body":{"statement":"검증 기준"},"source_refs":source}
    ])
}

#[test]
fn observed_health_recomputes_for_queries_and_cached_replays_without_mutation() {
    let directory = tempdir().unwrap();
    std::fs::write(directory.path().join("source.txt"), "health marker one\n").unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started =
        service.handle_user_request(start_request("cmd-health-start", "req-health-start", None));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let evidence_request =
        evidence_request(&session_id, "cmd-health-evidence", "req-health-evidence");
    let first = service.handle_user_request(evidence_request.clone());
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["observed"]["evidence_current"], true);
    let store = PlanningStore::open_project(directory.path()).unwrap();
    let event_count = store.event_count(&session_id).unwrap();
    let state_hash =
        crate::planning::store::normalized_state_hash(&store.current(&session_id).unwrap());
    drop(store);

    std::fs::write(
        directory.path().join("source.txt"),
        "health marker changed\n",
    )
    .unwrap();
    for operation in ["planning.status", "planning.current"] {
        let response = service.handle_user_request(LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: format!("req-health-{operation}"),
            operation: operation.to_string(),
            command_id: None,
            session_id: Some(session_id.clone()),
            expected_revision: None,
            params: None,
        });
        assert_eq!(response["ok"], true, "{response}");
        assert_eq!(response["observed"]["evidence_current"], false);
        assert_eq!(response["observed"]["warnings"], json!(["EVIDENCE_STALE"]));
        assert_eq!(response["revision"], 2);
    }
    let mut replay_request = evidence_request;
    replay_request.request_id = "req-health-evidence-retry".to_string();
    let replay = service.handle_user_request(replay_request);
    assert_eq!(replay["ok"], true, "{replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"], first["result"]);
    assert_eq!(replay["revision"], 2);
    assert_eq!(replay["observed"]["evidence_current"], false);
    let store = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), event_count);
    assert_eq!(
        crate::planning::store::normalized_state_hash(&store.current(&session_id).unwrap()),
        state_hash
    );

    let no_snapshot_directory = tempdir().unwrap();
    let mut no_snapshot_service =
        PlanningService::open_project(no_snapshot_directory.path()).unwrap();
    let no_snapshot_start = no_snapshot_service.handle_user_request(start_request(
        "cmd-health-none",
        "req-health-none",
        None,
    ));
    let no_snapshot = no_snapshot_service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-health-none-status".to_string(),
        operation: "planning.status".to_string(),
        command_id: None,
        session_id: Some(
            no_snapshot_start["session_id"]
                .as_str()
                .unwrap()
                .to_string(),
        ),
        expected_revision: None,
        params: None,
    });
    assert_eq!(no_snapshot["observed"]["evidence_current"], false);
    assert_eq!(no_snapshot["observed"]["warnings"], json!([]));
}

#[test]
fn delta_audit_remains_available_when_cited_file_is_missing() {
    let directory = tempdir().unwrap();
    std::fs::write(directory.path().join("source.txt"), "delta source\n").unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started =
        service.handle_user_request(start_request("cmd-delta-start", "req-delta-start", None));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let evidence = service.handle_user_request(evidence_request(
        &session_id,
        "cmd-delta-evidence",
        "req-delta-evidence",
    ));
    assert_eq!(evidence["ok"], true, "{evidence}");
    let work = evidence["result"]["state"]["required_model_action"].clone();
    std::fs::remove_file(directory.path().join("source.txt")).unwrap();
    let response = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-delta-stale-file".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-delta-stale-file".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({
            "mode":"delta",
            "proposal":audit_proposal(&work, "delta", "continue", serde_json::to_value(question()).unwrap())
        })),
    });
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["error"], Value::Null);
    assert_eq!(response["revision"], 3);
    assert_eq!(response["observed"]["evidence_current"], false);
    assert_eq!(response["observed"]["warnings"], json!(["EVIDENCE_STALE"]));
    let store = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), 3);
    assert!(store
        .current(&session_id)
        .unwrap()
        .pending_question
        .is_some());
}

#[test]
fn stale_full_audit_checks_revision_before_evidence_or_proposal_shape() {
    let directory = tempdir().unwrap();
    std::fs::write(directory.path().join("source.txt"), "audit source\n").unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(start_request(
        "cmd-precedence-start",
        "req-precedence-start",
        None,
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let evidence = service.handle_user_request(evidence_request(
        &session_id,
        "cmd-precedence-evidence",
        "req-precedence-evidence",
    ));
    assert_eq!(evidence["ok"], true, "{evidence}");
    let work = evidence["result"]["state"]["required_model_action"].clone();
    let delta = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-precedence-delta".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-precedence-delta".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({
            "mode":"delta",
            "proposal":audit_proposal(&work, "delta", "request_full_audit", Value::Null)
        })),
    });
    assert_eq!(delta["ok"], true, "{delta}");
    std::fs::write(directory.path().join("source.txt"), "changed\n").unwrap();
    let before = PlanningStore::open_project(directory.path()).unwrap();
    let before_state = before.current(&session_id).unwrap().clone();
    let before_hash =
        crate::planning::store::normalized_state_hash(&before.current(&session_id).unwrap());
    let before_events = before.event_count(&session_id).unwrap();
    drop(before);
    let wrong_revision = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-precedence-wrong-revision".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-precedence-wrong-revision".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({"mode":"full","proposal":{}})),
    });
    assert_eq!(
        wrong_revision["error"]["code"], "REVISION_CONFLICT",
        "{wrong_revision}"
    );
    let stale = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-precedence-stale".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-precedence-stale".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(3),
        params: Some(json!({"mode":"full","proposal":{}})),
    });
    assert_eq!(stale["error"]["code"], "EVIDENCE_STALE", "{stale}");
    let after = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(after.event_count(&session_id).unwrap(), before_events);
    assert_eq!(
        crate::planning::store::normalized_state_hash(&after.current(&session_id).unwrap()),
        before_hash
    );
    assert_eq!(after.current(&session_id).unwrap(), before_state);
}

#[test]
fn invalid_phase_precedes_stale_evidence_health_for_full_audit() {
    let directory = tempdir().unwrap();
    std::fs::write(directory.path().join("source.txt"), "phase source\n").unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started =
        service.handle_user_request(start_request("cmd-phase-start", "req-phase-start", None));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let evidence = service.handle_user_request(evidence_request(
        &session_id,
        "cmd-phase-evidence",
        "req-phase-evidence",
    ));
    assert_eq!(evidence["ok"], true, "{evidence}");
    let delta_work = evidence["result"]["state"]["required_model_action"].clone();
    let delta = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-phase-delta".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-phase-delta".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({
            "mode":"delta",
            "proposal":{
                "schema":"megara.audit-proposal/v1","mode":"delta",
                "work_item_id":delta_work["work_item_id"],"base_revision":delta_work["base_revision"],
                "base_domain_revision":delta_work["base_domain_revision"],"input_hash":delta_work["input_hash"],
                "readiness":"request_full_audit","next_question":null,
                "entity_ops":required_entity_ops(),"edge_ops":[{
                    "op":"add","kind":"has_acceptance_criterion",
                    "from":{"temp_ref":"requirement"},"to":{"temp_ref":"criterion"},
                    "source_refs":[{"kind":"initial_request","id":"request"}]
                }],"blocker_ops":[],"counterexample_review":null
            }
        })),
    });
    assert_eq!(delta["ok"], true, "{delta}");
    let full_work = delta["result"]["state"]["required_model_action"].clone();
    let full = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-phase-full".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-phase-full".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(3),
        params: Some(json!({
            "mode":"full",
            "proposal":{
                "schema":"megara.audit-proposal/v1","mode":"full",
                "work_item_id":full_work["work_item_id"],"base_revision":full_work["base_revision"],
                "base_domain_revision":full_work["base_domain_revision"],"input_hash":full_work["input_hash"],
                "readiness":"ready","next_question":null,"entity_ops":[],"edge_ops":[],"blocker_ops":[],
                "counterexample_review":{"performed":true,"challenged_entity_ids":[],"findings":[]}
            }
        })),
    });
    assert_eq!(full["ok"], true, "{full}");
    assert_eq!(full["result"]["state"]["phase"], "specification");
    let before = PlanningStore::open_project(directory.path()).unwrap();
    let before_state = before.current(&session_id).unwrap().clone();
    let before_hash = crate::planning::store::normalized_state_hash(&before_state);
    let before_events = before.event_count(&session_id).unwrap();
    drop(before);
    std::fs::write(
        directory.path().join("source.txt"),
        "phase source changed\n",
    )
    .unwrap();
    let response = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-phase-stale".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-phase-stale".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(4),
        params: Some(json!({"mode":"full","proposal":{}})),
    });
    assert_eq!(response["error"]["code"], "INVALID_PHASE", "{response}");
    let after = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(after.event_count(&session_id).unwrap(), before_events);
    assert_eq!(after.current(&session_id).unwrap(), before_state);
    assert_eq!(
        crate::planning::store::normalized_state_hash(&after.current(&session_id).unwrap()),
        before_hash
    );
}

#[cfg(unix)]
#[test]
fn missing_project_root_reports_health_io_without_mutating_state() {
    use std::os::unix::fs::MetadataExt;

    let directory = tempdir().unwrap();
    std::fs::write(directory.path().join("source.txt"), "io source\n").unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(start_request("cmd-io-start", "req-io-start", None));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let evidence = service.handle_user_request(evidence_request(
        &session_id,
        "cmd-io-evidence",
        "req-io-evidence",
    ));
    assert_eq!(evidence["ok"], true, "{evidence}");
    let before = PlanningStore::open_project(directory.path()).unwrap();
    let before_state = before.current(&session_id).unwrap().clone();
    let before_hash = crate::planning::store::normalized_state_hash(&before_state);
    let before_events = before.event_count(&session_id).unwrap();
    let original_dev = std::fs::metadata(directory.path()).unwrap().dev();
    drop(before);
    let moved = directory.path().with_file_name(format!(
        "{}-moved",
        directory.path().file_name().unwrap().to_string_lossy()
    ));
    std::fs::rename(directory.path(), &moved).unwrap();
    let response = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-io-status".to_string(),
        operation: "planning.status".to_string(),
        command_id: None,
        session_id: Some(session_id.clone()),
        expected_revision: None,
        params: None,
    });
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["observed"]["evidence_current"], false);
    assert_eq!(
        response["observed"]["warnings"],
        json!(["EVIDENCE_HEALTH_IO"])
    );
    std::fs::rename(&moved, directory.path()).unwrap();
    assert_eq!(
        std::fs::metadata(directory.path()).unwrap().dev(),
        original_dev
    );
    let after = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(after.event_count(&session_id).unwrap(), before_events);
    assert_eq!(after.current(&session_id).unwrap(), before_state);
    assert_eq!(
        crate::planning::store::normalized_state_hash(&after.current(&session_id).unwrap()),
        before_hash
    );
}
