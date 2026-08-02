use super::planning_store_support::{open_store, snapshot, start};
use crate::planning::{
    domain::{BlockerKind, BlockerSeverity, EntityBody, SourceRef},
    engine::{
        AnswerCommand, AuditCommand, AuditMode, AuditReadiness, BlockerOp, EntityOp, StartCommand,
    },
    store::{
        canonical_project_identity, normalized_state_hash, replay_events, EventActor, EventAdapter,
        EventEnvelope, EventMetadata, EventType, PlanningStore, StoreError,
        EVENT_ENVELOPE_SCHEMA_VERSION,
    },
};
use crate::planning_support::question;
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn start_inserts_session_before_event_and_restart_replays_exact_event_history() {
    let (directory, identity, mut store) = open_store();
    let started = start(&mut store, "cmd_start");
    let canonical = canonical_project_identity(directory.path()).unwrap();
    assert!(canonical.project_id.starts_with("prj_"));
    assert_eq!(started.state.revision, 1);
    assert_eq!(store.event_count("pln_store").unwrap(), 1);
    let envelopes = store.event_envelopes("pln_store").unwrap();
    let _: EventEnvelope = envelopes[0].clone();
    let _: EventMetadata = envelopes[0].metadata.clone();
    let _: EventType = envelopes[0].event_type;
    let _: EventActor = EventActor::System;
    let _: EventAdapter = EventAdapter::Core;
    assert_eq!(envelopes[0].schema_version, EVENT_ENVELOPE_SCHEMA_VERSION);
    assert_eq!(envelopes[0].event_type.operation(), "planning.start");
    let replayed = replay_events(&envelopes).unwrap();
    assert_eq!(
        replayed.events(),
        envelopes
            .iter()
            .map(|envelope| envelope.semantic_payload.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(replayed.state("pln_store"), Some(&started.state));

    let database_path = directory.path().join("planning.db");
    drop(store);
    let reopened = PlanningStore::open(database_path, identity).unwrap();
    assert_eq!(reopened.current("pln_store").unwrap(), started.state);
    let reopened_events = reopened.event_envelopes("pln_store").unwrap();
    assert_eq!(reopened_events, envelopes);
}

#[test]
fn question_answer_entity_blocker_ids_survive_restart_replay_and_hash_normalization() {
    let (directory, identity, mut store) = open_store();
    let started = start(&mut store, "cmd_start");
    let work = started.state.required_model_action.clone().unwrap();
    let interviewed = store
        .apply_audit(
            "cmd_question",
            "sha256:question",
            AuditCommand {
                session_id: started.state.session_id.clone(),
                expected_revision: started.state.revision,
                work_item_id: work.work_item_id,
                mode: AuditMode::Delta,
                base_revision: work.base_revision,
                base_domain_revision: work.base_domain_revision,
                input_hash: work.input_hash,
                readiness: AuditReadiness::Continue,
                next_question: Some(question()),
                entity_ops: Vec::new(),
                edge_ops: Vec::new(),
                blocker_ops: Vec::new(),
                counterexample_review: None,
            },
        )
        .unwrap();
    let pending = interviewed.state.pending_question.clone().unwrap();
    let answered = store
        .apply_answer(
            "cmd_answer",
            "sha256:answer",
            AnswerCommand {
                session_id: interviewed.state.session_id.clone(),
                expected_revision: interviewed.state.revision,
                question_id: pending.question_id,
                based_on_revision: pending.based_on_revision,
                text: "답을 보존한다.".to_string(),
                selected_choice_ids: Vec::new(),
            },
        )
        .unwrap();
    let work = answered.state.required_model_action.clone().unwrap();
    let source_refs = vec![SourceRef::InitialRequest {
        id: "request".to_string(),
    }];
    let with_records = store
        .apply_audit(
            "cmd_records",
            "sha256:records",
            AuditCommand {
                session_id: answered.state.session_id.clone(),
                expected_revision: answered.state.revision,
                work_item_id: work.work_item_id,
                mode: AuditMode::Delta,
                base_revision: work.base_revision,
                base_domain_revision: work.base_domain_revision,
                input_hash: work.input_hash,
                readiness: AuditReadiness::Continue,
                next_question: Some(question()),
                entity_ops: vec![EntityOp::Create {
                    temp_ref: "problem".to_string(),
                    body: EntityBody::Problem {
                        statement: "재생 가능한 문제".to_string(),
                    },
                    source_refs: source_refs.clone(),
                }],
                edge_ops: Vec::new(),
                blocker_ops: vec![BlockerOp::Create {
                    temp_ref: "blocker".to_string(),
                    kind: BlockerKind::EvidenceRequired,
                    severity: BlockerSeverity::Blocking,
                    statement: "추가 근거가 필요하다.".to_string(),
                    source_refs,
                }],
                counterexample_review: None,
            },
        )
        .unwrap();
    let before = with_records.state.clone();
    let hash_before = normalized_state_hash(&before);
    let envelopes = store.event_envelopes("pln_store").unwrap();
    let database_path = directory.path().join("planning.db");
    drop(store);

    let reopened = PlanningStore::open(database_path, identity).unwrap();
    let after = reopened.current("pln_store").unwrap();
    assert_eq!(after, before);
    assert_eq!(normalized_state_hash(&after), hash_before);
    assert_eq!(reopened.event_envelopes("pln_store").unwrap(), envelopes);
    let replayed = replay_events(&envelopes).unwrap();
    assert_eq!(replayed.state("pln_store"), Some(&after));
    assert_eq!(
        replayed.events(),
        envelopes
            .iter()
            .map(|envelope| envelope.semantic_payload.clone())
            .collect::<Vec<_>>()
    );
    assert!(after
        .transcript
        .answers
        .last()
        .is_some_and(|answer| answer.answer_id.starts_with("ans_")));
    assert_eq!(after.blockers.len(), 1);
    assert_eq!(
        after
            .entities
            .current_count(crate::planning::domain::EntityKind::Problem),
        1
    );
}

#[test]
fn idempotency_replays_historical_result_after_restart_and_detects_tampering() {
    let (directory, identity, mut store) = open_store();
    let started = start(&mut store, "cmd_start");
    let refreshed = store
        .refresh_evidence(
            "cmd_refresh",
            "sha256:refresh",
            crate::planning::engine::EvidenceRefreshCommand {
                session_id: "pln_store".to_string(),
                expected_revision: started.state.revision,
                snapshot: snapshot("sha256:evidence"),
            },
        )
        .unwrap();
    assert_eq!(refreshed.state.revision, started.state.revision + 1);
    let database_path = directory.path().join("planning.db");
    drop(store);
    let mut store = PlanningStore::open(database_path, identity).unwrap();
    let replayed = start(&mut store, "cmd_start");
    assert!(replayed.replayed);
    assert_eq!(replayed.state, started.state);
    assert_eq!(replayed.state.revision, 1);
    assert_eq!(replayed.event.as_ref().map(|event| event.seq), Some(1));
    assert_eq!(store.event_count("pln_store").unwrap(), 2);
    let conflict = store.start(
        "cmd_start",
        "sha256:different",
        StartCommand {
            session_id: Some("pln_other".to_string()),
            project_id: store.project_id().to_string(),
            request: "다른 요청".to_string(),
            title: None,
        },
    );
    assert!(matches!(conflict, Err(StoreError::CommandIdReuse)));

    let connection = Connection::open(store.database_path()).unwrap();
    connection
        .execute(
            "UPDATE command_results SET core_response_json=?1 WHERE command_id='cmd_start'",
            [json!({"state": started.state, "event": null}).to_string()],
        )
        .unwrap();
    assert!(matches!(
        store.start(
            "cmd_start",
            "sha256:request",
            StartCommand {
                session_id: Some("pln_store".to_string()),
                project_id: store.project_id().to_string(),
                request: "저장과 재생을 검증한다.".to_string(),
                title: None,
            },
        ),
        Err(StoreError::DbCorrupt(_))
    ));
}

#[test]
fn event_column_and_cache_tampering_have_distinct_typed_errors() {
    for (column, value) in [
        ("schema_version", "99"),
        ("event_type", "planning.unknown"),
        ("semantic_payload_hash", "sha256:bad"),
        ("revision_after", "99"),
        ("state_hash_after", "sha256:bad"),
    ] {
        let (_directory, _identity, mut store) = open_store();
        start(&mut store, "cmd_start");
        let connection = Connection::open(store.database_path()).unwrap();
        connection
            .execute(
                &format!("UPDATE events SET {column}=?1 WHERE seq=1"),
                [value],
            )
            .unwrap();
        assert!(
            matches!(store.current("pln_store"), Err(StoreError::DbCorrupt(_)),),
            "tampered column {column}"
        );
    }

    for tamper in [
        json!({"unexpected": true}),
        json!({"schema": "megara.event/v1", "effects": [{"kind": "model_action_requested", "unexpected": true}]}),
    ] {
        let (_directory, _identity, mut store) = open_store();
        start(&mut store, "cmd_start");
        let connection = Connection::open(store.database_path()).unwrap();
        let payload: String = connection
            .query_row(
                "SELECT semantic_payload_json FROM events WHERE seq=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        if tamper.get("effects").is_some() {
            payload["effects"] = tamper["effects"].clone();
        } else {
            payload["primary"]["unexpected"] = tamper["unexpected"].clone();
        }
        connection
            .execute(
                "UPDATE events SET semantic_payload_json=?1 WHERE seq=1",
                [payload.to_string()],
            )
            .unwrap();
        assert!(matches!(
            store.current("pln_store"),
            Err(StoreError::DbCorrupt(_))
        ));
    }

    let (_directory, _identity, mut store) = open_store();
    start(&mut store, "cmd_start");
    let connection = Connection::open(store.database_path()).unwrap();
    connection
        .execute(
            "UPDATE events SET semantic_payload_json=?1 WHERE seq=1",
            ["{"],
        )
        .unwrap();
    assert!(matches!(
        store.current("pln_store"),
        Err(StoreError::DbCorrupt(_))
    ));

    let (_directory, _identity, mut store) = open_store();
    start(&mut store, "cmd_start");
    let connection = Connection::open(store.database_path()).unwrap();
    connection
        .execute(
            "UPDATE sessions SET state_json=?1 WHERE session_id='pln_store'",
            ["{}"],
        )
        .unwrap();
    assert!(matches!(
        store.current("pln_store"),
        Err(StoreError::ProjectionDiverged(_))
    ));

    let (_directory, _identity, mut store) = open_store();
    start(&mut store, "cmd_start");
    let connection = Connection::open(store.database_path()).unwrap();
    connection
        .execute(
            "UPDATE sessions SET normalized_state_hash='sha256:tampered' WHERE session_id='pln_store'",
            [],
        )
        .unwrap();
    assert!(matches!(
        store.current("pln_store"),
        Err(StoreError::ProjectionDiverged(_))
    ));
}

#[test]
fn recomputed_payload_hash_does_not_bypass_reducer_semantic_checks() {
    for tamper in ["primary_sibling", "command_unknown", "effect_unknown"] {
        let (_directory, _identity, mut store) = open_store();
        start(&mut store, "cmd_start");
        let connection = Connection::open(store.database_path()).unwrap();
        let payload: String = connection
            .query_row(
                "SELECT semantic_payload_json FROM events WHERE seq=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        match tamper {
            "primary_sibling" => payload["primary"]["unexpected"] = json!(true),
            "command_unknown" => payload["primary"]["command"]["unexpected"] = json!(true),
            "effect_unknown" => payload["effects"][0]["unexpected"] = json!(true),
            _ => unreachable!(),
        }
        let hash = semantic_hash(&payload);
        connection
            .execute(
                "UPDATE events SET semantic_payload_json=?1, semantic_payload_hash=?2 WHERE seq=1",
                [&payload.to_string(), &hash],
            )
            .unwrap();
        assert!(
            matches!(store.current("pln_store"), Err(StoreError::DbCorrupt(_))),
            "tamper {tamper}"
        );
    }
}

fn semantic_hash(payload: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(crate::planning::canonical::canonical_json_bytes(payload));
    format!("sha256:{:x}", hasher.finalize())
}
