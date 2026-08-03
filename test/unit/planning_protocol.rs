use crate::planning::protocol::{
    decode_jsonl_frame, encode_jsonl, LogicalRequest, OperationKind, ProtocolError,
    LOGICAL_OPERATIONS, MAX_JSONL_FRAME_BYTES, PROTOCOL_VERSION, RESULT_SCHEMA,
};
use serde_json::{json, Value};

fn answer_request(selected_choice_ids: Value) -> LogicalRequest {
    LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req_test".to_string(),
        operation: "planning.answer".to_string(),
        command_id: Some("cmd_test".to_string()),
        session_id: Some("pln_test".to_string()),
        expected_revision: Some(1),
        params: Some(json!({
            "question_id": "qst_test",
            "text": "답변",
            "selected_choice_ids": selected_choice_ids,
        })),
    }
}

#[test]
fn protocol_fixture_enumerates_all_seventeen_operations() {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string("test/fixtures/planning/protocol/operations-v1.json").unwrap(),
    )
    .unwrap();
    let expected = fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["operation"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        crate::planning::protocol::supported_operations(),
        expected.as_slice()
    );
    assert_eq!(LOGICAL_OPERATIONS.len(), 17);
    assert_eq!(fixture["result_schema"], RESULT_SCHEMA);
    let kind = OperationKind::Mutation;
    assert!(matches!(kind, OperationKind::Mutation));
}

#[test]
fn typed_params_reject_wrong_types_unknown_enums_and_semantic_hashes() {
    let mut wrong_type = answer_request(json!([]));
    wrong_type.params = Some(json!({
        "question_id": 1,
        "text": [],
    }));
    assert!(matches!(
        wrong_type.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));

    let mut unknown = answer_request(json!([]));
    unknown
        .params
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(
            "unexpected".to_string(),
            Value::String("reject".to_string()),
        );
    assert!(matches!(
        unknown.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));

    let invalid_enum = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req_test".to_string(),
        operation: "planning.list".to_string(),
        command_id: None,
        session_id: None,
        expected_revision: None,
        params: Some(json!({"phase": "not-a-phase"})),
    };
    assert!(matches!(
        invalid_enum.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));

    let invalid_hash = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req_test".to_string(),
        operation: "planning.spec.approve".to_string(),
        command_id: Some("cmd_test".to_string()),
        session_id: Some("pln_test".to_string()),
        expected_revision: Some(1),
        params: Some(json!({
            "candidate_id": "cand_test",
            "semantic_hash": "sha256:UPPER",
            "base_domain_revision": 1,
        })),
    };
    assert!(matches!(
        invalid_hash.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));
}

#[test]
fn jsonl_prescan_handles_depth_escaped_delimiters_and_frame_size() {
    let escaped = json!({
        "protocol_version": 1,
        "request_id": "req_test",
        "operation": "planning.start",
        "command_id": "cmd_test",
        "params": {"request": "literal { [ escaped ] }"},
    });
    let frame = serde_json::to_vec(&escaped).unwrap();
    assert!(decode_jsonl_frame(&frame).is_ok());
    assert!(encode_jsonl(&escaped).unwrap().ends_with('\n'));

    fn nested_objects(count: usize) -> Value {
        let mut value = Value::String("end".to_string());
        for _ in 0..count {
            value = json!({"nested": value});
        }
        value
    }
    let depth_64 = json!({
        "protocol_version": 1,
        "request_id": "req_test",
        "operation": "planning.spec.generate",
        "command_id": "cmd_test",
        "session_id": "pln_test",
        "expected_revision": 1,
        "params": {"proposal": {"nested": nested_objects(61)}},
    });
    assert!(decode_jsonl_frame(&serde_json::to_vec(&depth_64).unwrap()).is_ok());
    let depth_65 = json!({
        "protocol_version": 1,
        "request_id": "req_test",
        "operation": "planning.spec.generate",
        "command_id": "cmd_test",
        "session_id": "pln_test",
        "expected_revision": 1,
        "params": {"proposal": {"nested": nested_objects(62)}},
    });
    assert!(matches!(
        decode_jsonl_frame(&serde_json::to_vec(&depth_65).unwrap()),
        Err(ProtocolError::InvalidRequest(message)) if message.contains("depth 64")
    ));

    let mut exact = frame.clone();
    exact.resize(MAX_JSONL_FRAME_BYTES, b' ');
    assert!(decode_jsonl_frame(&exact).is_ok());
    exact.push(b' ');
    assert!(matches!(
        decode_jsonl_frame(&exact),
        Err(ProtocolError::FrameTooLarge)
    ));
    assert!(matches!(
        decode_jsonl_frame(b"{"),
        Err(ProtocolError::InvalidJson(_))
    ));
    assert!(matches!(
        decode_jsonl_frame(br#"{"protocol_version":1"#),
        Err(ProtocolError::InvalidJson(_))
    ));
    assert!(matches!(
        decode_jsonl_frame(b"[]"),
        Err(ProtocolError::InvalidRequest(_))
    ));
    assert!(matches!(
        decode_jsonl_frame(br#"{"protocol_version":1,"request_id":"r","operation":"planning.start","command_id":"c","actor":"model","params":{"request":"x"}}"#),
        Err(ProtocolError::InvalidRequest(_))
    ));
    assert!(matches!(
        decode_jsonl_frame(br#"{"protocol_version":1,"request_id":"r","operation":"planning.start","command_id":"c","session_id":null,"expected_revision":null,"params":{"request":"x"}}"#),
        Err(ProtocolError::InvalidRequest(message)) if message.contains("session_id")
    ));
    assert!(matches!(
        decode_jsonl_frame(br#"{"protocol_version":1,"request_id":"r","operation":"planning.status","command_id":null}"#),
        Err(ProtocolError::InvalidRequest(message)) if message.contains("command_id")
    ));
    assert!(matches!(
        decode_jsonl_frame(&[0xff]),
        Err(ProtocolError::InvalidUtf8)
    ));
}

#[test]
fn wire_limits_apply_to_text_ids_paths_and_operation_arrays() {
    let mut text = answer_request(json!([]));
    text.params
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("text".to_string(), Value::String("x".repeat(64 * 1024)));
    assert!(text.validate().is_ok());
    text.params
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("text".to_string(), Value::String("x".repeat(64 * 1024 + 1)));
    assert!(matches!(
        text.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));

    let mut ids = answer_request(json!(["i".repeat(128)]));
    assert!(ids.validate().is_ok());
    ids.params.as_mut().unwrap().as_object_mut().unwrap()["selected_choice_ids"] =
        json!(["i".repeat(129)]);
    assert!(matches!(
        ids.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));

    let mut path = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req_test".to_string(),
        operation: "planning.spec.generate".to_string(),
        command_id: Some("cmd_test".to_string()),
        session_id: Some("pln_test".to_string()),
        expected_revision: Some(1),
        params: Some(json!({
            "proposal": {"change_surface": ["p".repeat(4 * 1024)]}
        })),
    };
    assert!(path.validate().is_ok());
    path.params.as_mut().unwrap().as_object_mut().unwrap()["proposal"]["change_surface"][0] =
        Value::String("p".repeat(4 * 1024 + 1));
    assert!(matches!(
        path.validate(),
        Err(ProtocolError::InvalidRequest(_))
    ));

    let mut operations = LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req_test".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd_test".to_string()),
        session_id: Some("pln_test".to_string()),
        expected_revision: Some(1),
        params: Some(json!({
            "mode": "delta",
            "proposal": {"entity_ops": Value::Array(vec![Value::Null; 10_000])}
        })),
    };
    assert!(operations.validate().is_ok());
    operations.params.as_mut().unwrap().as_object_mut().unwrap()["proposal"]["entity_ops"] =
        Value::Array(vec![Value::Null; 10_001]);
    assert!(matches!(
        operations.validate(),
        Err(ProtocolError::InvalidRequest(message)) if message.contains("10000")
    ));
}

#[test]
fn canonical_hash_uses_typed_defaults_and_keeps_export_force_semantic() {
    let omitted = answer_request(Value::Null);
    let mut omitted = omitted;
    omitted
        .params
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("selected_choice_ids");
    let explicit = answer_request(json!([]));
    let mut null_default = answer_request(Value::Null);
    null_default
        .params
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("selected_choice_ids".to_string(), Value::Null);
    assert_eq!(
        omitted.canonical_request_hash("prj_test").unwrap(),
        explicit.canonical_request_hash("prj_test").unwrap()
    );
    assert_eq!(
        explicit.canonical_request_hash("prj_test").unwrap(),
        null_default.canonical_request_hash("prj_test").unwrap()
    );

    let export = |force: Option<Value>, include: Option<Value>| LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req_a".to_string(),
        operation: "planning.export".to_string(),
        command_id: Some("cmd_a".to_string()),
        session_id: None,
        expected_revision: None,
        params: Some(json!({
            "out": "bundle.zip",
            "format": "bundle",
            "force": force,
            "include_transcript": include,
        })),
    };
    let omitted = export(None, None);
    let defaults = export(Some(Value::Bool(false)), Some(Value::Bool(false)));
    let forced = export(Some(Value::Bool(true)), Some(Value::Bool(false)));
    assert_eq!(
        omitted.canonical_request_hash("prj_test").unwrap(),
        defaults.canonical_request_hash("prj_test").unwrap()
    );
    assert_ne!(
        omitted.canonical_request_hash("prj_test").unwrap(),
        forced.canonical_request_hash("prj_test").unwrap()
    );
}
