use crate::planning::engine::EvidenceRefreshCommand;
use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;
use crate::planning::store::{EventActor, EventAdapter, PlanningStore};
use crate::planning_service_support::{prepare_pending_question, question, start_request};
use crate::planning_store_support::snapshot;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn service_preserves_start_title_and_entrypoint_event_metadata() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let response = service.handle_user_request(start_request(
        "cmd-title",
        "req-title",
        Some("초심자용 계획"),
    ));
    assert_eq!(response["ok"], true);
    assert_eq!(response["command_id"], "cmd-title");
    assert_eq!(response["result"]["state"]["title"], "초심자용 계획");
    let session_id = response["session_id"].as_str().unwrap();

    let store = PlanningStore::open_project(directory.path()).unwrap();
    let event = store.event_envelopes(session_id).unwrap().remove(0);
    assert!(matches!(event.metadata.actor, EventActor::User));
    assert!(matches!(event.metadata.adapter, EventAdapter::Cli));
    assert_eq!(event.metadata.request_id.as_deref(), Some("req-title"));
    assert_eq!(
        event.semantic_payload.primary["command"]["title"],
        "초심자용 계획"
    );
    assert_eq!(
        store.current(session_id).unwrap().title.as_deref(),
        Some("초심자용 계획")
    );

    let model_directory = tempdir().unwrap();
    let mut model_service = PlanningService::open_project(model_directory.path()).unwrap();
    let model_response =
        model_service.handle_request(start_request("cmd-model", "req-model", Some("model path")));
    let model_session = model_response["session_id"].as_str().unwrap();
    let model_store = PlanningStore::open_project(model_directory.path()).unwrap();
    let model_event = model_store
        .event_envelopes(model_session)
        .unwrap()
        .remove(0);
    assert!(matches!(model_event.metadata.actor, EventActor::Model));
    assert!(matches!(model_event.metadata.adapter, EventAdapter::Pi));
    assert_eq!(
        model_event.metadata.request_id.as_deref(),
        Some("req-model")
    );
}

#[test]
fn answer_success_replays_after_pending_question_is_consumed_and_restart() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(start_request("cmd-start", "req-start", None));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let mut store = PlanningStore::open_project(directory.path()).unwrap();
    let question_id = prepare_pending_question(&mut store, &session_id);
    let expected_question_id = question_id.clone();
    let revision = store.current(&session_id).unwrap().revision;
    drop(store);

    let answer = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-answer".to_string(),
        operation: "planning.answer".to_string(),
        command_id: Some("cmd-answer".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(revision),
        params: Some(json!({"question_id":question_id, "text":"보존 가능한 결과"})),
    };
    let first = service.handle_user_request(answer.clone());
    assert_eq!(first["ok"], true);
    assert_eq!(first["replayed"], false);
    let first_state = first["result"]["state"].clone();
    let first_state_typed =
        serde_json::from_value::<crate::planning::domain::PlanningState>(first_state.clone())
            .unwrap();
    assert_eq!(
        first_state_typed.transcript.answers.last().unwrap().text,
        "보존 가능한 결과"
    );
    assert_eq!(
        first_state_typed
            .transcript
            .answers
            .last()
            .unwrap()
            .question_id,
        expected_question_id
    );

    let mut store = PlanningStore::open_project(directory.path()).unwrap();
    let current = store.current(&session_id).unwrap();
    store
        .refresh_evidence(
            "cmd-evidence-after-answer",
            "sha256:evidence-after-answer",
            EvidenceRefreshCommand {
                session_id: session_id.clone(),
                expected_revision: current.revision,
                snapshot: snapshot("sha256:evidence-after-answer"),
            },
        )
        .unwrap();
    let event_count = store.event_count(&session_id).unwrap();
    let answer_event = store.event_envelopes(&session_id).unwrap()[3].clone();
    assert!(matches!(answer_event.metadata.actor, EventActor::User));
    assert!(matches!(answer_event.metadata.adapter, EventAdapter::Cli));
    assert_eq!(
        answer_event.metadata.request_id.as_deref(),
        Some("req-answer")
    );
    let answer_revision = first_state_typed.revision;
    drop(store);

    let mut reopened = PlanningService::open_project(directory.path()).unwrap();
    let mut retry = answer;
    retry.request_id = "req-answer-retry".to_string();
    let replayed = reopened.handle_user_request(retry);
    assert_eq!(replayed["ok"], true);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["request_id"], "req-answer-retry");
    assert_eq!(replayed["revision"], answer_revision);
    assert_eq!(replayed["result"]["state"], first_state);
    let store = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), event_count);
    assert_eq!(store.current(&session_id).unwrap().revision, event_count);
}

#[test]
fn invalid_audit_proposal_can_retry_with_same_work_item_and_new_command_id() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(start_request(
        "cmd-start-qst-retry",
        "req-start-qst-retry",
        None,
    ));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let mut store = PlanningStore::open_project(directory.path()).unwrap();
    let current = store.current(&session_id).unwrap();
    store
        .refresh_evidence(
            "cmd-evidence-qst-retry",
            "sha256:evidence-qst-retry",
            EvidenceRefreshCommand {
                session_id: session_id.clone(),
                expected_revision: current.revision,
                snapshot: snapshot("sha256:evidence-qst-retry"),
            },
        )
        .unwrap();
    let current = store.current(&session_id).unwrap();
    let work = current.required_model_action.clone().unwrap();
    let mut proposal = json!({
        "schema":"megara.audit-proposal/v1", "mode":"delta",
        "work_item_id":work.work_item_id, "base_revision":work.base_revision,
        "base_domain_revision":work.base_domain_revision, "input_hash":work.input_hash,
        "readiness":"continue", "next_question":serde_json::to_value(question()).unwrap(),
        "entity_ops":[], "edge_ops":[], "blocker_ops":[], "counterexample_review":null
    });
    let original_work = current.required_model_action.clone();
    let event_count = store.event_count(&session_id).unwrap();
    drop(store);

    proposal.as_object_mut().unwrap().remove("next_question");
    let invalid = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-qst-invalid".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-qst-invalid".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({"mode":"delta","proposal":proposal.clone()})),
    });
    assert_eq!(invalid["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    let store = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), event_count);
    assert_eq!(
        store.current(&session_id).unwrap().required_model_action,
        original_work
    );
    drop(store);

    proposal["next_question"] = serde_json::to_value(question()).unwrap();
    let corrected = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-qst-corrected".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-qst-corrected".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({"mode":"delta","proposal":proposal})),
    });
    assert_eq!(corrected["ok"], true, "{corrected}");
    assert_eq!(corrected["replayed"], false);
    assert_eq!(
        corrected["result"]["state"]["pending_question"]["proposal"]["question"],
        "어떤 결과를 원하시나요?"
    );
    let store = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), event_count + 1);
}

#[test]
fn rpc_model_authority_cannot_purge_and_preserves_state() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(start_request("cmd-start", "req-start", None));
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let request = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-purge".to_string(),
        operation: "planning.purge".to_string(),
        command_id: Some("cmd-purge".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(1),
        params: Some(json!({"confirm":session_id})),
    };
    let response = service.handle_request(request);
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "USER_ENTRYPOINT_REQUIRED");
    let store = PlanningStore::open_project(directory.path()).unwrap();
    assert_eq!(store.event_count(&session_id).unwrap(), 1);
    assert!(store.current(&session_id).is_ok());
}

#[test]
fn service_session_selection_and_protocol_required_session_are_typed_and_read_only() {
    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let missing = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-missing-status".to_string(),
        operation: "planning.status".to_string(),
        command_id: None,
        session_id: None,
        expected_revision: None,
        params: None,
    });
    assert_eq!(missing["error"]["code"], "SESSION_NOT_FOUND");
    let list = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-empty-list".to_string(),
        operation: "planning.list".to_string(),
        command_id: None,
        session_id: None,
        expected_revision: None,
        params: None,
    });
    assert_eq!(list["ok"], true);
    assert_eq!(list["result"]["sessions"].as_array().unwrap().len(), 0);

    service.handle_user_request(start_request("cmd-one", "req-one", None));
    service.handle_user_request(start_request("cmd-two", "req-two", None));
    let ambiguous = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-ambiguous".to_string(),
        operation: "planning.current".to_string(),
        command_id: None,
        session_id: None,
        expected_revision: None,
        params: None,
    });
    assert_eq!(ambiguous["error"]["code"], "SESSION_AMBIGUOUS");

    let missing_answer = service.handle_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-rpc-missing-session".to_string(),
        operation: "planning.answer".to_string(),
        command_id: Some("cmd-rpc-missing-session".to_string()),
        session_id: None,
        expected_revision: Some(1),
        params: Some(json!({"question_id":"qst-1", "text":"answer"})),
    });
    assert_eq!(missing_answer["error"]["code"], "SESSION_REQUIRED");
}
