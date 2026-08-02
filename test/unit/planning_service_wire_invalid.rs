use crate::planning::service::PlanningService;
use crate::planning::store::PlanningStore;
use crate::planning_service_wire_support::{audit_proposal, delta_proposal, request};
use serde_json::{json, Value};
use tempfile::tempdir;

#[test]
fn service_rejects_legacy_entity_shapes_and_plan_entities_as_proposal_schema() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(request(
        "planning.start",
        "req-start-wire-negative",
        Some("cmd-start-wire-negative"),
        None,
        None,
        json!({"request":"wire negatives"}),
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let refreshed = service.handle_user_request(request(
        "planning.evidence.refresh",
        "req-evidence-wire-negative",
        Some("cmd-evidence-wire-negative"),
        Some(session_id.clone()),
        Some(1),
        json!({"citations":[]}),
    ));
    let work_item = refreshed["result"]["state"]["required_model_action"].clone();
    for (command_id, body, kind) in [
        (
            "cmd-wire-pascal",
            json!({"Problem":{"statement":"legacy"}}),
            "problem",
        ),
        (
            "cmd-wire-plan-step",
            json!({"objective":"execute","change_surface":["src"],"rollback_or_recovery":"undo"}),
            "plan_step",
        ),
    ] {
        let before = PlanningStore::open_project(directory.path()).unwrap();
        let event_count = before.event_count(&session_id).unwrap();
        let response = service.handle_user_request(request(
            "planning.audit.apply",
            "req-wire-negative",
            Some(command_id),
            Some(session_id.clone()),
            Some(2),
            json!({
                "mode":"delta",
                "proposal":audit_proposal(&work_item, "delta", json!([{
                    "op":"create", "temp_ref":"bad", "kind":kind, "body":body,
                    "source_refs":[{"kind":"initial_request","id":"request"}]
                }]), "request_full_audit")
            }),
        ));
        assert_eq!(
            response["error"]["code"], "PROPOSAL_SCHEMA_INVALID",
            "{response}"
        );
        let after = PlanningStore::open_project(directory.path()).unwrap();
        assert_eq!(after.event_count(&session_id).unwrap(), event_count);
        assert_eq!(after.current(&session_id).unwrap().revision, 2);
    }
}

fn fresh_wire_session() -> (tempfile::TempDir, PlanningService, String, Value) {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(request(
        "planning.start",
        "req-start-negative-matrix",
        Some("cmd-start-negative-matrix"),
        None,
        None,
        json!({"request":"wire negative matrix"}),
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let refreshed = service.handle_user_request(request(
        "planning.evidence.refresh",
        "req-evidence-negative-matrix",
        Some("cmd-evidence-negative-matrix"),
        Some(session_id.clone()),
        Some(1),
        json!({"citations":[]}),
    ));
    let work_item = refreshed["result"]["state"]["required_model_action"].clone();
    (directory, service, session_id, work_item)
}

#[test]
fn service_entity_wire_negative_matrix_is_typed_and_atomic() {
    let cases = [
        (
            "missing create kind",
            json!({
                "op":"create", "temp_ref":"missing-kind",
                "body":{"statement":"문제"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }),
        ),
        (
            "missing body field",
            json!({
                "op":"create", "temp_ref":"missing-field", "kind":"outcome",
                "body":{"statement":"결과"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }),
        ),
        (
            "unknown body field",
            json!({
                "op":"create", "temp_ref":"unknown-field", "kind":"problem",
                "body":{"statement":"문제","unknown":"금지"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }),
        ),
        (
            "cross kind body",
            json!({
                "op":"create", "temp_ref":"cross-kind", "kind":"outcome",
                "body":{"statement":"결과","selected_option":"잘못된 body"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }),
        ),
        (
            "verification create",
            json!({
                "op":"create", "temp_ref":"verification", "kind":"verification",
                "body":{"method":"command","procedure":"검사","expected_result":"통과"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }),
        ),
        (
            "unknown entity revise",
            json!({
                "op":"revise", "entity_id":"PROB-999", "base_entity_revision":1,
                "body":{"statement":"알 수 없음"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }),
        ),
    ];
    for (index, (label, entity_op)) in cases.into_iter().enumerate() {
        let (_directory, mut service, session_id, work_item) = fresh_wire_session();
        let response = service.handle_user_request(request(
            "planning.audit.apply",
            &format!("req-negative-{index}"),
            Some(&format!("cmd-negative-{index}")),
            Some(session_id.clone()),
            Some(2),
            json!({
                "mode":"delta",
                "proposal":delta_proposal(&work_item, json!([entity_op]), json!([]))
            }),
        ));
        assert_eq!(
            response["error"]["code"], "PROPOSAL_SCHEMA_INVALID",
            "{label}: {response}"
        );
        let store = PlanningStore::open_project(_directory.path()).unwrap();
        let state = store.current(&session_id).unwrap();
        assert_eq!(state.revision, 2, "{label}");
        assert_eq!(
            state.required_model_action,
            Some(serde_json::from_value(work_item.clone()).unwrap()),
            "{label}"
        );
        assert_eq!(store.event_count(&session_id).unwrap(), 2, "{label}");
    }

    let (_directory, mut service, session_id, work_item) = fresh_wire_session();
    let duplicate = delta_proposal(
        &work_item,
        json!([{
            "op":"create", "temp_ref":"duplicate", "kind":"problem",
            "body":{"statement":"문제"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        }]),
        json!([{
            "op":"create", "temp_ref":"duplicate", "kind":"missing_problem",
            "severity":"blocking", "statement":"중복 temp ref",
            "source_refs":[{"kind":"initial_request","id":"request"}]
        }]),
    );
    let response = service.handle_user_request(request(
        "planning.audit.apply",
        "req-duplicate-temp-ref",
        Some("cmd-duplicate-temp-ref"),
        Some(session_id.clone()),
        Some(2),
        json!({"mode":"delta","proposal":duplicate}),
    ));
    assert_eq!(response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    let store = PlanningStore::open_project(_directory.path()).unwrap();
    let state = store.current(&session_id).unwrap();
    assert_eq!(state.revision, 2);
    assert_eq!(
        state.required_model_action,
        Some(serde_json::from_value(work_item).unwrap())
    );
    assert_eq!(store.event_count(&session_id).unwrap(), 2);
}

#[test]
fn service_revise_uses_current_entity_kind_and_rejects_legacy_revision_names() {
    let (_directory, mut service, session_id, work_item) = fresh_wire_session();
    let created = service.handle_user_request(request(
        "planning.audit.apply",
        "req-create-for-revise",
        Some("cmd-create-for-revise"),
        Some(session_id.clone()),
        Some(2),
        json!({
            "mode":"delta",
            "proposal":delta_proposal(&work_item, json!([{
                "op":"create", "temp_ref":"problem", "kind":"problem",
                "body":{"statement":"처음 문제"},
                "source_refs":[{"kind":"initial_request","id":"request"}]
            }]), json!([]))
        }),
    ));
    assert_eq!(created["ok"], true, "{created}");
    let full_work = created["result"]["state"]["required_model_action"].clone();
    for (index, op) in [
        json!({
            "op":"revise", "entity_id":"PROB-001", "base_revision":1,
            "body":{"statement":"legacy revise"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        }),
        json!({
            "op":"reject", "entity_id":"PROB-001", "base_revision":1,
            "reason":"legacy reject",
            "source_refs":[{"kind":"initial_request","id":"request"}]
        }),
    ]
    .into_iter()
    .enumerate()
    {
        let response = service.handle_user_request(request(
            "planning.audit.apply",
            &format!("req-legacy-revision-{index}"),
            Some(&format!("cmd-legacy-revision-{index}")),
            Some(session_id.clone()),
            Some(3),
            json!({
                "mode":"full",
                "proposal":{
                    "schema":"megara.audit-proposal/v1", "mode":"full",
                    "work_item_id":full_work["work_item_id"],
                    "base_revision":full_work["base_revision"],
                    "base_domain_revision":full_work["base_domain_revision"],
                    "input_hash":full_work["input_hash"],
                    "readiness":"continue", "next_question":null,
                    "entity_ops":[op], "edge_ops":[], "blocker_ops":[],
                    "counterexample_review":{"performed":true,"challenged_entity_ids":[],"findings":[]}
                }
            }),
        ));
        assert_eq!(
            response["error"]["code"], "PROPOSAL_SCHEMA_INVALID",
            "{response}"
        );
        let store = PlanningStore::open_project(_directory.path()).unwrap();
        let state = store.current(&session_id).unwrap();
        assert_eq!(state.revision, 3);
        assert_eq!(
            state.required_model_action,
            Some(serde_json::from_value(full_work.clone()).unwrap())
        );
        assert_eq!(store.event_count(&session_id).unwrap(), 3);
    }

    let response = service.handle_user_request(request(
        "planning.audit.apply",
        "req-canonical-revise",
        Some("cmd-canonical-revise"),
        Some(session_id.clone()),
        Some(3),
        json!({
            "mode":"full",
            "proposal":{
                "schema":"megara.audit-proposal/v1", "mode":"full",
                "work_item_id":full_work["work_item_id"],
                "base_revision":full_work["base_revision"],
                "base_domain_revision":full_work["base_domain_revision"],
                "input_hash":full_work["input_hash"],
                "readiness":"continue", "next_question":null,
                "entity_ops":[{
                    "op":"revise", "entity_id":"PROB-001", "base_entity_revision":1,
                    "body":{"statement":"새 문제"},
                    "source_refs":[{"kind":"initial_request","id":"request"}]
                }],
                "edge_ops":[], "blocker_ops":[],
                "counterexample_review":{"performed":true,"challenged_entity_ids":[],"findings":[]}
            }
        }),
    ));
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(
        response["result"]["state"]["entities"]["revisions"]["PROB-001"][1]["body"],
        json!({"statement":"새 문제"})
    );
}
