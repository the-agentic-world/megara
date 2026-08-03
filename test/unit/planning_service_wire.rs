use crate::planning::service::PlanningService;
use crate::planning::store::PlanningStore;
use crate::planning_service_wire_support::{audit_proposal, request};
use serde_json::{json, Value};
use tempfile::tempdir;

fn canonical_entity_ops() -> Value {
    json!([
        {
            "op":"create", "temp_ref":"problem", "kind":"problem",
            "body":{"statement":"문제 본문"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"outcome", "kind":"outcome",
            "body":{"statement":"결과 본문","observable_result":"관찰 가능한 결과"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"fact", "kind":"fact",
            "body":{"statement":"근거 본문","evidence_refs":["EVID-001"]},
            "source_refs":[{"kind":"evidence","id":"EVID-001"}]
        },
        {
            "op":"create", "temp_ref":"decision", "kind":"decision",
            "body":{"statement":"결정 본문","selected_option":"선택"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"boundary", "kind":"decision_boundary",
            "body":{"autonomous_scope":["검증"],"requires_user_approval":["승인"]},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"requirement", "kind":"requirement",
            "body":{"statement":"요구사항 본문","priority":"must"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"criterion", "kind":"acceptance_criterion",
            "body":{"statement":"검증 기준 본문"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"constraint", "kind":"constraint",
            "body":{"statement":"제약 본문"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"non_goal", "kind":"non_goal",
            "body":{"statement":"비목표 본문"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"assumption", "kind":"assumption",
            "body":{"statement":"가정 본문","validation_status":"unverified"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
        {
            "op":"create", "temp_ref":"risk", "kind":"risk",
            "body":{"statement":"위험 본문","impact":"high","mitigation":"완화한다"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        },
    ])
}

#[test]
fn service_entity_wire_is_canonical_and_state_projection_is_plain() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(request(
        "planning.start",
        "req-start",
        Some("cmd-start"),
        None,
        None,
        json!({"request":"wire entity"}),
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let refreshed = service.handle_user_request(request(
        "planning.evidence.refresh",
        "req-evidence",
        Some("cmd-evidence"),
        Some(session_id.clone()),
        Some(1),
        json!({"citations":[]}),
    ));
    assert_eq!(refreshed["ok"], true);
    let work_item = refreshed["result"]["state"]["required_model_action"].clone();
    let created = service.handle_user_request(request(
        "planning.audit.apply",
        "req-audit-create",
        Some("cmd-audit-create"),
        Some(session_id.clone()),
        Some(2),
        json!({
            "mode":"delta",
            "proposal":audit_proposal(
                &work_item,
                "delta",
                json!([{
                    "op":"create",
                    "temp_ref":"tmp_problem",
                    "kind":"problem",
                    "body":{"statement":"wire 문제"},
                    "source_refs":[{"kind":"initial_request","id":"request"}]
                }]),
                "request_full_audit"
            )
        }),
    ));
    assert_eq!(created["ok"], true, "{created}");
    assert_eq!(
        created["result"]["state"]["entities"]["revisions"]["PROB-001"][0]["kind"],
        "problem"
    );
    assert_eq!(
        created["result"]["state"]["entities"]["revisions"]["PROB-001"][0]["body"],
        json!({"statement":"wire 문제"})
    );
    assert!(!created["result"]["state"]
        .to_string()
        .contains("\"Problem\""));

    let status = service.handle_user_request(request(
        "planning.status",
        "req-status",
        None,
        Some(session_id.clone()),
        None,
        json!({}),
    ));
    assert_eq!(
        status["result"]["state"]["entities"]["revisions"]["PROB-001"][0]["body"],
        json!({"statement":"wire 문제"})
    );
    let list = service.handle_user_request(request(
        "planning.list",
        "req-list",
        None,
        None,
        None,
        json!({}),
    ));
    assert_eq!(
        list["result"]["sessions"][0]["entities"]["revisions"]["PROB-001"][0]["body"],
        json!({"statement":"wire 문제"})
    );
}

#[test]
fn malformed_entity_wire_is_proposal_schema_error_and_atomic() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(request(
        "planning.start",
        "req-start",
        Some("cmd-start"),
        None,
        None,
        json!({"request":"wire invalid"}),
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let refreshed = service.handle_user_request(request(
        "planning.evidence.refresh",
        "req-evidence",
        Some("cmd-evidence"),
        Some(session_id.clone()),
        Some(1),
        json!({"citations":[]}),
    ));
    let work_item = refreshed["result"]["state"]["required_model_action"].clone();
    let before = PlanningStore::open_project(directory.path()).unwrap();
    let event_count = before.event_count(&session_id).unwrap();
    let response = service.handle_user_request(request(
        "planning.audit.apply",
        "req-invalid",
        Some("cmd-invalid"),
        Some(session_id.clone()),
        Some(2),
        json!({
            "mode":"delta",
            "proposal":audit_proposal(
                &work_item,
                "delta",
                json!([{
                    "op":"create",
                    "temp_ref":"tmp_problem",
                    "kind":"requirement",
                    "body":{"statement":"교차 kind"},
                    "source_refs":[{"kind":"initial_request","id":"request"}]
                }]),
                "request_full_audit"
            )
        }),
    ));
    assert_eq!(response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    let after = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(after.event_count(&session_id).unwrap(), event_count);
    assert_eq!(after.current(&session_id).unwrap().revision, 2);
}

#[test]
fn service_accepts_every_canonical_audit_entity_body_and_projects_plain_fields() {
    let directory = tempdir().unwrap();
    std::fs::write(
        directory.path().join("evidence.txt"),
        "unique evidence marker\n",
    )
    .unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(request(
        "planning.start",
        "req-start-all-kinds",
        Some("cmd-start-all-kinds"),
        None,
        None,
        json!({"request":"all canonical entities"}),
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let refreshed = service.handle_user_request(request(
        "planning.evidence.refresh",
        "req-evidence-all-kinds",
        Some("cmd-evidence-all-kinds"),
        Some(session_id.clone()),
        Some(1),
        json!({"citations":[{
            "temp_ref":"evidence",
            "path":"evidence.txt",
            "ranges":[],
            "claim":"evidence marker is cited"
        }]}),
    ));
    assert_eq!(refreshed["ok"], true, "{refreshed}");
    let work_item = refreshed["result"]["state"]["required_model_action"].clone();
    let mut proposal = audit_proposal(
        &work_item,
        "delta",
        canonical_entity_ops(),
        "request_full_audit",
    );
    proposal["edge_ops"] = json!([{
        "op":"add", "kind":"has_acceptance_criterion",
        "from":{"temp_ref":"requirement"}, "to":{"temp_ref":"criterion"},
        "source_refs":[{"kind":"initial_request","id":"request"}]
    }]);
    let created = service.handle_user_request(request(
        "planning.audit.apply",
        "req-audit-all-kinds",
        Some("cmd-audit-all-kinds"),
        Some(session_id.clone()),
        Some(2),
        json!({
            "mode":"delta",
            "proposal":proposal
        }),
    ));
    assert_eq!(created["ok"], true, "{created}");
    let state = &created["result"]["state"];
    for (id, kind) in [
        ("PROB-001", "problem"),
        ("OUT-001", "outcome"),
        ("FACT-001", "fact"),
        ("DEC-001", "decision"),
        ("DBND-001", "decision_boundary"),
        ("REQ-001", "requirement"),
        ("AC-001", "acceptance_criterion"),
        ("CON-001", "constraint"),
        ("NG-001", "non_goal"),
        ("ASM-001", "assumption"),
        ("RISK-001", "risk"),
    ] {
        let record = &state["entities"]["revisions"][id][0];
        assert_eq!(record["kind"], kind);
        assert!(record["body"].is_object());
    }
    assert!(!state.to_string().contains("\"Problem\""));
    assert!(!state.to_string().contains("\"DecisionBoundary\""));
    let full_work_item = state["required_model_action"].clone();
    let full = service.handle_user_request(request(
        "planning.audit.apply",
        "req-full-all-kinds",
        Some("cmd-full-all-kinds"),
        Some(session_id.clone()),
        Some(3),
        json!({
            "mode":"full",
            "proposal":audit_proposal(&full_work_item, "full", json!([]), "ready")
        }),
    ));
    assert_eq!(full["ok"], true, "{full}");
    assert_eq!(full["result"]["state"]["phase"], "specification");
    let listed = service.handle_user_request(request(
        "planning.list",
        "req-list-all-kinds",
        None,
        None,
        None,
        json!({}),
    ));
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(
        listed["result"]["sessions"][0]["entities"]["revisions"]["REQ-001"][0]["body"],
        json!({"statement":"요구사항 본문","priority":"must"})
    );
}
